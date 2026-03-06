use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tiny_http::{Header, Method, Response, Server, StatusCode};

use crate::agents::agent::MessageContext;
use crate::configs::{
    default_agents, delete_custom_agent, load_agents, load_channels, load_models, load_security, save_agents,
    save_channels, save_custom_agent, save_models, save_security, AgentsFile, ChannelConfig, CustomAgentConfig,
    ModelProviderEntry, ModelsFile, SecurityConfig,
};
use crate::core::channel_router::route_channel_message;
use crate::core::hand_scheduler::HandScheduler;
use crate::core::logger;
use crate::core::storage::TaskStorage;
use crate::core::task::Task;
use crate::core::task_service::submit_task;
use crate::plugins;
use crate::providers::openai_compatible::OpenAiCompatibleProvider;
use crate::providers::traits::LlmProvider;
use crate::providers::types::ChatRequest;
use crate::skills;
use crate::skills::marketplace;
use tokio_util::sync::CancellationToken;

const INDEX_HTML: &str = include_str!("../src/web/static/index.html");
const STYLE_CSS: &str = include_str!("../src/web/static/style.css");
const SCRIPT_JS: &str = include_str!("../src/web/static/script.js");
const HANDS_HTML: &str = include_str!("../src/web/static/pages/hands.html");
const MARKETPLACE_HTML: &str = include_str!("../src/web/static/pages/marketplace.html");
const CHANNEL_DETAIL_HTML: &str = include_str!("../src/web/static/pages/channel_detail.html");
const AUDIT_HTML: &str = include_str!("../src/web/static/pages/audit.html");

static SERVER_STARTED_AT: OnceLock<Instant> = OnceLock::new();
static MODEL_STATUS: OnceLock<Arc<Mutex<ModelConnectionState>>> = OnceLock::new();
static HAND_SCHEDULER: OnceLock<Arc<HandScheduler>> = OnceLock::new();
static CHAT_PENDING: OnceLock<Arc<Mutex<HashMap<String, serde_json::Value>>>> = OnceLock::new();

#[derive(Clone, Debug, Serialize, Default)]
struct ModelConnectionState {
    status: String,
    message: String,
    provider: String,
    last_checked_at: String,
}

