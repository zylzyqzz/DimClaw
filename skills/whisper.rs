use anyhow::{anyhow, Result};

use crate::skills::shell_command::run_command_capture;
use crate::skills::{Skill, SkillContext, SkillResult};

pub struct WhisperSkill;

#[async_trait::async_trait]
impl Skill for WhisperSkill {
    async fn run(&self, ctx: SkillContext, input: serde_json::Value) -> Result<SkillResult> {
        let audio = input
            .get("audio")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("whisper 缺少 audio"))?
            .trim();
        let lang = input
            .get("language")
            .and_then(|v| v.as_str())
            .unwrap_or("zh");
        let cmd = format!("whisper \"{}\" --language {}", audio, lang);
        run_command_capture(&cmd, ctx.timeout_secs, ctx.cancellation).await
    }
}
