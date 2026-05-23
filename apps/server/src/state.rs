use sqlx::SqlitePool;
use std::sync::Arc;
use media_core::task_manager::TaskManager;
use media_core::scanner::streaming::StreamingService;
use media_core::db::Repositories;
use media_core::scraper::service::ScraperService;
use media_core::scanner::service::ScannerService;

use media_core::services::{LibraryService, PlaybackService};

pub struct AppState {
    pub pool: SqlitePool,
    pub repos: Arc<Repositories>,
    pub task_manager: Arc<TaskManager>,
    pub stream_manager: Arc<dyn StreamingService>,
    pub scraper_service: Arc<dyn ScraperService>,
    pub scanner_service: Arc<dyn ScannerService>,
    pub library_service: Arc<LibraryService>,
    pub playback_service: Arc<PlaybackService>,
}
