use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::agents::agent::{llm_generate, Agent, AgentContext, AgentLlm, AgentOutcome, MessageContext};
use crate::agents::llm_json::RecoveryOutput;
use crate::core::logger;
use crate::core::task::{Task, TaskStatus};

pub struct RecoveryAgent {
    llm: Option<AgentLlm>,
}

impl RecoveryAgent {
    pub fn new(llm: Option<AgentLlm>) -> Self {
        Self { llm }
    }
}

#[async_trait]
impl Agent for RecoveryAgent {
    fn name(&self) -> &str {
        "Recovery"
    }

    async fn handle(&self, task: &mut Task, ctx: AgentContext) -> AgentOutcome {
        if ctx.cancellation.is_cancelled() {
            return AgentOutcome::cancelled("恢复阶段收到取消信号");
        }

        logger::log(format!("[Recovery] id={} retry_count={}", task.id, task.retry_count));

        let output = if let Some(llm) = &self.llm {
            let prompt = format!(
                "错误: {}\n重试次数:{}\n请输出 JSON: {{\"action\":\"retry_planning|retry_running|fail_task\",\"reason\":\"...\",\"retryable\":true}}",
                task.error.clone().unwrap_or_default(),
                task.retry_count
            );
            let text = llm_generate(
                llm,
                "你是恢复智能体，只输出 JSON。".to_string(),
                prompt,
                ctx.cancellation.clone(),
            )
            .await
            .unwrap_or_default();
            crate::agents::llm_json::parse_json_with_extract::<RecoveryOutput>(&text).unwrap_or(RecoveryOutput {
                action: "fail_task".to_string(),
                reason: "recovery_fallback_json_parse".to_string(),
                retryable: false,
            })
        } else if task.retry_count >= 3 {
            RecoveryOutput {
                action: "fail_task".to_string(),
                reason: "max_retries_reached".to_string(),
                retryable: false,
            }
        } else {
            RecoveryOutput {
                action: "retry_planning".to_string(),
                reason: "default_retry".to_string(),
                retryable: true,
            }
        };

        if let Some(obj) = task.payload.as_object_mut() {
            obj.insert(
                "recovery_result".to_string(),
                serde_json::to_value(&output).unwrap_or_default(),
            );
        }

        match output.action.as_str() {
            "retry_running" => AgentOutcome::success_with_next(TaskStatus::Running),
            "retry_planning" => AgentOutcome::success_with_next(TaskStatus::Planning),
            _ => AgentOutcome::success_with_next(TaskStatus::Failed),
        }
    }

    fn should_handle(&self, ctx: &MessageContext) -> bool {
        let m = ctx.message.to_lowercase();
        m.contains("恢复") || m.contains("重试") || m.contains("retry")
    }

    async fn generate_reply(&self, ctx: &MessageContext, _history: &[Value]) -> Result<String> {
        if let Some(llm) = &self.llm {
            return llm_generate(
                llm,
                "你是恢复智能体，擅长故障处理。".to_string(),
                ctx.message.clone(),
                tokio_util::sync::CancellationToken::new(),
            )
            .await;
        }
        Ok(format!("[Recovery] 可尝试先缩小问题范围并重试：{}", ctx.message))
    }
}
