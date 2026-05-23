// media_core/src/services/library_service.rs
use std::sync::Arc;
use std::path::PathBuf;
use crate::CoreContext;
use crate::scanner::service::ScannerService;
use crate::scraper::service::ScraperService;
use crate::models::{Library, LibraryId};
use crate::db::{LibraryReader, SettingsRepository};
use crate::errors::{Result, CoreError};
use crate::cleanup::CleanupService;

/// Domain service wrapping all operations related to libraries,
/// including metadata enrichment, directory cleaning, and real-time watchers.
pub struct LibraryService {
    ctx: CoreContext,
    scanner: Arc<dyn ScannerService>,
    scraper: Arc<dyn ScraperService>,
}

impl LibraryService {
    pub fn new(
        ctx: CoreContext,
        scanner: Arc<dyn ScannerService>,
        scraper: Arc<dyn ScraperService>,
    ) -> Self {
        Self { ctx, scanner, scraper }
    }

    /// Triggers library directory scan for new and modified media files.
    pub async fn scan_library(&self, library: &Library, task_id: String) -> Result<()> {
        self.scanner.scan_library(library, task_id).await.map_err(|e| CoreError::RuntimeError(e.to_string()))
    }

    /// Triggers targeted file scanning.
    pub async fn scan_single_file(&self, library: &Library, path: PathBuf, task_id: String) -> Result<()> {
        self.scanner.scan_single_file(library, path, task_id).await.map_err(|e| CoreError::RuntimeError(e.to_string()))
    }

    /// Runs batch scraper enrichments on unmatched items.
    pub async fn bulk_scrape_library(&self, library_id: LibraryId, task_id: String) -> Result<()> {
        self.scraper.bulk_scrape_library(library_id, task_id).await.map_err(|e| CoreError::RuntimeError(e.to_string()))
    }

    /// Cleans duplicate artwork assets (posters, backdrops) under the library directory.
    pub async fn cleanup_duplicates(&self, library_id: LibraryId) -> Result<Vec<PathBuf>> {
        let libraries = self.ctx.repos.library.find_all().await?;
        if let Some(lib) = libraries.into_iter().find(|l| l.id == library_id) {
            let cleanup = CleanupService::new(PathBuf::from(lib.path));
            cleanup.remove_duplicate_artwork().map_err(|e| CoreError::RuntimeError(e.to_string()))
        } else {
            Err(CoreError::RuntimeError("Library not found".to_string()))
        }
    }

    /// Deletes all empty folders under the library directory.
    pub async fn cleanup_empty_folders(&self, library_id: LibraryId) -> Result<Vec<PathBuf>> {
        let libraries = self.ctx.repos.library.find_all().await?;
        if let Some(lib) = libraries.into_iter().find(|l| l.id == library_id) {
            let cleanup = CleanupService::new(PathBuf::from(lib.path));
            cleanup.remove_empty_folders().map_err(|e| CoreError::RuntimeError(e.to_string()))
        } else {
            Err(CoreError::RuntimeError("Library not found".to_string()))
        }
    }

    /// Spawns the watchdog background thread monitoring library path filesystem updates.
    pub fn start_watchdog(&self) {
        let repos = self.ctx.repos.clone();
        let scanner = self.scanner.clone();
        tokio::spawn(async move {
            let watchdog = crate::scanner::watchdog::Watchdog::new(repos, scanner);
            if let Err(e) = watchdog.start().await {
                tracing::error!("Watchdog failed: {}", e);
            }
        });
    }

    /// Subscribes to backend task updates and publishes completion details to Discord if configured.
    pub fn start_notification_monitor(&self) {
        let task_manager = self.ctx.task_manager.clone();
        let repos = self.ctx.repos.clone();
        tokio::spawn(async move {
            let mut rx = task_manager.subscribe();
            let notifier = crate::notifications::Notifier::new();
            while let Ok(update) = rx.recv().await {
                if update.status == "completed" || update.status == "error" {
                    if let Ok(settings) = repos.settings.get_all().await {
                        if let Some(url) = settings.get("discord_webhook_url") {
                            if !url.is_empty() {
                                let _ = notifier.send_discord_webhook(url, &update).await;
                            }
                        }
                    }
                }
            }
        });
    }
}
