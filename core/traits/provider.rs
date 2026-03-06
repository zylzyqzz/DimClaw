use async_trait::async_trait;

#[derive(Clone, Debug, Default)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug, Default)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Clone, Debug, Default)]
pub struct ToolCall {
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Clone, Debug, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Clone, Debug, Default)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
}

#[derive(Clone, Debug, Default)]
pub struct ChatRequest {
    pub messages: Vec<Message>,
    pub temperature: f32,
    pub max_tokens: u32,
    pub tools: Option<Vec<ToolDefinition>>,
}

#[derive(Clone, Debug, Default)]
pub struct ChatResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: TokenUsage,
}

#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    async fn chat(&self, req: ChatRequest) -> anyhow::Result<ChatResponse>;
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>>;
    fn models(&self) -> Vec<ModelInfo>;
}
