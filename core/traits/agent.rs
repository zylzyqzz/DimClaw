use std::collections::HashMap;

use async_trait::async_trait;

use super::provider::Provider;

#[derive(Clone, Debug, Default)]
pub struct MessageContext {
    pub channel: String,
    pub thread_id: String,
    pub user_id: String,
    pub text: String,
    pub group_id: Option<String>,
}

#[derive(Clone, Debug)]
pub enum AgentOutcome {
    Success,
    Retry(String),
    Fail(String),
    NextState(crate::core::task::TaskStatus),
}

#[async_trait]
pub trait Agent: Send + Sync {
    fn name(&self) -> &str;
    fn should_handle(&self, ctx: &MessageContext) -> bool;
    async fn generate(&self, ctx: &MessageContext) -> anyhow::Result<String>;
    async fn handle_task(&self, task: &mut crate::core::task::Task) -> AgentOutcome;
}

#[derive(Clone, Debug, Default)]
pub struct AieosIdentity {
    pub identity: serde_json::Value,
    pub psychology: serde_json::Value,
    pub linguistics: serde_json::Value,
    pub motivations: serde_json::Value,
}

impl AieosIdentity {
    pub fn build_prompt(&self) -> String {
        format!(
            "identity={}\npsychology={}\nlinguistics={}\nmotivations={}",
            self.identity, self.psychology, self.linguistics, self.motivations
        )
    }
}

pub struct GenericAgent {
    pub name: String,
    pub system_prompt: String,
    pub provider: std::sync::Arc<dyn Provider>,
    pub model: String,
    pub temperature: f32,
    pub keywords: Vec<String>,
    pub phase: String,
    pub extra: HashMap<String, String>,
}

#[async_trait]
impl Agent for GenericAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn should_handle(&self, ctx: &MessageContext) -> bool {
        if self.keywords.is_empty() {
            return false;
        }
        self.keywords.iter().any(|k| ctx.text.contains(k))
    }

    async fn generate(&self, ctx: &MessageContext) -> anyhow::Result<String> {
        let req = crate::core::traits::provider::ChatRequest {
            messages: vec![
                crate::core::traits::provider::Message {
                    role: "system".to_string(),
                    content: self.system_prompt.clone(),
                },
                crate::core::traits::provider::Message {
                    role: "user".to_string(),
                    content: ctx.text.clone(),
                },
            ],
            temperature: self.temperature,
            max_tokens: 1024,
            tools: None,
        };
        let out = self.provider.chat(req).await?;
        Ok(out.content)
    }

    async fn handle_task(&self, _task: &mut crate::core::task::Task) -> AgentOutcome {
        AgentOutcome::Success
    }
}
