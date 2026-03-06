use anyhow::Result;

use crate::skills::{Skill, SkillContext, SkillResult};

pub struct BrowserOpenSkill;

#[async_trait::async_trait]
impl Skill for BrowserOpenSkill {
    async fn run(&self, ctx: SkillContext, input: serde_json::Value) -> Result<SkillResult> {
        let skill = crate::skills::browser_automator::BrowserAutomatorSkill;
        skill
            .run(
                ctx,
                serde_json::json!({
                    "action": "open",
                    "url": input.get("url").and_then(|v| v.as_str()).unwrap_or("https://www.baidu.com"),
                    "browser": input.get("browser").and_then(|v| v.as_str()).unwrap_or("")
                }),
            )
            .await
    }
}
