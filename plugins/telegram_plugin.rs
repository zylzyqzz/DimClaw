use crate::plugins::plugin_trait::{PluginConfigFile, PluginMeta};

pub fn default_meta() -> PluginMeta {
    PluginMeta {
        name: "telegram".to_string(),
        description: "Telegram 通道插件".to_string(),
        version: "0.1.0".to_string(),
        download_url: String::new(),
        archive_ext: String::new(),
        entry: if cfg!(target_os = "windows") {
            "telegram-plugin.exe".to_string()
        } else {
            "telegram-plugin".to_string()
        },
    }
}

pub fn default_config() -> PluginConfigFile {
    PluginConfigFile {
        enabled: false,
        host: "127.0.0.1".to_string(),
        port: 19792,
        entry: default_meta().entry,
        args: vec![],
        app_id: String::new(),
        app_secret: String::new(),
        verify_token: String::new(),
        bot_token: String::new(),
        extra: std::collections::HashMap::new(),
    }
}

pub fn merge_config(base: &mut PluginConfigFile, updates: &PluginConfigFile) {
    if !updates.bot_token.is_empty() {
        base.bot_token = updates.bot_token.clone();
    }
    if !updates.entry.is_empty() {
        base.entry = updates.entry.clone();
    }
    if !updates.args.is_empty() {
        base.args = updates.args.clone();
    }
    if updates.port != 0 {
        base.port = updates.port;
    }
    if !updates.host.is_empty() {
        base.host = updates.host.clone();
    }
    base.enabled = updates.enabled;
    for (k, v) in &updates.extra {
        base.extra.insert(k.clone(), v.clone());
    }
}

