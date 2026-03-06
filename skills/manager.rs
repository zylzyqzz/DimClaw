use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::configs::{
    delete_custom_skill, list_custom_skills, load_custom_skill, save_custom_skill, CustomSkillConfig,
};
use crate::skills::openclaw_adapter::parse_openclaw_skill;
use crate::skills::{SkillContext, SkillRegistry, SkillResult};

#[derive(Clone, Debug, Serialize)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub exec_type: String,
    pub builtin: bool,
    pub params_schema: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SkillUpsertRequest {
    pub name: String,
    pub description: String,
    pub exec_type: String,
    #[serde(default)]
    pub params_schema: serde_json::Value,
    #[serde(default)]
    pub command_template: String,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub body_template: String,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    pub timeout_secs: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillExportPayload {
    pub version: u32,
    pub kind: String,
    pub skill: CustomSkillConfig,
}

pub fn builtin_skill_infos() -> Vec<SkillInfo> {
    vec![
        info("shell_command", "执行终端命令", "builtin"),
        info("browser_automator", "打开浏览器并访问 URL", "builtin"),
        info("browser_open", "通过浏览器打开网页", "builtin"),
        info("browser_screenshot", "网页截图（基础实现）", "builtin"),
        info("browser_click", "点击网页元素（占位实现）", "builtin"),
        info("browser_fill", "填写网页表单（占位实现）", "builtin"),
        info("script_execute", "执行多行脚本", "builtin"),
        info("file_read", "读取文件", "builtin"),
        info("file_write", "写入文件", "builtin"),
        info("file_delete", "删除文件或空目录", "builtin"),
        info("file_list", "列出目录内容", "builtin"),
        info("file_move", "移动/重命名文件", "builtin"),
        info("file_copy", "复制文件", "builtin"),
        info("http_request", "执行 HTTP 请求", "builtin"),
        info("system_monitor", "查询系统资源使用", "builtin"),
        info("process_kill", "终止指定进程", "builtin"),
        info("process_list", "列出系统进程", "builtin"),
        info("schedule_task", "创建系统定时任务", "builtin"),
        info("service_control", "控制系统服务", "builtin"),
        info("yt_dlp", "视频下载", "builtin"),
        info("ffmpeg", "视频处理", "builtin"),
        info("whisper", "语音识别", "builtin"),
    ]
}

pub fn list_skill_infos() -> Result<Vec<SkillInfo>> {
    let mut out = builtin_skill_infos();
    for cfg in list_custom_skills()? {
        out.push(SkillInfo {
            name: cfg.name,
            description: cfg.description,
            exec_type: cfg.exec_type,
            builtin: false,
            params_schema: cfg.params_schema,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub fn save_skill_from_request(req: SkillUpsertRequest) -> Result<SkillInfo> {
    if req.name.trim().is_empty() {
        return Err(anyhow!("技能名称不能为空"));
    }

    let exec_type = req.exec_type.trim().to_lowercase();
    if exec_type != "shell" && exec_type != "http" {
        return Err(anyhow!("exec_type 仅支持 shell/http"));
    }

    let cfg = CustomSkillConfig {
        name: req.name.trim().to_string(),
        description: req.description.trim().to_string(),
        exec_type: exec_type.clone(),
        params_schema: req.params_schema,
        command_template: req.command_template,
        method: if req.method.trim().is_empty() {
            "GET".to_string()
        } else {
            req.method.trim().to_uppercase()
        },
        url: req.url,
        body_template: req.body_template,
        headers: req.headers,
        timeout_secs: req.timeout_secs.unwrap_or(20).max(1),
    };

    save_custom_skill(&cfg)?;

    Ok(SkillInfo {
        name: cfg.name,
        description: cfg.description,
        exec_type: cfg.exec_type,
        builtin: false,
        params_schema: cfg.params_schema,
    })
}

pub fn delete_custom(name: &str) -> Result<()> {
    delete_custom_skill(name)
}

pub fn export_custom(name: &str) -> Result<SkillExportPayload> {
    if is_builtin_skill(name) {
        return Err(anyhow!("内置技能不支持导出"));
    }
    let skill = load_custom_skill(name)?;
    Ok(SkillExportPayload {
        version: 1,
        kind: "custom_skill".to_string(),
        skill,
    })
}

pub fn import_custom(payload: Value, overwrite: bool, rename_to: Option<String>) -> Result<SkillInfo> {
    let mut cfg: CustomSkillConfig = if payload.get("skill").is_some() {
        serde_json::from_value(payload.get("skill").cloned().unwrap_or_default())
            .map_err(|e| anyhow!("技能导入格式错误: {}", e))?
    } else {
        serde_json::from_value(payload).map_err(|e| anyhow!("技能导入格式错误: {}", e))?
    };

    if let Some(rename) = rename_to.map(|v| v.trim().to_string()) {
        if !rename.is_empty() {
            cfg.name = rename;
        }
    }

    if cfg.name.trim().is_empty() {
        return Err(anyhow!("技能名称不能为空"));
    }
    if is_builtin_skill(&cfg.name) {
        return Err(anyhow!("技能名称与内置技能冲突: {}", cfg.name));
    }
    if load_custom_skill(&cfg.name).is_ok() && !overwrite {
        return Err(anyhow!("技能已存在，请勾选覆盖或使用重命名"));
    }

    cfg.timeout_secs = cfg.timeout_secs.max(1);
    save_custom_skill(&cfg)?;
    Ok(SkillInfo {
        name: cfg.name,
        description: cfg.description,
        exec_type: cfg.exec_type,
        builtin: false,
        params_schema: cfg.params_schema,
    })
}

pub fn import_openclaw(json: &str, overwrite: bool, rename_to: Option<String>) -> Result<SkillInfo> {
    let mut cfg = parse_openclaw_skill(json)?;
    if let Some(rename) = rename_to {
        let trimmed = rename.trim();
        if !trimmed.is_empty() {
            cfg.name = trimmed.to_string();
        }
    }
    if load_custom_skill(&cfg.name).is_ok() && !overwrite {
        return Err(anyhow!("技能已存在，请勾选覆盖或使用重命名"));
    }
    save_custom_skill(&cfg)?;
    Ok(SkillInfo {
        name: cfg.name,
        description: cfg.description,
        exec_type: cfg.exec_type,
        builtin: false,
        params_schema: cfg.params_schema,
    })
}

pub async fn test_skill(name: &str, input: serde_json::Value, timeout_secs: u64) -> Result<SkillResult> {
    let registry = SkillRegistry::default();
    let skill = registry
        .get(name)
        .ok_or_else(|| anyhow!("技能不存在: {}", name))?;

    let default_timeout = if let Ok(cfg) = load_custom_skill(name) {
        cfg.timeout_secs.max(1)
    } else {
        timeout_secs.max(1)
    };

    let timeout = if timeout_secs > 0 { timeout_secs } else { default_timeout };
    let ctx = SkillContext {
        task_id: "skill-test".to_string(),
        timeout_secs: timeout,
        cancellation: tokio_util::sync::CancellationToken::new(),
    };

    let normalized = normalize_test_input(name, input);
    skill.run(ctx, normalized).await
}

fn normalize_test_input(name: &str, input: Value) -> Value {
    let mut normalized = super::custom::normalize_input_payload(input);
    if name != "shell_command" {
        return normalized;
    }

    if let Some(command) = normalized.as_str().map(|v| v.trim()).filter(|v| !v.is_empty()) {
        return serde_json::json!({ "command": command });
    }

    let Some(obj) = normalized.as_object_mut() else {
        return normalized;
    };

    if let Some(command) = obj
        .get("command")
        .and_then(|v| v.as_str())
        .or_else(|| obj.get("cmd").and_then(|v| v.as_str()))
        .or_else(|| obj.get("script").and_then(|v| v.as_str()))
        .or_else(|| obj.get("value").and_then(|v| v.as_str()))
        .or_else(|| obj.get("msg").and_then(|v| v.as_str()))
    {
        return serde_json::json!({ "command": command });
    }

    Value::Object(obj.clone())
}

fn is_builtin_skill(name: &str) -> bool {
    matches!(
        name,
        "shell_command"
            | "browser_automator"
            | "browser_open"
            | "browser_screenshot"
            | "browser_click"
            | "browser_fill"
            | "script_execute"
            | "file_read"
            | "file_write"
            | "file_delete"
            | "file_list"
            | "file_move"
            | "file_copy"
            | "http_request"
            | "system_monitor"
            | "process_kill"
            | "process_list"
            | "schedule_task"
            | "service_control"
            | "yt_dlp"
            | "ffmpeg"
            | "whisper"
    )
}

fn info(name: &str, description: &str, exec_type: &str) -> SkillInfo {
    SkillInfo {
        name: name.to_string(),
        description: description.to_string(),
        exec_type: exec_type.to_string(),
        builtin: true,
        params_schema: serde_json::json!({"type": "object"}),
    }
}

