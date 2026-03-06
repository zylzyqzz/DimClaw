#[path = "support/mod.rs"]
mod common;

use anyhow::Result;
use reqwest::Client;
use serde_json::json;

#[tokio::test]
async fn test_chat_intent_execute_tools() -> Result<()> {
    let server = common::start_server(common::ServerOptions::default()).await?;
    let client = Client::new();

    let create: serde_json::Value = client
        .post(format!("{}/api/chat", server.base_url))
        .json(&json!({"message":"创建一个文件 intent_a.txt 内容为 hello"}))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(create.get("agent_name").and_then(|v| v.as_str()), Some("Executor"));

    let list: serde_json::Value = client
        .post(format!("{}/api/chat", server.base_url))
        .json(&json!({"message":"列出当前目录"}))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(list.get("agent_name").and_then(|v| v.as_str()), Some("Executor"));

    let cmd: serde_json::Value = client
        .post(format!("{}/api/chat", server.base_url))
        .json(&json!({"message":"执行 echo intent-ok"}))
        .send()
        .await?
        .json()
        .await?;
    assert!(cmd.get("reply").and_then(|v| v.as_str()).unwrap_or_default().contains("执行"));

    Ok(())
}
