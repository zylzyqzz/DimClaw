use async_trait::async_trait;

use crate::agents::agent::{Agent, AgentContext, AgentLlm, AgentOutcome};
use crate::agents::llm_json::{parse_json_with_extract, PlannerOutput};
use crate::configs::load_agents;
use crate::core::logger;
use crate::core::task::Task;
use crate::providers::types::ChatRequest;

pub struct PlannerAgent {
    llm: Option<AgentLlm>,
}

impl PlannerAgent {
    pub fn new(llm: Option<AgentLlm>) -> Self {
        Self { llm }
    }

    fn fallback(task: &Task) -> PlannerOutput {
        PlannerOutput {
            goal: task.title.clone(),
            steps: vec![crate::agents::llm_json::PlanStep {
                id: 1,
                action: "fallback no_op".to_string(),
                tool: "no_op".to_string(),
                args: serde_json::json!({}),
            }],
        }
    }

    fn local_plan(task: &Task) -> PlannerOutput {
        let command = task
            .payload
            .get("input")
            .and_then(|v| v.get("command"))
            .and_then(|v| v.as_str())
            .unwrap_or("echo dimclaw_local_plan");
        let timeout_secs = task
            .payload
            .get("input")
            .and_then(|v| v.get("timeout_secs"))
            .and_then(|v| v.as_u64())
            .unwrap_or(10);
        PlannerOutput {
            goal: task.title.clone(),
            steps: vec![crate::agents::llm_json::PlanStep {
                id: 1,
                action: "execute command".to_string(),
                tool: "shell_command".to_string(),
                args: serde_json::json!({
                    "command": command,
                    "timeout_secs": timeout_secs
                }),
            }],
        }
    }
}

#[async_trait]
impl Agent for PlannerAgent {
    fn name(&self) -> &'static str {
        "PlannerAgent"
    }

    async fn handle(&self, task: &mut Task, ctx: AgentContext) -> AgentOutcome {
        if ctx.cancellation.is_cancelled() {
            return AgentOutcome::cancelled("规划阶段收到取消信号");
        }
        logger::log(format!("[Planner] id={} 开始规划", task.id));
        let llm = match &self.llm {
            Some(v) => v,
            None => {
                let plan = Self::local_plan(task);
                if let Some(obj) = task.payload.as_object_mut() {
                    obj.insert("plan".to_string(), serde_json::to_value(plan).unwrap_or_default());
                    obj.insert("plan_index".to_string(), serde_json::json!(0));
                }
                return AgentOutcome::success();
            }
        };

        let prompts = load_agents().ok();
        let system_prompt = prompts
            .as_ref()
            .map(|v| v.planner.system_prompt.clone())
            .unwrap_or_else(|| "你是 PlannerAgent。你必须仅输出 JSON。".to_string());
        let user_prompt_t = prompts
            .as_ref()
            .map(|v| v.planner.user_prompt.clone())
            .unwrap_or_else(|| "任务：{task_payload}".to_string());

        let request = ChatRequest {
            system_prompt,
            user_prompt: user_prompt_t.replace("{task_payload}", &task.payload.to_string()),
            model: llm.model.clone(),
            temperature: llm.temperature,
            max_tokens: llm.max_tokens,
        };

        let response = llm.provider.chat(request, ctx.cancellation.clone()).await;

        let plan = match response {
            Ok(resp) => match parse_json_with_extract::<PlannerOutput>(&resp.content) {
                Some(v) if !v.steps.is_empty() => v,
                _ => {
                    logger::log("[Planner] 模型输出解析失败，使用 fallback no_op 计划");
                    Self::fallback(task)
                }
            },
            Err(e) => {
                logger::log(format!("[Planner] 调用模型失败，使用 fallback 计划 err={}", e));
                Self::fallback(task)
            }
        };

        if let Some(obj) = task.payload.as_object_mut() {
            obj.insert("plan".to_string(), serde_json::to_value(plan).unwrap_or_default());
            obj.insert("plan_index".to_string(), serde_json::json!(0));
        }
        AgentOutcome::success()
    }
}
