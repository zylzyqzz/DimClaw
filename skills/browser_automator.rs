use anyhow::{anyhow, Result};

use crate::skills::guard::audit_command;
use crate::skills::shell_command::run_command_capture;
use crate::skills::{Skill, SkillContext, SkillResult};

pub struct BrowserAutomatorSkill;

#[async_trait::async_trait]
impl Skill for BrowserAutomatorSkill {
    async fn run(&self, ctx: SkillContext, input: serde_json::Value) -> Result<SkillResult> {
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("open")
            .to_lowercase();

        if action != "open" {
            return Err(anyhow!("browser_automator 目前仅支持 action=open"));
        }

        let url = input
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("browser_automator 缺少 url"))?
            .trim()
            .to_string();
        if url.is_empty() {
            return Err(anyhow!("url 不能为空"));
        }

        let browser = input
            .get("browser")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_lowercase();

        let cmd = build_open_command(&url, &browser);
        audit_command("browser_automator", &cmd, true, "open url");
        run_command_capture(&cmd, ctx.timeout_secs, ctx.cancellation).await
    }
}

fn build_open_command(url: &str, browser: &str) -> String {
    if cfg!(target_os = "windows") {
        if browser.is_empty() {
            return format!("start \"\" \"{}\"", url);
        }
        return format!("start \"\" {} \"{}\"", browser, url);
    }

    if cfg!(target_os = "macos") {
        if browser.is_empty() {
            return format!("open \"{}\"", url);
        }
        return format!("open -a \"{}\" \"{}\"", browser, url);
    }

    if browser.is_empty() {
        format!("xdg-open \"{}\"", url)
    } else {
        format!("{} \"{}\"", browser, url)
    }
}
