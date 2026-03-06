#[path = "support/mod.rs"]
mod common;

use anyhow::Result;
use reqwest::Client;
use serde_json::json;

#[tokio::test]
async fn test_custom_agents_crud() -> Result<()> {
    let server = common::start_server(common::ServerOptions::default()).await?;
    let client = Client::new();

    let created: serde_json::Value = client
        .post(format!("{}/api/agents/custom", server.base_url))
        .json(&json!({
            "name": "reviewer",
            "description": "code reviewer",
            "role": "Reviewer",
            "system_prompt_template": "你负责审查 {task_title}",
            "model": "",
            "phase": "after_running",
            "trigger_keywords": ["review"],
            "enabled": true
        }))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(created.get("name").and_then(|v| v.as_str()), Some("reviewer"));

    let list: serde_json::Value = client
        .get(format!("{}/api/agents/custom", server.base_url))
        .send()
        .await?
        .json()
        .await?;
    assert!(list.as_array().unwrap_or(&vec![]).iter().any(|x| x.get("name").and_then(|v| v.as_str()) == Some("reviewer")));

    let updated: serde_json::Value = client
        .put(format!("{}/api/agents/custom/reviewer", server.base_url))
        .json(&json!({
            "name": "ignored",
            "description": "updated",
            "role": "Reviewer",
            "system_prompt_template": "updated",
            "model": "",
            "phase": "after_verifying",
            "trigger_keywords": ["review", "check"],
            "enabled": true
        }))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(updated.get("phase").and_then(|v| v.as_str()), Some("after_verifying"));

    let _ = client
        .delete(format!("{}/api/agents/custom/reviewer", server.base_url))
        .send()
        .await?;

    let list2: serde_json::Value = client
        .get(format!("{}/api/agents/custom", server.base_url))
        .send()
        .await?
        .json()
        .await?;
    assert!(!list2.as_array().unwrap_or(&vec![]).iter().any(|x| x.get("name").and_then(|v| v.as_str()) == Some("reviewer")));

    Ok(())
}
