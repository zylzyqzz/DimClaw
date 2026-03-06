use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::agents::agent::{llm_generate, Agent, AgentContext, AgentLlm, AgentOutcome, MessageContext};
use crate::configs::CustomAgentConfig;
use crate::core::task::Task;

pub struct CustomAgent {
    pub cfg: CustomAgentConfig,
    llm: Option<AgentLlm>,
}

impl CustomAgent {
    pub fn new(cfg: CustomAgentConfig, llm: Option<AgentLlm>) -> Self {
        Self { cfg, llm }
    }

    fn render_system_prompt(&self, task_title: &str, payload: &Value) -> String {
        let mut tpl = self.cfg.system_prompt_template.clone();
        if tpl.trim().is_empty() {
            tpl = format!("你是自定义智能体 {}，角色是 {}", self.cfg.name, self.cfg.role);
        }
        tpl.replace("{task_title}", task_title)
            .replace("{payload}", &payload.to_string())
    }
}

#[async_trait]
impl Agent for CustomAgent {
    fn name(&self) -> &str {
        &self.cfg.name
    }

    async fn handle(&self, task: &mut Task, _ctx: AgentContext) -> AgentOutcome {
        if !self.cfg.enabled {
            return AgentOutcome::success();
        }

        let note = if let Some(llm) = &self.llm {
            let system_prompt = self.render_system_prompt(&task.title, &task.payload);
            let user_prompt = format!("请根据任务给出你的建议: {}", task.title);
            llm_generate(
                llm,
                system_prompt,
                user_prompt,
                tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap_or_else(|e| format!("custom_agent_error: {}", e))
        } else {
            format!("{}: LLM 未启用", self.cfg.name)
        };

        if let Some(obj) = task.payload.as_object_mut() {
            let notes = obj
                .entry("custom_agent_notes")
                .or_insert_with(|| serde_json::json!({}));
            if let Some(map) = notes.as_object_mut() {
                map.insert(self.cfg.name.clone(), Value::String(note));
            }
        }

        AgentOutcome::success()
    }

    fn should_handle(&self, ctx: &MessageContext) -> bool {
        if self.cfg.trigger_keywords.is_empty() {
            return false;
        }
        let lower = ctx.message.to_lowercase();
        self.cfg
            .trigger_keywords
            .iter()
            .any(|kw| !kw.trim().is_empty() && lower.contains(&kw.to_lowercase()))
    }

    async fn generate_reply(&self, ctx: &MessageContext, history: &[Value]) -> Result<String> {
        let Some(llm) = &self.llm else {
            return Ok(format!("{} 收到: {}", self.cfg.name, ctx.message));
        };

        let prompt = self
            .cfg
            .system_prompt_template
            .replace("{task_title}", &ctx.message)
            .replace("{payload}", &serde_json::json!({"history": history}).to_string());

        llm_generate(
            llm,
            if prompt.trim().is_empty() {
                format!("你是 {}，角色 {}", self.cfg.name, self.cfg.role)
            } else {
                prompt
            },
            ctx.message.clone(),
            tokio_util::sync::CancellationToken::new(),
        )
        .await
    }
}
