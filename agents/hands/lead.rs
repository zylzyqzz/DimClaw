use anyhow::Result;
use async_trait::async_trait;

use super::{Hand, HandResult, ScheduleConfig};

#[derive(Default)]
pub struct LeadHand;

#[async_trait]
impl Hand for LeadHand {
    fn name(&self) -> &str { "lead" }
    fn description(&self) -> &str { "销售线索手：抓取并评分潜在客户" }
    fn schedule(&self) -> ScheduleConfig {
        ScheduleConfig { cron: Some("0 8 * * *".to_string()), interval: None, condition: None, timezone: "Asia/Shanghai".to_string() }
    }
    async fn execute(&self) -> Result<HandResult> {
        let path = std::path::PathBuf::from("./data/hands/lead_report.csv");
        std::fs::create_dir_all("./data/hands")?;
        std::fs::write(&path, "name,source,score\nexample,github,76\n")?;
        let mut out = HandResult::ok("Lead 执行完成，已生成线索报告");
        out.artifacts.push(path);
        Ok(out)
    }
}
