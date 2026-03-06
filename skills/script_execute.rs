use anyhow::{anyhow, Result};

use crate::skills::guard::audit_command;
use crate::skills::shell_command::run_command_capture;
use crate::skills::{Skill, SkillContext, SkillResult};

pub struct ScriptExecuteSkill;

#[async_trait::async_trait]
impl Skill for ScriptExecuteSkill {
    async fn run(&self, ctx: SkillContext, input: serde_json::Value) -> Result<SkillResult> {
        let script = input
            .get("script")
            .or_else(|| input.get("content"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("script_execute 缺少 script"))?
            .trim()
            .to_string();
        if script.is_empty() {
            return Err(anyhow!("script 不能为空"));
        }

        let shell = input
            .get("shell")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        let command = if cfg!(target_os = "windows") {
            if shell.eq_ignore_ascii_case("cmd") {
                format!("cmd /C \"{}\"", script.replace('"', "\\\""))
            } else {
                format!(
                    "powershell -NoProfile -ExecutionPolicy Bypass -Command \"{}\"",
                    script.replace('"', "\\\"")
                )
            }
        } else if shell.eq_ignore_ascii_case("bash") || shell.is_empty() {
            format!("bash -lc '{}'", script.replace('\'', "'\"'\"'"))
        } else {
            format!("{} -lc '{}'", shell, script.replace('\'', "'\"'\"'"))
        };

        audit_command("script_execute", &command, true, "execute script");
        run_command_capture(&command, ctx.timeout_secs, ctx.cancellation).await
    }
}

