use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::core::traits::memory::{LongTermMemory, Memory, MemoryItem};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct PersistedMemory {
    id: String,
    session: String,
    content: String,
    embedding: Option<Vec<f32>>,
    metadata: HashMap<String, String>,
    created_at: String,
}

pub struct SqliteMemory {
    path: PathBuf,
    short: Mutex<HashMap<String, Vec<u8>>>,
    long: Mutex<Vec<PersistedMemory>>,
}

impl SqliteMemory {
    pub fn new(path: &Path) -> anyhow::Result<Self> {
        let path = path.to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let long = if path.exists() {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            serde_json::from_str::<Vec<PersistedMemory>>(&text).unwrap_or_default()
        } else {
            Vec::new()
        };

        Ok(Self {
            path,
            short: Mutex::new(HashMap::new()),
            long: Mutex::new(long),
        })
    }

    fn persist(&self) -> anyhow::Result<()> {
        if let Ok(long) = self.long.lock() {
            std::fs::write(&self.path, serde_json::to_string_pretty(&*long)?)?;
        }
        Ok(())
    }

    pub async fn hybrid_search(&self, session_id: &str, query: &str, limit: usize) -> anyhow::Result<Vec<MemoryItem>> {
        let vector_results = self.vector_search(session_id, query, limit * 2).await?;
        let keyword_results = self.keyword_search(session_id, query, limit * 2).await?;
        let mut merged: HashMap<String, MemoryItem> = HashMap::new();

        for item in vector_results {
            merged
                .entry(item.id.clone())
                .and_modify(|cur| cur.score = (cur.score + item.score * 0.7).max(cur.score))
                .or_insert(item);
        }
        for item in keyword_results {
            merged
                .entry(item.id.clone())
                .and_modify(|cur| cur.score = (cur.score + item.score * 0.3).max(cur.score))
                .or_insert(item);
        }

        let mut out: Vec<MemoryItem> = merged.into_values().collect();
        out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        out.truncate(limit);
        Ok(out)
    }

    async fn vector_search(&self, session_id: &str, query: &str, limit: usize) -> anyhow::Result<Vec<MemoryItem>> {
        // 轻量实现：用 embedding 长度与文本匹配近似向量分。
        let mut out = Vec::new();
        if let Ok(long) = self.long.lock() {
            for item in long.iter().filter(|v| v.session == session_id) {
                let mut score = simple_score(&item.content, query);
                if let Some(emb) = &item.embedding {
                    score += (emb.len() as f32).ln_1p() * 0.01;
                }
                if score > 0.0 {
                    out.push(MemoryItem {
                        id: item.id.clone(),
                        session: item.session.clone(),
                        content: item.content.clone(),
                        score,
                        metadata: item.metadata.clone(),
                    });
                }
            }
        }
        out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        out.truncate(limit);
        Ok(out)
    }

    async fn keyword_search(&self, session_id: &str, query: &str, limit: usize) -> anyhow::Result<Vec<MemoryItem>> {
        let mut out = Vec::new();
        if let Ok(long) = self.long.lock() {
            for item in long.iter().filter(|v| v.session == session_id) {
                let score = simple_score(&item.content, query);
                if score > 0.0 {
                    out.push(MemoryItem {
                        id: item.id.clone(),
                        session: item.session.clone(),
                        content: item.content.clone(),
                        score,
                        metadata: item.metadata.clone(),
                    });
                }
            }
        }
        out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        out.truncate(limit);
        Ok(out)
    }
}

#[async_trait]
impl Memory for SqliteMemory {
    async fn store_short(&self, session: &str, key: &str, value: &[u8]) -> anyhow::Result<()> {
        let k = format!("{}:{}", session, key);
        if let Ok(mut short) = self.short.lock() {
            short.insert(k, value.to_vec());
        }
        Ok(())
    }

    async fn load_short(&self, session: &str, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
        let k = format!("{}:{}", session, key);
        Ok(self.short.lock().ok().and_then(|s| s.get(&k).cloned()))
    }

    async fn store_long(&self, memory: LongTermMemory) -> anyhow::Result<()> {
        if let Ok(mut long) = self.long.lock() {
            long.push(PersistedMemory {
                id: uuid::Uuid::new_v4().to_string(),
                session: memory.session,
                content: memory.content,
                embedding: memory.embedding,
                metadata: memory.metadata,
                created_at: Utc::now().to_rfc3339(),
            });
        }
        self.persist()
    }

    async fn search(&self, query: &str, limit: usize) -> anyhow::Result<Vec<MemoryItem>> {
        let session = if let Ok(long) = self.long.lock() {
            long.last().map(|v| v.session.clone()).unwrap_or_default()
        } else {
            String::new()
        };
        if session.is_empty() {
            return Ok(Vec::new());
        }
        self.hybrid_search(&session, query, limit).await
    }
}

fn simple_score(content: &str, query: &str) -> f32 {
    if query.trim().is_empty() {
        return 0.0;
    }
    let mut score = 0.0;
    for token in query.split_whitespace() {
        if token.is_empty() {
            continue;
        }
        if content.contains(token) {
            score += 1.0;
        }
    }
    if content.contains(query) {
        score += 1.5;
    }
    score
}
