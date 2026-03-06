use async_trait::async_trait;

#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>>;
}

#[derive(Clone, Default)]
pub struct OpenAiEmbedder {
    pub api_key: String,
    pub endpoint: String,
    pub model: String,
}

#[async_trait]
impl Embedder for OpenAiEmbedder {
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let mut out = Vec::new();
        let bytes = text.as_bytes();
        for i in 0..64 {
            let b = *bytes.get(i % bytes.len().max(1)).unwrap_or(&0) as f32;
            out.push(b / 255.0);
        }
        Ok(out)
    }
}
