use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::agents::agent::{llm_generate, Agent, AgentContext, AgentLlm, AgentOutcome, MessageContext};
use crate::agents::llm_json::{ExecutorOutput, PlannerOutput};
use crate::core::logger;
use crate::core::task::Task;

pub struct ExecutorAgent {
    llm: Option<AgentLlm>,
}

impl ExecutorAgent {
    pub fn new(llm: Option<AgentLlm>) -> Self {
        Self { llm }
    }
}

#[async_trait]
impl Agent for ExecutorAgent {
    fn name(&self) -> &str {
        "Executor"
    }

    async fn handle(&self, task: &mut Task, ctx: AgentContext) -> AgentOutcome {
        if ctx.cancellation.is_cancelled() {
            return AgentOutcome::cancelled("执行阶段收到取消信号");
        }

        logger::log(format!("[Executor] id={} 开始生成执行决策", task.id));

        let plan: Option<PlannerOutput> = task
            .payload
            .get("plan")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let plan_index = task
            .payload
            .get("plan_index")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let step = match plan.as_ref().and_then(|p| p.steps.get(plan_index)) {
            Some(v) => v.clone(),
            None => {
                return AgentOutcome::retry("计划步骤不存在");
            }
        };

        let decision = if let Some(llm) = &self.llm {
            let prompt = format!(
                "当前步骤是: {}\n工具:{}\n参数:{}\n请输出 JSON: {{\"decision\":\"execute|skip|fail\",\"tool\":\"...\",\"args\":{{}},\"reason\":\"...\"}}",
                step.action,
                step.tool,
                step.args
            );
            let text = llm_generate(
                llm,
                "你是执行智能体，只输出 JSON。".to_string(),
                prompt,
                ctx.cancellation.clone(),
            )
            .await
            .unwrap_or_default();
            crate::agents::llm_json::parse_json_with_extract::<ExecutorOutput>(&text).unwrap_or(ExecutorOutput {
                decision: "execute".to_string(),
                tool: step.tool.clone(),
                args: step.args.clone(),
                reason: "executor_fallback_json_parse".to_string(),
            })
        } else {
            ExecutorOutput {
                decision: "execute".to_string(),
                tool: step.tool.clone(),
                args: step.args.clone(),
                reason: "llm_disabled".to_string(),
            }
        };

        if let Some(obj) = task.payload.as_object_mut() {
            obj.insert(
                "executor_decision".to_string(),
                serde_json::to_value(&decision).unwrap_or_default(),
            );
        }

        if decision.decision == "fail" {
            AgentOutcome::retry("执行决策失败")
        } else {
            AgentOutcome::success()
        }
    }

    fn should_handle(&self, ctx: &MessageContext) -> bool {
        let m = ctx.message.to_lowercase();
        m.contains("执行") || m.contains("run") || m.contains("command")
    }

    async fn generate_reply(&self, ctx: &MessageContext, _history: &[Value]) -> Result<String> {
        if let Some(llm) = &self.llm {
            return llm_generate(
                llm,
                "你是执行智能体，请回答执行层面的建议。".to_string(),
                ctx.message.clone(),
                tokio_util::sync::CancellationToken::new(),
            )
            .await;
        }
        Ok(format!("[Executor] 可执行建议：{}", ctx.message))
    }
}
