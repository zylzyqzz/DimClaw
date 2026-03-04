use async_trait::async_trait;
use tokio::time::{sleep, Duration};

use crate::agents::agent::{Agent, AgentContext, AgentOutcome};
use crate::core::logger;
use crate::core::task::{Task, TaskStatus};

pub struct RecoveryAgent;

#[async_trait]
impl Agent for RecoveryAgent {
    fn name(&self) -> &'static str {
        "RecoveryAgent"
    }

    async fn handle(&self, task: &mut Task, ctx: AgentContext) -> AgentOutcome {
        logger::log(format!(
            "[Recovery] id={} 第 {} 次重试恢复",
            task.id, task.retry_count
        ));
        tokio::select! {
            _ = ctx.cancellation.cancelled() => AgentOutcome::cancelled("恢复阶段被取消"),
            _ = sleep(Duration::from_millis(150)) => {
                if task.retry_count >= 3 {
                    AgentOutcome::success_with_next(TaskStatus::Failed)
                } else if task.retry_count % 2 == 0 {
                    AgentOutcome::success_with_next(TaskStatus::Planning)
                } else {
                    AgentOutcome::success_with_next(TaskStatus::Running)
                }
            }
        }
    }
}
