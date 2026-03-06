use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use tokio::sync::mpsc::Sender;
use tokio::time;
use tokio_util::sync::CancellationToken;

use crate::agents::agent::{Agent, AgentContext, AgentLlm, AgentOutcome, OutcomeKind};
use crate::agents::llm_json::{ExecutorOutput, PlannerOutput};
use crate::agents::{CustomAgent, ExecutorAgent, PlannerAgent, RecoveryAgent, VerifierAgent};
use crate::configs::{list_custom_agents, load_models, RuntimeConfig};
use crate::core::logger;
use crate::core::queue::TaskQueue;
use crate::core::state_machine;
use crate::core::storage::TaskStorage;
use crate::core::task::{Task, TaskStatus};
use crate::providers::openai_compatible::OpenAiCompatibleProvider;
use crate::providers::traits::LlmProvider;
use crate::skills::{SkillContext, SkillRegistry};

pub struct Runtime {
    config: RuntimeConfig,
    storage: Arc<TaskStorage>,
    queue: TaskQueue,
    cancel: CancellationToken,
    planner: PlannerAgent,
    executor: ExecutorAgent,
    verifier: VerifierAgent,
    recovery: RecoveryAgent,
    custom_agents: Vec<CustomAgent>,
    skills: SkillRegistry,
}

impl Runtime {
    pub async fn new(
        config: RuntimeConfig,
        storage: Arc<TaskStorage>,
        cancel: CancellationToken,
    ) -> Result<Self> {
        let llm = build_llm_ctx(&config)?;
        let mut custom_agents = Vec::new();
        for cfg in list_custom_agents().unwrap_or_default() {
            custom_agents.push(CustomAgent::new(cfg, llm.clone()));
        }

        Ok(Self {
            config,
            storage,
            queue: TaskQueue::new(1024),
            cancel,
            planner: PlannerAgent::new(llm.clone()),
            executor: ExecutorAgent::new(llm.clone()),
            verifier: VerifierAgent::new(llm.clone()),
            recovery: RecoveryAgent::new(llm),
            custom_agents,
            skills: SkillRegistry::default(),
        })
    }

    pub fn queue_sender(&self) -> Sender<String> {
        self.queue.sender()
    }

    pub async fn bootstrap_enqueue_unfinished(&self) -> Result<()> {
        let tasks = self.storage.list_tasks().await?;
        for t in tasks {
            if !t.status.is_terminal() {
                self.queue.enqueue(t.id.clone()).await?;
                logger::log(format!("[启动] 入队未完成任务 id={} status={}", t.id, t.status));
            }
        }
        Ok(())
    }

    pub async fn run_loop(&mut self, once: bool) -> Result<()> {
        logger::log("[运行时] 主循环启动");
        loop {
            let timeout = Duration::from_millis(self.config.poll_interval_ms);
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    logger::log("[运行时] 收到取消信号，准备退出主循环");
                    break;
                }
                maybe_task_id = time::timeout(timeout, self.queue.dequeue()) => {
                    let maybe_task_id = maybe_task_id.ok().flatten();
                    if let Some(task_id) = maybe_task_id {
                        self.process_task_until_wait(&task_id).await?;
                    }
                }
            }

