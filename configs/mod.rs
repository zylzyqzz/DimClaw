use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    pub data_dir: PathBuf,
    pub log_dir: Option<PathBuf>,
    pub max_retries: u32,
    pub poll_interval_ms: u64,
    pub llm: LlmRuntimeConfig,
    pub providers: HashMap<String, ProviderConfig>,
    pub config_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct LlmRuntimeConfig {
    pub enabled: bool,
    pub provider: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ProviderConfig {
    pub protocol: String,
    pub provider_name: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout_secs: u64,
    pub max_tokens: u32,
    pub temperature: f32,
}

#[derive(Debug, Deserialize, Default)]
struct RuntimeFile {
    runtime: Option<RuntimeSection>,
    llm: Option<LlmSection>,
    providers: Option<HashMap<String, ProviderConfig>>,
}

#[derive(Debug, Deserialize, Default)]
struct RuntimeSection {
    data_dir: Option<String>,
    log_dir: Option<String>,
    max_retries: Option<u32>,
    poll_interval_ms: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
struct LlmSection {
    enabled: Option<bool>,
    provider: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelProviderEntry {
    pub name: String,
    pub protocol: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout_secs: u64,
    pub max_tokens: u32,
    pub temperature: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ModelsFile {
    pub providers: Vec<ModelProviderEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromptPair {
    pub system_prompt: String,
    pub user_prompt: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentsFile {
    pub planner: PromptPair,
    pub executor: PromptPair,
    pub verifier: PromptPair,
    pub recovery: PromptPair,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FeishuChannelConfig {
    pub enabled: bool,
    pub app_id: String,
    pub app_secret: String,
    pub verification_token: String,
    pub webhook_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelsFile {
    pub feishu: FeishuChannelConfig,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self::load().unwrap_or_else(|_| Self::fallback())
    }
}

impl RuntimeConfig {
    pub fn load() -> Result<Self> {
        let mut cfg = Self::fallback();
        cfg.config_path = PathBuf::from("./configs/runtime.toml");

        if cfg.config_path.exists() {
            let body = std::fs::read_to_string(&cfg.config_path)
                .with_context(|| format!("读取配置失败: {}", cfg.config_path.display()))?;
            let parsed: RuntimeFile = toml::from_str(&body).context("解析 runtime.toml 失败")?;
            cfg.apply_file(parsed);
        }

        cfg.apply_env_overrides();
        Ok(cfg)
    }

    fn fallback() -> Self {
        let mut providers = HashMap::new();
        providers.insert(
            "default".to_string(),
            ProviderConfig {
                protocol: "openai_compatible".to_string(),
                provider_name: "nvidia".to_string(),
                base_url: "https://integrate.api.nvidia.com/v1".to_string(),
                api_key: String::new(),
                model: "nvidia/qwen/qwen3.5-397b-a17b".to_string(),
                timeout_secs: 60,
                max_tokens: 2048,
                temperature: 0.2,
            },
        );
        Self {
            data_dir: PathBuf::from("./data"),
            log_dir: Some(PathBuf::from("./logs")),
            max_retries: 3,
            poll_interval_ms: 400,
            llm: LlmRuntimeConfig {
                enabled: true,
                provider: "default".to_string(),
            },
            providers,
            config_path: PathBuf::from("./configs/runtime.toml"),
        }
    }

    fn apply_file(&mut self, parsed: RuntimeFile) {
        if let Some(runtime) = parsed.runtime {
            if let Some(v) = runtime.data_dir {
                self.data_dir = PathBuf::from(v);
            }
            if let Some(v) = runtime.log_dir {
                if v.trim().is_empty() {
                    self.log_dir = None;
                } else {
                    self.log_dir = Some(PathBuf::from(v));
                }
            }
            if let Some(v) = runtime.max_retries {
                self.max_retries = v;
            }
            if let Some(v) = runtime.poll_interval_ms {
                self.poll_interval_ms = v;
            }
        }

        if let Some(llm) = parsed.llm {
            if let Some(v) = llm.enabled {
                self.llm.enabled = v;
            }
            if let Some(v) = llm.provider {
                self.llm.provider = v;
            }
        }

        if let Some(providers) = parsed.providers {
            self.providers = providers;
        }
    }

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
        if let Ok(v) = std::env::var("DIMCLAW_LLM_ENABLED") {
            self.llm.enabled = matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES");
        }
        if let Ok(v) = std::env::var("DIMCLAW_LLM_PROVIDER") {
            if !v.trim().is_empty() {
                self.llm.provider = v;
            }
        }
        if let Ok(v) = std::env::var("DIMCLAW_API_KEY") {
            if let Some(p) = self.providers.get_mut(&self.llm.provider) {
                p.api_key = v;
            }
        }
    }

    pub fn selected_provider(&self) -> Result<&ProviderConfig> {
        self.providers
            .get(&self.llm.provider)
            .ok_or_else(|| anyhow!("未找到 provider: {}", self.llm.provider))
    }

    pub fn config_exists(&self) -> bool {
        Path::new(&self.config_path).exists()
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

pub fn ensure_config_files() -> Result<()> {
    std::fs::create_dir_all("./configs")?;
    if !Path::new("./configs/models.toml").exists() {
        save_models(&ModelsFile::default())?;
    }
    if !Path::new("./configs/agents.toml").exists() {
        save_agents(&default_agents())?;
    }
    if !Path::new("./configs/channels.toml").exists() {
        save_channels(&default_channels())?;
    }
    Ok(())
}

pub fn load_models() -> Result<ModelsFile> {
    ensure_config_files()?;
    let body = std::fs::read_to_string("./configs/models.toml")?;
    let file: ModelsFile = toml::from_str(&body).context("解析 models.toml 失败")?;
    Ok(file)
}

pub fn save_models(file: &ModelsFile) -> Result<()> {
    let body = toml::to_string_pretty(file)?;
    std::fs::write("./configs/models.toml", body)?;
    Ok(())
}

pub fn load_agents() -> Result<AgentsFile> {
    ensure_config_files()?;
    let body = std::fs::read_to_string("./configs/agents.toml")?;
    let file: AgentsFile = toml::from_str(&body).context("解析 agents.toml 失败")?;
    Ok(file)
}

pub fn save_agents(file: &AgentsFile) -> Result<()> {
    let body = toml::to_string_pretty(file)?;
    std::fs::write("./configs/agents.toml", body)?;
    Ok(())
}

pub fn load_channels() -> Result<ChannelsFile> {
    ensure_config_files()?;
    let body = std::fs::read_to_string("./configs/channels.toml")?;
    let file: ChannelsFile = toml::from_str(&body).context("解析 channels.toml 失败")?;
    Ok(file)
}

pub fn save_channels(file: &ChannelsFile) -> Result<()> {
    let body = toml::to_string_pretty(file)?;
    std::fs::write("./configs/channels.toml", body)?;
    Ok(())
}

pub fn default_agents() -> AgentsFile {
    AgentsFile {
        planner: PromptPair {
            system_prompt: "你是一个任务规划专家。".to_string(),
            user_prompt: "任务：{task_payload}".to_string(),
        },
        executor: PromptPair {
            system_prompt: "你是一个执行专家。".to_string(),
            user_prompt: "步骤：{step_description}".to_string(),
        },
        verifier: PromptPair {
            system_prompt: "你是一个验证专家。".to_string(),
            user_prompt: "执行结果：{result}".to_string(),
        },
        recovery: PromptPair {
            system_prompt: "你是一个恢复专家。".to_string(),
            user_prompt: "错误：{error}，重试次数：{retry_count}".to_string(),
        },
    }
}

pub fn default_channels() -> ChannelsFile {
    ChannelsFile {
        feishu: FeishuChannelConfig {
            enabled: false,
            app_id: String::new(),
            app_secret: String::new(),
            verification_token: String::new(),
            webhook_url: String::new(),
        },
    }
}
