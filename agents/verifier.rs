use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::agents::agent::{llm_generate, Agent, AgentContext, AgentLlm, AgentOutcome, MessageContext};
use crate::agents::llm_json::VerifierOutput;
use crate::core::logger;
use crate::core::task::Task;

pub struct VerifierAgent {
    llm: Option<AgentLlm>,
}

impl VerifierAgent {
    pub fn new(llm: Option<AgentLlm>) -> Self {
        Self { llm }
    }
}

#[async_trait]
impl Agent for VerifierAgent {
    fn name(&self) -> &str {
        "Verifier"
    }

    async fn handle(&self, task: &mut Task, ctx: AgentContext) -> AgentOutcome {
        if ctx.cancellation.is_cancelled() {
            return AgentOutcome::cancelled("校验阶段收到取消信号");
        }

        logger::log(format!("[Verifier] id={} 开始校验", task.id));

        let verdict = if let Some(llm) = &self.llm {
            let result = task.payload.get("execution_result").cloned().unwrap_or_default();
            let text = llm_generate(
                llm,
                "你是验证智能体。仅输出 JSON: {\"verdict\":\"pass|fail|retry\",\"reason\":\"...\",\"evidence\":\"...\"}".to_string(),
                format!("执行结果: {}", result),
                ctx.cancellation.clone(),
            )
            .await
            .unwrap_or_default();

            crate::agents::llm_json::parse_json_with_extract::<VerifierOutput>(&text).unwrap_or(VerifierOutput {
                verdict: "retry".to_string(),
                reason: "verifier_fallback_json_parse".to_string(),
                evidence: "invalid_json".to_string(),
            })
        } else {
            let code = task
                .payload
                .get("execution_result")
                .and_then(|v| v.get("exit_code"))
                .and_then(|v| v.as_i64());
            if code == Some(0) {
                VerifierOutput {
                    verdict: "pass".to_string(),
                    reason: "exit_code=0".to_string(),
                    evidence: "local".to_string(),
                }
            } else {
                VerifierOutput {
                    verdict: "retry".to_string(),
                    reason: format!("exit_code={:?}", code),
                    evidence: "local".to_string(),
                }
            }
        };

        if let Some(obj) = task.payload.as_object_mut() {
            obj.insert(
                "verifier_result".to_string(),
                serde_json::to_value(&verdict).unwrap_or_default(),
            );
        }

        match verdict.verdict.as_str() {
            "pass" => AgentOutcome::success(),
            "fail" => AgentOutcome::fail(verdict.reason),
            _ => AgentOutcome::retry(verdict.reason),
        }
    }

    fn should_handle(&self, ctx: &MessageContext) -> bool {
        let m = ctx.message.to_lowercase();
        m.contains("验证") || m.contains("检查") || m.contains("verify")
    }

    async fn generate_reply(&self, ctx: &MessageContext, _history: &[Value]) -> Result<String> {
        if let Some(llm) = &self.llm {
            return llm_generate(
                llm,
                "你是验证智能体，擅长审查正确性。".to_string(),
                ctx.message.clone(),
                tokio_util::sync::CancellationToken::new(),
            )
            .await;
        }
        Ok(format!("[Verifier] 建议检查执行结果与期望是否一致：{}", ctx.message))
    }
}
