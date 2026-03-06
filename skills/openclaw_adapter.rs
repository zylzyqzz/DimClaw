use anyhow::{anyhow, Result};
use serde::Deserialize;

use crate::configs::CustomSkillConfig;

#[derive(Debug, Deserialize)]
struct OpenClawSkill {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    command: String,
    #[serde(default)]
    http: Option<OpenClawHttp>,
    #[serde(default)]
    params: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct OpenClawHttp {
    #[serde(default)]
    method: String,
    url: String,
    #[serde(default)]
    body: String,
}

pub fn parse_openclaw_skill(json: &str) -> Result<CustomSkillConfig> {
    let parsed: OpenClawSkill = serde_json::from_str(json).map_err(|e| anyhow!("OpenClaw JSON 解析失败: {e}"))?;
    if parsed.name.trim().is_empty() {
        return Err(anyhow!("OpenClaw 技能 name 不能为空"));
    }

    if !parsed.command.trim().is_empty() {
        return Ok(CustomSkillConfig {
            name: parsed.name,
            description: if parsed.description.trim().is_empty() {
                "Imported from OpenClaw command skill".to_string()
            } else {
                parsed.description
            },
            exec_type: "shell".to_string(),
            params_schema: if parsed.params.is_null() {
                serde_json::json!({"type":"object"})
            } else {
                parsed.params
            },
            command_template: parsed.command,
            method: "GET".to_string(),
            url: String::new(),
            body_template: String::new(),
            headers: std::collections::HashMap::new(),
            timeout_secs: 20,
        });
    }

    if let Some(http) = parsed.http {
        if http.url.trim().is_empty() {
            return Err(anyhow!("OpenClaw http.url 不能为空"));
        }
        return Ok(CustomSkillConfig {
            name: parsed.name,
            description: if parsed.description.trim().is_empty() {
                "Imported from OpenClaw http skill".to_string()
            } else {
                parsed.description
            },
            exec_type: "http".to_string(),
            params_schema: if parsed.params.is_null() {
                serde_json::json!({"type":"object"})
            } else {
                parsed.params
            },
            command_template: String::new(),
            method: if http.method.trim().is_empty() {
                "GET".to_string()
            } else {
                http.method.to_uppercase()
            },
            url: http.url,
            body_template: http.body,
            headers: std::collections::HashMap::new(),
            timeout_secs: 20,
        });
    }

    Err(anyhow!("仅支持可映射到 command/http 的 OpenClaw 技能"))
}