#[derive(Serialize)]
struct TaskView {
    id: String,
    title: String,
    status: String,
    step: u64,
    retry_count: u32,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
struct DashboardStats {
    uptime_secs: u64,
    queue_length: usize,
    model_count: usize,
}

#[derive(Serialize)]
struct ApiResult {
    success: bool,
    message: String,
}

#[derive(Serialize)]
struct TaskDetailView {
    id: String,
    title: String,
    status: String,
    step: u64,
    retry_count: u32,
    created_at: String,
    updated_at: String,
    error: Option<String>,
    payload: serde_json::Value,
}

#[derive(Deserialize)]
struct CreateTaskRequest {
    title: String,
    command: String,
    timeout_secs: Option<u64>,
}

#[derive(Deserialize)]
struct SetDefaultModelRequest {
    name: String,
}

#[derive(Deserialize)]
struct ChatApiRequest {
    message: String,
    #[serde(default)]
    history: Vec<serde_json::Value>,
    #[serde(default)]
    channel: String,
    #[serde(default)]
    session_id: String,
    #[serde(default)]
    user_id: String,
    #[serde(default)]
    chat_id: String,
    #[serde(default)]
    metadata: serde_json::Value,
}

#[derive(Serialize)]
struct ChatApiResponse {
    reply: String,
    agent_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_summary: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct SkillTestRequest {
    #[serde(default)]
    input: serde_json::Value,
    timeout_secs: Option<u64>,
}

#[derive(Deserialize)]
struct SkillImportRequest {
    #[serde(default)]
    skill: serde_json::Value,
    #[serde(default)]
    overwrite: bool,
    #[serde(default)]
    rename_to: String,
}

#[derive(Deserialize)]
struct MarketplaceImportRequest {
    #[serde(default)]
    repo_url: String,
}

pub fn run_server(host: String, port: u16, storage: Arc<TaskStorage>) -> Result<()> {
    SERVER_STARTED_AT.get_or_init(Instant::now);
    ensure_model_status();
    ensure_hand_scheduler();
    ensure_chat_pending();

    let bind = format!("{host}:{port}");
    let server = Server::http(&bind).map_err(|e| anyhow!("start web server failed: {e}"))?;
    logger::log(format!("[Web] listening on http://{bind}"));

    let api_base = format!("http://{}:{}", host, port);
    tokio_block_on(async {
        let _ = plugins::ensure_initialized(api_base).await;
        let _ = plugins::auto_start_enabled_plugins().await;
        let _ = test_default_model_connection().await;
        if let Some(s) = HAND_SCHEDULER.get() {
            let _ = s.start().await;
        }
    });

    for mut req in server.incoming_requests() {
        let url = req.url().to_string();
        let method = req.method().clone();
        let response = handle_request(&method, &url, &mut req, storage.clone());
        if let Err(e) = req.respond(response) {
            logger::log(format!("[Web] respond failed err={}", e));
        }
    }
    Ok(())
}

fn handle_request(
    method: &Method,
    full_url: &str,
    req: &mut tiny_http::Request,
    storage: Arc<TaskStorage>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let (url, query) = split_url_query(full_url);

    match (method, url.as_str()) {
        (&Method::Get, "/")
        | (&Method::Get, "/dashboard")
        | (&Method::Get, "/chat")
        | (&Method::Get, "/hands")
        | (&Method::Get, "/marketplace")
        | (&Method::Get, "/channel-detail")
        | (&Method::Get, "/audit")
        | (&Method::Get, "/settings/agents")
        | (&Method::Get, "/settings/models")
        | (&Method::Get, "/settings/channels")
        | (&Method::Get, "/settings/skills")
        | (&Method::Get, "/settings/plugins") => {
            return text_response(200, "text/html; charset=utf-8", INDEX_HTML.as_bytes().to_vec());
        }
        (&Method::Get, "/pages/hands.html") => {
            return text_response(200, "text/html; charset=utf-8", HANDS_HTML.as_bytes().to_vec());
        }
        (&Method::Get, "/pages/marketplace.html") => {
            return text_response(200, "text/html; charset=utf-8", MARKETPLACE_HTML.as_bytes().to_vec());
        }
        (&Method::Get, "/pages/channel_detail.html") => {
            return text_response(200, "text/html; charset=utf-8", CHANNEL_DETAIL_HTML.as_bytes().to_vec());
        }
        (&Method::Get, "/pages/audit.html") => {
            return text_response(200, "text/html; charset=utf-8", AUDIT_HTML.as_bytes().to_vec());
        }
        (&Method::Get, "/style.css") => {
            return text_response(200, "text/css; charset=utf-8", STYLE_CSS.as_bytes().to_vec());
        }
        (&Method::Get, "/script.js") => {
            return text_response(
                200,
                "application/javascript; charset=utf-8",
                SCRIPT_JS.as_bytes().to_vec(),
            );
        }
        _ => {}
    }

    if method == &Method::Get && url.starts_with("/task/") {
        return text_response(200, "text/html; charset=utf-8", INDEX_HTML.as_bytes().to_vec());
    }

    if method == &Method::Get && url == "/api/config/security" {
        return respond(load_security());
    }
    if method == &Method::Put && url == "/api/config/security" {
        return match parse_body_json::<SecurityConfig>(req) {
            Ok(v) => respond(save_security(&v).map(|_| v)),
            Err(e) => err_response(400, e),
        };
    }

    if method == &Method::Get && url == "/api/config/models" {
        return respond(load_models().map(|mut m| {
            normalize_models_file(&mut m);
            let _ = save_models(&m);
            m
        }));
    }
    if method == &Method::Post && url == "/api/config/models" {
        return match parse_body_json::<ModelProviderEntry>(req) {
            Ok(new_item) => respond(load_models().and_then(|mut m| {
                m.providers.retain(|v| v.name != new_item.name);
                m.providers.push(new_item);
                normalize_models_file(&mut m);
                save_models(&m)?;
                Ok(m)
            })),
            Err(e) => err_response(400, e),
        };
    }
    if method == &Method::Post && url == "/api/config/models/default" {
        return match parse_body_json::<SetDefaultModelRequest>(req) {
            Ok(body) => respond(load_models().and_then(|mut m| {
                let mut found = false;
                for p in &mut m.providers {
                    let is_target = p.name == body.name;
                    p.r#default = is_target;
                    if is_target {
                        found = true;
                    }
                }
                if !found {
                    return Err(anyhow!("provider not found"));
                }
                normalize_models_file(&mut m);
                save_models(&m)?;
                Ok(m)
            })),
            Err(e) => err_response(400, e),
        };
    }
    if method == &Method::Put && url.starts_with("/api/config/models/") {
        let name = url.trim_start_matches("/api/config/models/").to_string();
        return match parse_body_json::<ModelProviderEntry>(req) {
            Ok(mut item) => {
                item.name = name.clone();
                respond(load_models().and_then(|mut m| {
                    let mut updated = false;
                    for v in &mut m.providers {
                        if v.name == name {
                            *v = item.clone();
                            updated = true;
                        }
                    }
                    if !updated {
                        m.providers.push(item);
                    }
                    normalize_models_file(&mut m);
                    save_models(&m)?;
                    Ok(m)
                }))
            }
            Err(e) => err_response(400, e),
        };
    }
    if method == &Method::Delete && url.starts_with("/api/config/models/") {
        let name = url.trim_start_matches("/api/config/models/").to_string();
        return respond(load_models().and_then(|mut m| {
            m.providers.retain(|v| v.name != name);
            normalize_models_file(&mut m);
            save_models(&m)?;
            Ok(m)
        }));
    }

    if method == &Method::Get && url == "/api/config/agents" {
        return respond(load_agents());
    }
    if method == &Method::Put && url == "/api/config/agents" {
        return match parse_body_json::<AgentsFile>(req) {
            Ok(v) => respond(save_agents(&v).map(|_| v)),
            Err(e) => err_response(400, e),
        };
    }
    if method == &Method::Post && url == "/api/config/agents/reset" {
        let mut defaults = default_agents();
        if let Ok(existing) = load_agents() {
            defaults.master = existing.master;
        }
        return respond(save_agents(&defaults).map(|_| defaults));
    }

    if method == &Method::Get && url == "/api/config/channels" {
        return respond(load_channels());
    }
    if method == &Method::Get && url.starts_with("/api/config/channels/") {
        let name = url.trim_start_matches("/api/config/channels/").to_string();
        return respond(load_channels().and_then(|all| match name.as_str() {
            "feishu" => Ok(all.feishu),
            "telegram" => Ok(all.telegram),
            _ => Err(anyhow!("unknown channel")),
        }));
    }
    if method == &Method::Put && url.starts_with("/api/config/channels/") {
        let name = url.trim_start_matches("/api/config/channels/").to_string();
        return match parse_body_json::<ChannelConfig>(req) {
            Ok(channel) => respond(load_channels().and_then(|mut all| {
                match name.as_str() {
                    "feishu" => all.feishu = channel,
                    "telegram" => all.telegram = channel,
                    _ => return Err(anyhow!("unknown channel")),
                }
                save_channels(&all)?;
                Ok(all)
            })),
            Err(e) => err_response(400, e),
        };
    }

    if method == &Method::Get && url == "/api/agents/custom" {
        return respond(crate::configs::list_custom_agents());
    }
    if method == &Method::Post && url == "/api/agents/custom" {
        return match parse_body_json::<CustomAgentConfig>(req) {
            Ok(v) => respond(save_custom_agent(&v).map(|_| v)),
            Err(e) => err_response(400, e),
        };
    }
    if method == &Method::Put && url.starts_with("/api/agents/custom/") {
        let name = url.trim_start_matches("/api/agents/custom/").to_string();
        return match parse_body_json::<CustomAgentConfig>(req) {
            Ok(mut v) => {
                v.name = name;
                respond(save_custom_agent(&v).map(|_| v))
            }
            Err(e) => err_response(400, e),
        };
    }
    if method == &Method::Delete && url.starts_with("/api/agents/custom/") {
        let name = url.trim_start_matches("/api/agents/custom/");
        return respond(delete_custom_agent(name).map(|_| serde_json::json!({"success": true})));
    }

    if method == &Method::Get && url == "/api/skills" {
        return respond(skills::manager::list_skill_infos());
    }
    if method == &Method::Post && url == "/api/skills" {
        return match parse_body_json::<skills::manager::SkillUpsertRequest>(req) {
            Ok(v) => respond(skills::manager::save_skill_from_request(v)),
            Err(e) => err_response(400, e),
        };
    }
    if method == &Method::Post && url == "/api/skills/openclaw/import" {
        return match parse_body_json::<SkillImportRequest>(req) {
            Ok(v) => {
                let source = if v.skill.is_null() { "{}".to_string() } else { v.skill.to_string() };
                let rename = if v.rename_to.trim().is_empty() {
                    None
                } else {
                    Some(v.rename_to)
                };
                respond(skills::manager::import_openclaw(&source, v.overwrite, rename))
            }
            Err(e) => err_response(400, e),
        };
    }
    if method == &Method::Get && url.starts_with("/api/skills/export/") {
        let name = url.trim_start_matches("/api/skills/export/").trim();
        return respond(skills::manager::export_custom(name));
    }
    if method == &Method::Post && url == "/api/skills/import" {
        return match parse_body_json::<serde_json::Value>(req) {
            Ok(body) => {
                let parsed = serde_json::from_value::<SkillImportRequest>(body.clone()).unwrap_or(SkillImportRequest {
                    skill: body.clone(),
                    overwrite: false,
                    rename_to: String::new(),
                });
                let skill_payload = if parsed.skill.is_null() { body } else { parsed.skill };
                let rename = if parsed.rename_to.trim().is_empty() {
                    None
                } else {
                    Some(parsed.rename_to)
                };
                respond(skills::manager::import_custom(skill_payload, parsed.overwrite, rename))
            }
            Err(e) => err_response(400, e),
        };
    }
    if method == &Method::Delete && url.starts_with("/api/skills/") {
        let name = url.trim_start_matches("/api/skills/");
        return respond(skills::manager::delete_custom(name).map(|_| serde_json::json!({"success": true})));
    }
    if method == &Method::Post && url.starts_with("/api/skills/") && url.ends_with("/test") {
        let name = url
            .trim_start_matches("/api/skills/")
            .trim_end_matches("/test")
            .trim_end_matches('/');
        return match parse_body_json::<SkillTestRequest>(req) {
            Ok(body) => {
                let out = tokio_block_on(skills::manager::test_skill(
                    name,
                    body.input,
                    body.timeout_secs.unwrap_or(15),
                ));
                respond(out)
            }
            Err(e) => err_response(400, e),
        };
    }

    if method == &Method::Get && url == "/api/marketplace" {
        let q = query.get("q").cloned().unwrap_or_default();
        return respond(tokio_block_on(marketplace::list_marketplace(&q)));
    }
    if method == &Method::Post && url.starts_with("/api/marketplace/install/") {
        let name = url.trim_start_matches("/api/marketplace/install/");
        return respond(tokio_block_on(marketplace::install_marketplace_skill(name)));
    }
    if method == &Method::Post && url == "/api/marketplace/import" {
        return match parse_body_json::<MarketplaceImportRequest>(req) {
            Ok(body) => respond(tokio_block_on(marketplace::import_openclaw_online(&body.repo_url))),
            Err(e) => err_response(400, e),
        };
    }

    if method == &Method::Get && url == "/api/hands" {
        let statuses = HAND_SCHEDULER
            .get()
            .map(|s| s.get_status())
            .unwrap_or_default();
        return respond(Ok(statuses));
    }
    if method == &Method::Post && url.starts_with("/api/hands/trigger/") {
        let name = url.trim_start_matches("/api/hands/trigger/").to_string();
        let out = tokio_block_on(async {
            let scheduler = HAND_SCHEDULER.get().ok_or_else(|| anyhow!("scheduler not initialized"))?;
            scheduler.trigger_now(&name).await
        });
        return respond(out);
    }
    if method == &Method::Post && url.starts_with("/api/hands/pause/") {
        let name = url.trim_start_matches("/api/hands/pause/");
        if let Some(s) = HAND_SCHEDULER.get() {
            s.pause(name);
        }
        return respond(Ok(serde_json::json!({"success": true})));
    }
    if method == &Method::Post && url.starts_with("/api/hands/resume/") {
        let name = url.trim_start_matches("/api/hands/resume/");
        if let Some(s) = HAND_SCHEDULER.get() {
            s.resume(name);
        }
        return respond(Ok(serde_json::json!({"success": true})));
    }
    if method == &Method::Put && url.starts_with("/api/hands/") {
        return respond(Ok(serde_json::json!({"success": true, "message": "当前版本暂不支持持久化修改，已保留接口"})));
    }

    if method == &Method::Get && url == "/api/plugins" {
        return respond(tokio_block_on(plugins::list_installed_plugins()));
    }
    if method == &Method::Get && url == "/api/plugins/available" {
        return respond(tokio_block_on(plugins::list_available_plugins()));
    }
    if method == &Method::Post && url.starts_with("/api/plugins/install/") {
        let name = url.trim_start_matches("/api/plugins/install/");
        let payload = parse_body_json::<serde_json::Value>(req).unwrap_or_default();
        let cfg = plugins::parse_plugin_config_json(payload);
        let out = tokio_block_on(plugins::install_plugin(name, cfg));
        if out.is_ok() {
            let _ = mark_channel_plugin_installed(name, true);
        }
        return respond(out);
    }
    if method == &Method::Post && url.starts_with("/api/plugins/uninstall/") {
        let name = url.trim_start_matches("/api/plugins/uninstall/");
        let out = tokio_block_on(plugins::uninstall_plugin(name));
        if out.is_ok() {
            let _ = mark_channel_plugin_installed(name, false);
        }
        return respond(out);
    }
    if method == &Method::Post && url.starts_with("/api/plugins/enable/") {
        let name = url.trim_start_matches("/api/plugins/enable/");
        let payload = parse_body_json::<serde_json::Value>(req).unwrap_or_default();
        let cfg = plugins::parse_plugin_config_json(payload);
        return respond(tokio_block_on(plugins::enable_plugin(name, Some(cfg))));
    }
    if method == &Method::Post && url.starts_with("/api/plugins/disable/") {
        let name = url.trim_start_matches("/api/plugins/disable/");
        return respond(tokio_block_on(plugins::disable_plugin(name)));
    }
    if method == &Method::Put && url.starts_with("/api/plugins/config/") {
        let name = url.trim_start_matches("/api/plugins/config/");
        let payload = parse_body_json::<serde_json::Value>(req).unwrap_or_default();
        let auto_restart = payload.get("auto_restart").and_then(|v| v.as_bool()).unwrap_or(true);
        let cfg = plugins::parse_plugin_config_json(payload);
        return respond(tokio_block_on(plugins::update_plugin_config(name, cfg, auto_restart)));
    }
    if method == &Method::Get && url.starts_with("/api/plugins/status/") {
        let name = url.trim_start_matches("/api/plugins/status/");
        return respond(tokio_block_on(plugins::plugin_status(name)));
    }
    if method == &Method::Get && url == "/api/status/connections" {
        let out = tokio_block_on(async {
            let model = current_model_status();
            let plugins = plugins::connection_status_map().await?;
            Ok::<_, anyhow::Error>(serde_json::json!({ "model": model, "plugins": plugins }))
        });
        return respond(out);
    }
    if method == &Method::Get && url == "/api/feishu/status" {
        return respond(tokio_block_on(plugins::plugin_status("feishu")));
    }

    if method == &Method::Post && url == "/api/test/model" {
        let provider_name = query
            .get("name")
            .cloned()
            .filter(|v| !v.trim().is_empty())
            .or_else(get_default_provider_name)
            .unwrap_or_else(|| "default".to_string());
        return respond(tokio_block_on(test_model_connection(provider_name)));
    }

    if method == &Method::Post && url == "/api/chat" {
        return match parse_body_json::<ChatApiRequest>(req) {
            Ok(body) => respond(tokio_block_on(handle_chat(body))),
            Err(e) => err_response(400, e),
        };
    }

    if method == &Method::Post && url == "/api/plugins/task" {
        return match parse_body_json::<CreateTaskRequest>(req) {
            Ok(body) => respond(tokio_block_on(submit_task(
                storage.clone(),
                body.title,
                body.command,
                body.timeout_secs.unwrap_or(15),
            ))),
            Err(e) => err_response(400, e),
        };
    }
    if method == &Method::Get && url == "/api/tasks" {
        let out = tokio_block_on(async {
            let mut tasks = storage.list_tasks().await?;
            tasks.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
            let data: Vec<TaskView> = tasks
                .into_iter()
                .take(50)
                .map(|t| TaskView {
                    id: t.id,
                    title: t.title,
                    status: t.status.to_string(),
                    step: t.step,
                    retry_count: t.retry_count,
                    created_at: t.created_at.to_rfc3339(),
                    updated_at: t.updated_at.to_rfc3339(),
                })
                .collect();
            Ok::<_, anyhow::Error>(data)
        });
        return respond(out);
    }
    if method == &Method::Post && url == "/api/tasks" {
        return match parse_body_json::<CreateTaskRequest>(req) {
            Ok(body) => respond(tokio_block_on(submit_task(
                storage.clone(),
                body.title,
                body.command,
                body.timeout_secs.unwrap_or(15),
            ))),
            Err(e) => err_response(400, e),
        };
    }
    if method == &Method::Get && url.starts_with("/api/tasks/") {
        let id = url.trim_start_matches("/api/tasks/");
        let out = tokio_block_on(async {
            let task = storage.get_task(id).await?;
            Ok::<_, anyhow::Error>(make_task_detail_view(task))
        });
        return respond(out);
    }
    if method == &Method::Get && url == "/api/logs/recent" {
        let level = query.get("level").cloned();
        let lines = read_recent_logs(80, level.as_deref());
        return respond(Ok(serde_json::json!({ "lines": lines })));
    }
    if method == &Method::Get && url == "/api/dashboard/stats" {
        let out = tokio_block_on(async {
            let tasks = storage.list_tasks().await?;
            let queue_length = tasks.iter().filter(|t| !t.status.is_terminal()).count();
            let models = load_models()?;
            let uptime_secs = SERVER_STARTED_AT.get().map(|i| i.elapsed().as_secs()).unwrap_or(0);
            Ok::<_, anyhow::Error>(DashboardStats {
                uptime_secs,
                queue_length,
                model_count: models.providers.len(),
            })
        });
        return respond(out);
    }

    err_response(404, "not found".to_string())
}

fn make_task_detail_view(task: Task) -> TaskDetailView {
    TaskDetailView {
        id: task.id,
        title: task.title,
        status: task.status.to_string(),
        step: task.step,
        retry_count: task.retry_count,
        created_at: task.created_at.to_rfc3339(),
        updated_at: task.updated_at.to_rfc3339(),
        error: task.error,
        payload: task.payload,
    }
}

async fn handle_chat(body: ChatApiRequest) -> Result<ChatApiResponse> {
    let session_key = build_session_key(&body);
    if let Some(intent) = detect_intent(&body.message, &session_key) {
        let result = skills::manager::test_skill(&intent.skill, intent.args.clone(), 30).await;
        return Ok(match result {
            Ok(v) if v.success => ChatApiResponse {
                reply: format!("执行成功：{}。{}", intent.human_label, normalize_output(&v.stdout)),
                agent_name: "Executor".to_string(),
                tool_summary: Some(serde_json::json!({
                    "tool": intent.skill,
                    "args": intent.args,
                    "success": true,
                    "stdout": v.stdout,
                    "stderr": v.stderr,
                    "exit_code": v.exit_code
                })),
            },
            Ok(v) => ChatApiResponse {
                reply: format!("执行失败：{}。错误：{}", intent.human_label, normalize_output(&v.stderr)),
                agent_name: "Executor".to_string(),
                tool_summary: Some(serde_json::json!({
                    "tool": intent.skill,
                    "args": intent.args,
                    "success": false,
                    "stdout": v.stdout,
                    "stderr": v.stderr,
                    "exit_code": v.exit_code
                })),
            },
            Err(e) => ChatApiResponse {
                reply: format!("执行失败：{}。错误：{}", intent.human_label, e),
                agent_name: "Executor".to_string(),
                tool_summary: Some(serde_json::json!({
                    "tool": intent.skill,
                    "args": intent.args,
                    "success": false,
                    "error": e.to_string()
                })),
            },
        });
    }

    if !body.channel.trim().is_empty() {
        let ctx = MessageContext {
            channel: body.channel,
            message: body.message,
            session_id: body.session_id,
            user_id: body.user_id,
            chat_id: body.chat_id,
            metadata: body.metadata,
        };
        let routed = route_channel_message(ctx, body.history).await?;
        return Ok(ChatApiResponse {
            reply: routed.reply,
            agent_name: routed.agent_name,
            tool_summary: None,
        });
    }
    chat_with_default_provider(body).await
}

#[derive(Clone)]
struct DetectedIntent {
    skill: String,
    args: serde_json::Value,
    human_label: String,
}

fn detect_intent(message: &str, session_key: &str) -> Option<DetectedIntent> {
    let text = message.trim();
    let lower = text.to_lowercase();

    if (lower.contains("创建") || lower.contains("写入")) && lower.contains("文件") {
        let path = extract_between(text, "文件", "内容").unwrap_or_else(|| "test.txt".to_string());
        let content = extract_after(text, "内容为").or_else(|| extract_after(text, "内容")).unwrap_or_default();
        if content.trim().is_empty() {
            put_pending(session_key, serde_json::json!({"kind":"file_write","path":path.trim(),"mode":"create"}));
            return Some(DetectedIntent {
                skill: "shell_command".to_string(),
                args: serde_json::json!({"command":"echo 请补充文件内容，例如：内容为 hello"}),
                human_label: "等待补全文件内容".to_string(),
            });
        }
        clear_pending(session_key);
        return Some(DetectedIntent {
            skill: "file_write".to_string(),
            args: serde_json::json!({"path": path.trim(),"content": content.trim(),"mode": "create"}),
            human_label: "创建文件".to_string(),
        });
    }

    if let Some(pending) = get_pending(session_key) {
        if pending.get("kind").and_then(|v| v.as_str()) == Some("file_write") && (lower.contains("内容") || !text.is_empty()) {
            let path = pending.get("path").and_then(|v| v.as_str()).unwrap_or("test.txt");
            let content = extract_after(text, "内容为").or_else(|| extract_after(text, "内容")).unwrap_or_else(|| text.to_string());
            clear_pending(session_key);
            return Some(DetectedIntent {
                skill: "file_write".to_string(),
                args: serde_json::json!({ "path": path, "content": content.trim(), "mode": "create" }),
                human_label: "补全并创建文件".to_string(),
            });
        }
    }

    if lower.contains("读取") && lower.contains(".txt") {
        let path = text.split_whitespace().find(|s| s.contains(".txt")).unwrap_or("test.txt");
        return Some(DetectedIntent {
            skill: "file_read".to_string(),
            args: serde_json::json!({ "path": path }),
            human_label: "读取文件".to_string(),
        });
    }

    if lower.contains("删除") && lower.contains(".txt") {
        let path = text.split_whitespace().find(|s| s.contains(".txt")).unwrap_or("test.txt");
        return Some(DetectedIntent {
            skill: "file_delete".to_string(),
            args: serde_json::json!({ "path": path, "confirm": true }),
            human_label: "删除文件".to_string(),
        });
    }

    if lower.contains("列出当前目录") || (lower.contains("列出") && lower.contains("目录")) {
        return Some(DetectedIntent {
            skill: "file_list".to_string(),
            args: serde_json::json!({ "path": "." }),
            human_label: "列出目录".to_string(),
        });
    }

    if lower.starts_with("执行 ") || lower.contains("执行 ls") || lower.contains("执行 dir") {
        let command = text.strip_prefix("执行 ").unwrap_or(text).trim();
        return Some(DetectedIntent {
            skill: "shell_command".to_string(),
            args: serde_json::json!({ "command": command }),
            human_label: "执行命令".to_string(),
        });
    }

    if lower.contains("打开百度") || lower.contains("baidu") {
        return Some(DetectedIntent {
            skill: "browser_open".to_string(),
            args: serde_json::json!({ "url": "https://www.baidu.com" }),
            human_label: "打开百度".to_string(),
        });
    }

    if lower.contains("打开谷歌") || lower.contains("google") {
        return Some(DetectedIntent {
            skill: "browser_open".to_string(),
            args: serde_json::json!({ "url": "https://www.google.com" }),
            human_label: "打开谷歌".to_string(),
        });
    }

    if lower.contains("截图") {
        return Some(DetectedIntent {
            skill: "browser_screenshot".to_string(),
            args: serde_json::json!({ "path": "./screenshot.png" }),
            human_label: "截图当前页面".to_string(),
        });
    }

    if lower.contains("cpu") || lower.contains("使用率") {
        return Some(DetectedIntent {
            skill: "system_monitor".to_string(),
            args: serde_json::json!({ "metrics": ["cpu"] }),
            human_label: "查看 CPU 使用率".to_string(),
        });
    }

    if lower.contains("杀掉进程") || lower.contains("kill") {
        if let Some(pid) = text.split_whitespace().find_map(|p| p.parse::<u32>().ok()) {
            return Some(DetectedIntent {
                skill: "process_kill".to_string(),
                args: serde_json::json!({ "pid": pid }),
                human_label: "终止进程".to_string(),
            });
        }
    }

    if lower.contains("重启") && lower.contains("服务") {
        let svc = text.replace("重启", "").replace("服务", "").trim().to_string();
        return Some(DetectedIntent {
            skill: "service_control".to_string(),
            args: serde_json::json!({ "name": if svc.is_empty() {"nginx"} else {svc.as_str()}, "action": "restart" }),
            human_label: "重启服务".to_string(),
        });
    }

    None
}

fn build_session_key(body: &ChatApiRequest) -> String {
    if !body.session_id.trim().is_empty() {
        return body.session_id.clone();
    }
    let channel = if body.channel.trim().is_empty() { "local" } else { body.channel.trim() };
    let user = if body.user_id.trim().is_empty() { "anonymous" } else { body.user_id.trim() };
    format!("{}:{}", channel, user)
}

fn ensure_chat_pending() {
    CHAT_PENDING.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));
}

