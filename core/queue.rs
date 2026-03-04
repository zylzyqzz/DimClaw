use anyhow::Result;
use tokio::sync::mpsc;

#[derive(Debug)]
pub struct TaskQueue {
    tx: mpsc::Sender<String>,
    rx: mpsc::Receiver<String>,
}

impl TaskQueue {
    pub fn new(cap: usize) -> Self {
        let (tx, rx) = mpsc::channel(cap);
        Self { tx, rx }
    }

    pub fn sender(&self) -> mpsc::Sender<String> {
        self.tx.clone()
    }

    pub async fn enqueue(&self, task_id: String) -> Result<()> {
        self.tx.send(task_id).await?;
        Ok(())
    }

    pub async fn dequeue(&mut self) -> Option<String> {
        self.rx.recv().await
    }
}
