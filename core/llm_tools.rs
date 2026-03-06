use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::agents::llm_json::parse_json_with_extract;
use crate::core::logger;
use crate::providers::openai_compatible::OpenAiCompatibleProvider;
use crate::providers::traits::LlmProvider;
use crate::providers::types::ChatRequest;
use crate::skills;

#[derive(Debug, Deserialize)]
struct ToolCallDirective {
    tool: String,
    #[serde(default)]
    args: Value,
}

#[derive(Clone)]
struct ToolExecutionOutcome {
    summary: String,
}

pub async fn run_tool_call_flow(
    provider: &OpenAiCompatibleProvider,
    agent_name: &str,
    model: &str,
    temperature: f32,
    max_tokens: u32,
    message: &str,
    history: &[Value],
) -> Result<Option<String>> {
    if !maybe_tool_intent(message) {
        return Ok(None);
    }

    let tools = skills::manager::list_skill_infos()?;
    if tools.is_empty() {
        return Ok(None);
    }

    let tools_schema = serde_json::json!(
        tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "params_schema": t.params_schema
                })
            })
            .collect::<Vec<_>>()
    );

    let router_prompt = build_router_user_prompt(message, history, &tools_schema);
    let route_req = ChatRequest {
        system_prompt: "你是工具路由器。请严格只输出一个 JSON 对象，不要输出 markdown。格式：{\"tool\":\"技能名或none\",\"args\":{}}".to_string(),
        user_prompt: router_prompt,
        model: model.to_string(),
        temperature: 0.0,
        max_tokens: 320,
    };

    let route_resp = provider.chat(route_req, CancellationToken::new()).await;
    let directive = match route_resp {
        Ok(resp) => parse_json_with_extract::<ToolCallDirective>(&resp.content),
        Err(e) => {
            logger::log(format!("[tool-calling] router model failed err={}", e));
            None
        }
    };

    let outcome = if let Some(directive) = directive {
        let tool_name = directive.tool.trim().to_string();
        if tool_name.is_empty() || tool_name == "none" {
            return Ok(None);
        }
        execute_tool_by_name(tool_name, directive.args).await?
    } else {
        let fallback = detect_fallback_tool(message);
        match fallback {
            Some((tool, args)) => execute_tool_by_name(tool, args).await?,
            None => return Ok(None),
        }
    };

    let mut history_lines = Vec::new();
    for item in history.iter().rev().take(8).rev() {
        let role = item.get("role").and_then(|v| v.as_str()).unwrap_or("user");
        let content = item.get("content").and_then(|v| v.as_str()).unwrap_or("");
        if !content.trim().is_empty() {
            history_lines.push(format!("{}: {}", role, content));
        }
    }

    let final_user_prompt = if history_lines.is_empty() {
        format!(
            "用户消息：{}\n\n工具执行结果：{}\n\n请给出最终回答。",
            message, outcome.summary
        )
    } else {
        format!(
            "历史对话：\n{}\n\n用户消息：{}\n\n工具执行结果：{}\n\n请给出最终回答。",
            history_lines.join("\n"),
            message,
            outcome.summary
        )
    };

    let final_req = ChatRequest {
        system_prompt: format!("你是{}，请用简洁中文回复。", agent_name),
        user_prompt: final_user_prompt,
        model: model.to_string(),
        temperature,
        max_tokens: max_tokens.min(1024),
    };

    let final_reply = match provider.chat(final_req, CancellationToken::new()).await {
        Ok(resp) => format!("{}\n\n{}", outcome.summary, resp.content),
        Err(e) => {
            logger::log(format!("[tool-calling] final model failed err={}", e));
            format!("{}\n\n模型暂时不可用，请检查配置", outcome.summary)
        }
    };

    Ok(Some(final_reply))
}

fn build_router_user_prompt(message: &str, history: &[Value], tools_schema: &Value) -> String {
    let mut history_lines = Vec::new();
    for item in history.iter().rev().take(8).rev() {
        let role = item.get("role").and_then(|v| v.as_str()).unwrap_or("user");
        let content = item.get("content").and_then(|v| v.as_str()).unwrap_or("");
        if !content.trim().is_empty() {
            history_lines.push(format!("{}: {}", role, content));
        }
    }

    if history_lines.is_empty() {
        format!(
            "可用工具列表：{}\n\n用户消息：{}\n\n请判断是否需要调用工具。若无需调用，输出 {{\"tool\":\"none\",\"args\":{{}}}}。",
            tools_schema, message
        )
    } else {
        format!(
            "可用工具列表：{}\n\n历史对话：\n{}\n\n用户消息：{}\n\n请判断是否需要调用工具。若无需调用，输出 {{\"tool\":\"none\",\"args\":{{}}}}。",
            tools_schema,
            history_lines.join("\n"),
            message
        )
    }
}

