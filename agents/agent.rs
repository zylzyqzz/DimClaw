use async_trait::async_trait;
use std::sync::Arc;

use crate::core::task::TaskStatus;
use crate::providers::traits::LlmProvider;
use tokio_util::sync::CancellationToken;

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
    fn name(&self) -> &'static str;
    async fn handle(
        &self,
        task: &mut crate::core::task::Task,
        ctx: AgentContext,
    ) -> AgentOutcome;
}
