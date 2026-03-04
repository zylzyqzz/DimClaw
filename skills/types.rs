use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct SkillContext {
    pub task_id: String,
    pub timeout_secs: u64,
    pub cancellation: CancellationToken,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u128,
}

#[async_trait]
pub trait Skill: Send + Sync {
    async fn run(&self, ctx: SkillContext, input: Value) -> Result<SkillResult>;
}
