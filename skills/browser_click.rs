use anyhow::Result;

use crate::skills::{Skill, SkillContext, SkillResult};

pub struct BrowserClickSkill;

#[async_trait::async_trait]
impl Skill for BrowserClickSkill {
    async fn run(&self, _ctx: SkillContext, input: serde_json::Value) -> Result<SkillResult> {
        let selector = input.get("selector").and_then(|v| v.as_str()).unwrap_or("");
        Ok(SkillResult {
            success: true,
            stdout: format!("browser_click 已记录 selector={} (占位实现)", selector),
            stderr: String::new(),
            exit_code: Some(0),
            duration_ms: 0,
        })
    }
}
