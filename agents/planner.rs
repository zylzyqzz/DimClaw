use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::agents::agent::{llm_generate, Agent, AgentContext, AgentLlm, AgentOutcome, MessageContext};
use crate::agents::llm_json::{parse_json_with_extract, PlanStep, PlannerOutput};
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
            steps: vec![PlanStep {
                id: 1,
                action: "execute input command".to_string(),
                tool: "shell_command".to_string(),
                args: serde_json::json!({
                    "command": task.payload
                        .get("input")
                        .and_then(|v| v.get("command"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("echo planner-fallback"),
                    "timeout_secs": task.payload
                        .get("input")
                        .and_then(|v| v.get("timeout_secs"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(10)
                }),
            }],
        }
    }
}

#[async_trait]
impl Agent for PlannerAgent {
    fn name(&self) -> &str {
        "Planner"
    }

    async fn handle(&self, task: &mut Task, ctx: AgentContext) -> AgentOutcome {
        if ctx.cancellation.is_cancelled() {
            return AgentOutcome::cancelled("规划阶段收到取消信号");
        }

        logger::log(format!("[Planner] id={} 开始规划", task.id));
        let llm = match &self.llm {
            Some(v) => v,
            None => {
                let plan = Self::fallback(task);
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
            .map(|v| v.build_system_prompt(&v.planner.system_prompt))
            .unwrap_or_else(|| {
                "你是 Planner。必须只输出 JSON：{\"goal\":\"...\",\"steps\":[{\"id\":1,\"action\":\"...\",\"tool\":\"...\",\"args\":{}}]}".to_string()
            });
        let user_prompt_t = prompts
            .as_ref()
            .map(|v| v.planner.user_prompt.clone())
            .unwrap_or_else(|| "任务: {task_payload}".to_string());

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
                    logger::log("[Planner] 模型输出解析失败，使用 fallback 计划");
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

    fn should_handle(&self, ctx: &MessageContext) -> bool {
        let m = ctx.message.to_lowercase();
        m.contains("计划") || m.contains("规划") || m.contains("plan")
    }

    async fn generate_reply(&self, ctx: &MessageContext, history: &[Value]) -> Result<String> {
        let Some(llm) = &self.llm else {
            return Ok(format!("[Planner] 建议先拆解任务：{}", ctx.message));
        };

        let history_text = history
            .iter()
            .rev()
            .take(6)
            .rev()
            .map(|v| {
                format!(
                    "{}: {}",
                    v.get("role").and_then(|x| x.as_str()).unwrap_or("user"),
                    v.get("content").and_then(|x| x.as_str()).unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let user_prompt = format!(
            "历史:\n{}\n\n用户消息:{}\n请给出规划建议。",
            history_text, ctx.message
        );
        llm_generate(
            llm,
            "你是规划智能体，请用简洁中文回复。".to_string(),
            user_prompt,
            tokio_util::sync::CancellationToken::new(),
        )
        .await
    }
}
