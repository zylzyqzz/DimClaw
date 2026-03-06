use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

pub const DEFAULT_MANIFEST_URL: &str =
    "https://github.com/zylzyqzz/DimClaw-plugins/releases/latest/download/manifest.json";

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct PluginManifest {
    pub plugins: Vec<ManifestPlugin>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ManifestPlugin {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub entry: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub ext: String,
    #[serde(default)]
    pub platforms: HashMap<String, PlatformDownload>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct PlatformDownload {
    pub url: String,
    #[serde(default)]
    pub ext: String,
    #[serde(default)]
    pub entry: String,
}

pub async fn fetch_manifest() -> Result<PluginManifest> {
    let url = std::env::var("DIMCLAW_PLUGIN_MANIFEST_URL").unwrap_or_else(|_| DEFAULT_MANIFEST_URL.to_string());
    let client = reqwest::Client::new();
    let res = client.get(&url).send().await;
    let text = match res {
        Ok(resp) if resp.status().is_success() => resp.text().await.unwrap_or_default(),
        _ => return Ok(default_manifest()),
    };

    if let Ok(parsed) = serde_json::from_str::<PluginManifest>(&text) {
        return Ok(parsed);
    }

    let list = serde_json::from_str::<Vec<ManifestPlugin>>(&text)
        .with_context(|| format!("manifest 解析失败: {}", url))?;
    Ok(PluginManifest { plugins: list })
}

pub fn platform_key() -> String {
    format!(
        "{}-{}",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

pub fn resolve_download(plugin: &ManifestPlugin) -> Result<PlatformDownload> {
    if plugin.name.trim().is_empty() {
        return Err(anyhow!("manifest 插件名称为空"));
    }

    let key = platform_key();
    if let Some(item) = plugin.platforms.get(&key) {
        return Ok(item.clone());
    }

    if !plugin.url.trim().is_empty() {
        return Ok(PlatformDownload {
            url: plugin.url.clone(),
            ext: plugin.ext.clone(),
            entry: plugin.entry.clone(),
        });
    }

    Err(anyhow!("manifest 缺少平台下载地址: {} {}", plugin.name, key))
}

pub fn default_manifest() -> PluginManifest {
    let ext = if cfg!(target_os = "windows") { "zip" } else { "tar.gz" };
    let platform = platform_short();

    PluginManifest {
        plugins: vec![
            ManifestPlugin {
                name: "feishu".to_string(),
                description: "飞书通道插件".to_string(),
                version: "0.1.0".to_string(),
                entry: default_entry("feishu"),
                url: format!(
                    "https://github.com/zylzyqzz/DimClaw-plugins/releases/latest/download/feishu-{}.{}",
                    platform, ext
                ),
                ext: ext.to_string(),
                platforms: HashMap::new(),
            },
            ManifestPlugin {
                name: "telegram".to_string(),
                description: "Telegram 通道插件".to_string(),
                version: "0.1.0".to_string(),
                entry: default_entry("telegram"),
                url: format!(
                    "https://github.com/zylzyqzz/DimClaw-plugins/releases/latest/download/telegram-{}.{}",
                    platform, ext
                ),
                ext: ext.to_string(),
                platforms: HashMap::new(),
            },
        ],
    }
}

fn default_entry(name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{}-plugin.exe", name)
    } else {
        format!("{}-plugin", name)
    }
}

fn platform_short() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => "windows-amd64",
        ("linux", "x86_64") => "linux-amd64",
        ("macos", "x86_64") => "darwin-amd64",
        ("macos", "aarch64") => "darwin-arm64",
        _ => "linux-amd64",
    }
}

