#![allow(dead_code)]

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use reqwest::Client;
use tokio::time::sleep;

pub struct ServerOptions {
    pub model_base_url: String,
    pub model_api_key: String,
    pub extra_env: Vec<(String, String)>,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            model_base_url: "http://127.0.0.1:9".to_string(),
            model_api_key: "test-key".to_string(),
            extra_env: Vec::new(),
        }
    }
}

pub struct TestServer {
    pub base_url: String,
    pub root_dir: PathBuf,
    child: Child,
}

impl TestServer {
    pub fn path(&self) -> &Path {
        &self.root_dir
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.root_dir);
    }
}

pub async fn start_server(options: ServerOptions) -> Result<TestServer> {
    let root_dir = make_temp_workspace()?;
    prepare_layout(&root_dir, &options)?;

    let port = find_free_port()?;
    let base_url = format!("http://127.0.0.1:{}", port);
    let bin = resolve_bin_path()?;

    let mut cmd = Command::new(bin);
    cmd.arg("server")
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(port.to_string())
        .current_dir(&root_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for (k, v) in options.extra_env {
        cmd.env(k, v);
    }

    let child = cmd.spawn()?;
    let server = TestServer {
        base_url: base_url.clone(),
        root_dir,
        child,
    };
    wait_until_ready(&base_url).await?;
    Ok(server)
}

fn prepare_layout(root: &Path, options: &ServerOptions) -> Result<()> {
    std::fs::create_dir_all(root.join("configs"))?;
    std::fs::create_dir_all(root.join("data"))?;
    std::fs::create_dir_all(root.join("logs"))?;

    let models_toml = format!(
        r#"[[providers]]
name = "test-provider"
protocol = "openai_compatible"
base_url = "{base_url}"
api_key = "{api_key}"
model = "test-model"
timeout_secs = 30
max_tokens = 512
temperature = 0.2
enabled = true
default = true
"#,
        base_url = options.model_base_url,
        api_key = options.model_api_key
    );
    std::fs::write(root.join("configs/models.toml"), models_toml)?;

    let channels_toml = r#"[feishu]
enabled = false
mode = "single"
single_agent = "Planner"
agents = ["Planner", "Executor", "Verifier", "Recovery"]

[telegram]
enabled = false
mode = "single"
single_agent = "Planner"
agents = ["Planner", "Executor", "Verifier", "Recovery"]
"#;
    std::fs::write(root.join("configs/channels.toml"), channels_toml)?;

    let security_toml = "unrestricted_mode = false\n";
    std::fs::write(root.join("configs/security.toml"), security_toml)?;

    Ok(())
}

fn find_free_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

async fn wait_until_ready(base_url: &str) -> Result<()> {
    let client = Client::new();
    for _ in 0..50 {
        if let Ok(resp) = client
            .get(format!("{}/api/dashboard/stats", base_url))
            .send()
            .await
        {
            if resp.status().is_success() {
                return Ok(());
            }
        }
        sleep(Duration::from_millis(200)).await;
    }
    Err(anyhow!("server startup timeout"))
}

fn make_temp_workspace() -> Result<PathBuf> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let name = format!("dimclaw_test_{}_{}", std::process::id(), stamp);
    let path = std::env::temp_dir().join(name);
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

pub fn read_file(path: PathBuf) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn resolve_bin_path() -> Result<String> {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_dimclaw") {
        return Ok(path);
    }

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target");
    path.push("debug");
    path.push(if cfg!(windows) { "dimclaw.exe" } else { "dimclaw" });
    if path.exists() {
        return Ok(path.display().to_string());
    }

    Err(anyhow!("missing dimclaw binary, expected {}", path.display()))
}
