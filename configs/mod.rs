use std::path::PathBuf;

#[derive(Clone)]
pub struct RuntimeConfig {
    pub data_dir: PathBuf,
    pub log_dir: Option<PathBuf>,
    pub max_retries: u32,
    pub poll_interval_ms: u64,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        let mut cfg = Self {
            data_dir: PathBuf::from("./data"),
            log_dir: Some(PathBuf::from("./logs")),
            max_retries: 3,
            poll_interval_ms: 400,
        };
        cfg.apply_env_overrides();
        cfg
    }
}

impl RuntimeConfig {
    fn apply_env_overrides(&mut self) {
        if let Ok(v) = std::env::var("DIMCLAW_DATA_DIR") {
            if !v.trim().is_empty() {
                self.data_dir = PathBuf::from(v);
            }
        }
        if let Ok(v) = std::env::var("DIMCLAW_LOG_DIR") {
            if v.trim().is_empty() {
                self.log_dir = None;
            } else {
                self.log_dir = Some(PathBuf::from(v));
            }
        }
        if let Ok(v) = std::env::var("DIMCLAW_MAX_RETRIES") {
            if let Ok(n) = v.parse::<u32>() {
                self.max_retries = n;
            }
        }
        if let Ok(v) = std::env::var("DIMCLAW_POLL_INTERVAL_MS") {
            if let Ok(n) = v.parse::<u64>() {
                self.poll_interval_ms = n;
            }
        }
    }

    pub fn data_dir_display(&self) -> String {
        self.data_dir.display().to_string()
    }

    pub fn log_dir_display(&self) -> String {
        self.log_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(已禁用文件日志)".to_string())
    }
}
