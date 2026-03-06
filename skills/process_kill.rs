use anyhow::{anyhow, Result};

use crate::skills::guard::audit_command;
use crate::skills::shell_command::run_command_capture;
use crate::skills::{Skill, SkillContext, SkillResult};

pub struct ProcessKillSkill;

#[async_trait::async_trait]
impl Skill for ProcessKillSkill {
    async fn run(&self, ctx: SkillContext, input: serde_json::Value) -> Result<SkillResult> {
        let pid = input
            .get("pid")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow!("process_kill 缺少 pid"))?;

        let command = if cfg!(target_os = "windows") {
            format!("taskkill /PID {} /F", pid)
        } else {
            format!("kill -9 {}", pid)
        };

        audit_command("process_kill", &command, true, "kill process");
        run_command_capture(&command, ctx.timeout_secs, ctx.cancellation).await
    }
}
