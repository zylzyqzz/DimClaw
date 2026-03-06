use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

use crate::core::logger;
use crate::plugins::downloader::download_and_extract;
use crate::plugins::feishu_plugin;
use crate::plugins::manifest::{fetch_manifest, resolve_download, ManifestPlugin};
use crate::plugins::plugin_trait::{AvailablePlugin, Plugin, PluginConfigFile, PluginMeta, PluginStatus};
use crate::plugins::telegram_plugin;

static REGISTRY: std::sync::OnceLock<Arc<RwLock<PluginRegistry>>> = std::sync::OnceLock::new();
static MONITOR_STARTED: std::sync::OnceLock<()> = std::sync::OnceLock::new();

pub struct PluginRegistry {
    api_base: String,
    plugins: HashMap<String, ManagedPlugin>,
}

struct ManagedPlugin {
    meta: PluginMeta,
    status: PluginStatus,
    config: PluginConfigFile,
    dir: PathBuf,
    api_base: String,
    child: Option<Child>,
    restart_attempts: u8,
}

#[async_trait::async_trait]
impl Plugin for ManagedPlugin {
    fn name(&self) -> &str {
        &self.meta.name
    }

    fn version(&self) -> &str {
        &self.meta.version
    }

    fn status(&self) -> PluginStatus {
        self.status.clone()
    }

    fn config(&self) -> PluginConfigFile {
        self.config.clone()
    }

    async fn install(&mut self, config_updates: PluginConfigFile) -> Result<PluginStatus> {
        self.config = config_updates;
        self.status.installed = true;
        self.persist_config()?;
        Ok(self.status())
    }

    async fn uninstall(&mut self) -> Result<PluginStatus> {
        self.stop().await?;
        if self.dir.exists() {
            std::fs::remove_dir_all(&self.dir)?;
        }
        self.status.installed = false;
        self.status.enabled = false;
        self.status.running = false;
        self.status.pid = None;
        Ok(self.status())
    }

    async fn start(&mut self) -> Result<PluginStatus> {
        self.start_inner()?;
        Ok(self.status())
    }

    async fn stop(&mut self) -> Result<PluginStatus> {
        self.stop_inner()?;
        Ok(self.status())
    }

    async fn update_config(&mut self, config_updates: PluginConfigFile) -> Result<PluginStatus> {
        self.config = config_updates;
        self.persist_config()?;
        Ok(self.status())
    }
}

pub async fn ensure_initialized(api_base: String) -> Result<()> {
    if REGISTRY.get().is_none() {
        let mut registry = PluginRegistry {
            api_base,
            plugins: HashMap::new(),
        };
        registry.load_or_create_builtin_plugin("feishu")?;
        registry.load_or_create_builtin_plugin("telegram")?;
        let _ = REGISTRY.set(Arc::new(RwLock::new(registry)));
    }

    MONITOR_STARTED.get_or_init(|| {
        let registry = REGISTRY.get().cloned().expect("registry not initialized");
        tokio::spawn(async move {
            monitor_loop(registry).await;
        });
    });

    Ok(())
}

