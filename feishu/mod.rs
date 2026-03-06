use std::collections::{HashSet, VecDeque};
use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use serde::Serialize;
use tokio::time::{interval, sleep};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::configs::{load_agents, load_channels};
use crate::core::logger;

const FEISHU_PLUGIN_BINARY: &str = "./plugins/feishu/feishu_sidecar.bin";
const MAX_RESTART_ATTEMPTS: u32 = 3;
const MAX_SIDECAR_LOGS: usize = 200;

#[derive(Clone, Serialize)]
pub struct FeishuRuntimeStatus {
    pub installed: bool,
    pub running: bool,
    pub enabled: bool,
    pub mode: String,
    pub pid: Option<u32>,
    pub message: String,
    pub restart_attempts: u32,
    pub recent_logs: Vec<String>,
}

struct FeishuRuntimeManager {
    master_api: String,
    child: Option<Child>,
    installed: bool,
    restart_attempts: u32,
    recent_logs: VecDeque<String>,
}

static MANAGER: OnceLock<Arc<Mutex<FeishuRuntimeManager>>> = OnceLock::new();
static MONITOR_STARTED: OnceLock<()> = OnceLock::new();

pub fn init_manager(master_api: String) {
    MANAGER.get_or_init(|| {
        Arc::new(Mutex::new(FeishuRuntimeManager {
            master_api,
            child: None,
            installed: std::path::Path::new(FEISHU_PLUGIN_BINARY).exists(),
            restart_attempts: 0,
            recent_logs: VecDeque::new(),
        }))
    });
    MONITOR_STARTED.get_or_init(|| {
        spawn_monitor_thread();
    });
}

pub fn set_installed(installed: bool) -> Result<()> {
    let mgr = get_manager()?;
    let mut guard = mgr
        .lock()
        .map_err(|_| anyhow!("feishu manager lock failed"))?;
    guard.installed = installed;
    push_recent_log(&mut guard, format!("[feishu] installed={}", installed));
    Ok(())
}

pub async fn apply_desired_state() -> Result<()> {
    let enabled = load_channels()?.feishu.enabled;
    if enabled {
        start_sidecar().await
    } else {
        stop_sidecar()
    }
}

pub async fn start_sidecar() -> Result<()> {
    let mgr = get_manager()?;
    let mut guard = mgr
        .lock()
        .map_err(|_| anyhow!("feishu manager lock failed"))?;
    spawn_sidecar_locked(&mut guard, false)
}

pub fn stop_sidecar() -> Result<()> {
    let mgr = get_manager()?;
    let mut guard = mgr
        .lock()
        .map_err(|_| anyhow!("feishu manager lock failed"))?;

    if let Some(mut child) = guard.child.take() {
        let _ = child.kill();
        let _ = child.wait();
        push_recent_log(&mut guard, "[feishu] sidecar process stopped".to_string());
        logger::log("[feishu] sidecar process stopped");
    }
    guard.restart_attempts = 0;
    Ok(())
}

pub fn status() -> Result<FeishuRuntimeStatus> {
    let mgr = get_manager()?;
    let channels = load_channels()?;
    let mut guard = mgr
        .lock()
        .map_err(|_| anyhow!("feishu manager lock failed"))?;

    let mut running = false;
    let mut pid = None;

    if let Some(child) = guard.child.as_mut() {
        if child.try_wait()?.is_none() {
            running = true;
            pid = Some(child.id());
        } else {
            guard.child = None;
        }
    }

    guard.installed = std::path::Path::new(FEISHU_PLUGIN_BINARY).exists() || guard.installed;

    Ok(FeishuRuntimeStatus {
        installed: guard.installed,
        running,
        enabled: channels.feishu.enabled,
        mode: "process".to_string(),
        pid,
        message: if running {
            "运行中".to_string()
        } else {
            "已停止".to_string()
        },
        restart_attempts: guard.restart_attempts,
        recent_logs: guard.recent_logs.iter().cloned().collect(),
    })
}

fn get_manager() -> Result<Arc<Mutex<FeishuRuntimeManager>>> {
    MANAGER
        .get()
        .cloned()
        .ok_or_else(|| anyhow!("feishu manager is not initialized"))
}

