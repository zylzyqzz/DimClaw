#[path = "support/mod.rs"]
mod common;

use std::thread;

use anyhow::Result;
use reqwest::Client;
use serde_json::json;
use tiny_http::{Response, Server};

#[tokio::test]
async fn test_plugins_install_enable_disable_uninstall() -> Result<()> {
    let manifest_port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        listener.local_addr()?.port()
    };
    let manifest_addr = format!("127.0.0.1:{}", manifest_port);
    let manifest_base = format!("http://{}", manifest_addr);

    let plugin_bytes = b"dummy-plugin".to_vec();
    let manifest_base_for_thread = manifest_base.clone();
    let handler = thread::spawn(move || {
        let server = Server::http(&manifest_addr).expect("manifest server");
        for _ in 0..10 {
            let req = match server.recv() {
                Ok(v) => v,
                Err(_) => break,
            };
            let url = req.url().to_string();
            if url.contains("manifest.json") {
                let body = json!({
                    "plugins": [
                        {
                            "name": "feishu",
                            "description": "feishu",
                            "version": "0.1.0",
                            "entry": "plugin.bin",
                            "url": format!("{}/feishu.bin", manifest_base_for_thread),
                            "ext": "bin"
                        }
                    ]
                })
                .to_string();
                let _ = req.respond(Response::from_string(body).with_status_code(200));
            } else if url.contains("feishu.bin") {
                let _ = req.respond(Response::from_data(plugin_bytes.clone()).with_status_code(200));
            } else {
                let _ = req.respond(Response::from_string("{}").with_status_code(404));
            }
        }
    });

    let server = common::start_server(common::ServerOptions {
        model_base_url: "http://127.0.0.1:9".to_string(),
        model_api_key: "x".to_string(),
        extra_env: vec![(
            "DIMCLAW_PLUGIN_MANIFEST_URL".to_string(),
            format!("{}/manifest.json", manifest_base),
        )],
    })
    .await?;

    let client = Client::new();

    let install: serde_json::Value = client
        .post(format!("{}/api/plugins/install/feishu", server.base_url))
        .json(&json!({}))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(install.get("installed").and_then(|v| v.as_bool()), Some(true));

    let _ = client
        .post(format!("{}/api/plugins/enable/feishu", server.base_url))
        .json(&json!({
            "entry": "plugin.bin",
            "enabled": true
        }))
        .send()
        .await?;

    let status1: serde_json::Value = client
        .get(format!("{}/api/plugins/status/feishu", server.base_url))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(status1.get("installed").and_then(|v| v.as_bool()), Some(true));

    let _ = client
        .post(format!("{}/api/plugins/disable/feishu", server.base_url))
        .send()
        .await?;

    let status2: serde_json::Value = client
        .get(format!("{}/api/plugins/status/feishu", server.base_url))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(status2.get("enabled").and_then(|v| v.as_bool()), Some(false));

    let uninst: serde_json::Value = client
        .post(format!("{}/api/plugins/uninstall/feishu", server.base_url))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(uninst.get("installed").and_then(|v| v.as_bool()), Some(false));

    drop(handler);
    Ok(())
}
