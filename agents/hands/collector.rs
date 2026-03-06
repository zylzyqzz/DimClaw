use anyhow::Result;
use async_trait::async_trait;

use super::{Condition, Hand, HandResult, ScheduleConfig};

#[derive(Default)]
pub struct CollectorHand;

#[async_trait]
impl Hand for CollectorHand {
    fn name(&self) -> &str { "collector" }
    fn description(&self) -> &str { "情报手：周期抓取并检测变化" }
    fn schedule(&self) -> ScheduleConfig {
        ScheduleConfig {
            cron: None,
            interval: Some(600),
            condition: Some(Condition::Custom("target_changed".to_string())),
            timezone: "Asia/Shanghai".to_string(),
        }
    }
    async fn execute(&self) -> Result<HandResult> {
        Ok(HandResult::ok("Collector 已完成本次巡检"))
    }
}
