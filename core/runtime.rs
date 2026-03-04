use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use tokio::sync::mpsc::Sender;
use tokio::time;
use tokio_util::sync::CancellationToken;

use crate::agents::agent::{Agent, AgentContext, AgentOutcome};
use crate::agents::{ExecutorAgent, PlannerAgent, RecoveryAgent, VerifierAgent};
use crate::configs::RuntimeConfig;
use crate::core::logger;
use crate::core::queue::TaskQueue;
use crate::core::state_machine;
use crate::core::storage::TaskStorage;
use crate::core::task::{Task, TaskStatus};
use crate::skills::SkillRegistry;

pub struct Runtime {
    config: RuntimeConfig,
    storage: Arc<TaskStorage>,
    queue: TaskQueue,
    cancel: CancellationToken,
    planner: PlannerAgent,
    executor: ExecutorAgent,
    verifier: VerifierAgent,
    recovery: RecoveryAgent,
}

impl Runtime {
    pub async fn new(
        config: RuntimeConfig,
        storage: Arc<TaskStorage>,
        cancel: CancellationToken,
    ) -> Result<Self> {
        let skills = SkillRegistry::default();
        Ok(Self {
            config,
            storage,
            queue: TaskQueue::new(1024),
            cancel,
            planner: PlannerAgent,
            executor: ExecutorAgent::new(skills),
            verifier: VerifierAgent,
            recovery: RecoveryAgent,
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
                task.current_agent = Some("PlannerAgent".to_string());
                task.step += 1;
                task.touch();
                self.storage.save_task(&task).await?;
                logger::log(format!(
                    "[状态机] id={} 预流转 -> status={} step={}",
                    task.id, task.status, task.step
                ));
                continue;
            }

            let agent: &dyn Agent = match task.status {
                TaskStatus::Planning => &self.planner,
                TaskStatus::Running => &self.executor,
                TaskStatus::Verifying => &self.verifier,
                TaskStatus::Retrying => &self.recovery,
                _ => return Ok(()),
            };

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
            let outcome = agent.handle(&mut task, ctx).await;
            self.apply_transition(task, outcome).await?;
        }
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
