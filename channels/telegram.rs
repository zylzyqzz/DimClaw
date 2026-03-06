use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::core::traits::channel::{Channel, IncomingMessage, MessageHandler, OutgoingMessage};

#[derive(Clone, Default)]
pub struct TelegramChannel {
    pub bot_token: String,
    handler: Arc<Mutex<Option<Arc<dyn MessageHandler>>>>,
}

#[async_trait]
impl Channel for TelegramChannel {
    fn name(&self) -> &str {
        "telegram"
    }

    async fn start(&self) -> anyhow::Result<()> {
        crate::core::logger::log("[Channel:Telegram] start");
        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        crate::core::logger::log("[Channel:Telegram] stop");
        Ok(())
    }

    async fn send(&self, msg: OutgoingMessage) -> anyhow::Result<()> {
        crate::core::logger::log(format!("[Channel:Telegram] send thread={} text={}", msg.thread_id, msg.text));
        Ok(())
    }

    fn set_handler(&self, handler: Arc<dyn MessageHandler>) {
        if let Ok(mut guard) = self.handler.lock() {
            *guard = Some(handler);
        }
    }
}

impl TelegramChannel {
    pub async fn mock_receive(&self, msg: IncomingMessage) -> anyhow::Result<String> {
        let handler = self.handler.lock().ok().and_then(|g| g.clone());
        if let Some(h) = handler {
            h.handle(msg).await
        } else {
            Ok(String::new())
        }
    }
}
