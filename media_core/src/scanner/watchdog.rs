// core/src/scanner/watchdog.rs
use std::path::{Path, PathBuf};
use notify::{Watcher, RecursiveMode, Config};
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;
use crate::task_manager::TaskManager;
use crate::scanner::worker;

pub struct Watchdog {
    pool: SqlitePool,
    task_manager: Arc<TaskManager>,
}

impl Watchdog {
    pub fn new(pool: SqlitePool, task_manager: Arc<TaskManager>) -> Self {
        Self { pool, task_manager }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let (tx, mut rx) = mpsc::channel(100);

        let mut watcher = notify::RecommendedWatcher::new(move |res| {
            if let Ok(event) = res {
                let _ = tx.blocking_send(event);
            }
        }, Config::default())?;

        // Fetch all libraries to watch
        let libraries = crate::db::queries::get_all_libraries(&self.pool).await?;
        for lib in libraries {
            info!("Watching library: {}", lib.path);
            watcher.watch(Path::new(&lib.path), RecursiveMode::Recursive)?;
        }

        info!("Watchdog started, monitoring for file changes...");

        // Process events
        while let Some(event) = rx.recv().await {
            match event.kind {
                notify::EventKind::Create(_) | notify::EventKind::Modify(notify::event::ModifyKind::Name(_)) => {
                    for path in event.paths {
                        self.handle_change(path).await;
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn handle_change(&self, path: PathBuf) {
        if !path.is_file() { return; }
        
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_lowercase();
        let video_extensions = ["mp4", "mkv", "avi", "mov", "wmv", "m4v"];
        
        if video_extensions.contains(&ext.as_str()) {
            info!("New file detected: {:?}", path);
            
            // Find which library this belongs to
            let libraries = crate::db::queries::get_all_libraries(&self.pool).await.unwrap_or_default();
            if let Some(lib) = libraries.into_iter().find(|l| path.starts_with(&l.path)) {
                let task_id = format!("watchdog-{}", uuid::Uuid::new_v4());
                let pool = self.pool.clone();
                let task_manager = self.task_manager.clone();
                
                // Trigger a targeted scan or full scan
                // For simplicity, we trigger a scan for that library
                tokio::spawn(async move {
                    let _ = worker::scan_library(&pool, &lib, task_id, &task_manager).await;
                });
            }
        }
    }
}
