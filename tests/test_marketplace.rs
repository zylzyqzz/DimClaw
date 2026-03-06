#[path = "support/mod.rs"]
mod common;

use anyhow::Result;
use reqwest::Client;
use serde_json::json;

#[tokio::test]
async fn test_marketplace_endpoints() -> Result<()> {
    let server = common::start_server(common::ServerOptions::default()).await?;
    let client = Client::new();

    let list: serde_json::Value = client
        .get(format!("{}/api/marketplace?q=邮件", server.base_url))
        .send()
        .await?
        .json()
        .await?;
    assert!(list.get("skills").is_some());

    let install: serde_json::Value = client
        .post(format!("{}/api/marketplace/install/email-automation", server.base_url))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(install.get("success").and_then(|v| v.as_bool()), Some(true));

    let import: serde_json::Value = client
        .post(format!("{}/api/marketplace/import", server.base_url))
        .json(&json!({"repo_url":"https://github.com/VoltAgent/awesome-openclaw-skills.git"}))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(import.get("success").and_then(|v| v.as_bool()), Some(true));

    Ok(())
}
