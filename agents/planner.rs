use async_trait::async_trait;
use tokio::time::{sleep, Duration};

use crate::agents::agent::{Agent, AgentContext, AgentOutcome};
use crate::core::logger;
use crate::core::task::Task;

pub struct PlannerAgent;

#[async_trait]
impl Agent for PlannerAgent {
    fn name(&self) -> &'static str {
        "PlannerAgent"
    }

    async fn handle(&self, task: &mut Task, ctx: AgentContext) -> AgentOutcome {
        logger::log(format!("[Planner] id={} 开始规划: {}", task.id, task.title));
        tokio::select! {
            _ = ctx.cancellation.cancelled() => AgentOutcome::cancelled("规划阶段被取消"),
            _ = sleep(Duration::from_millis(300)) => AgentOutcome::success(),
        }
    }
}
