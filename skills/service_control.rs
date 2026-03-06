use anyhow::{anyhow, Result};

use crate::skills::guard::audit_command;
use crate::skills::shell_command::run_command_capture;
use crate::skills::{Skill, SkillContext, SkillResult};

pub struct ServiceControlSkill;

#[async_trait::async_trait]
impl Skill for ServiceControlSkill {
    async fn run(&self, ctx: SkillContext, input: serde_json::Value) -> Result<SkillResult> {
        let service = input
            .get("service")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("service_control 缺少 service"))?
            .trim();
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("status")
            .trim()
            .to_lowercase();

        if service.is_empty() {
            return Err(anyhow!("service 不能为空"));
        }

        let cmd = if cfg!(target_os = "windows") {
            match action.as_str() {
                "start" => format!("sc start \"{}\"", service),
                "stop" => format!("sc stop \"{}\"", service),
                "restart" => format!("sc stop \"{0}\" && sc start \"{0}\"", service),
                _ => format!("sc query \"{}\"", service),
            }
        } else {
            match action.as_str() {
                "start" | "stop" | "restart" | "status" => {
                    format!("systemctl {} {}", action, service)
                }
                _ => format!("systemctl status {}", service),
            }
        };

        audit_command("service_control", &cmd, true, "service action");
        run_command_capture(&cmd, ctx.timeout_secs, ctx.cancellation).await
    }
}

