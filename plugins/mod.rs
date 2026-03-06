pub mod downloader;
pub mod feishu_plugin;
pub mod manifest;
pub mod manager;
pub mod plugin_trait;
pub mod telegram_plugin;

pub use manager::{
    auto_start_enabled_plugins, connection_status_map, disable_plugin, enable_plugin, ensure_initialized,
    install_plugin, list_available_plugins, list_installed_plugins, parse_plugin_config_json, plugin_status, uninstall_plugin,
    update_plugin_config,
};

