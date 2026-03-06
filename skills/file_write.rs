use std::io::Write;
use std::time::Instant;

use anyhow::{anyhow, Result};

use crate::skills::guard::ensure_path_allowed;
use crate::skills::{Skill, SkillContext, SkillResult};

pub struct FileWriteSkill;

#[async_trait::async_trait]
impl Skill for FileWriteSkill {
    async fn run(&self, _ctx: SkillContext, input: serde_json::Value) -> Result<SkillResult> {
        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("file_write 缺少 input.path"))?;
        let content = input
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("file_write 缺少 input.content"))?;
        let mode = input
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| {
                if input.get("append").and_then(|v| v.as_bool()) == Some(true) {
                    "append"
                } else {
                    "overwrite"
                }
            });

        let allowed = ensure_path_allowed(path, "file_write")?;
        let start = Instant::now();

        if let Some(parent) = allowed.parent() {
            std::fs::create_dir_all(parent)?;
        }

        match mode {
            "create" => {
                if allowed.exists() {
                    return Err(anyhow!("file_write create 模式下文件已存在"));
                }
                let mut file = std::fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&allowed)?;
                file.write_all(content.as_bytes())?;
            }
            "append" => {
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&allowed)?;
                file.write_all(content.as_bytes())?;
            }
            "overwrite" => {
                std::fs::write(&allowed, content)?;
            }
            _ => return Err(anyhow!("file_write mode 仅支持 create/overwrite/append")),
        }

        Ok(SkillResult {
            success: true,
            stdout: format!("write ok: {}", allowed.display()),
            stderr: String::new(),
            exit_code: Some(0),
            duration_ms: start.elapsed().as_millis(),
        })
    }
}

