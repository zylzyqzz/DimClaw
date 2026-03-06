use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct ToolContext {
    pub cancellation: CancellationToken,
}

#[derive(Clone, Debug, Default)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> serde_json::Value;
    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolResult>;
}
