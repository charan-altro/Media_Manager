// media_core/src/scanner/streaming.rs
use tokio::process::Child;
use tokio::sync::Mutex as TokioMutex;
use std::sync::Arc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::errors::Result;
use std::process::Stdio;
use tokio::io::AsyncReadExt;

pub struct StreamSession {
    pub process: Child,
    pub output_dir: PathBuf,
    pub input_path: PathBuf,
    pub last_access: std::time::Instant,
    pub start_segment: usize,
    pub last_requested_segment: usize,
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
        self.start_hls_at(id, input_path, 0).await
    }

    pub async fn start_hls_at(&self, id: &str, input_path: &Path, start_segment: usize) -> Result<PathBuf> {
        // Stop existing session if any
        self.stop_stream(id).await;

        let output_dir = self.base_output_dir.join(id);
        if output_dir.exists() {
            let _ = std::fs::remove_dir_all(&output_dir);
        }
        std::fs::create_dir_all(&output_dir)?;

        let playlist_path = output_dir.join("playlist.m3u8");
        let segment_pattern = output_dir.join("seg_%03d.ts");
        let normalized_input = crate::paths::normalize_slashes(input_path.to_str().unwrap());

        let encoder = if cfg!(target_arch = "aarch64") { "h264_v4l2m2m" } 
                      else if cfg!(target_os = "macos") { "h264_videotoolbox" } 
                      else { "libx264" };

        let start_time = (start_segment * 10).to_string();

        let mut process = tokio::process::Command::new(crate::config::get_ffmpeg_path())
            .args(&[
                "-loglevel", "error",
                "-ss", &start_time,
                "-i", &normalized_input,
                "-map", "0:v:0",
                "-map", "0:a:0?",
                "-c:v", encoder,
                "-pix_fmt", "yuv420p",
                "-preset", "ultrafast",
                "-crf", "26",
                "-c:a", "aac",
                "-b:a", "128k",
                "-ac", "2",
                "-sn",
                "-f", "hls",
                "-hls_time", "10", 
                "-hls_list_size", "0", 
                "-hls_flags", "independent_segments",
                "-force_key_frames", "expr:gte(t,n_forced*10)",
                "-start_number", &start_segment.to_string(),
                "-hls_segment_type", "mpegts",
                "-hls_segment_filename", segment_pattern.to_str().unwrap(),
                playlist_path.to_str().unwrap(),
            ])
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        // Wait a small amount of time to see if FFmpeg crashes immediately
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if let Ok(Some(status)) = process.try_wait() {
            let mut stderr_content = String::new();
            if let Some(mut stderr) = process.stderr.take() {
                let _ = stderr.read_to_string(&mut stderr_content).await;
            }
            return Err(crate::errors::CoreError::RuntimeError(format!(
                "FFmpeg exited immediately with status {}. Stderr: {}", 
                status, stderr_content
            )));
        }

        let mut sessions = self.sessions.lock().await;
        sessions.insert(id.to_string(), StreamSession {
            process,
            output_dir: output_dir.clone(),
            input_path: input_path.to_path_buf(),
            last_access: std::time::Instant::now(),
            start_segment,
            last_requested_segment: start_segment,
        });

        Ok(playlist_path)
    }

    pub async fn request_segment(&self, id: &str, input_path: &Path, segment_index: usize) -> Result<()> {
        let mut sessions = self.sessions.lock().await;
        
        let needs_restart = if let Some(session) = sessions.get_mut(id) {
            session.last_access = std::time::Instant::now();
            
            // If segment is before current start or too far ahead
            // (Client jumped more than 1 segment ahead of what we are currently producing)
            if segment_index < session.start_segment || segment_index > session.last_requested_segment + 1 {
                true
            } else {
                session.last_requested_segment = segment_index;
                false
            }
        } else {
            true
        };

        if needs_restart {
            drop(sessions);
            self.start_hls_at(id, input_path, segment_index).await?;
        }

        Ok(())
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
