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
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub r#default: bool,
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
pub struct MasterConfig {
    pub name: String,
    pub persona: String,
    pub initialized: bool,
}

impl Default for MasterConfig {
    fn default() -> Self {
        Self {
            name: "小D".to_string(),
            persona: "你是专业、可靠、执行力强的本地多智能体助手。".to_string(),
            initialized: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentsFile {
    pub planner: PromptPair,
    pub executor: PromptPair,
    pub verifier: PromptPair,
    pub recovery: PromptPair,
    #[serde(default)]
    pub master: Option<MasterConfig>,
    #[serde(default)]
    pub model_bindings: HashMap<String, String>,
}

impl Default for AgentsFile {
    fn default() -> Self {
        default_agents()
    }
}

impl AgentsFile {
    pub fn master_or_default(&self) -> MasterConfig {
        self.master.clone().unwrap_or_default()
    }

    pub fn build_system_prompt(&self, base: &str) -> String {
        let master = self.master_or_default();
        if !master.initialized || master.name.trim().is_empty() || master.persona.trim().is_empty() {
            return base.to_string();
        }
        format!(
            "你是{}，人格设定是：{}。\n{}",
            master.name.trim(),
            master.persona.trim(),
            base
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentRouteConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub style: String,
}

impl Default for AgentRouteConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            keywords: Vec::new(),
            style: String::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub plugin_installed: bool,
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub app_secret: String,
    #[serde(default)]
    pub verify_token: String,
    #[serde(default)]
    pub bot_token: String,
    #[serde(default = "default_channel_mode")]
    pub mode: String,
    #[serde(default = "default_single_agent")]
    pub single_agent: String,
    #[serde(default = "default_single_agent")]
    pub default_agent: String,
    #[serde(default = "default_channel_agents")]
    pub agents: Vec<String>,
    #[serde(default)]
    pub agent_map: HashMap<String, AgentRouteConfig>,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            plugin_installed: false,
            app_id: String::new(),
            app_secret: String::new(),
            verify_token: String::new(),
            bot_token: String::new(),
            mode: default_channel_mode(),
            single_agent: default_single_agent(),
            default_agent: default_single_agent(),
            agents: default_channel_agents(),
            agent_map: default_channel_agent_map(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ChannelsFile {
    #[serde(default)]
    pub feishu: ChannelConfig,
    #[serde(default)]
    pub telegram: ChannelConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecurityConfig {
    #[serde(default)]
    pub unrestricted_mode: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            unrestricted_mode: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomSkillConfig {
    pub name: String,
    pub description: String,
    pub exec_type: String,
    #[serde(default)]
    pub params_schema: serde_json::Value,
    #[serde(default)]
    pub command_template: String,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub body_template: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default = "default_skill_timeout_secs")]
    pub timeout_secs: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomAgentConfig {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub system_prompt_template: String,
    #[serde(default)]
    pub model: String,
    #[serde(default = "default_agent_phase")]
    pub phase: String,
    #[serde(default)]
    pub trigger_keywords: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
struct LegacyChannelsFile {
    #[serde(default)]
    feishu: LegacyFeishuChannelConfig,
}

#[derive(Debug, Deserialize, Default)]
struct LegacyFeishuChannelConfig {
    #[serde(default)]
    enabled: bool,
}

fn default_true() -> bool {
    true
}

fn default_skill_timeout_secs() -> u64 {
    20
}

fn default_channel_mode() -> String {
    "single".to_string()
}

fn default_single_agent() -> String {
    "Planner".to_string()
}

fn default_channel_agents() -> Vec<String> {
    vec![
        "Planner".to_string(),
        "Executor".to_string(),
        "Verifier".to_string(),
        "Recovery".to_string(),
    ]
}

fn default_channel_agent_map() -> HashMap<String, AgentRouteConfig> {
    let mut map = HashMap::new();
    map.insert(
        "Planner".to_string(),
        AgentRouteConfig {
            enabled: true,
            keywords: vec!["规划".to_string(), "计划".to_string()],
            style: "formal".to_string(),
        },
    );
    map.insert(
        "Executor".to_string(),
        AgentRouteConfig {
            enabled: true,
            keywords: vec!["执行".to_string(), "运行".to_string()],
            style: "concise".to_string(),
        },
    );
    map.insert(
        "Verifier".to_string(),
        AgentRouteConfig {
            enabled: true,
            keywords: vec!["检查".to_string(), "验证".to_string()],
            style: "strict".to_string(),
        },
    );
    map.insert(
        "Recovery".to_string(),
        AgentRouteConfig {
            enabled: true,
            keywords: vec!["恢复".to_string(), "重试".to_string()],
            style: "helpful".to_string(),
        },
    );
    map
}

fn default_agent_phase() -> String {
    "after_planning".to_string()
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
                provider_name: "default".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                api_key: String::new(),
                model: "gpt-4o-mini".to_string(),
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
            .unwrap_or_else(|| "(已禁用日志目录)".to_string())
    }
}

pub fn ensure_config_files() -> Result<()> {
    std::fs::create_dir_all("./configs")?;
    std::fs::create_dir_all("./configs/plugins")?;
    std::fs::create_dir_all("./configs/agents/custom")?;
    std::fs::create_dir_all("./plugins")?;

    if !Path::new("./configs/models.toml").exists() {
        save_models(&ModelsFile::default())?;
    }
    if !Path::new("./configs/agents.toml").exists() {
        save_agents(&default_agents())?;
    }
    if !Path::new("./configs/channels.toml").exists() {
        save_channels(&default_channels())?;
    }
    if !Path::new("./configs/security.toml").exists() {
        save_security(&SecurityConfig::default())?;
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
    let mut file: AgentsFile = toml::from_str(&body).context("解析 agents.toml 失败")?;
    if file.master.is_none() {
        file.master = Some(MasterConfig::default());
        let _ = save_agents(&file);
    }
    Ok(file)
}

pub fn save_agents(file: &AgentsFile) -> Result<()> {
    let body = toml::to_string_pretty(file)?;
    std::fs::write("./configs/agents.toml", body)?;
    Ok(())
}

pub fn load_master() -> Result<MasterConfig> {
    let agents = load_agents()?;
    Ok(agents.master_or_default())
}

pub fn save_master(master: MasterConfig) -> Result<MasterConfig> {
    let mut agents = load_agents().unwrap_or_default();
    let mut normalized = master;
    normalized.initialized = true;
    agents.master = Some(normalized.clone());
    save_agents(&agents)?;
    Ok(normalized)
}

pub fn load_channels() -> Result<ChannelsFile> {
    ensure_config_files()?;
    let body = std::fs::read_to_string("./configs/channels.toml")?;
    if let Ok(mut file) = toml::from_str::<ChannelsFile>(&body) {
        ensure_channel_agent_map(&mut file)?;
        return Ok(file);
    }

    let legacy: LegacyChannelsFile = toml::from_str(&body).context("解析 channels.toml 失败")?;
    let migrated = ChannelsFile {
        feishu: ChannelConfig {
            enabled: legacy.feishu.enabled,
            ..ChannelConfig::default()
        },
        telegram: ChannelConfig::default(),
    };
    let mut migrated = migrated;
    ensure_channel_agent_map(&mut migrated)?;
    let _ = save_channels(&migrated);
    Ok(migrated)
}

pub fn save_channels(file: &ChannelsFile) -> Result<()> {
    let mut normalized = file.clone();
    ensure_channel_agent_map(&mut normalized)?;
    let body = toml::to_string_pretty(&normalized)?;
    std::fs::write("./configs/channels.toml", body)?;
    Ok(())
}

pub fn load_security() -> Result<SecurityConfig> {
    ensure_config_files()?;
    let body = std::fs::read_to_string("./configs/security.toml")?;
    let parsed: SecurityConfig = toml::from_str(&body).context("解析 security.toml 失败")?;
    Ok(parsed)
}

pub fn save_security(file: &SecurityConfig) -> Result<()> {
    let body = toml::to_string_pretty(file)?;
    std::fs::write("./configs/security.toml", body)?;
    Ok(())
}

pub fn default_agents() -> AgentsFile {
    AgentsFile {
        planner: PromptPair {
            system_prompt: "你是规划智能体。请输出结构化 JSON 计划。".to_string(),
            user_prompt: "任务输入: {task_payload}".to_string(),
        },
        executor: PromptPair {
            system_prompt: "你是执行智能体。请输出结构化 JSON 决策。".to_string(),
            user_prompt: "当前步骤: {step_description}".to_string(),
        },
        verifier: PromptPair {
            system_prompt: "你是验证智能体。请输出结构化 JSON 结论。".to_string(),
            user_prompt: "执行结果: {result}".to_string(),
        },
        recovery: PromptPair {
            system_prompt: "你是恢复智能体。请输出结构化 JSON 策略。".to_string(),
            user_prompt: "错误: {error}，重试次数: {retry_count}".to_string(),
        },
        master: Some(MasterConfig::default()),
        model_bindings: HashMap::new(),
    }
}

pub fn default_channels() -> ChannelsFile {
    ChannelsFile {
        feishu: ChannelConfig::default(),
        telegram: ChannelConfig::default(),
    }
}

fn ensure_channel_agent_map(file: &mut ChannelsFile) -> Result<()> {
    let names = known_agent_names()?;
    ensure_channel_for_names(&mut file.feishu, &names);
    ensure_channel_for_names(&mut file.telegram, &names);
    Ok(())
}

fn ensure_channel_for_names(channel: &mut ChannelConfig, names: &[String]) {
    if channel.default_agent.trim().is_empty() {
        channel.default_agent = "Planner".to_string();
    }
    if channel.single_agent.trim().is_empty() {
        channel.single_agent = channel.default_agent.clone();
    }
    if channel.agent_map.is_empty() {
        channel.agent_map = default_channel_agent_map();
    }

    for name in names {
        channel
            .agent_map
            .entry(name.clone())
            .or_insert_with(AgentRouteConfig::default);
    }

    channel.agent_map.retain(|name, _| names.contains(name));
}

pub fn known_agent_names() -> Result<Vec<String>> {
    let mut names = vec![
        "Planner".to_string(),
        "Executor".to_string(),
        "Verifier".to_string(),
        "Recovery".to_string(),
    ];
    for custom in list_custom_agents().unwrap_or_default() {
        if !names.contains(&custom.name) {
            names.push(custom.name);
        }
    }
    Ok(names)
}

pub fn ensure_custom_skill_dir() -> Result<()> {
    std::fs::create_dir_all("./skills/custom")?;
    Ok(())
}

pub fn list_custom_skills() -> Result<Vec<CustomSkillConfig>> {
    ensure_custom_skill_dir()?;
    let mut out = Vec::new();
    for ent in std::fs::read_dir("./skills/custom")? {
        let ent = ent?;
        let path = ent.path();
        if path.extension().and_then(|v| v.to_str()) != Some("toml") {
            continue;
        }
        let body = std::fs::read_to_string(&path)?;
        let parsed: CustomSkillConfig =
            toml::from_str(&body).with_context(|| format!("解析技能配置失败: {}", path.display()))?;
        out.push(parsed);
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub fn load_custom_skill(name: &str) -> Result<CustomSkillConfig> {
    ensure_custom_skill_dir()?;
    let sanitized = sanitize_skill_name(name)?;
    let path = PathBuf::from(format!("./skills/custom/{}.toml", sanitized));
    let body = std::fs::read_to_string(&path)
        .with_context(|| format!("读取技能配置失败: {}", path.display()))?;
    let parsed: CustomSkillConfig =
        toml::from_str(&body).with_context(|| format!("解析技能配置失败: {}", path.display()))?;
    Ok(parsed)
}

pub fn save_custom_skill(skill: &CustomSkillConfig) -> Result<()> {
    ensure_custom_skill_dir()?;
    let sanitized = sanitize_skill_name(&skill.name)?;
    let path = PathBuf::from(format!("./skills/custom/{}.toml", sanitized));
    let body = toml::to_string_pretty(skill)?;
    std::fs::write(path, body)?;
    Ok(())
}

pub fn delete_custom_skill(name: &str) -> Result<()> {
    ensure_custom_skill_dir()?;
    let sanitized = sanitize_skill_name(name)?;
    let path = PathBuf::from(format!("./skills/custom/{}.toml", sanitized));
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

pub fn ensure_custom_agent_dir() -> Result<()> {
    std::fs::create_dir_all("./configs/agents/custom")?;
    Ok(())
}

pub fn list_custom_agents() -> Result<Vec<CustomAgentConfig>> {
    ensure_custom_agent_dir()?;
    let mut out = Vec::new();
    for ent in std::fs::read_dir("./configs/agents/custom")? {
        let ent = ent?;
        let path = ent.path();
        if path.extension().and_then(|v| v.to_str()) != Some("toml") {
            continue;
        }
        let body = std::fs::read_to_string(&path)?;
        let parsed: CustomAgentConfig =
            toml::from_str(&body).with_context(|| format!("解析自定义智能体失败: {}", path.display()))?;
        out.push(parsed);
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub fn load_custom_agent(name: &str) -> Result<CustomAgentConfig> {
    ensure_custom_agent_dir()?;
    let sanitized = sanitize_agent_name(name)?;
    let path = PathBuf::from(format!("./configs/agents/custom/{}.toml", sanitized));
    let body = std::fs::read_to_string(&path)
        .with_context(|| format!("读取自定义智能体失败: {}", path.display()))?;
    let parsed: CustomAgentConfig =
        toml::from_str(&body).with_context(|| format!("解析自定义智能体失败: {}", path.display()))?;
    Ok(parsed)
}

pub fn save_custom_agent(agent: &CustomAgentConfig) -> Result<()> {
    ensure_custom_agent_dir()?;
    let sanitized = sanitize_agent_name(&agent.name)?;
    let path = PathBuf::from(format!("./configs/agents/custom/{}.toml", sanitized));
    let body = toml::to_string_pretty(agent)?;
    std::fs::write(path, body)?;
    Ok(())
}

pub fn delete_custom_agent(name: &str) -> Result<()> {
    ensure_custom_agent_dir()?;
    let sanitized = sanitize_agent_name(name)?;
    let path = PathBuf::from(format!("./configs/agents/custom/{}.toml", sanitized));
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

fn sanitize_skill_name(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("技能名称不能为空"));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(anyhow!("技能名称仅允许字母/数字/-/_"));
    }
    Ok(trimmed.to_string())
}

fn sanitize_agent_name(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("智能体名称不能为空"));
    }
    if trimmed
        .chars()
        .any(|c| matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
    {
        return Err(anyhow!("智能体名称包含非法文件名字符"));
    }
    Ok(trimmed.replace(' ', "_"))
}

