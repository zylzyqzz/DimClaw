use anyhow::{anyhow, Result};

use crate::skills::shell_command::run_command_capture;
use crate::skills::{Skill, SkillContext, SkillResult};

pub struct YtDlpSkill;

#[async_trait::async_trait]
impl Skill for YtDlpSkill {
    async fn run(&self, ctx: SkillContext, input: serde_json::Value) -> Result<SkillResult> {
        let url = input
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("yt_dlp 缺少 url"))?
            .trim();
        if url.is_empty() {
            return Err(anyhow!("yt_dlp url 不能为空"));
        }
        let format = input.get("format").and_then(|v| v.as_str()).unwrap_or("best");
        let cmd = format!("yt-dlp -f \"{}\" \"{}\"", format, url);
        run_command_capture(&cmd, ctx.timeout_secs, ctx.cancellation).await
    }
}
