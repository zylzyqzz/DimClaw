use async_trait::async_trait;

use crate::agents::agent::{Agent, AgentContext, AgentOutcome};
use crate::core::logger;
use crate::core::task::Task;
use crate::skills::{SkillContext, SkillRegistry};

pub struct ExecutorAgent {
    skills: SkillRegistry,
}

impl ExecutorAgent {
    pub fn new(skills: SkillRegistry) -> Self {
        Self { skills }
    }
}

#[async_trait]
impl Agent for ExecutorAgent {
    fn name(&self) -> &'static str {
        "ExecutorAgent"
    }

    async fn handle(&self, task: &mut Task, ctx: AgentContext) -> AgentOutcome {
        let skill_name = task
            .payload
            .get("skill")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let skill_input = task.payload.get("input").cloned().unwrap_or_default();
        let timeout_secs = task
            .payload
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(10);
        let Some(skill) = self.skills.get(&skill_name) else {
            return AgentOutcome::fail(format!("技能不存在: {skill_name}"));
        };

        let sk_ctx = SkillContext {
            task_id: task.id.clone(),
            timeout_secs,
            cancellation: ctx.cancellation.clone(),
        };
        let result = skill.run(sk_ctx, skill_input).await;
        match result {
            Ok(r) if r.success => {
                if let Some(obj) = task.payload.as_object_mut() {
                    obj.insert(
                        "execution_result".to_string(),
                        serde_json::json!({
                            "stdout": r.stdout,
                            "stderr": r.stderr,
                            "exit_code": r.exit_code,
                            "duration_ms": r.duration_ms
                        }),
                    );
                }
                logger::log(format!(
                    "[Executor] 技能执行成功 skill={} code={:?} 耗时={}ms",
                    skill_name, r.exit_code, r.duration_ms
                ));
                AgentOutcome::success()
            }
            Ok(r) => {
                if let Some(obj) = task.payload.as_object_mut() {
                    obj.insert(
                        "execution_result".to_string(),
                        serde_json::json!({
                            "stdout": r.stdout,
                            "stderr": r.stderr,
                            "exit_code": r.exit_code,
                            "duration_ms": r.duration_ms
                        }),
                    );
                }
                AgentOutcome::retry(format!(
                    "技能退出异常 skill={} code={:?} stderr={}",
                    skill_name, r.exit_code, r.stderr
                ))
            }
            Err(e) => AgentOutcome::retry(format!("技能执行失败 skill={} err={}", skill_name, e)),
        }
    }
}