fn spawn_monitor_thread() {
    let Some(mgr) = MANAGER.get().cloned() else {
        return;
    };

    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(2));
        let enabled = load_channels().map(|v| v.feishu.enabled).unwrap_or(false);

        let mut guard = match mgr.lock() {
            Ok(g) => g,
            Err(_) => continue,
        };

        if !enabled {
            guard.restart_attempts = 0;
            continue;
        }

        let exited = match guard.child.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(Some(status)) => {
                    let msg = format!("[feishu] sidecar exited: {}", status);
                    push_recent_log(&mut guard, msg.clone());
                    logger::log(msg);
                    guard.child = None;
                    true
                }
                Ok(None) => false,
                Err(e) => {
                    let msg = format!("[feishu] sidecar wait error: {}", e);
                    push_recent_log(&mut guard, msg.clone());
                    logger::log(msg);
                    guard.child = None;
                    true
                }
            },
            None => true,
        };

        if !exited {
            continue;
        }

        if guard.restart_attempts >= MAX_RESTART_ATTEMPTS {
            push_recent_log(
                &mut guard,
                format!(
                    "[feishu] sidecar restart skipped: reached max attempts ({})",
                    MAX_RESTART_ATTEMPTS
                ),
            );
            continue;
        }

        guard.restart_attempts += 1;
        if let Err(e) = spawn_sidecar_locked(&mut guard, true) {
            let attempts = guard.restart_attempts;
            push_recent_log(
                &mut guard,
                format!(
                    "[feishu] sidecar restart failed attempt={}: {}",
                    attempts, e
                ),
            );
        }
    });
}

fn spawn_sidecar_locked(guard: &mut FeishuRuntimeManager, by_monitor: bool) -> Result<()> {
    if let Some(child) = guard.child.as_mut() {
        if child.try_wait()?.is_none() {
            return Ok(());
        }
        guard.child = None;
    }

    let exe = std::env::current_exe()?;
    let mut cmd = Command::new(exe);
    cmd.arg("feishu-sidecar")
        .arg("--master-api")
        .arg(guard.master_api.clone())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| anyhow!("启动 sidecar 失败: {}", e))?;
    let pid = child.id();
    if let Some(stdout) = child.stdout.take() {
        spawn_log_pipe_reader(stdout, "stdout");
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_log_pipe_reader(stderr, "stderr");
    }
    guard.child = Some(child);
    if !by_monitor {
        guard.restart_attempts = 0;
    }
    let log = if by_monitor {
        format!(
            "[feishu] sidecar restarted pid={} attempt={}",
            pid, guard.restart_attempts
        )
    } else {
        format!("[feishu] sidecar process started pid={}", pid)
    };
    push_recent_log(guard, log.clone());
    logger::log(log);
    Ok(())
}

fn spawn_log_pipe_reader<R: Read + Send + 'static>(reader: R, source: &'static str) {
    let mgr = MANAGER.get().cloned();
    thread::spawn(move || {
        let buf = BufReader::new(reader);
        for line in buf.lines().map_while(Result::ok) {
            let text = format!("[feishu-sidecar:{}] {}", source, line);
            logger::log(text.clone());
            if let Some(m) = &mgr {
                if let Ok(mut guard) = m.lock() {
                    push_recent_log(&mut guard, text.clone());
                }
            }
        }
    });
}

fn push_recent_log(guard: &mut FeishuRuntimeManager, line: String) {
    if guard.recent_logs.len() >= MAX_SIDECAR_LOGS {
        guard.recent_logs.pop_front();
    }
    guard.recent_logs.push_back(line);
}

fn sidecar_log(msg: impl Into<String>) {
    let text = msg.into();
    logger::log(text.clone());
    println!("{}", text);
}

