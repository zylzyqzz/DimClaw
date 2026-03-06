use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agents::agent::{Agent, AgentLlm, MessageContext};
use crate::agents::{CustomAgent, ExecutorAgent, PlannerAgent, RecoveryAgent, VerifierAgent};
use crate::configs::{load_channels, list_custom_agents, load_models};
use crate::providers::openai_compatible::OpenAiCompatibleProvider;
use crate::providers::traits::LlmProvider;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutedReply {
    pub reply: String,
    pub agent_name: String,
}

pub async fn route_channel_message(ctx: MessageContext, history: Vec<Value>) -> Result<RoutedReply> {
    let channels = load_channels()?;
    let channel_cfg = match ctx.channel.as_str() {
        "feishu" => channels.feishu,
        "telegram" => channels.telegram,
        _ => channels.feishu,
    };

    let agent_map = build_chat_agents().await?;
    let selected = pick_agent_name(&ctx, &channel_cfg, &agent_map);

    let agent = agent_map
        .get(&selected)
        .or_else(|| agent_map.get("Planner"))
        .ok_or_else(|| anyhow!("未找到可用智能体"))?;

    let reply = agent
        .generate_reply(&ctx, &history)
        .await
        .unwrap_or_else(|e| format!("{} 回复失败: {}", agent.name(), e));
    Ok(RoutedReply {
        reply,
        agent_name: agent.name().to_string(),
    })
}

fn pick_agent_name(
    ctx: &MessageContext,
    channel_cfg: &crate::configs::ChannelConfig,
    agent_map: &HashMap<String, Arc<dyn Agent>>,
) -> String {
    let text = ctx.message.to_lowercase();

    for (name, route) in &channel_cfg.agent_map {
        if !route.enabled {
            continue;
        }
        if route.keywords.is_empty() {
            continue;
        }
        let hit = route
            .keywords
            .iter()
            .any(|k| !k.trim().is_empty() && text.contains(&k.to_lowercase()));
        if hit && agent_map.contains_key(name) {
            return name.clone();
        }
    }

    if channel_cfg.mode == "single" && agent_map.contains_key(&channel_cfg.single_agent) {
        return channel_cfg.single_agent.clone();
    }

    for name in &channel_cfg.agents {
        if let Some(agent) = agent_map.get(name) {
            if agent.should_handle(ctx) {
                return name.clone();
            }
        }
    }

    if !channel_cfg.default_agent.trim().is_empty() && agent_map.contains_key(&channel_cfg.default_agent) {
        return channel_cfg.default_agent.clone();
    }
    "Planner".to_string()
}

pub async fn build_chat_agents() -> Result<HashMap<String, Arc<dyn Agent>>> {
    let llm = build_default_llm()?;

    let mut map: HashMap<String, Arc<dyn Agent>> = HashMap::new();
    map.insert("Planner".to_string(), Arc::new(PlannerAgent::new(llm.clone())));
    map.insert("Executor".to_string(), Arc::new(ExecutorAgent::new(llm.clone())));
    map.insert("Verifier".to_string(), Arc::new(VerifierAgent::new(llm.clone())));
    map.insert("Recovery".to_string(), Arc::new(RecoveryAgent::new(llm.clone())));

    for cfg in list_custom_agents().unwrap_or_default() {
        if !cfg.enabled {
            continue;
        }
        let name = cfg.name.clone();
        map.insert(name, Arc::new(CustomAgent::new(cfg, llm.clone())));
    }

    Ok(map)
}

fn build_default_llm() -> Result<Option<AgentLlm>> {
    let models = load_models().unwrap_or_default();
    let provider = models
        .providers
        .into_iter()
        .find(|p| p.enabled && p.r#default)
        .or_else(|| {
            load_models()
                .ok()
                .and_then(|m| m.providers.into_iter().find(|p| p.enabled))
        });

    let Some(provider) = provider else {
        return Ok(None);
    };

    if provider.protocol != "openai_compatible" {
        return Ok(None);
    }
    if provider.api_key.trim().is_empty() || provider.base_url.trim().is_empty() || provider.model.trim().is_empty() {
        return Ok(None);
    }

    let client = OpenAiCompatibleProvider::new(
        provider.name,
        provider.base_url,
        provider.api_key,
        provider.timeout_secs,
        1,
    )?;
    let arc: Arc<dyn LlmProvider> = Arc::new(client);

    Ok(Some(AgentLlm {
        provider: arc,
        model: provider.model,
        temperature: provider.temperature,
        max_tokens: provider.max_tokens,
    }))
}