fn put_pending(session_key: &str, value: serde_json::Value) {
    ensure_chat_pending();
    if let Some(m) = CHAT_PENDING.get() {
        if let Ok(mut guard) = m.lock() {
            guard.insert(session_key.to_string(), value);
        }
    }
}

fn get_pending(session_key: &str) -> Option<serde_json::Value> {
    ensure_chat_pending();
    CHAT_PENDING
        .get()
        .and_then(|m| m.lock().ok())
        .and_then(|guard| guard.get(session_key).cloned())
}

fn clear_pending(session_key: &str) {
    ensure_chat_pending();
    if let Some(m) = CHAT_PENDING.get() {
        if let Ok(mut guard) = m.lock() {
            guard.remove(session_key);
        }
    }
}

fn extract_between(source: &str, begin: &str, end: &str) -> Option<String> {
    let start = source.find(begin)? + begin.len();
    let rest = &source[start..];
    let stop = rest.find(end)?;
    Some(rest[..stop].trim().to_string())
}

fn extract_after(source: &str, marker: &str) -> Option<String> {
    let idx = source.find(marker)? + marker.len();
    Some(source[idx..].trim().to_string())
}

fn normalize_output(v: &str) -> String {
    let trimmed = v.trim();
    if trimmed.is_empty() {
        "无输出".to_string()
    } else {
        trimmed.to_string()
    }
}

