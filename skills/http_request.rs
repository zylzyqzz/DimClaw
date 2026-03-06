use std::time::Instant;

use anyhow::{anyhow, Result};
use tokio::time::{timeout, Duration};

use crate::skills::{Skill, SkillContext, SkillResult};

pub struct HttpRequestSkill;

#[async_trait::async_trait]
impl Skill for HttpRequestSkill {
    async fn run(&self, ctx: SkillContext, input: serde_json::Value) -> Result<SkillResult> {
        let url = input
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("http_request 缺少 input.url"))?;
        let method = input
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("GET")
            .to_uppercase();

        let body_text = input
            .get("body")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string());

        let headers = input
            .get("headers")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();

        let start = Instant::now();
        let client = reqwest::Client::new();
        let mut req = match method.as_str() {
            "GET" => client.get(url),
            "POST" => client.post(url),
            "PUT" => client.put(url),
            "DELETE" => client.delete(url),
            "PATCH" => client.patch(url),
            _ => return Err(anyhow!("http_request method 不支持: {}", method)),
        };

        for (k, v) in headers {
            if let Some(s) = v.as_str() {
                req = req.header(k, s);
            }
        }

        if let Some(body) = body_text {
            req = req.body(body);
        }

        let fut = req.send();
        let resp = tokio::select! {
            _ = ctx.cancellation.cancelled() => {
                return Err(anyhow!("http_request 被取消"));
            }
            res = timeout(Duration::from_secs(ctx.timeout_secs), fut) => {
                match res {
                    Ok(v) => v?,
                    Err(_) => return Err(anyhow!("http_request 超时")),
                }
            }
        };

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        let ok = status.is_success();

        Ok(SkillResult {
            success: ok,
            stdout: if ok { text.clone() } else { String::new() },
            stderr: if ok {
                String::new()
            } else {
                format!("HTTP {} {}", status, text)
            },
            exit_code: Some(status.as_u16() as i32),
            duration_ms: start.elapsed().as_millis(),
        })
    }
}