async fn execute_tool_by_name(name: String, args: Value) -> Result<ToolExecutionOutcome> {
    let result = skills::manager::test_skill(&name, args, 20).await;
    let outcome = match result {
        Ok(v) if v.success => {
            let mut summary = format!("系统技能 `{}` 执行成功。", name);
            let output = v.stdout.trim();
            if !output.is_empty() {
                summary.push_str(&format!("输出：{}", truncate_text(output, 260)));
            }
            ToolExecutionOutcome { summary }
        }
        Ok(v) => {
            let detail = if !v.stderr.trim().is_empty() {
                v.stderr.trim()
            } else {
                v.stdout.trim()
            };
            ToolExecutionOutcome {
                summary: format!(
                    "系统技能 `{}` 执行失败：{}",
                    name,
                    truncate_text(detail, 260)
                ),
            }
        }
        Err(e) => ToolExecutionOutcome {
            summary: format!("系统技能 `{}` 执行异常：{}", name, e),
        },
    };
    Ok(outcome)
}

fn maybe_tool_intent(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    message.contains("/skill")
        || message.contains("/cmd ")
        || message.contains("执行命令")
        || message.contains("创建文件")
        || message.contains("新建文件")
        || message.contains("读取文件")
        || message.contains("查看文件")
        || lower.contains("file")
}

fn detect_fallback_tool(message: &str) -> Option<(String, Value)> {
    let trimmed = message.trim();
    if let Some(rest) = trimmed.strip_prefix("/skill ") {
        let mut parts = rest.trim().splitn(2, ' ');
        let name = parts.next().unwrap_or_default().trim().to_string();
        if name.is_empty() {
            return None;
        }
        let args = parts.next().unwrap_or("{}").trim();
        let parsed = if args.is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str::<Value>(args).unwrap_or_else(|_| serde_json::json!({}))
        };
        return Some((name, parsed));
    }

    if let Some(command) = trimmed
        .strip_prefix("/cmd ")
        .or_else(|| trimmed.strip_prefix("执行命令 "))
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        return Some((
            "shell_command".to_string(),
            serde_json::json!({ "command": command }),
        ));
    }

    let create_file = (trimmed.contains("创建") || trimmed.contains("新建") || trimmed.contains("生成"))
        && (trimmed.contains("文件") || trimmed.to_ascii_lowercase().contains("file"));
    if create_file {
        let path = extract_path_candidate(trimmed).unwrap_or_else(|| "new_file.txt".to_string());
        return Some((
            "file_write".to_string(),
            serde_json::json!({
                "path": path,
                "content": extract_content_candidate(trimmed).unwrap_or_default(),
                "append": false
            }),
        ));
    }

    let read_file = (trimmed.contains("读取") || trimmed.contains("查看"))
        && (trimmed.contains("文件") || trimmed.to_ascii_lowercase().contains("file"));
    if read_file {
        if let Some(path) = extract_path_candidate(trimmed) {
            return Some((
                "file_read".to_string(),
                serde_json::json!({
                    "path": path,
                    "max_bytes": 262144u64
                }),
            ));
        }
    }

    None
}

fn extract_path_candidate(message: &str) -> Option<String> {
    for (left, right) in [("\"", "\""), ("'", "'"), ("“", "”")] {
        if let Some(start) = message.find(left) {
            let remain = &message[start + left.len()..];
            if let Some(end) = remain.find(right) {
                let piece = remain[..end].trim();
                if !piece.is_empty() {
                    return Some(piece.to_string());
                }
            }
        }
    }

    for token in message.split_whitespace() {
        let cleaned = token.trim_matches(|c: char| {
            c == ',' || c == '.' || c == '，' || c == '。' || c == ':' || c == '：' || c == ';'
        });
        let cleaned = cleaned
            .trim_start_matches("叫")
            .trim_start_matches("名为")
            .trim_start_matches("为");
        if cleaned.is_empty() {
            continue;
        }
        if cleaned.contains('/') || cleaned.contains('\\') || cleaned.contains('.') {
            return Some(cleaned.to_string());
        }
    }
    None
}

fn extract_content_candidate(message: &str) -> Option<String> {
    for marker in ["内容为", "内容是", "内容:", "内容："] {
        if let Some(pos) = message.find(marker) {
            let mut part = message[pos + marker.len()..].trim().to_string();
            part = part
                .trim_matches('"')
                .trim_matches('\'')
                .trim_matches('“')
                .trim_matches('”')
                .to_string();
            if !part.is_empty() {
                return Some(part);
            }
        }
    }
    None
}

fn truncate_text(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let mut out = String::new();
    for ch in input.chars().take(max_chars) {
        out.push(ch);
    }
    out.push_str("...");
    out
}
