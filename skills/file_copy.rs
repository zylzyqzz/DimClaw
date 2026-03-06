use std::time::Instant;

use anyhow::{anyhow, Result};

use crate::skills::guard::ensure_path_allowed;
use crate::skills::{Skill, SkillContext, SkillResult};

pub struct FileCopySkill;

#[async_trait::async_trait]
impl Skill for FileCopySkill {
    async fn run(&self, _ctx: SkillContext, input: serde_json::Value) -> Result<SkillResult> {
        let from = input
            .get("from")
            .or_else(|| input.get("src"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("file_copy 缺少 from"))?;
        let to = input
            .get("to")
            .or_else(|| input.get("dst"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("file_copy 缺少 to"))?;

        let from_path = ensure_path_allowed(from, "file_copy")?;
        let to_path = ensure_path_allowed(to, "file_copy")?;
        let start = Instant::now();

        if let Some(parent) = to_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&from_path, &to_path)?;

        Ok(SkillResult {
            success: true,
            stdout: format!("copy ok: {} -> {}", from_path.display(), to_path.display()),
            stderr: String::new(),
            exit_code: Some(0),
            duration_ms: start.elapsed().as_millis(),
        })
    }
}

