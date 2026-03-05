use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::providers::types::{ChatRequest, ChatResponse, ProviderError};

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(
        &self,
        request: ChatRequest,
        cancellation: CancellationToken,
    ) -> Result<ChatResponse, ProviderError>;
}
