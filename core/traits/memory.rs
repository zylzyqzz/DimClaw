use std::collections::HashMap;

use async_trait::async_trait;

#[derive(Clone, Debug, Default)]
pub struct LongTermMemory {
    pub session: String,
    pub content: String,
    pub embedding: Option<Vec<f32>>,
    pub metadata: HashMap<String, String>,
}

#[derive(Clone, Debug, Default)]
pub struct MemoryItem {
    pub id: String,
    pub session: String,
    pub content: String,
    pub score: f32,
    pub metadata: HashMap<String, String>,
}

#[async_trait]
pub trait Memory: Send + Sync {
    async fn store_short(&self, session: &str, key: &str, value: &[u8]) -> anyhow::Result<()>;
    async fn load_short(&self, session: &str, key: &str) -> anyhow::Result<Option<Vec<u8>>>;

    async fn store_long(&self, memory: LongTermMemory) -> anyhow::Result<()>;
    async fn search(&self, query: &str, limit: usize) -> anyhow::Result<Vec<MemoryItem>>;
}
