use async_trait::async_trait;

use crate::agents::agent::{Agent, AgentContext, AgentLlm, AgentOutcome};
use crate::agents::llm_json::{parse_json_with_extract, ExecutorOutput, PlannerOutput};
use crate::configs::load_agents;
use crate::core::logger;
use crate::core::task::Task;
use crate::providers::types::ChatRequest;

pub struct ExecutorAgent {
    llm: Option<AgentLlm>,
}

impl ExecutorAgent {
    pub fn new(llm: Option<AgentLlm>) -> Self {
        Self { llm }
    }

    fn fallback() -> ExecutorOutput {
        ExecutorOutput {
            decision: "fail".to_string(),
            tool: "no_op".to_string(),
            args: serde_json::json!({}),
            reason: "executor_fallback".to_string(),
        }
    }
}

#[async_trait]
impl Agent for ExecutorAgent {
    fn name(&self) -> &'static str {
        "ExecutorAgent"
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

        let current_step = plan.as_ref().and_then(|p| p.steps.get(plan_index)).cloned();

        let Some(step) = current_step else {
            let d = Self::fallback();
            if let Some(obj) = task.payload.as_object_mut() {
                obj.insert(
                    "executor_decision".to_string(),
                    serde_json::to_value(&d).unwrap_or_default(),
                );
            }
            return AgentOutcome::retry("计划步骤不存在");
        };

        let llm = match &self.llm {
            Some(v) => v,
            None => {
                let d = ExecutorOutput {
                    decision: if step.tool == "shell_command" || step.tool == "no_op" {
                        "execute".to_string()
                    } else {
                        "fail".to_string()
                    },
                    tool: step.tool.clone(),
                    args: step.args.clone(),
                    reason: "llm_disabled".to_string(),
                };
                if let Some(obj) = task.payload.as_object_mut() {
                    obj.insert(
                        "executor_decision".to_string(),
                        serde_json::to_value(&d).unwrap_or_default(),
                    );
                }
                return if d.decision == "fail" {
                    AgentOutcome::retry("执行决策失败")
                } else {
                    AgentOutcome::success()
                };
            }
        };

        let prompts = load_agents().ok();
        let system_prompt = prompts
            .as_ref()
            .map(|v| v.executor.system_prompt.clone())
            .unwrap_or_else(|| "你是 ExecutorAgent。请输出 JSON。".to_string());
        let user_prompt_t = prompts
            .as_ref()
            .map(|v| v.executor.user_prompt.clone())
            .unwrap_or_else(|| "步骤：{step_description}".to_string());

        let request = ChatRequest {
            system_prompt,
            user_prompt: user_prompt_t.replace(
                "{step_description}",
                &serde_json::to_string(&step).unwrap_or_default(),
            ),
            model: llm.model.clone(),
            temperature: llm.temperature,
            max_tokens: llm.max_tokens,
        };

        let decision = match llm.provider.chat(request, ctx.cancellation.clone()).await {
            Ok(resp) => parse_json_with_extract::<ExecutorOutput>(&resp.content).unwrap_or_else(|| {
                logger::log("[Executor] 模型输出非 JSON，使用 fail fallback");
                Self::fallback()
            }),
            Err(e) => {
                logger::log(format!("[Executor] 调用模型失败，使用 fail fallback err={}", e));
                Self::fallback()
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
}
