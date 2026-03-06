use anyhow::{anyhow, Result};

use crate::skills::shell_command::run_command_capture;
use crate::skills::{Skill, SkillContext, SkillResult};

pub struct FfmpegSkill;

#[async_trait::async_trait]
impl Skill for FfmpegSkill {
    async fn run(&self, ctx: SkillContext, input: serde_json::Value) -> Result<SkillResult> {
        let input_file = input
            .get("input")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("ffmpeg 缺少 input"))?
            .trim();
        let output_file = input
            .get("output")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("ffmpeg 缺少 output"))?
            .trim();
        let options = input
            .get("options")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
        let cmd = format!("ffmpeg -y -i \"{}\" {} \"{}\"", input_file, options, output_file);
        run_command_capture(&cmd, ctx.timeout_secs, ctx.cancellation).await
    }
}
