use std::time::Instant;

use anyhow::{anyhow, Result};
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use crate::core::logger;
use crate::skills::{Skill, SkillContext, SkillResult};

pub struct ShellCommandSkill;

#[async_trait::async_trait]
impl Skill for ShellCommandSkill {
    async fn run(&self, ctx: SkillContext, input: serde_json::Value) -> Result<SkillResult> {
        let command = input
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("shell_command 缺少 input.command"))?
            .to_string();
        let start = Instant::now();
        logger::log(format!(
            "[Skill:shell_command] task={} cmd=\"{}\" timeout={}s",
            ctx.task_id, command, ctx.timeout_secs
        ));

        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(command.clone());
            c
        } else {
            let mut c = Command::new("sh");
            c.arg("-lc").arg(command.clone());
            c
        };
        let child = cmd
            .kill_on_drop(true)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let mut wait = tokio::spawn(async move { child.wait_with_output().await });
        let result = tokio::select! {
            _ = ctx.cancellation.cancelled() => {
                wait.abort();
                return Err(anyhow!("shell_command 被取消"));
            }
            r = timeout(Duration::from_secs(ctx.timeout_secs), &mut wait) => r
        };

        let output = match result {
            Ok(Ok(Ok(out))) => out,
            Ok(Ok(Err(e))) => return Err(anyhow!("shell_command 等待失败: {e}")),
            Ok(Err(e)) => return Err(anyhow!("shell_command 执行任务中断: {e}")),
            Err(_) => {
                wait.abort();
                return Err(anyhow!("shell_command 超时"));
            }
        };

        let duration_ms = start.elapsed().as_millis();
        let code = output.status.code();
        logger::log(format!(
            "[Skill:shell_command] task={} code={:?} duration={}ms",
            ctx.task_id, code, duration_ms
        ));
        Ok(SkillResult {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            exit_code: code,
            duration_ms,
        })
    }
}