async fn chat_with_default_provider(body: ChatApiRequest) -> Result<ChatApiResponse> {
    let mut models = load_models()?;
    normalize_models_file(&mut models);
    let provider = select_default_enabled_provider(&models).ok_or_else(|| anyhow!("no provider"))?;

    let client = OpenAiCompatibleProvider::new(
        provider.name.clone(),
        provider.base_url,
        provider.api_key,
        provider.timeout_secs,
        1,
    )?;
    let req = ChatRequest {
        system_prompt: "你是 DimClaw 智能助手，请用简洁中文回复。".to_string(),
        user_prompt: body.message,
        model: provider.model,
        temperature: provider.temperature,
        max_tokens: provider.max_tokens.min(1024),
    };
    let reply = client
        .chat(req, CancellationToken::new())
        .await
        .map(|v| v.content)
        .unwrap_or_else(|_| "模型暂时不可用，请检查配置。".to_string());
    Ok(ChatApiResponse {
        reply,
        agent_name: "Planner".to_string(),
        tool_summary: None,
    })
}

async fn test_model_connection(name: String) -> Result<ApiResult> {
    let mut models = load_models()?;
    normalize_models_file(&mut models);
    let item = models
        .providers
        .into_iter()
        .find(|v| v.name == name)
        .ok_or_else(|| anyhow!("provider not found"))?;

    let provider = OpenAiCompatibleProvider::new(
        item.name.clone(),
        item.base_url.clone(),
        item.api_key.clone(),
        item.timeout_secs,
        1,
    )?;
    let request = ChatRequest {
        system_prompt: "You are a connection test assistant.".to_string(),
        user_prompt: "Please reply: ok".to_string(),
        model: item.model,
        temperature: item.temperature,
        max_tokens: 16,
    };
    let result = provider.chat(request, CancellationToken::new()).await;
    let ok = result.is_ok();
    Ok(ApiResult {
        success: ok,
        message: if ok {
            "连接成功".to_string()
        } else {
            format!("连接失败: {}", result.err().unwrap())
        },
    })
}

