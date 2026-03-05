use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub system_prompt: String,
    pub user_prompt: String,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: String,
    pub raw_text: String,
    pub usage: Option<Usage>,
    pub provider_name: String,
    pub model: String,
}

#[derive(Debug, Clone)]
pub enum ProviderError {
    Config(String),
    Timeout(String),
    Http(String),
    Parse(String),
    InvalidResponse(String),
    Cancelled,
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderError::Config(s) => write!(f, "配置错误: {s}"),
            ProviderError::Timeout(s) => write!(f, "请求超时: {s}"),
            ProviderError::Http(s) => write!(f, "网络错误: {s}"),
            ProviderError::Parse(s) => write!(f, "解析错误: {s}"),
            ProviderError::InvalidResponse(s) => write!(f, "响应错误: {s}"),
            ProviderError::Cancelled => write!(f, "请求已取消"),
        }
    }
}

impl std::error::Error for ProviderError {}
