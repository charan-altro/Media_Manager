// media_core/src/scanner/streaming.rs
use tokio::process::Child;
use tokio::sync::Mutex as TokioMutex;
use std::sync::Arc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::errors::Result;
use std::process::Stdio;
// use tokio::io::AsyncReadExt;
use crate::scanner::mediainfo;

pub struct StreamSession {
    pub process: Child,
    pub output_dir: PathBuf,
    pub input_path: PathBuf,
    pub last_access: std::time::Instant,
    pub start_segment: usize,
    pub last_requested_segment: usize,
    pub latest_segment_rx: tokio::sync::watch::Receiver<Option<usize>>,
}

pub struct StreamManager {
    sessions: Arc<TokioMutex<HashMap<String, StreamSession>>>,
    pending_restarts: Arc<TokioMutex<HashMap<String, usize>>>,
    base_output_dir: PathBuf,
    hw_encoder: String,
}

impl StreamManager {
    pub fn new(base_output_dir: PathBuf) -> Self {
        if !base_output_dir.exists() {
            let _ = std::fs::create_dir_all(&base_output_dir);
        }

        let supported_codecs = crate::scanner::ffmpeg::FfmpegEngine::probe_hw_codecs();
        let hw_encoder = if let Some(codec) = supported_codecs.first() {
            tracing::info!("Using detected hardware encoder: {}", codec);
            codec.clone()
        } else {
            tracing::info!("No hardware encoder detected, falling back to libx264");
            "libx264".to_string()
        };

        Self {
            sessions: Arc::new(TokioMutex::new(HashMap::new())),
            pending_restarts: Arc::new(TokioMutex::new(HashMap::new())),
            base_output_dir,
            hw_encoder,
        }
    }

    pub async fn start_hls(&self, id: &str, input_path: &Path) -> Result<PathBuf> {
        self.start_hls_at(id, input_path, 0).await
    }

