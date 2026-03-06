use anyhow::Result;

use crate::skills::guard::audit_command;
use crate::skills::shell_command::run_command_capture;
use crate::skills::{Skill, SkillContext, SkillResult};

pub struct BrowserScreenshotSkill;

#[async_trait::async_trait]
impl Skill for BrowserScreenshotSkill {
    async fn run(&self, ctx: SkillContext, input: serde_json::Value) -> Result<SkillResult> {
        let url = input
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("https://www.baidu.com");
        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("./browser_screenshot.txt");

        let open_cmd = crate::skills::browser_automator::BrowserAutomatorSkill;
        let _ = open_cmd
            .run(
                SkillContext {
                    task_id: ctx.task_id.clone(),
                    timeout_secs: ctx.timeout_secs,
                    cancellation: ctx.cancellation.clone(),
                },
                serde_json::json!({"action":"open","url":url}),
            )
            .await;

        let cmd = if cfg!(target_os = "windows") {
            format!("powershell -NoProfile -Command \"Set-Content -Path '{}' -Value 'screenshot placeholder for {}'\"", path, url)
        } else {
            format!("echo 'screenshot placeholder for {}' > '{}'", url, path)
        };
        audit_command("browser_screenshot", &cmd, true, "create placeholder screenshot");
        run_command_capture(&cmd, ctx.timeout_secs, ctx.cancellation).await
    }
}
