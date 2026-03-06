#[path = "support/mod.rs"]
mod common;

use std::thread;

use anyhow::Result;
use reqwest::Client;
use tiny_http::{Response, Server};

#[tokio::test]
async fn test_connections_status_endpoint() -> Result<()> {
    let model_port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        listener.local_addr()?.port()
    };
    let model_addr = format!("127.0.0.1:{}", model_port);
    let model_url = format!("http://{}", model_addr);

    let handler = thread::spawn(move || {
        let server = Server::http(&model_addr).expect("mock model");
        for _ in 0..10 {
            let req = match server.recv() {
                Ok(v) => v,
                Err(_) => break,
            };
            let _ = req.respond(
                Response::from_string(r#"{"choices":[{"message":{"role":"assistant","content":"ok"}}]}"#)
                    .with_status_code(200),
            );
        }
    });

    let server = common::start_server(common::ServerOptions {
        model_base_url: model_url,
        model_api_key: "mock".to_string(),
        extra_env: vec![],
    })
    .await?;

    let client = Client::new();
    let out: serde_json::Value = client
        .get(format!("{}/api/status/connections", server.base_url))
        .send()
        .await?
        .json()
        .await?;

    assert!(out.get("model").is_some());
    assert!(out.get("plugins").is_some());

    drop(handler);
    Ok(())
}
