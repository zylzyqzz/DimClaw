use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures_util::StreamExt;
use reqwest::Client;
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tokio_util::sync::CancellationToken;

use crate::configs::load_channels;
use crate::core::logger;
use crate::core::storage::TaskStorage;
use crate::core::task_service::submit_task;

pub struct FeishuSidecar {
    storage: Arc<TaskStorage>,
    cancellation: CancellationToken,
    http: Client,
}

impl FeishuSidecar {
    pub fn new(storage: Arc<TaskStorage>, cancellation: CancellationToken) -> Self {
        Self {
            storage,
            cancellation,
            http: Client::new(),
        }
    }

    pub async fn run(self) -> Result<()> {
        logger::log("[飞书] sidecar 启动");
        loop {
            if self.cancellation.is_cancelled() {
                logger::log("[飞书] sidecar 收到停止信号");
                return Ok(());
            }

            let channels = match load_channels() {
                Ok(v) => v,
                Err(e) => {
                    logger::log(format!("[飞书] 读取配置失败 err={}", e));
                    sleep(Duration::from_secs(3)).await;
                    continue;
                }
            };

            if !channels.feishu.enabled {
                sleep(Duration::from_secs(2)).await;
                continue;
            }

            let ws_url = std::env::var("DIMCLAW_FEISHU_WS_URL")
                .unwrap_or_else(|_| "wss://open.feishu.cn/open-apis/ws".to_string());
            logger::log(format!("[飞书] 尝试连接 WebSocket {}", ws_url));
            let conn = connect_async(&ws_url).await;
            let (mut stream, _) = match conn {
                Ok(v) => v,
                Err(e) => {
                    logger::log(format!("[飞书] 连接失败 err={}", e));
                    sleep(Duration::from_secs(5)).await;
                    continue;
                }
            };
            logger::log("[飞书] WebSocket 已连接，开始接收事件");

            while let Some(msg) = stream.next().await {
                if self.cancellation.is_cancelled() {
                    return Ok(());
                }
                let msg = match msg {
                    Ok(v) => v,
                    Err(e) => {
                        logger::log(format!("[飞书] 收包失败 err={}", e));
                        break;
                    }
                };
                if !msg.is_text() {
                    continue;
                }
                let text = msg.into_text().unwrap_or_default();
                logger::log(format!("[飞书] 收到事件 {}", text));
                if let Some(command) = extract_text_command(&text) {
                    let created = submit_task(
                        self.storage.clone(),
                        "飞书任务".to_string(),
                        command.clone(),
                        15,
                    )
                    .await;
                    match created {
                        Ok(task) => {
                            logger::log(format!("[飞书] 已提交任务 id={} command={}", task.id, command));
                            if !channels.feishu.webhook_url.trim().is_empty() {
                                let _ = self
                                    .http
                                    .post(&channels.feishu.webhook_url)
                                    .json(&serde_json::json!({
                                        "msg_type": "text",
                                        "content": {"text": format!("DimClaw 已接收任务: {}", task.id)}
                                    }))
                                    .send()
                                    .await;
                            }
                        }
                        Err(e) => logger::log(format!("[飞书] 提交任务失败 err={}", e)),
                    }
                }
            }
            logger::log("[飞书] WebSocket 连接断开，准备重连");
            sleep(Duration::from_secs(2)).await;
        }
    }
}

fn extract_text_command(raw: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    if let Some(s) = v
        .get("text")
        .and_then(|v| v.as_str())
        .or_else(|| v.pointer("/event/message/text").and_then(|v| v.as_str()))
    {
        return Some(s.to_string());
    }
    if let Some(content) = v
        .pointer("/event/message/content")
        .and_then(|v| v.as_str())
    {
        if let Ok(inner) = serde_json::from_str::<serde_json::Value>(content) {
            if let Some(text) = inner.get("text").and_then(|v| v.as_str()) {
                return Some(text.to_string());
            }
        }
    }
    None
}