async fn test_default_model_connection() -> Result<()> {
    ensure_model_status();
    let mut state = current_model_status();
    state.status = "connecting".to_string();
    state.message = "正在连接".to_string();
    set_model_status(state.clone());

    let mut models = load_models().unwrap_or_default();
    normalize_models_file(&mut models);
    let provider = select_default_enabled_provider(&models);
    let Some(provider) = provider else {
        state.status = "disconnected".to_string();
        state.message = "未配置启用模型".to_string();
        state.last_checked_at = chrono::Utc::now().to_rfc3339();
        set_model_status(state);
        return Ok(());
    };

    state.provider = provider.name.clone();
    let out = test_model_connection(provider.name).await?;
    state.status = if out.success { "connected" } else { "disconnected" }.to_string();
    state.message = out.message;
    state.last_checked_at = chrono::Utc::now().to_rfc3339();
    set_model_status(state);
    Ok(())
}

fn ensure_model_status() {
    MODEL_STATUS.get_or_init(|| Arc::new(Mutex::new(ModelConnectionState::default())));
}

fn current_model_status() -> ModelConnectionState {
    ensure_model_status();
    MODEL_STATUS
        .get()
        .and_then(|v| v.lock().ok().map(|g| g.clone()))
        .unwrap_or_default()
}

