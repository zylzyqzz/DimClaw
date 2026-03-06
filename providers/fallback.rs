use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;

use crate::core::traits::provider::{ChatRequest, ChatResponse, ModelInfo, Provider};

pub struct FallbackProvider {
    pub providers: Vec<Arc<dyn Provider>>,
    pub current: AtomicUsize,
}

impl FallbackProvider {
    pub fn new(providers: Vec<Arc<dyn Provider>>) -> Self {
        Self {
            providers,
            current: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl Provider for FallbackProvider {
    fn name(&self) -> &str {
        "fallback"
    }

    async fn chat(&self, req: ChatRequest) -> anyhow::Result<ChatResponse> {
        if self.providers.is_empty() {
            return Err(anyhow::anyhow!("no providers"));
        }

        let start = self.current.load(Ordering::Relaxed);
        for i in 0..self.providers.len() {
            let idx = (start + i) % self.providers.len();
            let provider = &self.providers[idx];
            match provider.chat(req.clone()).await {
                Ok(resp) => {
                    self.current.store(idx, Ordering::Relaxed);
                    return Ok(resp);
                }
                Err(_) => continue,
            }
        }
        Err(anyhow::anyhow!("all providers failed"))
    }

    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        for p in &self.providers {
            if let Ok(v) = p.embed(text).await {
                return Ok(v);
            }
        }
        Err(anyhow::anyhow!("all providers failed for embed"))
    }

    fn models(&self) -> Vec<ModelInfo> {
        self.providers.iter().flat_map(|p| p.models()).collect()
    }
}
