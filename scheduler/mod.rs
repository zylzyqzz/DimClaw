use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;
use tokio::time::interval;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::core::logger;
use crate::core::storage::TaskStorage;
use crate::core::task::Task;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleEntry {
    pub id: String,
    pub title: String,
    pub interval_secs: u64,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl ScheduleEntry {
    pub fn new(title: String, interval_secs: u64, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            title,
            interval_secs,
            payload,
            created_at: Utc::now(),
        }
    }
}

#[derive(Clone)]
pub struct SchedulerStore {
    data_dir: PathBuf,
}

impl SchedulerStore {
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    fn path(&self) -> PathBuf {
        self.data_dir.join("schedules.json")
    }

    pub async fn ensure_dir(&self) -> Result<()> {
        tokio::fs::create_dir_all(&self.data_dir).await?;
        Ok(())
    }

    pub async fn list_schedules(&self) -> Result<Vec<ScheduleEntry>> {
        let path = self.path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let bytes = tokio::fs::read(path).await?;
        let entries: Vec<ScheduleEntry> = serde_json::from_slice(&bytes)?;
        Ok(entries)
    }

    pub async fn add_schedule(&self, entry: ScheduleEntry) -> Result<()> {
        let mut entries = self.list_schedules().await?;
        entries.push(entry);
        let body = serde_json::to_vec_pretty(&entries)?;
        tokio::fs::write(self.path(), body).await?;
        Ok(())
    }
}

pub fn spawn_scheduler(
    entries: Vec<ScheduleEntry>,
    queue_tx: Sender<String>,
    storage: Arc<TaskStorage>,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        for entry in entries {
            let tx = queue_tx.clone();
            let storage = storage.clone();
            let cancel = cancel.clone();
            tokio::spawn(async move {
                let mut tick = interval(Duration::from_secs(entry.interval_secs));
                loop {
                    tokio::select! {
                        _ = cancel.cancelled() => {
                            logger::log(format!("[定时] id={} 已停止", entry.id));
                            break;
                        }
                        _ = tick.tick() => {
                            let task = Task::new(format!("[定时] {}", entry.title), entry.payload.clone());
                            if let Err(e) = storage.create_task(&task).await {
                                logger::log(format!("[定时] 创建任务失败 id={} err={}", entry.id, e));
                                continue;
                            }
                            if let Err(e) = tx.send(task.id.clone()).await {
                                logger::log(format!("[定时] 入队失败 id={} err={}", entry.id, e));
                            } else {
                                logger::log(format!("[定时] 已投递 task_id={} schedule_id={}", task.id, entry.id));
                            }
                        }
                    }
                }
            });
        }
        let _ = cancel.cancelled().await;
    })
}
