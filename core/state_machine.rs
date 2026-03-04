use crate::agents::agent::{AgentOutcome, OutcomeKind};
use crate::core::task::{Task, TaskStatus};

#[derive(Debug, Clone)]
pub struct Transition {
    pub next_status: TaskStatus,
    pub bump_retry: bool,
    pub set_error: Option<String>,
}

pub fn pre_agent_transition(status: &TaskStatus) -> Option<TaskStatus> {
    match status {
        TaskStatus::Pending => Some(TaskStatus::Planning),
        _ => None,
    }
}

pub fn decide_transition(task: &Task, outcome: &AgentOutcome, max_retries: u32) -> Transition {
    let status = &task.status;
    match outcome.kind {
        OutcomeKind::Cancelled => Transition {
            next_status: TaskStatus::Cancelled,
            bump_retry: false,
            set_error: outcome.message.clone(),
        },
        OutcomeKind::Success => {
            let next = match status {
                TaskStatus::Planning => TaskStatus::Running,
                TaskStatus::Running => TaskStatus::Verifying,
                TaskStatus::Verifying => TaskStatus::Success,
                TaskStatus::Retrying => outcome
                    .suggestion
                    .clone()
                    .unwrap_or(TaskStatus::Planning),
                _ => status.clone(),
            };
            Transition {
                next_status: next,
                bump_retry: false,
                set_error: None,
            }
        }
        OutcomeKind::Failure | OutcomeKind::Retry => {
            let will_retry = task.retry_count < max_retries;
            let next = if will_retry {
                TaskStatus::Retrying
            } else {
                TaskStatus::Failed
            };
            Transition {
                next_status: next,
                bump_retry: will_retry,
                set_error: outcome.message.clone(),
            }
        }
    }
}
