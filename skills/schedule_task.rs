use anyhow::{anyhow, Result};

use crate::skills::guard::audit_command;
use crate::skills::shell_command::run_command_capture;
use crate::skills::{Skill, SkillContext, SkillResult};

pub struct ScheduleTaskSkill;

#[async_trait::async_trait]
impl Skill for ScheduleTaskSkill {
    async fn run(&self, ctx: SkillContext, input: serde_json::Value) -> Result<SkillResult> {
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("schedule_task 缺少 name"))?
            .trim();
        let command = input
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("schedule_task 缺少 command"))?
            .trim();
        if name.is_empty() || command.is_empty() {
            return Err(anyhow!("name/command 不能为空"));
        }

        let schedule = input
            .get("schedule")
            .and_then(|v| v.as_str())
            .unwrap_or("*/5 * * * *")
            .trim();

        let cmd = if cfg!(target_os = "windows") {
            // schedule format example: MINUTE /MO 5
            let sc = if schedule.is_empty() { "MINUTE /MO 5" } else { schedule };
            format!(
                "schtasks /Create /TN \"{}\" /TR \"{}\" /SC {} /F",
                name,
                command.replace('"', "\\\""),
                sc
            )
        } else {
            let escaped = command.replace('"', "\\\"");
            format!(
                "(crontab -l 2>/dev/null; echo \"{} {} # dimclaw:{}\") | crontab -",
                schedule,
                escaped,
                name
            )
        };

        audit_command("schedule_task", &cmd, true, "create schedule");
        let out = run_command_capture(&cmd, ctx.timeout_secs, ctx.cancellation).await?;
        Ok(SkillResult {
            stdout: if out.stdout.is_empty() {
                format!("schedule created: {}", name)
            } else {
                out.stdout
            },
            ..out
        })
    }
}

