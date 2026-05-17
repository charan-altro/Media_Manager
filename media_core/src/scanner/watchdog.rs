// core/src/scanner/watchdog.rs
use std::path::{Path, PathBuf};
use notify::{Watcher, RecursiveMode, Config};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tracing::info;
use crate::scanner::service::ScannerService;
use std::collections::HashSet;
use crate::db::{Repositories, LibraryReader};

pub struct Watchdog {
    repos: Arc<Repositories>,
    scanner_service: Arc<dyn ScannerService>,
}

impl Watchdog {
    pub fn new(repos: Arc<Repositories>, scanner_service: Arc<dyn ScannerService>) -> Self {
        Self { repos, scanner_service }
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

        let repos_clone = self.repos.clone();
        
        loop {
            // Check for new libraries every 30 seconds
            if let Ok(libraries) = repos_clone.library.find_all().await {
                for lib in libraries {
                    if !watched_paths.contains(&lib.path) {
                        info!("Watchdog: New library detected, watching: {}", lib.path);
                        if let Err(e) = watcher.watch(Path::new(&lib.path), RecursiveMode::Recursive) {
                            tracing::error!("Watchdog failed to watch {}: {}", lib.path, e);
                        } else {
                            watched_paths.insert(lib.path.clone());
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
            let libraries = self.repos.library.find_all().await.unwrap_or_default();
            
            // Normalize path for comparison
            let normalized_path = crate::paths::normalize_slashes(&path.to_string_lossy());
            
            if let Some(lib) = libraries.into_iter().find(|l| {
                let normalized_lib = crate::paths::normalize_slashes(&l.path);
                normalized_path.starts_with(&normalized_lib)
            }) {
                let task_id = format!("watchdog-{}", uuid::Uuid::new_v4());
                let scanner_service = self.scanner_service.clone();
                let path_clone = path.clone();
                
                // Trigger targeted scan
                tokio::spawn(async move {
                    let _ = scanner_service.scan_single_file(&lib, path_clone, task_id).await;
                });
            }
        }
    }
}
