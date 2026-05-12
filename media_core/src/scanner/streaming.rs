// media_core/src/scanner/streaming.rs
use tokio::process::Child;
use tokio::sync::Mutex as TokioMutex;
use std::sync::Arc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::errors::Result;
use notify::{Watcher, RecursiveMode, Config, event::EventKind};
use tokio::sync::watch;

pub struct StreamSession {
    pub process: Child,
    pub output_dir: PathBuf,
    pub last_access: std::time::Instant,
}

pub struct StreamManager {
    sessions: Arc<TokioMutex<HashMap<String, StreamSession>>>,
    base_output_dir: PathBuf,
}

impl StreamManager {
    pub fn new(base_output_dir: PathBuf) -> Self {
        if !base_output_dir.exists() {
            let _ = std::fs::create_dir_all(&base_output_dir);
        }
        Self {
            sessions: Arc::new(TokioMutex::new(HashMap::new())),
            base_output_dir,
        }
    }

    pub async fn start_hls(&self, id: &str, input_path: &Path) -> Result<PathBuf> {
        {
            let mut sessions = self.sessions.lock().await;
            if let Some(session) = sessions.get_mut(id) {
                session.last_access = std::time::Instant::now();
                return Ok(session.output_dir.join("playlist.m3u8"));
            }
        }

        let output_dir = self.base_output_dir.join(id);
        if output_dir.exists() {
            let _ = std::fs::remove_dir_all(&output_dir);
        }
        std::fs::create_dir_all(&output_dir)?;

        let playlist_path = output_dir.join("playlist.m3u8");
        let segment_pattern = output_dir.join("seg_%03d.ts");

        let process = tokio::process::Command::new(crate::config::get_ffmpeg_path())
            .args(&[
                "-loglevel", "error",
                "-i", input_path.to_str().unwrap(),
                "-c:v", "libx264",
                "-preset", "ultrafast",
                "-tune", "zerolatency",
                "-crf", "26",
                "-c:a", "aac",
                "-b:a", "128k",
                "-ac", "2",
                "-f", "hls",
                "-hls_time", "2", 
                "-hls_list_size", "0", 
                "-hls_flags", "independent_segments+delete_segments+split_by_time",
                "-hls_segment_type", "mpegts",
                "-movflags", "+faststart",
                "-hls_segment_filename", segment_pattern.to_str().unwrap(),
                playlist_path.to_str().unwrap(),
            ])
            .kill_on_drop(true)
            .spawn()?;

        let (tx, mut rx) = watch::channel(false);
        let playlist_path_clone = playlist_path.clone();
        
        let mut watcher = notify::RecommendedWatcher::new(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                    for path in event.paths {
                        if path == playlist_path_clone {
                            if let Ok(metadata) = std::fs::metadata(&path) {
                                if metadata.len() > 0 {
                                    let _ = tx.send(true);
                                }
                            }
                        }
                    }
                }
            }
        }, Config::default())?;

        watcher.watch(&output_dir, RecursiveMode::NonRecursive)?;

        if playlist_path.exists() {
            if let Ok(metadata) = std::fs::metadata(&playlist_path) {
                if metadata.len() > 0 {
                    let _ = watcher.unwatch(&output_dir);
                    let mut sessions = self.sessions.lock().await;
                    sessions.insert(id.to_string(), StreamSession {
                        process,
                        output_dir,
                        last_access: std::time::Instant::now(),
                    });
                    return Ok(playlist_path);
                }
            }
        }

        let wait_for_file = tokio::time::timeout(std::time::Duration::from_secs(15), async {
            while !*rx.borrow() {
                if rx.changed().await.is_err() {
                    break;
                }
            }
        });
        
        match wait_for_file.await {
            Ok(_) => {
                let _ = watcher.unwatch(&output_dir);
                let mut sessions = self.sessions.lock().await;
                sessions.insert(id.to_string(), StreamSession {
                    process,
                    output_dir,
                    last_access: std::time::Instant::now(),
                });
                Ok(playlist_path)
            }
            _ => {
                let _ = watcher.unwatch(&output_dir);
                let mut p = process;
                let _ = p.kill().await;
                Err(crate::errors::CoreError::RuntimeError("Streaming failed to start: playlist timeout".into()))
            }
        }
    }

    pub async fn stop_stream(&self, id: &str) {
        let mut sessions = self.sessions.lock().await;
        if let Some(mut session) = sessions.remove(id) {
            let _ = session.process.kill().await;
            let _ = std::fs::remove_dir_all(&session.output_dir);
        }
    }

    pub async fn cleanup_stale_streams(&self) {
        let mut session_ids_to_remove = Vec::new();
        {
            let sessions = self.sessions.lock().await;
            let now = std::time::Instant::now();
            let timeout = std::time::Duration::from_secs(120);

            for (id, session) in sessions.iter() {
                if now.duration_since(session.last_access) > timeout {
                    session_ids_to_remove.push(id.clone());
                }
            }
        }

        for id in session_ids_to_remove {
            tracing::info!("Cleaning up stale stream session: {}", id);
            self.stop_stream(&id).await;
        }
    }

    pub async fn update_heartbeat(&self, id: &str) {
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get_mut(id) {
            session.last_access = std::time::Instant::now();
        }
    }
}
