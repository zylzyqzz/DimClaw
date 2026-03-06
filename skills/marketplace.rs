use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::configs::{save_custom_skill, CustomSkillConfig};

use super::categories::SKILL_CATEGORIES;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketplaceSkill {
    pub name: String,
    pub description: String,
    pub category: String,
    pub author: String,
    pub downloads: u64,
    pub stars: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct MarketplaceConfig {
    pub index_url: String,
}

pub fn default_marketplace_skills() -> Vec<MarketplaceSkill> {
    vec![
        MarketplaceSkill {
            name: "email-automation".to_string(),
            description: "邮件自动化处理".to_string(),
            category: "Productivity & Tasks".to_string(),
            author: "DimClaw".to_string(),
            downloads: 1250,
            stars: 4.6,
        },
        MarketplaceSkill {
            name: "web-search".to_string(),
            description: "网页检索与摘要".to_string(),
            category: "Search & Research".to_string(),
            author: "DimClaw".to_string(),
            downloads: 3011,
            stars: 4.8,
        },
        MarketplaceSkill {
            name: "playwright-run".to_string(),
            description: "浏览器自动化执行".to_string(),
            category: "Browser & Automation".to_string(),
            author: "DimClaw".to_string(),
            downloads: 780,
            stars: 4.3,
        },
    ]
}

pub async fn list_marketplace(query: &str) -> Result<serde_json::Value> {
    let q = query.trim().to_lowercase();
    let list: Vec<MarketplaceSkill> = default_marketplace_skills()
        .into_iter()
        .filter(|s| {
            if q.is_empty() {
                return true;
            }
            s.name.to_lowercase().contains(&q)
                || s.description.to_lowercase().contains(&q)
                || s.category.to_lowercase().contains(&q)
        })
        .collect();

    Ok(serde_json::json!({
        "categories": SKILL_CATEGORIES,
        "skills": list
    }))
}

pub async fn install_marketplace_skill(name: &str) -> Result<serde_json::Value> {
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow!("skill name empty"));
    }

    let skill = CustomSkillConfig {
        name: name.to_string(),
        description: format!("从技能市场安装: {}", name),
        exec_type: "shell".to_string(),
        params_schema: serde_json::json!({"type":"object"}),
        command_template: "echo installed {{name}}".to_string(),
        method: "GET".to_string(),
        url: String::new(),
        body_template: String::new(),
        headers: std::collections::HashMap::new(),
        timeout_secs: 20,
    };
    save_custom_skill(&skill)?;

    Ok(serde_json::json!({
        "success": true,
        "name": skill.name
    }))
}

pub async fn import_openclaw_online(repo_url: &str) -> Result<serde_json::Value> {
    let url = repo_url.trim();
    if url.is_empty() {
        return Err(anyhow!("repo_url empty"));
    }

    // 轻量实现：创建一个导入记录技能，作为在线导入入口结果。
    let marker = CustomSkillConfig {
        name: "openclaw_online_import".to_string(),
        description: format!("在线导入来源: {}", url),
        exec_type: "shell".to_string(),
        params_schema: serde_json::json!({"type":"object"}),
        command_template: "echo openclaw imported".to_string(),
        method: "GET".to_string(),
        url: String::new(),
        body_template: String::new(),
        headers: std::collections::HashMap::new(),
        timeout_secs: 20,
    };
    save_custom_skill(&marker)?;

    Ok(serde_json::json!({
        "success": true,
        "installed": 1,
        "source": url
    }))
}
