use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde::Serialize;

pub mod browser_hand;
pub mod clip;
pub mod collector;
pub mod lead;
pub mod monitor;
pub mod researcher;
pub mod scheduler_hand;

#[derive(Debug, Clone)]
pub struct ScheduleConfig {
    pub cron: Option<String>,
    pub interval: Option<u64>,
    pub condition: Option<Condition>,
    pub timezone: String,
}

#[derive(Debug, Clone)]
pub enum Condition {
    FileExists(PathBuf),
    ProcessRunning(String),
    CpuAbove(f32),
    MemoryAbove(f64),
    Custom(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct HandResult {
    pub success: bool,
    pub output: String,
    pub metrics: HashMap<String, f64>,
    pub artifacts: Vec<PathBuf>,
}

impl HandResult {
    pub fn ok(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: output.into(),
            metrics: HashMap::new(),
            artifacts: Vec::new(),
        }
    }

    pub fn fail(output: impl Into<String>) -> Self {
        Self {
            success: false,
            output: output.into(),
            metrics: HashMap::new(),
            artifacts: Vec::new(),
        }
    }
}

#[async_trait]
pub trait Hand: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schedule(&self) -> ScheduleConfig;
    async fn execute(&self) -> Result<HandResult>;
    fn on_success(&self) -> Option<String> {
        None
    }
    fn on_failure(&self) -> Option<String> {
        None
    }
    fn required_tools(&self) -> Vec<String> {
        Vec::new()
    }
    fn knowledge_base(&self) -> Option<PathBuf> {
        None
    }
}

pub fn builtin_hands() -> Vec<Arc<dyn Hand>> {
    vec![
        Arc::new(lead::LeadHand::default()),
        Arc::new(clip::ClipHand::default()),
        Arc::new(researcher::ResearcherHand::default()),
        Arc::new(collector::CollectorHand::default()),
        Arc::new(browser_hand::BrowserHand::default()),
        Arc::new(monitor::MonitorHand::default()),
        Arc::new(scheduler_hand::SchedulerHand::default()),
    ]
}