fn set_model_status(state: ModelConnectionState) {
    if let Some(s) = MODEL_STATUS.get() {
        if let Ok(mut guard) = s.lock() {
            *guard = state;
        }
    }
}

fn ensure_hand_scheduler() {
    HAND_SCHEDULER.get_or_init(|| Arc::new(HandScheduler::default()));
}

fn parse_body_json<T: serde::de::DeserializeOwned>(req: &mut tiny_http::Request) -> std::result::Result<T, String> {
    let mut body = String::new();
    req.as_reader()
        .read_to_string(&mut body)
        .map_err(|e| format!("read request body failed: {e}"))?;
    serde_json::from_str(&body).map_err(|e| format!("json parse failed: {e}"))
}

fn tokio_block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("create tokio runtime failed");
    rt.block_on(fut)
}

fn respond<T: Serialize>(result: Result<T>) -> Response<std::io::Cursor<Vec<u8>>> {
    match result.and_then(|v| Ok(serde_json::to_vec(&v)?)) {
        Ok(bytes) => json_response(200, bytes),
        Err(e) => err_response(500, e.to_string()),
    }
}

fn json_response(status: u16, body: Vec<u8>) -> Response<std::io::Cursor<Vec<u8>>> {
    let mut resp = Response::new(
        StatusCode(status),
        vec![Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap()],
        std::io::Cursor::new(body),
        None,
        None,
    );
    resp.add_header(Header::from_bytes("Cache-Control", "no-store").unwrap());
    resp
}