pub async fn list_installed_plugins() -> Result<Vec<PluginStatus>> {
    let reg = get_registry()?;
    let mut guard = reg.write().await;
    let mut out = Vec::new();
    for plugin in guard.plugins.values_mut() {
        plugin.refresh_runtime_status()?;
        plugin.refresh_connection_status().await;
        out.push(plugin.status());
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub async fn list_available_plugins() -> Result<Vec<AvailablePlugin>> {
    let manifest = fetch_manifest().await?;
    let reg = get_registry()?;
    let guard = reg.read().await;

    let mut out = Vec::new();
    for item in manifest.plugins {
        let installed = guard.plugins.get(&item.name).map(|p| p.status.installed).unwrap_or(false);
        let has_update = guard
            .plugins
            .get(&item.name)
            .map(|p| compare_version(&p.meta.version, &item.version))
            .unwrap_or(false);
        out.push(AvailablePlugin {
            name: item.name,
            description: item.description,
            version: item.version,
            installed,
            has_update,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub async fn install_plugin(name: &str, updates: PluginConfigFile) -> Result<PluginStatus> {
    let manifest = fetch_manifest().await?;
    let item = find_manifest_plugin(&manifest.plugins, name)
        .ok_or_else(|| anyhow!("未找到插件: {}", name))?;
    let download = resolve_download(&item)?;

    let reg = get_registry()?;
    let mut guard = reg.write().await;
    let plugin = guard
        .plugins
        .get_mut(name)
        .ok_or_else(|| anyhow!("插件未注册: {}", name))?;

    download_and_extract(&download.url, &download.ext, &plugin.dir).await?;

    plugin.meta.version = if item.version.is_empty() {
        plugin.meta.version.clone()
    } else {
        item.version
    };
    if !item.description.is_empty() {
        plugin.meta.description = item.description;
    }

    if !download.entry.is_empty() {
        plugin.config.entry = download.entry;
    }
    if plugin.config.entry.is_empty() {
        plugin.config.entry = plugin.meta.entry.clone();
    }

    plugin.merge_config_by_name(&updates);
    plugin.status.installed = true;
    plugin.status.enabled = plugin.config.enabled;
    plugin.status.last_log = "安装成功".to_string();
    plugin.persist_config()?;

    if plugin.config.enabled {
        plugin.start_inner()?;
    }

    Ok(plugin.status())
}

pub async fn uninstall_plugin(name: &str) -> Result<PluginStatus> {
    let reg = get_registry()?;
    let mut guard = reg.write().await;
    let plugin = guard
        .plugins
        .get_mut(name)
        .ok_or_else(|| anyhow!("插件不存在: {}", name))?;
    plugin.uninstall().await
}

pub async fn enable_plugin(name: &str, updates: Option<PluginConfigFile>) -> Result<PluginStatus> {
    let reg = get_registry()?;
    let mut guard = reg.write().await;
    let plugin = guard
        .plugins
        .get_mut(name)
        .ok_or_else(|| anyhow!("插件不存在: {}", name))?;

    if let Some(cfg) = updates {
        plugin.merge_config_by_name(&cfg);
    }

    plugin.config.enabled = true;
    plugin.status.enabled = true;
    plugin.persist_config()?;
    plugin.start().await
}

pub async fn disable_plugin(name: &str) -> Result<PluginStatus> {
    let reg = get_registry()?;
    let mut guard = reg.write().await;
    let plugin = guard
        .plugins
        .get_mut(name)
        .ok_or_else(|| anyhow!("插件不存在: {}", name))?;
    plugin.config.enabled = false;
    plugin.status.enabled = false;
    plugin.persist_config()?;
    plugin.stop().await
}

pub async fn update_plugin_config(name: &str, updates: PluginConfigFile, auto_restart: bool) -> Result<PluginStatus> {
    let reg = get_registry()?;
    let mut guard = reg.write().await;
    let plugin = guard
        .plugins
        .get_mut(name)
        .ok_or_else(|| anyhow!("插件不存在: {}", name))?;

    let was_running = plugin.status.running;
    plugin.merge_config_by_name(&updates);
    plugin.persist_config()?;

    if auto_restart && was_running {
        plugin.stop_inner()?;
        plugin.start_inner()?;
    }

    Ok(plugin.status())
}

pub async fn plugin_status(name: &str) -> Result<PluginStatus> {
    let reg = get_registry()?;
    let mut guard = reg.write().await;
    let plugin = guard
        .plugins
        .get_mut(name)
        .ok_or_else(|| anyhow!("插件不存在: {}", name))?;
    plugin.refresh_runtime_status()?;
    plugin.refresh_connection_status().await;
    Ok(plugin.status())
}

pub async fn auto_start_enabled_plugins() -> Result<()> {
    let reg = get_registry()?;
    let mut guard = reg.write().await;
    for plugin in guard.plugins.values_mut() {
        if plugin.config.enabled && plugin.status.installed {
            let _ = plugin.start_inner();
        }
    }
    Ok(())
}

pub async fn connection_status_map() -> Result<HashMap<String, PluginStatus>> {
    let reg = get_registry()?;
    let mut guard = reg.write().await;
    let mut out = HashMap::new();
    for (name, plugin) in &mut guard.plugins {
        plugin.refresh_runtime_status()?;
        plugin.refresh_connection_status().await;
        out.insert(name.clone(), plugin.status());
    }
    Ok(out)
}

pub fn parse_plugin_config_json(input: serde_json::Value) -> PluginConfigFile {
    let mut cfg = PluginConfigFile::default();
    if let Some(v) = input.get("enabled").and_then(|v| v.as_bool()) {
        cfg.enabled = v;
    }
    if let Some(v) = input.get("host").and_then(|v| v.as_str()) {
        cfg.host = v.to_string();
    }
    if let Some(v) = input.get("port").and_then(|v| v.as_u64()) {
        cfg.port = v as u16;
    }
    if let Some(v) = input.get("entry").and_then(|v| v.as_str()) {
        cfg.entry = v.to_string();
    }
    if let Some(v) = input.get("app_id").and_then(|v| v.as_str()) {
        cfg.app_id = v.to_string();
    }
    if let Some(v) = input.get("app_secret").and_then(|v| v.as_str()) {
        cfg.app_secret = v.to_string();
    }
    if let Some(v) = input.get("verify_token").and_then(|v| v.as_str()) {
        cfg.verify_token = v.to_string();
    }
    if let Some(v) = input.get("bot_token").and_then(|v| v.as_str()) {
        cfg.bot_token = v.to_string();
    }
    if let Some(arr) = input.get("args").and_then(|v| v.as_array()) {
        cfg.args = arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
    }
    cfg
}

impl PluginRegistry {
    fn load_or_create_builtin_plugin(&mut self, name: &str) -> Result<()> {
        std::fs::create_dir_all("./plugins")?;
        let dir = PathBuf::from(format!("./plugins/{}", name));
        std::fs::create_dir_all(&dir)?;

        let (meta, mut config) = match name {
            "feishu" => (feishu_plugin::default_meta(), feishu_plugin::default_config()),
            "telegram" => (telegram_plugin::default_meta(), telegram_plugin::default_config()),
            _ => {
                return Err(anyhow!("不支持的内置插件: {}", name));
            }
        };

        let config_path = dir.join("config.toml");
        if config_path.exists() {
            if let Ok(body) = std::fs::read_to_string(&config_path) {
                if let Ok(parsed) = toml::from_str::<PluginConfigFile>(&body) {
                    config = parsed;
                }
            }
        } else {
            let body = toml::to_string_pretty(&config)?;
            std::fs::write(&config_path, body)?;
        }

        let installed = has_plugin_entry_file(&dir, &config.entry, &meta.entry);
        let status = PluginStatus {
            name: name.to_string(),
            version: meta.version.clone(),
            description: meta.description.clone(),
            installed,
            running: false,
            enabled: config.enabled,
            pid: None,
            last_log: if installed {
                "已安装".to_string()
            } else {
                "未安装".to_string()
            },
            connection_status: "disconnected".to_string(),
            last_error: String::new(),
            last_connected_at: String::new(),
        };

        self.plugins.insert(
            name.to_string(),
            ManagedPlugin {
                meta,
                status,
                config,
                dir,
                api_base: self.api_base.clone(),
                child: None,
                restart_attempts: 0,
            },
        );

        Ok(())
    }
}

impl ManagedPlugin {
    fn config_path(&self) -> PathBuf {
        self.dir.join("config.toml")
    }

    fn persist_config(&self) -> Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let body = toml::to_string_pretty(&self.config)?;
        std::fs::write(self.config_path(), body)?;
        Ok(())
    }

    fn resolve_entry(&self) -> PathBuf {
        let entry = if self.config.entry.is_empty() {
            self.meta.entry.clone()
        } else {
            self.config.entry.clone()
        };

        let entry_path = PathBuf::from(&entry);
        if entry_path.is_absolute() {
            entry_path
        } else {
            self.dir.join(entry_path)
        }
    }

    fn refresh_runtime_status(&mut self) -> Result<()> {
        self.status.installed = has_plugin_entry_file(&self.dir, &self.config.entry, &self.meta.entry)
            || self.status.installed;

        if let Some(child) = self.child.as_mut() {
            if let Some(exit) = child.try_wait()? {
                self.status.running = false;
                self.status.pid = None;
                self.status.last_log = format!("进程退出: {}", exit);
                self.child = None;
            } else {
                self.status.running = true;
                self.status.pid = Some(child.id());
            }
        } else {
            self.status.running = false;
            self.status.pid = None;
        }

        Ok(())
    }

    async fn refresh_connection_status(&mut self) {
        if !self.status.running {
            self.status.connection_status = "disconnected".to_string();
            return;
        }

        let url = format!(
            "http://{}:{}/health",
            if self.config.host.is_empty() {
                "127.0.0.1"
            } else {
                &self.config.host
            },
            self.config.port
        );
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build();
        let Ok(client) = client else {
            self.status.connection_status = "running".to_string();
            return;
        };

        match client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => {
                self.status.connection_status = "connected".to_string();
                self.status.last_connected_at = chrono::Utc::now().to_rfc3339();
            }
            Ok(resp) => {
                self.status.connection_status = "disconnected".to_string();
                self.status.last_error = format!("health http {}", resp.status());
            }
            Err(e) => {
                self.status.connection_status = "disconnected".to_string();
                self.status.last_error = e.to_string();
            }
        }
    }

    fn start_inner(&mut self) -> Result<()> {
        if !self.status.installed {
            return Err(anyhow!("插件未安装: {}", self.meta.name));
        }
        if self.status.running {
            return Ok(());
        }

        let entry = self.resolve_entry();
        if !entry.exists() {
            return Err(anyhow!("插件入口不存在: {}", entry.display()));
        }

        let mut cmd = Command::new(&entry);
        for arg in &self.config.args {
            cmd.arg(arg);
        }
        cmd.env("DIMCLAW_MASTER_API", &self.api_base)
            .env("DIMCLAW_MASTER_API_BASE", &self.api_base)
            .env("DIMCLAW_PLUGIN_NAME", &self.status.name)
            .env("DIMCLAW_PLUGIN_HOST", &self.config.host)
            .env("DIMCLAW_PLUGIN_PORT", self.config.port.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let child = cmd
            .spawn()
            .with_context(|| format!("启动插件失败: {}", entry.display()))?;

        self.status.running = true;
        self.status.pid = Some(child.id());
        self.status.last_log = format!("进程已启动 pid={}", child.id());
        self.child = Some(child);
        self.restart_attempts = 0;
        logger::log(format!("[插件] {} 已启动", self.status.name));

        Ok(())
    }

    fn stop_inner(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.status.running = false;
        self.status.pid = None;
        self.status.connection_status = "disconnected".to_string();
        self.status.last_log = "进程已停止".to_string();
        logger::log(format!("[插件] {} 已停止", self.status.name));
        Ok(())
    }

    fn merge_config_by_name(&mut self, updates: &PluginConfigFile) {
        match self.meta.name.as_str() {
            "feishu" => feishu_plugin::merge_config(&mut self.config, updates),
            "telegram" => telegram_plugin::merge_config(&mut self.config, updates),
            _ => {
                self.config = updates.clone();
            }
        }
    }
}

async fn monitor_loop(registry: Arc<RwLock<PluginRegistry>>) {
    loop {
        sleep(Duration::from_secs(2)).await;
        let mut guard = registry.write().await;

        for plugin in guard.plugins.values_mut() {
            if plugin.refresh_runtime_status().is_err() {
                continue;
            }

            if plugin.status.running || !plugin.config.enabled {
                continue;
            }

            if plugin.status.installed && plugin.restart_attempts < 3 {
                plugin.restart_attempts += 1;
                let _ = plugin.start_inner();
            }
        }
    }
}

fn get_registry() -> Result<Arc<RwLock<PluginRegistry>>> {
    REGISTRY
        .get()
        .cloned()
        .ok_or_else(|| anyhow!("插件注册表未初始化"))
}

fn has_plugin_entry_file(dir: &Path, configured_entry: &str, default_entry: &str) -> bool {
    let candidate = if configured_entry.trim().is_empty() {
        default_entry
    } else {
        configured_entry
    };
    dir.join(candidate).exists()
}

fn find_manifest_plugin(list: &[ManifestPlugin], name: &str) -> Option<ManifestPlugin> {
    list.iter().find(|v| v.name == name).cloned()
}

fn compare_version(installed: &str, latest: &str) -> bool {
    if installed.trim().is_empty() || latest.trim().is_empty() {
        return false;
    }
    installed.trim() != latest.trim()
}

