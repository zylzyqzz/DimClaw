use async_trait::async_trait;

use crate::agents::agent::{Agent, AgentContext, AgentLlm, AgentOutcome};
use crate::agents::llm_json::{parse_json_with_extract, RecoveryOutput};
use crate::core::logger;
use crate::core::task::{Task, TaskStatus};
use crate::providers::types::ChatRequest;

pub struct RecoveryAgent {
    llm: Option<AgentLlm>,
}

impl RecoveryAgent {
    pub fn new(llm: Option<AgentLlm>) -> Self {
        Self { llm }
    }

    fn fallback() -> RecoveryOutput {
        RecoveryOutput {
            action: "fail_task".to_string(),
            reason: "recovery_fallback".to_string(),
            retryable: false,
        }
    }
}

#[async_trait]
impl Agent for RecoveryAgent {
    fn name(&self) -> &'static str {
        "RecoveryAgent"
    }

    async fn handle(&self, task: &mut Task, ctx: AgentContext) -> AgentOutcome {
        if ctx.cancellation.is_cancelled() {
            return AgentOutcome::cancelled("恢复阶段收到取消信号");
        }
        logger::log(format!(
            "[Recovery] id={} 第 {} 次重试恢复",
            task.id, task.retry_count
        ));

        let llm = match &self.llm {
            Some(v) => v,
            None => {
                return if task.retry_count >= 3 {
                    AgentOutcome::success_with_next(TaskStatus::Failed)
                } else {
                    AgentOutcome::success_with_next(TaskStatus::Planning)
                };
            }
        };

        let request = ChatRequest {
            system_prompt: "你是 RecoveryAgent。必须只输出 JSON：{\"action\":\"retry_planning|retry_running|fail_task\",\"reason\":\"...\",\"retryable\":true|false}".to_string(),
            user_prompt: format!(
                "任务: {}\n当前重试次数: {}\n错误: {}",
                task.title,
                task.retry_count,
                task.error.clone().unwrap_or_default()
            ),
            model: llm.model.clone(),
            temperature: llm.temperature,
            max_tokens: llm.max_tokens,
        };

        let output = match llm.provider.chat(request, ctx.cancellation.clone()).await {
            Ok(resp) => parse_json_with_extract::<RecoveryOutput>(&resp.content).unwrap_or_else(|| {
                logger::log("[Recovery] 模型输出解析失败，使用 fail_task fallback");
                Self::fallback()
            }),
            Err(e) => {
                logger::log(format!("[Recovery] 调用模型失败，使用 fail_task fallback err={}", e));
                Self::fallback()
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
}
