use async_trait::async_trait;
use tokio::time::{sleep, Duration};

use crate::agents::agent::{Agent, AgentContext, AgentOutcome};
use crate::core::logger;
use crate::core::task::Task;

pub struct VerifierAgent;

#[async_trait]
impl Agent for VerifierAgent {
    fn name(&self) -> &'static str {
        "VerifierAgent"
    }

    async fn handle(&self, task: &mut Task, ctx: AgentContext) -> AgentOutcome {
        logger::log(format!("[Verifier] id={} 开始校验", task.id));
        tokio::select! {
            _ = ctx.cancellation.cancelled() => AgentOutcome::cancelled("校验阶段被取消"),
            _ = sleep(Duration::from_millis(200)) => {
                let code = task
                    .payload
                    .get("execution_result")
                    .and_then(|v| v.get("exit_code"))
                    .and_then(|v| v.as_i64());
                if code == Some(0) {
                    AgentOutcome::success()
                } else {
                    AgentOutcome::retry(format!("校验未通过: exit_code={code:?}"))
                }
            },
        }
    }
}
