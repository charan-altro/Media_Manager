// core/src/models/task.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Task {
    pub id: String,
    pub task_type: String,
    pub status: String,
    pub progress: i32,
    pub total: i32,
    pub message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TaskUpdate {
    pub task_id: String,
    pub status: String,
    pub progress: i32,
    pub total: i32,
    pub message: String,
    pub started_at: Option<u64>, // Unix timestamp in ms
    pub debug_info: Option<String>,
}

impl TaskUpdate {
    pub fn new(task_id: String, status: String, progress: i32, total: i32, message: String) -> Self {
        Self {
            task_id,
            status,
            progress,
            total,
            message,
            started_at: Some(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64),
            debug_info: None,
        }
    }
}
