use std::time::Instant;

use anyhow::{anyhow, Result};

use crate::skills::guard::ensure_path_allowed;
use crate::skills::{Skill, SkillContext, SkillResult};

pub struct FileListSkill;

#[async_trait::async_trait]
impl Skill for FileListSkill {
    async fn run(&self, _ctx: SkillContext, input: serde_json::Value) -> Result<SkillResult> {
        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");
        let allowed = ensure_path_allowed(path, "file_list")?;

        if !allowed.is_dir() {
            return Err(anyhow!("file_list 目标不是目录: {}", allowed.display()));
        }

        let start = Instant::now();
        let mut entries = Vec::new();
        for ent in std::fs::read_dir(&allowed)? {
            let ent = ent?;
            let file_type = ent.file_type()?;
            let kind = if file_type.is_dir() {
                "dir"
            } else if file_type.is_file() {
                "file"
            } else {
                "other"
            };
            entries.push(serde_json::json!({
                "name": ent.file_name().to_string_lossy().to_string(),
                "kind": kind
            }));
        }
        entries.sort_by(|a, b| {
            a.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .cmp(b.get("name").and_then(|v| v.as_str()).unwrap_or_default())
        });

        Ok(SkillResult {
            success: true,
            stdout: serde_json::to_string_pretty(&entries).unwrap_or_default(),
            stderr: String::new(),
            exit_code: Some(0),
            duration_ms: start.elapsed().as_millis(),
        })
    }
}

