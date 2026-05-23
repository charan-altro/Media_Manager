// media_core/src/config.rs
use std::sync::Arc;
use crate::db::Repositories;
use crate::task_manager::TaskManager;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
    pub hls_transcode_dir: String,
}

#[derive(Clone)]
pub struct CoreContext {
    pub config: AppConfig,
    pub repos: Arc<Repositories>,
    pub task_manager: Arc<TaskManager>,
}

impl CoreContext {
    pub fn new(config: AppConfig, repos: Arc<Repositories>, task_manager: Arc<TaskManager>) -> Self {
        Self { config, repos, task_manager }
    }
}
