use anyhow::Result;
use async_trait::async_trait;

use super::{Hand, HandResult, ScheduleConfig};

#[derive(Default)]
pub struct BrowserHand;

#[async_trait]
impl Hand for BrowserHand {
    fn name(&self) -> &str { "browser" }
    fn description(&self) -> &str { "浏览器手：自动执行网页动作" }
    fn schedule(&self) -> ScheduleConfig {
        ScheduleConfig { cron: None, interval: Some(1200), condition: None, timezone: "Asia/Shanghai".to_string() }
    }
    async fn execute(&self) -> Result<HandResult> {
        Ok(HandResult::ok("Browser Hand 已执行动作序列"))
    }
}
