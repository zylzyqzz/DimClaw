#[path = "support/mod.rs"]
mod common;

use anyhow::Result;
use reqwest::Client;
use serde_json::json;

#[tokio::test]
async fn test_channels_ext_agent_map() -> Result<()> {
    let server = common::start_server(common::ServerOptions::default()).await?;
    let client = Client::new();

    let _ = client
        .post(format!("{}/api/agents/custom", server.base_url))
        .json(&json!({
            "name": "测试员",
            "description": "",
            "role": "",
            "system_prompt_template": "你是测试员",
            "model": "",
            "phase": "after_planning",
            "trigger_keywords": ["你好"],
            "enabled": true
        }))
        .send()
        .await?;

    let cfg: serde_json::Value = client
        .get(format!("{}/api/config/channels/feishu", server.base_url))
        .send()
        .await?
        .json()
        .await?;

    let mut cfg_obj = cfg.as_object().cloned().unwrap_or_default();
    cfg_obj.insert("enabled".to_string(), json!(true));
    cfg_obj.insert("default_agent".to_string(), json!("Planner"));
    cfg_obj.insert("agent_map".to_string(), json!({
        "Planner": {"enabled": true, "keywords": ["规划"], "style":"formal"},
        "测试员": {"enabled": true, "keywords": ["你好"], "style":"concise"}
    }));

    let _ = client
        .put(format!("{}/api/config/channels/feishu", server.base_url))
        .json(&cfg_obj)
        .send()
        .await?;

    let routed: serde_json::Value = client
        .post(format!("{}/api/chat", server.base_url))
        .json(&json!({"channel":"feishu","message":"你好","history":[]}))
        .send()
        .await?
        .json()
        .await?;

    assert_eq!(routed.get("agent_name").and_then(|v| v.as_str()), Some("测试员"));
    Ok(())
}
