use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub id: u32,
    pub action: String,
    pub tool: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannerOutput {
    pub goal: String,
    pub steps: Vec<PlanStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorOutput {
    pub decision: String,
    pub tool: String,
    pub args: serde_json::Value,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifierOutput {
    pub verdict: String,
    pub reason: String,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryOutput {
    pub action: String,
    pub reason: String,
    pub retryable: bool,
}

pub fn parse_json_with_extract<T: for<'de> Deserialize<'de>>(raw: &str) -> Option<T> {
    if let Ok(v) = serde_json::from_str::<T>(raw) {
        return Some(v);
    }
    if let (Some(start), Some(end)) = (raw.find('{'), raw.rfind('}')) {
        if start < end {
            let piece = &raw[start..=end];
            if let Ok(v) = serde_json::from_str::<T>(piece) {
                return Some(v);
            }
        }
    }
    None
}
