use std::time::Instant;

use anyhow::{anyhow, Result};

use crate::skills::guard::ensure_path_allowed;
use crate::skills::{Skill, SkillContext, SkillResult};

pub struct FileReadSkill;

#[async_trait::async_trait]
impl Skill for FileReadSkill {
    async fn run(&self, _ctx: SkillContext, input: serde_json::Value) -> Result<SkillResult> {
        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("file_read 缺少 input.path"))?;
        let max_bytes = input
            .get("max_bytes")
            .and_then(|v| v.as_u64())
            .unwrap_or(1024 * 256) as usize;

        let allowed = ensure_path_allowed(path, "file_read")?;
        let start = Instant::now();
        let data = std::fs::read(&allowed)?;
        let out = if data.len() > max_bytes {
            String::from_utf8_lossy(&data[..max_bytes]).to_string()
        } else {
            String::from_utf8_lossy(&data).to_string()
        };

        Ok(SkillResult {
            success: true,
            stdout: out,
            stderr: String::new(),
            exit_code: Some(0),
            duration_ms: start.elapsed().as_millis(),
        })
    }
}

