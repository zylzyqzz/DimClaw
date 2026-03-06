use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::core::traits::channel::{Channel, MessageHandler, OutgoingMessage};

#[derive(Clone, Default)]
pub struct WebhookChannel {
    pub endpoint: String,
    handler: Arc<Mutex<Option<Arc<dyn MessageHandler>>>>,
}

#[async_trait]
impl Channel for WebhookChannel {
    fn name(&self) -> &str {
        "webhook"
    }

    async fn start(&self) -> anyhow::Result<()> {
        crate::core::logger::log("[Channel:Webhook] start");
        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        crate::core::logger::log("[Channel:Webhook] stop");
        Ok(())
    }

    async fn send(&self, msg: OutgoingMessage) -> anyhow::Result<()> {
        crate::core::logger::log(format!("[Channel:Webhook] send thread={} text={}", msg.thread_id, msg.text));
        Ok(())
    }

    fn set_handler(&self, handler: Arc<dyn MessageHandler>) {
        if let Ok(mut guard) = self.handler.lock() {
            *guard = Some(handler);
        }
    }
}
