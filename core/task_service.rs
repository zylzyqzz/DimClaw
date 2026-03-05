use std::sync::Arc;

use anyhow::Result;

use crate::core::storage::TaskStorage;
use crate::core::task::Task;

pub async fn submit_task(
    storage: Arc<TaskStorage>,
    title: String,
    command: String,
    timeout_secs: u64,
) -> Result<Task> {
    let payload = serde_json::json!({
        "input": {
            "command": command,
            "timeout_secs": timeout_secs
        }
    });
    let task = Task::new(title, payload);
    storage.create_task(&task).await?;
    Ok(task)
}
