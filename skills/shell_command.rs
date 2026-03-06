use std::time::Instant;

use anyhow::{anyhow, Result};
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use crate::core::logger;
use crate::skills::guard::audit_command;
use crate::skills::{Skill, SkillContext, SkillResult};

pub struct ShellCommandSkill;

#[async_trait::async_trait]
impl Skill for ShellCommandSkill {
    async fn run(&self, ctx: SkillContext, input: serde_json::Value) -> Result<SkillResult> {
        let command = input
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("shell_command 缺少 input.command"))?
            .trim()
            .to_string();
        if command.is_empty() {
            return Err(anyhow!("shell_command command 不能为空"));
        }

        audit_command("shell_command", &command, true, "execute");
        logger::log(format!(
            "[技能:shell_command] task={} cmd=\"{}\" timeout={}s",
            ctx.task_id, command, ctx.timeout_secs
        ));

        run_command_capture(&command, ctx.timeout_secs, ctx.cancellation).await
    }
}

pub async fn run_command_capture(
    command: &str,
    timeout_secs: u64,
    cancellation: tokio_util::sync::CancellationToken,
) -> Result<SkillResult> {
    let start = Instant::now();

    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(command);
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-lc").arg(command);
        c
    };

    let child = cmd
        .kill_on_drop(true)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let mut wait = tokio::spawn(async move { child.wait_with_output().await });
    let result = tokio::select! {
        _ = cancellation.cancelled() => {
            wait.abort();
            return Err(anyhow!("命令执行被取消"));
        }
        r = timeout(Duration::from_secs(timeout_secs.max(1)), &mut wait) => r
    };

    let output = match result {
        Ok(Ok(Ok(out))) => out,
        Ok(Ok(Err(e))) => return Err(anyhow!("命令执行失败: {e}")),
        Ok(Err(e)) => return Err(anyhow!("命令执行任务中断: {e}")),
        Err(_) => {
            wait.abort();
            return Err(anyhow!("命令执行超时"));
        }
    };

    let duration_ms = start.elapsed().as_millis();
    let code = output.status.code();
    logger::log(format!(
        "[技能:shell_command] code={:?} duration={}ms",
        code, duration_ms
    ));

    Ok(SkillResult {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        exit_code: code,
        duration_ms,
    })
}

