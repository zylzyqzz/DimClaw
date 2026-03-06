use anyhow::Result;

use crate::skills::{Skill, SkillContext, SkillResult};

pub struct BrowserFillSkill;

#[async_trait::async_trait]
impl Skill for BrowserFillSkill {
    async fn run(&self, _ctx: SkillContext, input: serde_json::Value) -> Result<SkillResult> {
        let selector = input.get("selector").and_then(|v| v.as_str()).unwrap_or("");
        let value = input.get("value").and_then(|v| v.as_str()).unwrap_or("");
        Ok(SkillResult {
            success: true,
            stdout: format!("browser_fill 已记录 selector={} value={} (占位实现)", selector, value),
            stderr: String::new(),
            exit_code: Some(0),
            duration_ms: 0,
        })
    }
}
