use async_trait::async_trait;

use crate::agents::agent::{Agent, AgentContext, AgentLlm, AgentOutcome};
use crate::agents::llm_json::{parse_json_with_extract, VerifierOutput};
use crate::core::logger;
use crate::core::task::Task;
use crate::providers::types::ChatRequest;

pub struct VerifierAgent {
    llm: Option<AgentLlm>,
}

impl VerifierAgent {
    pub fn new(llm: Option<AgentLlm>) -> Self {
        Self { llm }
    }

    fn fallback() -> VerifierOutput {
        VerifierOutput {
            verdict: "retry".to_string(),
            reason: "verifier_fallback".to_string(),
            evidence: "invalid_json".to_string(),
        }
    }
}

#[async_trait]
impl Agent for VerifierAgent {
    fn name(&self) -> &'static str {
        "VerifierAgent"
    }

    async fn handle(&self, task: &mut Task, ctx: AgentContext) -> AgentOutcome {
        if ctx.cancellation.is_cancelled() {
            return AgentOutcome::cancelled("校验阶段收到取消信号");
        }
        logger::log(format!("[Verifier] id={} 开始校验", task.id));

        let llm = match &self.llm {
            Some(v) => v,
            None => {
                let code = task
                    .payload
                    .get("execution_result")
                    .and_then(|v| v.get("exit_code"))
                    .and_then(|v| v.as_i64());
                return if code == Some(0) {
                    AgentOutcome::success()
                } else {
                    AgentOutcome::retry(format!("校验未通过: exit_code={code:?}"))
                };
            }
        };

        let request = ChatRequest {
            system_prompt: "你是 VerifierAgent。必须只输出 JSON：{\"verdict\":\"pass|retry|fail\",\"reason\":\"...\",\"evidence\":\"...\"}".to_string(),
            user_prompt: format!(
                "任务: {}\n执行结果: {}",
                task.title,
                task.payload.get("execution_result").cloned().unwrap_or_default()
            ),
            model: llm.model.clone(),
            temperature: llm.temperature,
            max_tokens: llm.max_tokens,
        };

        let verdict = match llm.provider.chat(request, ctx.cancellation.clone()).await {
            Ok(resp) => parse_json_with_extract::<VerifierOutput>(&resp.content).unwrap_or_else(|| {
                logger::log("[Verifier] 模型输出解析失败，进入 retry fallback");
                Self::fallback()
            }),
            Err(e) => {
                logger::log(format!("[Verifier] 调用模型失败，进入 retry fallback err={}", e));
                Self::fallback()
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
}
