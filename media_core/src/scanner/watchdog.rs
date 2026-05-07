// core/src/scanner/watchdog.rs
use std::path::{Path, PathBuf};
use notify::{Watcher, RecursiveMode, Config};
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tracing::info;
use crate::task_manager::TaskManager;
use crate::scanner::worker;
use std::collections::HashSet;

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

        let mut watched_paths: HashSet<String> = HashSet::new();

        info!("Watchdog started, monitoring for file changes and new libraries...");

        let pool_clone = self.pool.clone();
        
        loop {
            // Check for new libraries every 30 seconds
            let pool_for_poll = pool_clone.clone();
            let mut new_paths_found = false;
            
            if let Ok(libraries) = crate::db::queries::get_all_libraries(&pool_for_poll).await {
                for lib in libraries {
                    if !watched_paths.contains(&lib.path) {
                        info!("Watchdog: New library detected, watching: {}", lib.path);
                        if let Err(e) = watcher.watch(Path::new(&lib.path), RecursiveMode::Recursive) {
                            tracing::error!("Watchdog failed to watch {}: {}", lib.path, e);
                        } else {
                            watched_paths.insert(lib.path.clone());
                            new_paths_found = true;
                        }
                    }
                }
            }

            // Process events for a while (or until timeout to re-poll libraries)
            let poll_duration = Duration::from_secs(30);
            let poll_future = sleep(poll_duration);
            tokio::pin!(poll_future);

            loop {
                tokio::select! {
                    _ = &mut poll_future => {
                        break; // Time to check for new libraries
                    }
                    Some(event) = rx.recv() => {
                        match event.kind {
                            notify::EventKind::Create(_) | notify::EventKind::Modify(notify::event::ModifyKind::Name(_)) => {
                                for path in event.paths {
                                    self.handle_change(path).await;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
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
                let path_clone = path.clone();
                
                // Trigger targeted scan
                tokio::spawn(async move {
                    let _ = worker::scan_single_file(&pool, &lib, path_clone, task_id, &task_manager).await;
                });
            }
        }
    }
}
