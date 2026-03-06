use anyhow::Result;
use async_trait::async_trait;

use super::{Hand, HandResult, ScheduleConfig};

#[derive(Default)]
pub struct ResearcherHand;

#[async_trait]
impl Hand for ResearcherHand {
    fn name(&self) -> &str { "researcher" }
    fn description(&self) -> &str { "研究手：多源收集并输出研究报告" }
    fn schedule(&self) -> ScheduleConfig {
        ScheduleConfig { cron: None, interval: Some(1800), condition: None, timezone: "Asia/Shanghai".to_string() }
    }
    async fn execute(&self) -> Result<HandResult> {
        std::fs::create_dir_all("./data/hands")?;
        let path = std::path::PathBuf::from("./data/hands/research.md");
        std::fs::write(&path, "# Research Report\n\n- source: placeholder\n")?;
        let mut out = HandResult::ok("Researcher 执行完成");
        out.artifacts.push(path);
        Ok(out)
    }
}
