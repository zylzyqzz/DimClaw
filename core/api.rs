use std::sync::Arc;

use anyhow::Result;
use serde::Serialize;
use tiny_http::{Header, Method, Response, Server, StatusCode};

use crate::configs::{
    load_agents, load_channels, load_models, save_agents, save_channels, save_models, AgentsFile,
    ChannelsFile, FeishuChannelConfig, ModelProviderEntry,
};
use crate::core::logger;
use crate::core::storage::TaskStorage;

const INDEX_HTML: &str = include_str!("../src/web/static/index.html");
const STYLE_CSS: &str = include_str!("../src/web/static/style.css");
const SCRIPT_JS: &str = include_str!("../src/web/static/script.js");

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

pub fn run_server(host: String, port: u16, storage: Arc<TaskStorage>) -> Result<()> {
    let bind = format!("{host}:{port}");
    let server = Server::http(&bind).map_err(|e| anyhow::anyhow!("启动 Web 服务失败: {e}"))?;
    logger::log(format!("[Web] 服务已启动 http://{bind}"));
    for mut req in server.incoming_requests() {
        let url = req.url().to_string();
        let method = req.method().clone();
        let response = handle_request(&method, &url, &mut req, storage.clone());
        if let Err(e) = req.respond(response) {
            logger::log(format!("[Web] 响应失败 err={}", e));
        }
    }
    Ok(())
}

fn handle_request(
    method: &Method,
    url: &str,
    req: &mut tiny_http::Request,
    storage: Arc<TaskStorage>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    match (method, url) {
        (&Method::Get, "/") | (&Method::Get, "/wizard") | (&Method::Get, "/dashboard") => {
            return text_response(200, "text/html; charset=utf-8", INDEX_HTML.as_bytes().to_vec());
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

    if method == &Method::Get && url == "/api/config/models" {
        return match load_models().and_then(|m| to_json_bytes(&m)) {
            Ok(v) => json_response(200, v),
            Err(e) => err_response(500, format!("读取模型配置失败: {e}")),
        };
    }

    if method == &Method::Post && url == "/api/config/models" {
        return match parse_body_json::<ModelProviderEntry>(req) {
            Ok(new_item) => match load_models().and_then(|mut m| {
                m.providers.retain(|v| v.name != new_item.name);
                m.providers.push(new_item);
                save_models(&m)?;
                to_json_bytes(&m)
            }) {
                Ok(v) => json_response(200, v),
                Err(e) => err_response(500, format!("保存模型配置失败: {e}")),
            },
            Err(e) => err_response(400, e),
        };
    }

    if method == &Method::Put && url.starts_with("/api/config/models/") {
        let name = url.trim_start_matches("/api/config/models/").to_string();
        return match parse_body_json::<ModelProviderEntry>(req) {
            Ok(mut item) => {
                item.name = name.clone();
                match load_models().and_then(|mut m| {
                    let mut updated = false;
                    for v in &mut m.providers {
                        if v.name == name {
                            *v = item.clone();
                            updated = true;
                            break;
                        }
                    }
                    if !updated {
                        m.providers.push(item);
                    }
                    save_models(&m)?;
                    to_json_bytes(&m)
                }) {
                    Ok(v) => json_response(200, v),
                    Err(e) => err_response(500, format!("更新模型配置失败: {e}")),
                }
            }
            Err(e) => err_response(400, e),
        };
    }

    if method == &Method::Delete && url.starts_with("/api/config/models/") {
        let name = url.trim_start_matches("/api/config/models/").to_string();
        return match load_models().and_then(|mut m| {
            m.providers.retain(|v| v.name != name);
            save_models(&m)?;
            to_json_bytes(&m)
        }) {
            Ok(v) => json_response(200, v),
            Err(e) => err_response(500, format!("删除模型配置失败: {e}")),
        };
    }

    if method == &Method::Get && url == "/api/config/agents" {
        return match load_agents().and_then(|m| to_json_bytes(&m)) {
            Ok(v) => json_response(200, v),
            Err(e) => err_response(500, format!("读取智能体配置失败: {e}")),
        };
    }

    if method == &Method::Put && url == "/api/config/agents" {
        return match parse_body_json::<AgentsFile>(req) {
            Ok(file) => match save_agents(&file).and_then(|_| to_json_bytes(&file)) {
                Ok(v) => json_response(200, v),
                Err(e) => err_response(500, format!("保存智能体配置失败: {e}")),
            },
            Err(e) => err_response(400, e),
        };
    }

    if method == &Method::Get && url == "/api/config/channels" {
        return match load_channels().and_then(|m| to_json_bytes(&m)) {
            Ok(v) => json_response(200, v),
            Err(e) => err_response(500, format!("读取通道配置失败: {e}")),
        };
    }

    if method == &Method::Put && url == "/api/config/channels/feishu" {
        let body = read_body(req);
        return match body {
            Ok(text) => {
                let parsed = serde_json::from_str::<FeishuChannelConfig>(&text)
                    .map(|f| ChannelsFile { feishu: f })
                    .or_else(|_| serde_json::from_str::<ChannelsFile>(&text));
                match parsed {
                    Ok(file) => match save_channels(&file).and_then(|_| to_json_bytes(&file)) {
                        Ok(v) => json_response(200, v),
                        Err(e) => err_response(500, format!("保存飞书配置失败: {e}")),
                    },
                    Err(e) => err_response(400, format!("JSON 解析失败: {e}")),
                }
            }
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
            anyhow::Ok(data)
        });
        return match out.and_then(|v| to_json_bytes(&v)) {
            Ok(v) => json_response(200, v),
            Err(e) => err_response(500, format!("读取任务列表失败: {e}")),
        };
    }

    err_response(404, "未找到接口".to_string())
}

fn parse_body_json<T: serde::de::DeserializeOwned>(
    req: &mut tiny_http::Request,
) -> std::result::Result<T, String> {
    let body = read_body(req)?;
    serde_json::from_str(&body).map_err(|e| format!("JSON 解析失败: {e}"))
}

fn read_body(req: &mut tiny_http::Request) -> std::result::Result<String, String> {
    let mut body = String::new();
    req.as_reader()
        .read_to_string(&mut body)
        .map_err(|e| format!("读取请求体失败: {e}"))?;
    Ok(body)
}

fn tokio_block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("create tokio runtime failed");
    rt.block_on(fut)
}

fn to_json_bytes<T: Serialize>(v: &T) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec(v)?)
}

fn json_response(status: u16, body: Vec<u8>) -> Response<std::io::Cursor<Vec<u8>>> {
    let mut resp = Response::new(
        StatusCode(status),
        headers_json(),
        std::io::Cursor::new(body),
        None,
        None,
    );
    resp.add_header(Header::from_bytes("Cache-Control", "no-store").unwrap());
    resp
}

fn text_response(
    status: u16,
    content_type: &str,
    body: Vec<u8>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::new(
        StatusCode(status),
        vec![Header::from_bytes("Content-Type", content_type).unwrap()],
        std::io::Cursor::new(body),
        None,
        None,
    )
}

fn err_response(status: u16, msg: String) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = serde_json::json!({ "error": msg });
    json_response(status, serde_json::to_vec(&body).unwrap_or_default())
}

fn headers_json() -> Vec<Header> {
    vec![Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap()]
}