            if once && self.no_unfinished_tasks().await? {
                logger::log("[运行时] --once 检测到无未完成任务，自动退出");
                break;
            }
        }
        Ok(())
    }

    async fn no_unfinished_tasks(&self) -> Result<bool> {
        let tasks = self.storage.list_tasks().await?;
        Ok(tasks.into_iter().all(|t| t.status.is_terminal()))
    }

    async fn process_task_until_wait(&self, task_id: &str) -> Result<()> {
        loop {
            if self.cancel.is_cancelled() {
                self.mark_cancelled_if_active(task_id).await?;
                return Ok(());
            }
            let mut task = self.storage.get_task(task_id).await?;
            if task.status.is_terminal() {
                return Ok(());
            }

            if let Some(next) = state_machine::pre_agent_transition(&task.status) {
                task.status = next;
                task.current_agent = Some("Planner".to_string());
                task.step += 1;
                task.touch();
                self.storage.save_task(&task).await?;
                logger::log(format!(
                    "[状态机] id={} 预流转 -> status={} step={}",
                    task.id, task.status, task.step
                ));
                continue;
            }

            let (agent, phase_key): (&dyn Agent, &str) = match task.status {
                TaskStatus::Planning => (&self.planner, "planning"),
                TaskStatus::Running => (&self.executor, "running"),
                TaskStatus::Verifying => (&self.verifier, "verifying"),
                TaskStatus::Retrying => (&self.recovery, "recovery"),
                _ => return Ok(()),
            };

            self.run_custom_phase(&format!("before_{}", phase_key), &mut task)
                .await;

            task.current_agent = Some(agent.name().to_string());
            task.updated_at = Utc::now();
            self.storage.save_task(&task).await?;

            logger::log(format!(
                "[执行] id={} agent={} status={} retry={}",
                task.id,
                agent.name(),
                task.status,
                task.retry_count
            ));

            let ctx = AgentContext {
                cancellation: self.cancel.child_token(),
            };
            let mut outcome = agent.handle(&mut task, ctx).await;

            if matches!(task.status, TaskStatus::Running)
                && matches!(outcome.kind, OutcomeKind::Success)
            {
                outcome = self.execute_executor_decision(&mut task).await;
            }

            if matches!(outcome.kind, OutcomeKind::Success) {
                self.run_custom_phase(&format!("after_{}", phase_key), &mut task)
                    .await;
            }

            self.apply_transition(task, outcome).await?;
        }
    }

    async fn run_custom_phase(&self, phase: &str, task: &mut Task) {
        for agent in &self.custom_agents {
            if !agent.cfg.enabled || agent.cfg.phase != phase {
                continue;
            }
            let ctx = AgentContext {
                cancellation: self.cancel.child_token(),
            };
            let _ = agent.handle(task, ctx).await;
        }
    }

    async fn execute_executor_decision(&self, task: &mut Task) -> AgentOutcome {
        let decision: Option<ExecutorOutput> = task
            .payload
            .get("executor_decision")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let Some(decision) = decision else {
            logger::log("[执行器] 缺少 executor_decision，标记重试");
            return AgentOutcome::retry("缺少执行决策");
        };

        let plan: Option<PlannerOutput> = task
            .payload
            .get("plan")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let total_steps = plan.map(|p| p.steps.len()).unwrap_or(1);
        let mut plan_index = task
            .payload
            .get("plan_index")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let outcome = match decision.decision.as_str() {
            "skip" => AgentOutcome::success(),
            "execute" => {
                if decision.tool == "no_op" {
                    AgentOutcome::success()
                } else {
                    let timeout_secs = decision
                        .args
                        .get("timeout_secs")
                        .and_then(|v| v.as_u64())
                        .or_else(|| {
                            task.payload
                                .get("input")
                                .and_then(|v| v.get("timeout_secs"))
                                .and_then(|v| v.as_u64())
                        })
                        .unwrap_or(10);

                    let Some(skill) = self.skills.get(&decision.tool) else {
                        return AgentOutcome::retry(format!("技能不存在: {}", decision.tool));
                    };

                    let mut skill_input = decision.args.clone();
                    if decision.tool == "shell_command"
                        && skill_input.get("command").and_then(|v| v.as_str()).is_none()
                    {
                        if let Some(command) = task
                            .payload
                            .get("input")
                            .and_then(|v| v.get("command"))
                            .and_then(|v| v.as_str())
                        {
                            skill_input = serde_json::json!({
                                "command": command
                            });
                        }
                    }

                    let sk_ctx = SkillContext {
                        task_id: task.id.clone(),
                        timeout_secs,
                        cancellation: self.cancel.child_token(),
                    };
                    match skill.run(sk_ctx, skill_input).await {
                        Ok(r) => {
                            if let Some(obj) = task.payload.as_object_mut() {
                                obj.insert(
                                    "execution_result".to_string(),
                                    serde_json::json!({
                                        "stdout": r.stdout,
                                        "stderr": r.stderr,
                                        "exit_code": r.exit_code,
                                        "duration_ms": r.duration_ms,
                                        "tool": decision.tool
                                    }),
                                );
                            }
                            if r.success {
                                AgentOutcome::success()
                            } else {
                                AgentOutcome::retry(format!(
                                    "技能执行失败 code={:?} stderr={}",
                                    r.exit_code, r.stderr
                                ))
                            }
                        }
                        Err(e) => AgentOutcome::retry(format!("技能执行异常 err={}", e)),
                    }
                }
            }
            _ => AgentOutcome::retry("执行决策失败".to_string()),
        };

        if matches!(outcome.kind, OutcomeKind::Success) {
            plan_index += 1;
            if let Some(obj) = task.payload.as_object_mut() {
                obj.insert("plan_index".to_string(), serde_json::json!(plan_index));
            }
            if plan_index < total_steps {
                return AgentOutcome::success_with_next(TaskStatus::Running);
            }
            return AgentOutcome::success_with_next(TaskStatus::Verifying);
        }

        outcome
    }

    async fn apply_transition(&self, mut task: Task, outcome: AgentOutcome) -> Result<()> {
        let tr = state_machine::decide_transition(&task, &outcome, self.config.max_retries);
        if tr.bump_retry {
            task.retry_count += 1;
        }
        if let Some(e) = tr.set_error {
            task.error = Some(e);
        } else if matches!(tr.next_status, TaskStatus::Success) {
            task.error = None;
        }
        task.status = tr.next_status;
        task.step += 1;
        task.touch();
        self.storage.save_task(&task).await?;
        logger::log(format!(
            "[状态机] id={} -> status={} retry={} step={}",
            task.id, task.status, task.retry_count, task.step
        ));
        Ok(())
    }

    async fn mark_cancelled_if_active(&self, task_id: &str) -> Result<()> {
        let mut task = self.storage.get_task(task_id).await?;
        if !task.status.is_terminal() {
            task.status = TaskStatus::Cancelled;
            task.error = Some("运行时收到取消信号".to_string());
            task.step += 1;
            task.touch();
            self.storage.save_task(&task).await?;
            logger::log(format!("[安全停止] id={} 已标记 cancelled", task.id));
        }
        Ok(())
    }
}

