use anyhow::Result;
use async_trait::async_trait;

use super::{Hand, HandResult, ScheduleConfig};

#[derive(Default)]
pub struct SchedulerHand;

#[async_trait]
impl Hand for SchedulerHand {
    fn name(&self) -> &str { "scheduler" }
    fn description(&self) -> &str { "定时任务手：通用任务调度执行器" }
    fn schedule(&self) -> ScheduleConfig {
        ScheduleConfig { cron: Some("*/10 * * * *".to_string()), interval: None, condition: None, timezone: "Asia/Shanghai".to_string() }
    }
    async fn execute(&self) -> Result<HandResult> {
        Ok(HandResult::ok("Scheduler Hand 本次执行成功"))
    }
}
