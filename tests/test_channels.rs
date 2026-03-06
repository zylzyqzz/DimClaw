#[path = "support/mod.rs"]
mod common;

use std::thread;

use anyhow::Result;
use reqwest::Client;
use serde_json::json;
use tiny_http::{Response, Server};

#[tokio::test]
async fn test_channel_config_and_routing() -> Result<()> {
    let model_port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        listener.local_addr()?.port()
    };
    let model_addr = format!("127.0.0.1:{}", model_port);
    let model_url = format!("http://{}", model_addr);

    let handler = thread::spawn(move || {
        let server = Server::http(&model_addr).expect("mock model");
        for _ in 0..30 {
            let req = match server.recv() {
                Ok(v) => v,
                Err(_) => break,
            };
            let body = r#"{"choices":[{"message":{"role":"assistant","content":"ok"}}]}"#;
            let _ = req.respond(Response::from_string(body).with_status_code(200));
        }
    });

    let server = common::start_server(common::ServerOptions {
        model_base_url: model_url,
        model_api_key: "mock-key".to_string(),
        extra_env: vec![],
    })
    .await?;
    let client = Client::new();

    let _ = client
        .put(format!("{}/api/config/channels/feishu", server.base_url))
        .json(&json!({
            "enabled": true,
            "mode": "single",
            "single_agent": "Planner",
            "agents": ["Planner", "Executor", "Verifier", "Recovery"]
        }))
        .send()
        .await?;

    let single: serde_json::Value = client
        .post(format!("{}/api/chat", server.base_url))
        .json(&json!({"message": "请规划", "channel": "feishu", "history": []}))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(single.get("agent_name").and_then(|v| v.as_str()), Some("Planner"));

    let _ = client
        .post(format!("{}/api/agents/custom", server.base_url))
        .json(&json!({
            "name": "测试员",
            "description": "test",
            "role": "tester",
            "system_prompt_template": "你负责测试",
            "model": "",
            "phase": "after_planning",
            "trigger_keywords": ["你好"],
            "enabled": true
        }))
        .send()
        .await?;

    let _ = client
        .put(format!("{}/api/config/channels/feishu", server.base_url))
        .json(&json!({
            "enabled": true,
            "mode": "multi",
            "single_agent": "Planner",
            "agents": ["测试员", "Planner"]
        }))
        .send()
        .await?;

    let multi: serde_json::Value = client
        .post(format!("{}/api/chat", server.base_url))
        .json(&json!({"message": "你好", "channel": "feishu", "history": []}))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(multi.get("agent_name").and_then(|v| v.as_str()), Some("测试员"));

    drop(handler);
    Ok(())
}
