use std::time::Instant;

use anyhow::{anyhow, Result};

use crate::skills::guard::ensure_path_allowed;
use crate::skills::{Skill, SkillContext, SkillResult};

pub struct FileDeleteSkill;

#[async_trait::async_trait]
impl Skill for FileDeleteSkill {
    async fn run(&self, _ctx: SkillContext, input: serde_json::Value) -> Result<SkillResult> {
        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("file_delete 缺少 input.path"))?;
        let confirm = input
            .get("confirm")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !confirm {
            return Err(anyhow!("file_delete 需要 confirm=true"));
        }

        let allowed = ensure_path_allowed(path, "file_delete")?;
        let start = Instant::now();
        if allowed.is_dir() {
            std::fs::remove_dir(&allowed)?;
        } else if allowed.is_file() {
            std::fs::remove_file(&allowed)?;
        } else {
            return Err(anyhow!("目标不存在: {}", allowed.display()));
        }

        Ok(SkillResult {
            success: true,
            stdout: format!("delete ok: {}", allowed.display()),
            stderr: String::new(),
            exit_code: Some(0),
            duration_ms: start.elapsed().as_millis(),
        })
    }
}

