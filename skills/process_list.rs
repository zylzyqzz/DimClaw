use anyhow::Result;

use crate::skills::guard::audit_command;
use crate::skills::shell_command::run_command_capture;
use crate::skills::{Skill, SkillContext, SkillResult};

pub struct ProcessListSkill;

#[async_trait::async_trait]
impl Skill for ProcessListSkill {
    async fn run(&self, ctx: SkillContext, _input: serde_json::Value) -> Result<SkillResult> {
        let command = if cfg!(target_os = "windows") {
            "tasklist".to_string()
        } else {
            "ps -ef".to_string()
        };
        audit_command("process_list", &command, true, "list processes");
        run_command_capture(&command, ctx.timeout_secs, ctx.cancellation).await
    }
}
