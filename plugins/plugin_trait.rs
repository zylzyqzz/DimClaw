use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct PluginConfigFile {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub entry: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub app_secret: String,
    #[serde(default)]
    pub verify_token: String,
    #[serde(default)]
    pub bot_token: String,
    #[serde(default)]
    pub extra: HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct PluginStatus {
    pub name: String,
    pub version: String,
    pub description: String,
    pub installed: bool,
    pub running: bool,
    pub enabled: bool,
    pub pid: Option<u32>,
    pub last_log: String,
    pub connection_status: String,
    pub last_error: String,
    pub last_connected_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct AvailablePlugin {
    pub name: String,
    pub description: String,
    pub version: String,
    pub installed: bool,
    pub has_update: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct PluginMeta {
    pub name: String,
    pub description: String,
    pub version: String,
    pub download_url: String,
    pub archive_ext: String,
    pub entry: String,
}

#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn status(&self) -> PluginStatus;
    fn config(&self) -> PluginConfigFile;
    async fn install(&mut self, config_updates: PluginConfigFile) -> Result<PluginStatus>;
    async fn uninstall(&mut self) -> Result<PluginStatus>;
    async fn start(&mut self) -> Result<PluginStatus>;
    async fn stop(&mut self) -> Result<PluginStatus>;
    async fn update_config(&mut self, config_updates: PluginConfigFile) -> Result<PluginStatus>;
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

