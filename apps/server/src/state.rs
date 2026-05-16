use sqlx::SqlitePool;
use std::sync::Arc;
use media_core::task_manager::TaskManager;
use media_core::scanner::streaming::StreamManager;

pub struct AppState {
    pub pool: SqlitePool,
    pub task_manager: Arc<TaskManager>,
    pub stream_manager: Arc<StreamManager>,
}
