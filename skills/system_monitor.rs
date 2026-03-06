use anyhow::Result;

use crate::skills::guard::audit_command;
use crate::skills::shell_command::run_command_capture;
use crate::skills::{Skill, SkillContext, SkillResult};

pub struct SystemMonitorSkill;

#[async_trait::async_trait]
impl Skill for SystemMonitorSkill {
    async fn run(&self, ctx: SkillContext, _input: serde_json::Value) -> Result<SkillResult> {
        let command = if cfg!(target_os = "windows") {
            "powershell -NoProfile -Command \"Get-Counter '\\Processor(_Total)\\% Processor Time' | Select-Object -ExpandProperty CounterSamples | Select-Object CookedValue; Get-CimInstance Win32_OperatingSystem | Select-Object TotalVisibleMemorySize,FreePhysicalMemory; Get-PSDrive -PSProvider FileSystem | Select-Object Name,Used,Free; Get-Process | Sort-Object CPU -Descending | Select-Object -First 20 Name,Id,CPU,WS\"".to_string()
        } else {
            "top -bn1 | head -n 5; free -m; df -h; ps -eo pid,comm,%cpu,%mem --sort=-%cpu | head -n 20"
                .to_string()
        };

        audit_command("system_monitor", &command, true, "collect system snapshot");
        run_command_capture(&command, ctx.timeout_secs, ctx.cancellation).await
    }
}