fn build_llm_ctx(config: &RuntimeConfig) -> Result<Option<AgentLlm>> {
    if !config.llm.enabled {
        logger::log("[Provider] LLM 已禁用，智能体将走本地兜底逻辑");
        return Ok(None);
    }

    let model_file_provider = load_models()
        .ok()
        .and_then(|m| {
            m.providers
                .into_iter()
                .find(|p| p.name == config.llm.provider)
        })
        .map(|p| {
            (
                p.protocol,
                p.name,
                p.base_url,
                p.api_key,
                p.model,
                p.timeout_secs,
                p.max_tokens,
                p.temperature,
            )
        });

    let provider_cfg = match config.selected_provider() {
        Ok(v) => v,
        Err(e) => {
            logger::log(format!("[Provider] 未找到 provider 配置: {}", e));
            return Ok(None);
        }
    };

    let (protocol, provider_name, base_url, api_key, model, timeout_secs, max_tokens, temperature) =
        if let Some(v) = model_file_provider {
            v
        } else {
            (
                provider_cfg.protocol.clone(),
                provider_cfg.provider_name.clone(),
                provider_cfg.base_url.clone(),
                provider_cfg.api_key.clone(),
                provider_cfg.model.clone(),
                provider_cfg.timeout_secs,
                provider_cfg.max_tokens,
                provider_cfg.temperature,
            )
        };

    if protocol != "openai_compatible" {
        logger::log(format!(
            "[Provider] 当前仅支持 openai_compatible，实际为 {}，自动禁用 LLM",
            protocol
        ));
        return Ok(None);
    }

    let key = api_key.trim();
    if key.is_empty() || key == "YOUR_API_KEY" {
        logger::log("[Provider] api_key 未配置，自动禁用 LLM");
        return Ok(None);
    }

    let provider = OpenAiCompatibleProvider::new(provider_name.clone(), base_url, api_key, timeout_secs, 2)?;
    let arc: Arc<dyn LlmProvider> = Arc::new(provider);

    Ok(Some(AgentLlm {
        provider: arc,
        model,
        temperature,
        max_tokens,
    }))
}
