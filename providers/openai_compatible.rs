use std::time::Duration;

use async_trait::async_trait;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use crate::core::logger;
use crate::providers::traits::LlmProvider;
use crate::providers::types::{ChatRequest, ChatResponse, ProviderError, Usage};

#[derive(Clone)]
pub struct OpenAiCompatibleProvider {
    pub provider_name: String,
    pub base_url: String,
    pub api_key: String,
    pub timeout_secs: u64,
    pub client: reqwest::Client,
    pub max_retries: u32,
}

impl OpenAiCompatibleProvider {
    pub fn new(
        provider_name: String,
        base_url: String,
        api_key: String,
        timeout_secs: u64,
        max_retries: u32,
    ) -> Result<Self, ProviderError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .map_err(|e| ProviderError::Config(format!("创建 HTTP 客户端失败: {e}")))?;
        Ok(Self {
            provider_name,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            timeout_secs,
            client,
            max_retries,
        })
    }
}

#[derive(Debug, Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    choices: Vec<Choice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: OpenAiMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
    total_tokens: Option<u32>,
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleProvider {
    async fn chat(
        &self,
        request: ChatRequest,
        cancellation: CancellationToken,
    ) -> Result<ChatResponse, ProviderError> {
        let url = format!("{}/chat/completions", self.base_url);
        let body = OpenAiRequest {
            model: request.model.clone(),
            messages: vec![
                OpenAiMessage {
                    role: "system".to_string(),
                    content: request.system_prompt,
                },
                OpenAiMessage {
                    role: "user".to_string(),
                    content: request.user_prompt,
                },
            ],
            temperature: request.temperature,
            max_tokens: request.max_tokens,
        };

        logger::log(format!(
            "[Provider] 开始请求 provider={} model={} timeout={}s",
            self.provider_name, request.model, self.timeout_secs
        ));

        let mut attempt = 0u32;
        loop {
            if cancellation.is_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            attempt += 1;
            let req = self
                .client
                .post(&url)
                .bearer_auth(&self.api_key)
                .json(&body);

            let resp = tokio::select! {
                _ = cancellation.cancelled() => return Err(ProviderError::Cancelled),
                out = req.send() => out,
            };

            match resp {
                Ok(r) => {
                    if !r.status().is_success() {
                        let status = r.status();
                        let text = r.text().await.unwrap_or_default();
                        let err = ProviderError::Http(format!("HTTP {}: {}", status, text));
                        if attempt <= self.max_retries && is_retryable(status) {
                            let delay = backoff_ms(attempt);
                            logger::log(format!(
                                "[Provider] 请求失败，准备重试 provider={} model={} attempt={}/{} delay={}ms",
                                self.provider_name,
                                request.model,
                                attempt,
                                self.max_retries + 1,
                                delay
                            ));
                            sleep(Duration::from_millis(delay)).await;
                            continue;
                        }
                        logger::log(format!(
                            "[Provider] 请求失败 provider={} model={} attempt={} err={}",
                            self.provider_name, request.model, attempt, err
                        ));
                        return Err(err);
                    }

                    let raw = r
                        .text()
                        .await
                        .map_err(|e| ProviderError::Parse(format!("读取响应失败: {e}")))?;
                    let parsed: OpenAiResponse = serde_json::from_str(&raw)
                        .map_err(|e| ProviderError::Parse(format!("解析响应 JSON 失败: {e}")))?;
                    let content = parsed
                        .choices
                        .first()
                        .map(|v| v.message.content.clone())
                        .ok_or_else(|| ProviderError::InvalidResponse("缺少 choices[0]".to_string()))?;
                    logger::log(format!(
                        "[Provider] 请求成功 provider={} model={} attempt={}",
                        self.provider_name, request.model, attempt
                    ));
                    return Ok(ChatResponse {
                        content: content.clone(),
                        raw_text: raw,
                        usage: parsed.usage.map(|u| Usage {
                            prompt_tokens: u.prompt_tokens,
                            completion_tokens: u.completion_tokens,
                            total_tokens: u.total_tokens,
                        }),
                        provider_name: self.provider_name.clone(),
                        model: request.model,
                    });
                }
                Err(e) => {
                    let err = if e.is_timeout() {
                        ProviderError::Timeout(e.to_string())
                    } else {
                        ProviderError::Http(e.to_string())
                    };
                    if attempt <= self.max_retries {
                        let delay = backoff_ms(attempt);
                        logger::log(format!(
                            "[Provider] 请求异常，准备重试 provider={} model={} attempt={}/{} delay={}ms err={}",
                            self.provider_name,
                            request.model,
                            attempt,
                            self.max_retries + 1,
                            delay,
                            err
                        ));
                        sleep(Duration::from_millis(delay)).await;
                        continue;
                    }
                    logger::log(format!(
                        "[Provider] 请求失败 provider={} model={} attempt={} err={}",
                        self.provider_name, request.model, attempt, err
                    ));
                    return Err(err);
                }
            }
        }
    }
}

fn is_retryable(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::REQUEST_TIMEOUT
            | StatusCode::TOO_MANY_REQUESTS
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    ) || status.is_server_error()
}

fn backoff_ms(attempt: u32) -> u64 {
    let base = 300u64;
    let pow = 1u64 << attempt.min(5);
    base * pow
}

pub struct OpenAiProviderAdapter {
    inner: OpenAiCompatibleProvider,
    model: String,
}

impl OpenAiProviderAdapter {
    pub fn new(inner: OpenAiCompatibleProvider, model: String) -> Self {
        Self { inner, model }
    }
}

#[async_trait]
impl crate::core::traits::provider::Provider for OpenAiProviderAdapter {
    fn name(&self) -> &str {
        &self.inner.provider_name
    }

    async fn chat(
        &self,
        req: crate::core::traits::provider::ChatRequest,
    ) -> anyhow::Result<crate::core::traits::provider::ChatResponse> {
        let user_prompt = req
            .messages
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        let out = self
            .inner
            .chat(
                crate::providers::types::ChatRequest {
                    system_prompt: "You are an assistant.".to_string(),
                    user_prompt,
                    model: self.model.clone(),
                    temperature: req.temperature,
                    max_tokens: req.max_tokens,
                },
                CancellationToken::new(),
            )
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        Ok(crate::core::traits::provider::ChatResponse {
            content: out.content,
            tool_calls: vec![],
            usage: crate::core::traits::provider::TokenUsage {
                prompt_tokens: out.usage.as_ref().and_then(|u| u.prompt_tokens).unwrap_or(0),
                completion_tokens: out
                    .usage
                    .as_ref()
                    .and_then(|u| u.completion_tokens)
                    .unwrap_or(0),
                total_tokens: out.usage.as_ref().and_then(|u| u.total_tokens).unwrap_or(0),
            },
        })
    }

    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let mut out = Vec::new();
        let bytes = text.as_bytes();
        for i in 0..64 {
            out.push(*bytes.get(i % bytes.len().max(1)).unwrap_or(&0) as f32 / 255.0);
        }
        Ok(out)
    }

    fn models(&self) -> Vec<crate::core::traits::provider::ModelInfo> {
        vec![crate::core::traits::provider::ModelInfo {
            id: self.model.clone(),
            display_name: self.model.clone(),
        }]
    }
}