pub async fn run_sidecar_process(master_api: String) -> Result<()> {
    sidecar_log(format!("[feishu-sidecar] started, master_api={}", master_api));
    let http = Client::new();
    let greeted = Arc::new(Mutex::new(HashSet::<String>::new()));
    let mut reconnect_backoff_secs = 1u64;

    loop {
        let channels = match load_channels() {
            Ok(v) => v,
            Err(e) => {
                sidecar_log(format!("[feishu-sidecar] load channels failed err={}", e));
                sleep(Duration::from_secs(3)).await;
                continue;
            }
        };

        if !channels.feishu.enabled {
            reconnect_backoff_secs = 1;
            sleep(Duration::from_secs(2)).await;
            continue;
        }

        let ws_url = std::env::var("DIMCLAW_FEISHU_WS_URL")
            .unwrap_or_else(|_| "wss://open.feishu.cn/open-apis/ws".to_string());
        let conn = connect_async(&ws_url).await;
        let (mut stream, _) = match conn {
            Ok(v) => v,
            Err(e) => {
                sidecar_log(format!(
                    "[feishu-sidecar] websocket connect failed err={}, retry_in={}s",
                    e, reconnect_backoff_secs
                ));
                sleep(Duration::from_secs(reconnect_backoff_secs)).await;
                reconnect_backoff_secs = (reconnect_backoff_secs * 2).min(60);
                continue;
            }
        };
        sidecar_log("[feishu-sidecar] websocket connected");
        reconnect_backoff_secs = 1;
        let mut heartbeat = interval(Duration::from_secs(15));

        loop {
            tokio::select! {
                _ = heartbeat.tick() => {
                    if let Err(e) = stream.send(Message::Ping(Vec::new().into())).await {
                        sidecar_log(format!("[feishu-sidecar] heartbeat ping failed err={}", e));
                        break;
                    }
                }
                maybe_msg = stream.next() => {
                    let Some(msg) = maybe_msg else {
                        sidecar_log("[feishu-sidecar] websocket stream closed");
                        break;
                    };
                    let msg = match msg {
                        Ok(v) => v,
                        Err(e) => {
                            sidecar_log(format!("[feishu-sidecar] websocket recv failed err={}", e));
                            break;
                        }
                    };
                    if !msg.is_text() {
                        continue;
                    }

                    let text = msg.into_text().unwrap_or_default();
                    let event: serde_json::Value = match serde_json::from_str(&text) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    let channels_now = match load_channels() {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                    let master = load_agents().map(|v| v.master_or_default()).unwrap_or_default();

                    if is_private_first_talk(&event, &greeted)
                        && master.initialized
                        && !channels_now.feishu.webhook_url.trim().is_empty()
                    {
                        let welcome = format!("你好，我是{}，{}", master.name, master.persona);
                        let _ = post_webhook_text(&http, &channels_now.feishu.webhook_url, &welcome).await;
                    }

                    if let Some(command) = extract_text_command(&event) {
                        let submit_body = serde_json::json!({
                            "title": "飞书任务",
                            "command": command,
                            "timeout_secs": 15
                        });
                        let res = http
                            .post(format!("{}/api/plugins/task", master_api.trim_end_matches('/')))
                            .json(&submit_body)
                            .send()
                            .await;
                        if let Err(e) = res {
                            sidecar_log(format!("[feishu-sidecar] submit task failed err={}", e));
                        }
                    }
                }
            }
        }

        sidecar_log(format!(
            "[feishu-sidecar] reconnect in {}s",
            reconnect_backoff_secs
        ));
        sleep(Duration::from_secs(reconnect_backoff_secs)).await;
        reconnect_backoff_secs = (reconnect_backoff_secs * 2).min(60);
    }
}

async fn post_webhook_text(client: &Client, url: &str, text: &str) -> Result<()> {
    let resp = client
        .post(url)
        .json(&serde_json::json!({
            "msg_type": "text",
            "content": { "text": text }
        }))
        .send()
        .await?;
    if !resp.status().is_success() {
        let code = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("HTTP {} {}", code, body));
    }
    Ok(())
}

fn is_private_first_talk(event: &serde_json::Value, greeted: &Arc<Mutex<HashSet<String>>>) -> bool {
    let is_private = event
        .pointer("/event/message/chat_type")
        .and_then(|v| v.as_str())
        .map(|v| v == "p2p")
        .unwrap_or(false);
    if !is_private {
        return false;
    }

    let chat_id = event
        .pointer("/event/message/chat_id")
        .and_then(|v| v.as_str())
        .or_else(|| event.pointer("/event/open_chat_id").and_then(|v| v.as_str()))
        .unwrap_or_default()
        .to_string();
    if chat_id.is_empty() {
        return false;
    }

    if let Ok(mut set) = greeted.lock() {
        if set.contains(&chat_id) {
            false
        } else {
            set.insert(chat_id);
            true
        }
    } else {
        false
    }
}

fn extract_text_command(v: &serde_json::Value) -> Option<String> {
    if let Some(s) = v
        .get("text")
        .and_then(|v| v.as_str())
        .or_else(|| v.pointer("/event/message/text").and_then(|v| v.as_str()))
    {
        return Some(s.to_string());
    }
    if let Some(content) = v.pointer("/event/message/content").and_then(|v| v.as_str()) {
        if let Ok(inner) = serde_json::from_str::<serde_json::Value>(content) {
            if let Some(text) = inner.get("text").and_then(|v| v.as_str()) {
                return Some(text.to_string());
            }
        }
    }
    None
}
