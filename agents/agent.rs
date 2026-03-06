use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::core::task::TaskStatus;
use crate::providers::traits::LlmProvider;
use crate::providers::types::ChatRequest;

#[derive(Clone)]
pub struct AgentContext {
    pub cancellation: CancellationToken,
}

#[derive(Clone)]
pub struct AgentLlm {
    pub provider: Arc<dyn LlmProvider>,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
}

#[derive(Debug, Clone)]
pub enum OutcomeKind {
    Success,
    Failure,
    Retry,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct AgentOutcome {
    pub kind: OutcomeKind,
    pub suggestion: Option<TaskStatus>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MessageContext {
    pub channel: String,
    pub message: String,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub user_id: String,
    #[serde(default)]
    pub chat_id: String,
    #[serde(default)]
    pub metadata: Value,
}

impl AgentOutcome {
    pub fn success() -> Self {
        Self {
            kind: OutcomeKind::Success,
            suggestion: None,
            message: None,
        }
    }

    pub fn success_with_next(next: TaskStatus) -> Self {
        Self {
            kind: OutcomeKind::Success,
            suggestion: Some(next),
            message: None,
        }
    }

    pub fn fail(msg: impl Into<String>) -> Self {
        Self {
            kind: OutcomeKind::Failure,
            suggestion: None,
            message: Some(msg.into()),
        }
    }

    pub fn retry(msg: impl Into<String>) -> Self {
        Self {
            kind: OutcomeKind::Retry,
            suggestion: None,
            message: Some(msg.into()),
        }
    }

    pub fn cancelled(msg: impl Into<String>) -> Self {
        Self {
            kind: OutcomeKind::Cancelled,
            suggestion: None,
            message: Some(msg.into()),
        }
    }
}

#[async_trait]
pub trait Agent: Send + Sync {
    fn name(&self) -> &str;

    async fn handle(
        &self,
        task: &mut crate::core::task::Task,
        ctx: AgentContext,
    ) -> AgentOutcome;

    fn should_handle(&self, _ctx: &MessageContext) -> bool {
        false
    }

    async fn generate_reply(&self, ctx: &MessageContext, _history: &[Value]) -> Result<String> {
        Ok(format!("{} 收到消息: {}", self.name(), ctx.message))
    }
}

pub async fn llm_generate(
    llm: &AgentLlm,
    system_prompt: String,
    user_prompt: String,
    cancellation: CancellationToken,
) -> Result<String> {
    let request = ChatRequest {
        system_prompt,
        user_prompt,
        model: llm.model.clone(),
        temperature: llm.temperature,
        max_tokens: llm.max_tokens.min(1024),
    };
    let resp = llm.provider.chat(request, cancellation).await?;
    Ok(resp.content)
}
