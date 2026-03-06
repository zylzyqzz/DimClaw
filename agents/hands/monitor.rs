use anyhow::Result;
use async_trait::async_trait;

use super::{Condition, Hand, HandResult, ScheduleConfig};

#[derive(Default)]
pub struct MonitorHand;

#[async_trait]
impl Hand for MonitorHand {
    fn name(&self) -> &str { "monitor" }
    fn description(&self) -> &str { "监控手：采集指标并触发处理" }
    fn schedule(&self) -> ScheduleConfig {
        ScheduleConfig {
            cron: None,
            interval: Some(60),
            condition: Some(Condition::CpuAbove(0.8)),
            timezone: "Asia/Shanghai".to_string(),
        }
    }
    async fn execute(&self) -> Result<HandResult> {
        let mut out = HandResult::ok("Monitor 检查完成");
        out.metrics.insert("cpu".to_string(), 0.1);
        Ok(out)
    }
}
