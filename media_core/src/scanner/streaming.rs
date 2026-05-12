// media_core/src/scanner/streaming.rs
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::errors::Result;
use crate::scanner::ffmpeg::FfmpegEngine;

pub struct StreamSession {
    pub process: Child,
    pub output_dir: PathBuf,
    pub last_access: std::time::Instant,
}

pub struct StreamManager {
    sessions: Arc<Mutex<HashMap<String, StreamSession>>>,
    base_output_dir: PathBuf,
}

impl StreamManager {
    pub fn new(base_output_dir: PathBuf) -> Self {
        if !base_output_dir.exists() {
            let _ = std::fs::create_dir_all(&base_output_dir);
        }
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            base_output_dir,
        }
    }

    pub fn start_hls(&self, id: &str, input_path: &Path) -> Result<PathBuf> {
        FfmpegEngine::check_ffmpeg()?;
        let mut sessions = self.sessions.lock().unwrap();
        
        // If session already exists, just return the playlist path
        if let Some(session) = sessions.get_mut(id) {
            session.last_access = std::time::Instant::now();
            return Ok(session.output_dir.join("playlist.m3u8"));
        }

        let output_dir = self.base_output_dir.join(id);
        if output_dir.exists() {
            let _ = std::fs::remove_dir_all(&output_dir);
        }
        std::fs::create_dir_all(&output_dir)?;

        let playlist_path = output_dir.join("playlist.m3u8");
        let segment_pattern = output_dir.join("seg_%03d.ts");

        // Optimized for Stash-like "Instant Play"
        let process = std::process::Command::new(crate::config::get_ffmpeg_path())
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
                "-hls_time", "2", // Smaller segments for faster start
                "-hls_list_size", "0", 
                "-hls_flags", "independent_segments+delete_segments+split_by_time",
                "-hls_segment_type", "mpegts",
                "-movflags", "+faststart",
                "-hls_segment_filename", segment_pattern.to_str().unwrap(),
                playlist_path.to_str().unwrap(),
            ])
            .spawn()?;

        sessions.insert(id.to_string(), StreamSession {
            process,
            output_dir,
            last_access: std::time::Instant::now(),
        });

        Ok(playlist_path)
    }

    pub fn stop_stream(&self, id: &str) {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(mut session) = sessions.remove(id) {
            let _ = session.process.kill();
            let _ = std::fs::remove_dir_all(&session.output_dir);
        }
    }

    pub fn cleanup_stale_streams(&self) {
        let mut sessions = self.sessions.lock().unwrap();
        let now = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(300); // 5 minutes of inactivity

        sessions.retain(|id, session| {
            if now.duration_since(session.last_access) > timeout {
                tracing::info!("Cleaning up stale stream session: {}", id);
                let _ = session.process.kill();
                let _ = std::fs::remove_dir_all(&session.output_dir);
                false
            } else {
                true
            }
        });
    }
}
