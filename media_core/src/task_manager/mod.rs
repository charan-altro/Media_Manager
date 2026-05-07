// core/src/task_manager/mod.rs
use tokio::sync::{broadcast, Semaphore};
use crate::models::TaskUpdate;
use std::sync::Arc;

pub struct TaskManager {
    pub sender: broadcast::Sender<TaskUpdate>,
    pub heavy_task_semaphore: Arc<Semaphore>,
}

impl TaskManager {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(256);
        // Limit to 4 concurrent heavy tasks (FFmpeg/Scraping) by default
        let heavy_task_semaphore = Arc::new(Semaphore::new(4));
        Self { sender, heavy_task_semaphore }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<TaskUpdate> {
        self.sender.subscribe()
    }

    pub fn broadcast(&self, update: TaskUpdate) {
        let _ = self.sender.send(update);
    }

    pub async fn acquire_heavy_permit(&self) -> tokio::sync::OwnedSemaphorePermit {
        self.heavy_task_semaphore.clone().acquire_owned().await.unwrap()
    }
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}