fn text_response(status: u16, content_type: &str, body: Vec<u8>) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::new(
        StatusCode(status),
        vec![Header::from_bytes("Content-Type", content_type).unwrap()],
        std::io::Cursor::new(body),
        None,
        None,
    )
}

fn err_response(status: u16, msg: String) -> Response<std::io::Cursor<Vec<u8>>> {
    json_response(status, serde_json::to_vec(&serde_json::json!({ "error": msg })).unwrap_or_default())
}

fn split_url_query(raw: &str) -> (String, HashMap<String, String>) {
    let mut it = raw.splitn(2, '?');
    let path = it.next().unwrap_or_default().to_string();
    let query = it.next().unwrap_or_default();
    let mut map = HashMap::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let mut kv = pair.splitn(2, '=');
        map.insert(
            kv.next().unwrap_or_default().to_string(),
            kv.next().unwrap_or_default().to_string(),
        );
    }
    (path, map)
}

fn normalize_models_file(file: &mut ModelsFile) {
    if file.providers.is_empty() {
        return;
    }
    let mut default_index: Option<usize> = None;
    for (idx, provider) in file.providers.iter_mut().enumerate() {
        if provider.r#default {
            if default_index.is_none() {
                default_index = Some(idx);
            } else {
                provider.r#default = false;
            }
        }
    }
    if default_index.is_none() {
        if let Some(idx) = file.providers.iter().position(|p| p.enabled) {
            file.providers[idx].r#default = true;
        } else if let Some(first) = file.providers.first_mut() {
            first.r#default = true;
        }
    }
}