    pub async fn start_hls_at(&self, id: &str, input_path: &Path, start_segment: usize) -> Result<PathBuf> {
        // Stop existing session if any
        self.stop_stream(id).await;

        let details = mediainfo::get_media_info(input_path).unwrap_or_else(|e| {
            tracing::warn!("Failed to probe file {:?}, falling back to full transcode: {}", input_path, e);
            mediainfo::MediaDetails::default()
        });

        let output_dir = self.base_output_dir.join(id);
        if output_dir.exists() {
            let _ = std::fs::remove_dir_all(&output_dir);
        }
        std::fs::create_dir_all(&output_dir)?;

        let playlist_path = output_dir.join("playlist.m3u8");
        let normalized_input = crate::paths::normalize_slashes(&input_path.to_string_lossy());

        let args = self.build_ffmpeg_args(
            &normalized_input,
            &details,
            start_segment,
            &playlist_path,
            &output_dir,
        );

        let mut process = tokio::process::Command::new(crate::config::get_ffmpeg_path())
            .args(&args)
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        let stderr = process.stderr.take().expect("Failed to take stderr");
        let (tx, rx) = tokio::sync::watch::channel(None);
        
        // Spawn a task to monitor FFmpeg output and update the channel
        let output_dir_clone = output_dir.clone();
        tokio::spawn(async move {
            use tokio::io::AsyncBufReadExt;
            let mut reader = tokio::io::BufReader::new(stderr).lines();
            let mut last_segment: Option<usize> = None;
            
            while let Ok(Some(line)) = reader.next_line().await {
                if line.contains("Opening '") && line.contains(".ts' for writing") {
                    if let Some(seg_str) = line.split("seg_").nth(1).and_then(|s| s.split('.').next()) {
                        match seg_str.parse::<usize>() {
                            Ok(idx) => {
                                // Rename the PREVIOUS segment if it exists
                                if let Some(prev_idx) = last_segment {
                                    let old_path = output_dir_clone.join(format!(".seg_{:03}.ts", prev_idx));
                                    let new_path = output_dir_clone.join(format!("seg_{:03}.ts", prev_idx));
                                    let _ = tokio::fs::rename(old_path, new_path).await;
                                }
                                last_segment = Some(idx);
                                let _ = tx.send(Some(idx));
                            }
                            Err(e) => {
                                tracing::warn!("Failed to parse segment index from line: '{}'. Error: {}", line, e);
                            }
                        }
                    }
                } else if line.contains("Error") || line.contains("failed") {
                    tracing::debug!("FFmpeg output: {}", line);
                }
            }

            // Handle final segment on exit
            if let Some(last_idx) = last_segment {
                let old_path = output_dir_clone.join(format!(".seg_{:03}.ts", last_idx));
                let new_path = output_dir_clone.join(format!("seg_{:03}.ts", last_idx));
                let _ = tokio::fs::rename(old_path, new_path).await;
            }
        });

        // Small delay to check if FFmpeg crashes immediately
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        if let Ok(Some(status)) = process.try_wait() {
            return Err(crate::errors::CoreError::RuntimeError(format!(
                "FFmpeg exited immediately with status {}. Check logs for details.", 
                status
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
            latest_segment_rx: rx,
        });

        Ok(playlist_path)
    }

    pub async fn wait_for_segment(&self, id: &str, segment_index: usize) -> Result<bool> {
        let mut rx = {
            let sessions = self.sessions.lock().await;
            if let Some(session) = sessions.get(id) {
                session.latest_segment_rx.clone()
            } else {
                return Ok(false);
            }
        };

        // Update heartbeat to prevent reaping while waiting
        self.update_heartbeat(id).await;

        let result = tokio::time::timeout(std::time::Duration::from_secs(20), async {            loop {
                let current = *rx.borrow();
                if let Some(idx) = current {
                    if idx > segment_index {
                        break;
                    }
                }
                if rx.changed().await.is_err() {
                    break;
                }
            }
        }).await;
        
        Ok(result.is_ok())
    }

    pub async fn request_segment(&self, id: &str, input_path: &Path, segment_index: usize) -> Result<()> {
        let mut sessions = self.sessions.lock().await;
        
        let needs_restart = if let Some(session) = sessions.get_mut(id) {
            session.last_access = std::time::Instant::now();
            
            let segment_path = self.get_segment_path(id, segment_index);
            
            // If segment is before current start, too far ahead, or MISSING on disk
            if segment_index < session.start_segment 
                || segment_index > session.last_requested_segment + 1
                || !segment_path.exists() 
            {
                true
            } else {
                session.last_requested_segment = segment_index;
                false
            }
        } else {
            true
        };

        if needs_restart {
            // Check if a restart for this id/segment is already in progress
            let mut pending = self.pending_restarts.lock().await;
            if let Some(&target) = pending.get(id) {
                if target == segment_index {
                    return Ok(());
                }
            }
            pending.insert(id.to_string(), segment_index);
            drop(pending);
            drop(sessions);

            let result = self.start_hls_at(id, input_path, segment_index).await;
            
            // Clear pending restart regardless of success
            let mut pending = self.pending_restarts.lock().await;
            pending.remove(id);
            
            return result.map(|_| ());
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

    fn get_segment_path(&self, id: &str, segment_index: usize) -> PathBuf {
        self.base_output_dir.join(id).join(format!("seg_{:03}.ts", segment_index))
    }

    fn build_ffmpeg_args(
        &self,
        input_path: &str,
        details: &mediainfo::MediaDetails,
        start_segment: usize,
        playlist_path: &Path,
        output_dir: &Path,
    ) -> Vec<String> {
        // Decision Logic
        let v_codec = if details.video_codec == "h264" { "copy" } else { &self.hw_encoder };
        let a_codec = if details.audio_codec == "aac" { "copy" } else { "aac" };

        let start_time = (start_segment * 10).to_string();

        let mut args = vec![
            "-loglevel".to_string(), "info".to_string(),
            "-ss".to_string(), start_time,
            "-i".to_string(), input_path.to_string(),
            "-map".to_string(), "0:v:0".to_string(),
            "-map".to_string(), "0:a:0?".to_string(),
        ];

        // Video codec and options
        args.push("-c:v".to_string());
        args.push(v_codec.to_string());
        if v_codec != "copy" {
            args.extend(vec![
                "-pix_fmt".to_string(), "yuv420p".to_string(),
                "-preset".to_string(), "ultrafast".to_string(),
                "-crf".to_string(), "26".to_string(),
            ]);
        }

        // Audio codec and options
        args.push("-c:a".to_string());
        args.push(a_codec.to_string());
        if a_codec != "copy" {
            args.extend(vec![
                "-b:a".to_string(), "128k".to_string(),
                "-ac".to_string(), "2".to_string(),
            ]);
        }

        // Common HLS options
        args.extend(vec![
            "-sn".to_string(),
            "-f".to_string(), "hls".to_string(),
            "-hls_time".to_string(), "10".to_string(),
            "-hls_list_size".to_string(), "0".to_string(),
            "-hls_flags".to_string(), "independent_segments".to_string(),
        ]);

        if v_codec != "copy" {
            args.push("-force_key_frames".to_string());
            args.push("expr:gte(t,n_forced*10)".to_string());
        }

        let temp_segment_pattern = output_dir.join(".seg_%03d.ts");
        args.extend(vec![
            "-start_number".to_string(), start_segment.to_string(),
            "-hls_segment_type".to_string(), "mpegts".to_string(),
            "-hls_segment_filename".to_string(), temp_segment_pattern.to_string_lossy().to_string(),
            playlist_path.to_string_lossy().to_string(),
        ]);

        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::mediainfo::MediaDetails;

    #[test]
    fn test_build_ffmpeg_args_smart_remux() {
        let manager = StreamManager::new(PathBuf::from("tmp"));
        let details = MediaDetails {
            width: 1920,
            height: 1080,
            video_codec: "h264".to_string(),
            audio_codec: "aac".to_string(),
            audio_channels: 2,
            size_bytes: 1000,
            duration_secs: 100,
        };

        let args = manager.build_ffmpeg_args(
            "input.mp4",
            &details,
            0,
            &PathBuf::from("playlist.m3u8"),
            &PathBuf::from("tmp"),
        );

        // Check -hls_segment_filename
        let hls_seg_idx = args.iter().position(|r| r == "-hls_segment_filename").unwrap() + 1;
        assert!(args[hls_seg_idx].contains(".seg_"));

        // Check if codecs are copy
        let v_codec_idx = args.iter().position(|r| r == "-c:v").unwrap() + 1;
        assert_eq!(args[v_codec_idx], "copy");
        let a_codec_idx = args.iter().position(|r| r == "-c:a").unwrap() + 1;
        assert_eq!(args[a_codec_idx], "copy");

        // Verify omitted flags
        assert!(!args.contains(&"-pix_fmt".to_string()));
        assert!(!args.contains(&"-preset".to_string()));
        assert!(!args.contains(&"-crf".to_string()));
        assert!(!args.contains(&"-b:a".to_string()));
        assert!(!args.contains(&"-ac".to_string()));
        assert!(!args.contains(&"-force_key_frames".to_string()));
    }

    #[test]
    fn test_build_ffmpeg_args_full_transcode() {
        let mut manager = StreamManager::new(PathBuf::from("tmp"));
        manager.hw_encoder = "libx264".to_string();
        
        let details = MediaDetails {
            width: 1920,
            height: 1080,
            video_codec: "hevc".to_string(),
            audio_codec: "mp3".to_string(),
            audio_channels: 2,
            size_bytes: 1000,
            duration_secs: 100,
        };

        let args = manager.build_ffmpeg_args(
            "input.mp4",
            &details,
            0,
            &PathBuf::from("playlist.m3u8"),
            &PathBuf::from("tmp"),
        );

        // Check -hls_segment_filename
        let hls_seg_idx = args.iter().position(|r| r == "-hls_segment_filename").unwrap() + 1;
        assert!(args[hls_seg_idx].contains(".seg_"));

        // Check if codecs are not copy
        let v_codec_idx = args.iter().position(|r| r == "-c:v").unwrap() + 1;
        assert_eq!(args[v_codec_idx], "libx264");
        let a_codec_idx = args.iter().position(|r| r == "-c:a").unwrap() + 1;
        assert_eq!(args[a_codec_idx], "aac");

        // Verify present flags
        assert!(args.contains(&"-pix_fmt".to_string()));
        assert!(args.contains(&"-preset".to_string()));
        assert!(args.contains(&"-crf".to_string()));
        assert!(args.contains(&"-b:a".to_string()));
        assert!(args.contains(&"-ac".to_string()));
        assert!(args.contains(&"-force_key_frames".to_string()));
    }
}
