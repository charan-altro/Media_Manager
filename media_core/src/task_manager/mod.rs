// core/src/task_manager/mod.rs
use tokio::sync::{broadcast, Semaphore, Mutex as TokioMutex};
use crate::models::{TaskUpdate, LibraryId};
use std::sync::{Arc, Mutex};
use std::collections::{HashSet, HashMap};

#[cfg_attr(test, mockall::automock)]
pub trait ProgressSink: Send + Sync {
    fn broadcast(&self, update: TaskUpdate);
}

pub struct TaskManager {
    pub sender: broadcast::Sender<TaskUpdate>,
    pub heavy_task_semaphore: Arc<Semaphore>,
    pub running_scans: Arc<TokioMutex<HashSet<LibraryId>>>,
    pub history: Arc<Mutex<HashMap<String, TaskUpdate>>>,
}

impl ProgressSink for TaskManager {
    fn broadcast(&self, update: TaskUpdate) {
        if let Ok(mut history) = self.history.lock() {
            history.insert(update.task_id.clone(), update.clone());
        }
        
        let _ = self.sender.send(update);
    }
}

impl TaskManager {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(256);
        // Limit to 4 concurrent heavy tasks (FFmpeg/Scraping) by default
        let heavy_task_semaphore = Arc::new(Semaphore::new(4));
        let running_scans = Arc::new(TokioMutex::new(HashSet::new()));
        let history = Arc::new(Mutex::new(HashMap::new()));
        Self { sender, heavy_task_semaphore, running_scans, history }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<TaskUpdate> {
        self.sender.subscribe()
    }

    pub fn get_history(&self) -> Vec<TaskUpdate> {
        if let Ok(history) = self.history.lock() {
            let mut tasks: Vec<_> = history.values().cloned().collect();
            // Sort by started_at descending (newest first)
            tasks.sort_by(|a, b| b.started_at.cmp(&a.started_at));
            tasks
        } else {
            vec![]
        }
    }

    pub async fn acquire_heavy_permit(&self) -> tokio::sync::OwnedSemaphorePermit {
        self.heavy_task_semaphore.clone().acquire_owned().await.unwrap()
    }

    pub async fn try_lock_library_scan(&self, library_id: LibraryId) -> bool {
        let mut running = self.running_scans.lock().await;
        if running.contains(&library_id) {
            false
        } else {
            running.insert(library_id);
            true
        }
    }

    pub async fn unlock_library_scan(&self, library_id: LibraryId) {
        let mut running = self.running_scans.lock().await;
        running.remove(&library_id);
    }
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}
