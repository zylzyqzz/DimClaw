use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;

use crate::core::traits::agent::{Agent, GenericAgent};
use crate::core::traits::channel::Channel;
use crate::core::traits::memory::Memory;
use crate::core::traits::provider::Provider;
use crate::core::traits::tool::Tool;
use crate::memory::sqlite_memory::SqliteMemory;

pub struct TraitRegistry {
    pub providers: HashMap<String, Arc<dyn Provider>>,
    pub channels: HashMap<String, Arc<dyn Channel>>,
    pub tools: HashMap<String, Arc<dyn Tool>>,
    pub memory: Arc<dyn Memory>,
    pub agents: HashMap<String, Arc<dyn Agent>>,
}

impl TraitRegistry {
    pub fn from_config(_config: &crate::configs::RuntimeConfig) -> Result<Self> {
        let memory: Arc<dyn Memory> = Arc::new(SqliteMemory::new(std::path::Path::new("./data/memory.db"))?);
        Ok(Self {
            providers: HashMap::new(),
            channels: HashMap::new(),
            tools: HashMap::new(),
            memory,
            agents: HashMap::new(),
        })
    }

    pub fn get_provider(&self, name: &str) -> Option<Arc<dyn Provider>> {
        self.providers.get(name).cloned()
    }

    pub fn get_channel(&self, name: &str) -> Option<Arc<dyn Channel>> {
        self.channels.get(name).cloned()
    }

    pub fn get_tool(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn register_agent(&mut self, agent: Arc<dyn Agent>) {
        self.agents.insert(agent.name().to_string(), agent);
    }

    pub fn register_generic_agent(&mut self, agent: GenericAgent) {
        self.register_agent(Arc::new(agent));
    }
}