fn get_default_provider_name() -> Option<String> {
    let mut models = load_models().ok()?;
    normalize_models_file(&mut models);
    models
        .providers
        .iter()
        .find(|p| p.r#default)
        .map(|p| p.name.clone())
        .or_else(|| models.providers.first().map(|p| p.name.clone()))
}

fn select_default_enabled_provider(models: &ModelsFile) -> Option<ModelProviderEntry> {
    models
        .providers
        .iter()
        .find(|p| p.enabled && p.r#default)
        .cloned()
        .or_else(|| models.providers.iter().find(|p| p.enabled).cloned())
}

fn read_recent_logs(limit: usize, level: Option<&str>) -> Vec<String> {
    let log_path = Path::new("./logs/dimclaw.log");
    if !log_path.exists() {
        return vec![];
    }
    let body = std::fs::read_to_string(log_path).unwrap_or_default();
    let mut lines: Vec<String> = body.lines().map(|s| s.to_string()).collect();
    if let Some(level_filter) = level {
        if !level_filter.trim().is_empty() {
            let upper = level_filter.to_ascii_uppercase();
            lines.retain(|line| line.to_ascii_uppercase().contains(&upper));
        }
    }
    if lines.len() > limit {
        lines.split_off(lines.len() - limit)
    } else {
        lines
    }
}

fn mark_channel_plugin_installed(name: &str, installed: bool) -> Result<()> {
    let mut channels = load_channels()?;
    match name {
        "feishu" => channels.feishu.plugin_installed = installed,
        "telegram" => channels.telegram.plugin_installed = installed,
        _ => {}
    }
    save_channels(&channels)
}


