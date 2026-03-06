use async_trait::async_trait;

#[derive(Clone, Debug, Default)]
pub struct IncomingMessage {
    pub channel: String,
    pub context: crate::core::traits::agent::MessageContext,
}

#[derive(Clone, Debug, Default)]
pub struct OutgoingMessage {
    pub channel: String,
    pub thread_id: String,
    pub text: String,
}

#[async_trait]
pub trait MessageHandler: Send + Sync {
    async fn handle(&self, msg: IncomingMessage) -> anyhow::Result<String>;
}

#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn start(&self) -> anyhow::Result<()>;
    async fn stop(&self) -> anyhow::Result<()>;
    async fn send(&self, msg: OutgoingMessage) -> anyhow::Result<()>;
    fn set_handler(&self, handler: std::sync::Arc<dyn MessageHandler>);
}
