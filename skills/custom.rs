use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::configs::CustomSkillConfig;
use crate::skills::{Skill, SkillContext, SkillResult};

pub struct CustomSkill {
    cfg: CustomSkillConfig,
}

impl CustomSkill {
    pub fn new(cfg: CustomSkillConfig) -> Self {
        Self { cfg }
    }
}

#[async_trait::async_trait]
impl Skill for CustomSkill {
    async fn run(&self, ctx: SkillContext, input: Value) -> Result<SkillResult> {
        let normalized_input = normalize_input_payload(input);
        match self.cfg.exec_type.as_str() {
            "shell" => {
                let tpl = if self.cfg.command_template.trim().is_empty() {
                    return Err(anyhow!("shell 技能 command_template 为空"));
                } else {
                    self.cfg.command_template.as_str()
                };
                let command = render_template(tpl, &normalized_input);
                if command.trim().is_empty() {
                    return Err(anyhow!("shell 技能生成命令为空"));
                }
                let shell = crate::skills::shell_command::ShellCommandSkill;
                shell
                    .run(
                        ctx,
                        serde_json::json!({
                            "command": command
                        }),
                    )
                    .await
            }
            "http" => {
                if self.cfg.url.trim().is_empty() {
                    return Err(anyhow!("http 技能 url 为空"));
                }
                let url = render_template(&self.cfg.url, &normalized_input);
                let body = if self.cfg.body_template.trim().is_empty() {
                    None
                } else {
                    Some(render_template(&self.cfg.body_template, &normalized_input))
                };
                let mut headers = serde_json::Map::new();
                for (k, v) in &self.cfg.headers {
                    headers.insert(k.clone(), Value::String(render_template(v, &normalized_input)));
                }
                let http = crate::skills::http_request::HttpRequestSkill;
                http
                    .run(
                        ctx,
                        serde_json::json!({
                            "url": url,
                            "method": if self.cfg.method.trim().is_empty() { "GET" } else { self.cfg.method.as_str() },
                            "body": body,
                            "headers": headers,
                        }),
                    )
                    .await
            }
            other => Err(anyhow!("不支持的自定义技能类型: {}", other)),
        }
    }
}

pub(crate) fn normalize_input_payload(input: Value) -> Value {
    if let Some(inner) = input.get("input").and_then(|v| v.as_object()) {
        return Value::Object(inner.clone());
    }
    input
}

fn render_template(template: &str, input: &Value) -> String {
    let mut out = template.to_string();
    if let Some(map) = input.as_object() {
        for (k, v) in map {
            let key = format!("{{{{{}}}}}", k);
            let value = if let Some(s) = v.as_str() {
                s.to_string()
            } else {
                v.to_string()
            };
            out = out.replace(&key, &value);
        }
    }
    out
}
