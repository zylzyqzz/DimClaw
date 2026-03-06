#[path = "support/mod.rs"]
mod common;

use anyhow::Result;
use reqwest::Client;
use serde_json::json;

#[tokio::test]
async fn test_extended_skills() -> Result<()> {
    let server = common::start_server(common::ServerOptions::default()).await?;
    let client = Client::new();

    let write: serde_json::Value = client
        .post(format!("{}/api/skills/file_write/test", server.base_url))
        .json(&json!({"input": {"path": "skill_a.txt", "content": "Hello", "mode": "create"}}))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(write.get("success").and_then(|v| v.as_bool()), Some(true));

    let append: serde_json::Value = client
        .post(format!("{}/api/skills/file_write/test", server.base_url))
        .json(&json!({"input": {"path": "skill_a.txt", "content": "-World", "mode": "append"}}))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(append.get("success").and_then(|v| v.as_bool()), Some(true));

    let read: serde_json::Value = client
        .post(format!("{}/api/skills/file_read/test", server.base_url))
        .json(&json!({"input": {"path": "skill_a.txt"}}))
        .send()
        .await?
        .json()
        .await?;
    assert!(read.get("stdout").and_then(|v| v.as_str()).unwrap_or_default().contains("Hello-World"));

    let copy: serde_json::Value = client
        .post(format!("{}/api/skills/file_copy/test", server.base_url))
        .json(&json!({"input": {"from": "skill_a.txt", "to": "skill_b.txt"}}))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(copy.get("success").and_then(|v| v.as_bool()), Some(true));

    let mv: serde_json::Value = client
        .post(format!("{}/api/skills/file_move/test", server.base_url))
        .json(&json!({"input": {"from": "skill_b.txt", "to": "skill_c.txt"}}))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(mv.get("success").and_then(|v| v.as_bool()), Some(true));

    let list: serde_json::Value = client
        .post(format!("{}/api/skills/file_list/test", server.base_url))
        .json(&json!({"input": {"path": "."}}))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(list.get("success").and_then(|v| v.as_bool()), Some(true));

    let script: serde_json::Value = client
        .post(format!("{}/api/skills/script_execute/test", server.base_url))
        .json(&json!({"input": {"script": "echo script-ok"}}))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(script.get("success").and_then(|v| v.as_bool()), Some(true));

    let monitor: serde_json::Value = client
        .post(format!("{}/api/skills/system_monitor/test", server.base_url))
        .json(&json!({"input": {}}))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(monitor.get("success").and_then(|v| v.as_bool()), Some(true));

    let del: serde_json::Value = client
        .post(format!("{}/api/skills/file_delete/test", server.base_url))
        .json(&json!({"input": {"path": "skill_c.txt", "confirm": true}}))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(del.get("success").and_then(|v| v.as_bool()), Some(true));

    let imported: serde_json::Value = client
        .post(format!("{}/api/skills/openclaw/import", server.base_url))
        .json(&json!({
            "skill": {
                "name": "openclaw_echo",
                "command": "echo {{msg}}"
            },
            "overwrite": true
        }))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(imported.get("name").and_then(|v| v.as_str()), Some("openclaw_echo"));

    let imported_test: serde_json::Value = client
        .post(format!("{}/api/skills/openclaw_echo/test", server.base_url))
        .json(&json!({"input": {"msg": "hello"}}))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(imported_test.get("success").and_then(|v| v.as_bool()), Some(true));
    assert!(imported_test
        .get("stdout")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .contains("hello"));

    Ok(())
}
