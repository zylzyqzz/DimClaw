use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tokio::fs;

use crate::core::task::Task;

#[derive(Clone)]
pub struct TaskStorage {
    data_dir: PathBuf,
}

impl TaskStorage {
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    pub async fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(self.tasks_dir()).await?;
        Ok(())
    }

    pub fn tasks_dir(&self) -> PathBuf {
        self.data_dir.join("tasks")
    }

    fn task_path(&self, id: &str) -> PathBuf {
        self.tasks_dir().join(format!("{id}.json"))
    }

    pub async fn create_task(&self, task: &Task) -> Result<()> {
        self.save_task(task).await
    }

    pub async fn save_task(&self, task: &Task) -> Result<()> {
        let path = self.task_path(&task.id);
        let body = serde_json::to_vec_pretty(task)?;
        fs::write(path, body).await?;
        Ok(())
    }

    pub async fn get_task(&self, id: &str) -> Result<Task> {
        let path = self.task_path(id);
        let bytes = fs::read(&path)
            .await
            .with_context(|| format!("读取任务失败: {}", path.display()))?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub async fn list_tasks(&self) -> Result<Vec<Task>> {
        let mut out = Vec::new();
        let dir = self.tasks_dir();
        if !Path::new(&dir).exists() {
            return Ok(out);
        }
        let mut rd = fs::read_dir(dir).await?;
        while let Some(entry) = rd.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|v| v.to_str()) != Some("json") {
                continue;
            }
            let bytes = fs::read(path).await?;
            let task: Task = serde_json::from_slice(&bytes)?;
            out.push(task);
        }
        Ok(out)
    }
}
