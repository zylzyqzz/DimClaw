#[path = "support/mod.rs"]
mod common;

use anyhow::Result;
use reqwest::Client;

#[tokio::test]
async fn test_hands_endpoints() -> Result<()> {
    let server = common::start_server(common::ServerOptions::default()).await?;
    let client = Client::new();

    let list: serde_json::Value = client
        .get(format!("{}/api/hands", server.base_url))
        .send()
        .await?
        .json()
        .await?;
    assert!(list.as_array().map(|v| !v.is_empty()).unwrap_or(false));

    let trigger: serde_json::Value = client
        .post(format!("{}/api/hands/trigger/lead", server.base_url))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(trigger.get("success").and_then(|v| v.as_bool()), Some(true));

    Ok(())
}
