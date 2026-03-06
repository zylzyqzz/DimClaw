mod browser_automator;
mod browser_click;
mod browser_fill;
mod browser_open;
mod browser_screenshot;
pub mod categories;
mod custom;
mod ffmpeg;
mod file_copy;
mod file_delete;
mod file_list;
mod file_move;
mod file_read;
mod file_write;
mod guard;
mod http_request;
pub mod manager;
pub mod marketplace;
pub mod openclaw_adapter;
mod process_kill;
mod process_list;
mod schedule_task;
mod script_execute;
mod service_control;
mod shell_command;
mod system_monitor;
mod types;
mod whisper;
mod yt_dlp;

use std::collections::HashMap;
use std::sync::Arc;

use crate::configs::load_custom_skill;

pub use types::{Skill, SkillContext, SkillResult};

#[derive(Clone)]
pub struct SkillRegistry {
    skills: HashMap<String, Arc<dyn Skill>>,
}

impl Default for SkillRegistry {
    fn default() -> Self {
        let mut skills: HashMap<String, Arc<dyn Skill>> = HashMap::new();
        skills.insert("shell_command".to_string(), Arc::new(shell_command::ShellCommandSkill));
        skills.insert(
            "browser_automator".to_string(),
            Arc::new(browser_automator::BrowserAutomatorSkill),
        );
        skills.insert("browser_open".to_string(), Arc::new(browser_open::BrowserOpenSkill));
        skills.insert(
            "browser_screenshot".to_string(),
            Arc::new(browser_screenshot::BrowserScreenshotSkill),
        );
        skills.insert("browser_click".to_string(), Arc::new(browser_click::BrowserClickSkill));
        skills.insert("browser_fill".to_string(), Arc::new(browser_fill::BrowserFillSkill));
        skills.insert("script_execute".to_string(), Arc::new(script_execute::ScriptExecuteSkill));
        skills.insert("file_read".to_string(), Arc::new(file_read::FileReadSkill));
        skills.insert("file_write".to_string(), Arc::new(file_write::FileWriteSkill));
        skills.insert("file_delete".to_string(), Arc::new(file_delete::FileDeleteSkill));
        skills.insert("file_list".to_string(), Arc::new(file_list::FileListSkill));
        skills.insert("file_move".to_string(), Arc::new(file_move::FileMoveSkill));
        skills.insert("file_copy".to_string(), Arc::new(file_copy::FileCopySkill));
        skills.insert("http_request".to_string(), Arc::new(http_request::HttpRequestSkill));
        skills.insert("system_monitor".to_string(), Arc::new(system_monitor::SystemMonitorSkill));
        skills.insert("process_kill".to_string(), Arc::new(process_kill::ProcessKillSkill));
        skills.insert("process_list".to_string(), Arc::new(process_list::ProcessListSkill));
        skills.insert("schedule_task".to_string(), Arc::new(schedule_task::ScheduleTaskSkill));
        skills.insert("service_control".to_string(), Arc::new(service_control::ServiceControlSkill));
        skills.insert("yt_dlp".to_string(), Arc::new(yt_dlp::YtDlpSkill));
        skills.insert("ffmpeg".to_string(), Arc::new(ffmpeg::FfmpegSkill));
        skills.insert("whisper".to_string(), Arc::new(whisper::WhisperSkill));
        Self { skills }
    }
}

impl SkillRegistry {
    pub fn get(&self, name: &str) -> Option<Arc<dyn Skill>> {
        if let Some(v) = self.skills.get(name) {
            return Some(v.clone());
        }

        let custom = load_custom_skill(name).ok()?;
        Some(Arc::new(custom::CustomSkill::new(custom)))
    }
}
