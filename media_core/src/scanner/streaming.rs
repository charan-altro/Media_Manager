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

const SEG_PREFIX: &str = "seg_";
const FFMPEG_SEG_PATTERN: &str = ".seg_%03d.ts";

pub fn generate_hls_manifest(duration_secs: f64) -> String {
    let mut manifest = String::from("#EXTM3U\n");
    manifest.push_str("#EXT-X-VERSION:3\n");
    manifest.push_str("#EXT-X-TARGETDURATION:12\n");
    manifest.push_str("#EXT-X-MEDIA-SEQUENCE:0\n");
    manifest.push_str("#EXT-X-PLAYLIST-TYPE:VOD\n");

    let segment_duration = 10.0;
    let num_segments = (duration_secs / segment_duration).ceil() as usize;

    for i in 0..num_segments {
        let remaining = duration_secs - (i as f64 * segment_duration);
        let current_seg_dur = if remaining < segment_duration {
            remaining
        } else {
            segment_duration
        };

        manifest.push_str(&format!("#EXTINF:{:.1},\n", current_seg_dur));
        manifest.push_str(&format!("seg_{:03}.ts\n", i));
    }

    manifest.push_str("#EXT-X-ENDLIST\n");
    manifest
}

pub fn generate_dash_manifest(duration_secs: f64, width: i32, height: i32) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<MPD xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
     xmlns="urn:mpeg:dash:schema:mpd:2011"
     xsi:schemaLocation="urn:mpeg:dash:schema:mpd:2011 DASH-MPD.xsd"
     profiles="urn:mpeg:dash:profile:isoff-live:2011"
     type="static"
     mediaPresentationDuration="PT{duration_secs:.1}S"
     minBufferTime="PT1.5S">
  <Period id="0">
    <AdaptationSet id="0" contentType="video" segmentAlignment="true" bitstreamSwitching="true">
      <Representation id="0" mimeType="video/webm" codecs="vp9" bandwidth="2000000" width="{width}" height="{height}" frameRate="30">
        <SegmentTemplate timescale="1000" duration="10000" initialization="init-stream$RepresentationID$.webm" media="chunk-stream$RepresentationID$-$Number%05d$.webm" startNumber="1"/>
      </Representation>
    </AdaptationSet>
    <AdaptationSet id="1" contentType="audio" segmentAlignment="true" bitstreamSwitching="true">
      <Representation id="1" mimeType="audio/webm" codecs="opus" bandwidth="128000" audioSamplingRate="48000">
        <AudioChannelConfiguration schemeIdUri="urn:mpeg:dash:23003:3:audio_channel_configuration:2011" value="2"/>
        <SegmentTemplate timescale="1000" duration="10000" initialization="init-stream$RepresentationID$.webm" media="chunk-stream$RepresentationID$-$Number%05d$.webm" startNumber="1"/>
      </Representation>
    </AdaptationSet>
  </Period>
</MPD>"#,
        duration_secs = duration_secs,
        width = width,
        height = height
    )
}

pub struct StreamSession {
    pub process: Child,
    pub output_dir: PathBuf,
    pub input_path: PathBuf,
    pub last_access: std::time::Instant,
    pub start_segment: usize,
    pub last_requested_segment: usize,
    pub latest_segment_rx: tokio::sync::watch::Receiver<Option<usize>>,
    pub is_dash: bool,
}

pub struct StreamManager {
    sessions: Arc<TokioMutex<HashMap<String, StreamSession>>>,
    pending_restarts: Arc<TokioMutex<HashMap<String, usize>>>,
    base_output_dir: PathBuf,
    hw_encoder: String,
    hw_decoders: Vec<String>,
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

        let hw_decoders = crate::scanner::ffmpeg::FfmpegEngine::probe_hw_decoders();

        Self {
            sessions: Arc::new(TokioMutex::new(HashMap::new())),
            pending_restarts: Arc::new(TokioMutex::new(HashMap::new())),
            base_output_dir,
            hw_encoder,
            hw_decoders,
        }
    }

    pub async fn start_hls(&self, id: &str, input_path: &Path) -> Result<PathBuf> {
        self.start_hls_at(id, input_path, 0).await
    }

    pub async fn start_hls_at(&self, id: &str, input_path: &Path, start_segment: usize) -> Result<PathBuf> {
        self.start_stream_at(id, input_path, start_segment, false).await
    }

    pub async fn start_dash(&self, id: &str, input_path: &Path) -> Result<PathBuf> {
        self.start_dash_at(id, input_path, 0).await
    }

    pub async fn start_dash_at(&self, id: &str, input_path: &Path, start_segment: usize) -> Result<PathBuf> {
        self.start_stream_at(id, input_path, start_segment, true).await
    }

    async fn start_stream_at(&self, id: &str, input_path: &Path, start_segment: usize, is_dash: bool) -> Result<PathBuf> {
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

        let manifest_name = if is_dash { "manifest.mpd" } else { "playlist.m3u8" };
        let manifest_path = output_dir.join(manifest_name);
        let normalized_input = crate::paths::normalize_slashes(&input_path.to_string_lossy());

        let args = self.build_ffmpeg_args(
            &normalized_input,
            &details,
            start_segment,
            &manifest_path,
            &output_dir,
            is_dash,
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
            let mut last_segments: HashMap<String, usize> = HashMap::new();
            
            while let Ok(Some(line)) = reader.next_line().await {
                if line.contains("Opening '") && line.contains("' for writing") {
                    // Handle both HLS (.ts) and DASH (.webm)
                    let (prefix, extension) = if is_dash {
                        ("chunk-stream", ".webm")
                    } else {
                        (SEG_PREFIX, ".ts")
                    };

                    if line.contains(extension) {
                        // Extract representation and segment index for DASH, or just segment for HLS
                        if is_dash {
                            // Example: Opening '.chunk-stream0-00001.webm' for writing
                            if let Some(part) = line.split(prefix).nth(1) {
                                let rep_id = part.split('-').next().unwrap_or("0");
                                if let Some(seg_str) = part.split('-').nth(1).and_then(|s| s.split('.').next()) {
                                    if let Ok(idx) = seg_str.parse::<usize>() {
                                        let key = format!("{}{}", prefix, rep_id);
                                        if let Some(prev_idx) = last_segments.get(&key) {
                                            let old_name = format!(".{prefix}{rep_id}-{:05}{extension}", prev_idx);
                                            let new_name = format!("{prefix}{rep_id}-{:05}{extension}", prev_idx);
                                            let old_path = output_dir_clone.join(old_name);
                                            let new_path = output_dir_clone.join(new_name);
                                            let _ = tokio::fs::rename(&old_path, &new_path).await;
                                        }
                                        last_segments.insert(key, idx);
                                        let _ = tx.send(Some(idx));
                                    }
                                }
                            }
                        } else {
                            if let Some(seg_str) = line.split(SEG_PREFIX).nth(1).and_then(|s| s.split('.').next()) {
                                if let Ok(idx) = seg_str.parse::<usize>() {
                                    if let Some(prev_idx) = last_segments.get("hls") {
                                        let old_path = output_dir_clone.join(format!(".seg_{:03}.ts", prev_idx));
                                        let new_path = output_dir_clone.join(format!("seg_{:03}.ts", prev_idx));
                                        let _ = tokio::fs::rename(&old_path, &new_path).await;
                                    }
                                    last_segments.insert("hls".to_string(), idx);
                                    let _ = tx.send(Some(idx));
                                }
                            }
                        }
                    }
                } else if line.contains("Error") || line.contains("failed") {
                    tracing::debug!("FFmpeg output: {}", line);
                }
            }

            // Handle final segments on exit
            for (key, last_idx) in last_segments {
                if is_dash {
                    let rep_id = key.strip_prefix("chunk-stream").unwrap_or("0");
                    let old_name = format!(".chunk-stream{rep_id}-{:05}.webm", last_idx);
                    let new_name = format!("chunk-stream{rep_id}-{:05}.webm", last_idx);
                    let _ = tokio::fs::rename(output_dir_clone.join(old_name), output_dir_clone.join(new_name)).await;
                } else {
                    let old_path = output_dir_clone.join(format!(".seg_{:03}.ts", last_idx));
                    let new_path = output_dir_clone.join(format!("seg_{:03}.ts", last_idx));
                    let _ = tokio::fs::rename(&old_path, &new_path).await;
                }
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
            is_dash,
        });

        Ok(manifest_path)
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

        let result = tokio::time::timeout(std::time::Duration::from_secs(20), async {
            loop {
                let current = *rx.borrow();
                if let Some(idx) = current {
                    if idx >= segment_index { 
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
        
        let (needs_restart, is_dash) = if let Some(session) = sessions.get_mut(id) {
            session.last_access = std::time::Instant::now();
            
            let segment_path = self.get_segment_path(id, segment_index);
            let temp_path = self.get_temp_segment_path(id, segment_index);
            
            // If segment is before current start, too far ahead, or MISSING on disk
            let needs_restart = segment_index < session.start_segment 
                || segment_index > session.last_requested_segment + 50 
                || (!segment_path.exists() && !temp_path.exists());

            if !needs_restart {
                session.last_requested_segment = segment_index;
            }
            (needs_restart, session.is_dash)
        } else {
            (true, id.contains("dash"))
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

            let result = self.start_stream_at(id, input_path, segment_index, is_dash).await;
            
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
        let mut throttled_session_ids = Vec::new();
        {
            let sessions = self.sessions.lock().await;
            let now = std::time::Instant::now();
            let timeout = std::time::Duration::from_secs(120);

            for (id, session) in sessions.iter() {
                if now.duration_since(session.last_access) > timeout {
                    session_ids_to_remove.push(id.clone());
                    continue;
                }

                if let Some(latest) = *session.latest_segment_rx.borrow() {
                    if latest > session.last_requested_segment + 30 { 
                        throttled_session_ids.push(id.clone());
                        session_ids_to_remove.push(id.clone());
                    }
                }
            }
        }

        for id in session_ids_to_remove {
            if throttled_session_ids.contains(&id) {
                tracing::info!("Throttling active transcoder: session {} is too far ahead", id);
            } else {
                tracing::info!("Cleaning up stale stream session: {}", id);
            }
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
        let hls_path = self.base_output_dir.join(id).join(format!("seg_{:03}.ts", segment_index));
        if hls_path.exists() {
            return hls_path;
        }
        self.base_output_dir.join(id).join(format!("chunk-stream0-{:05}.webm", segment_index + 1))
    }

    fn get_temp_segment_path(&self, id: &str, segment_index: usize) -> PathBuf {
        let hls_path = self.base_output_dir.join(id).join(format!(".seg_{:03}.ts", segment_index));
        if hls_path.exists() {
            return hls_path;
        }
        self.base_output_dir.join(id).join(format!(".chunk-stream0-{:05}.webm", segment_index + 1))
    }

    fn build_ffmpeg_args(
        &self,
        input_path: &str,
        details: &mediainfo::MediaDetails,
        start_segment: usize,
        playlist_path: &Path,
        output_dir: &Path,
        is_dash: bool,
    ) -> Vec<String> {
        if is_dash {
            return self.build_dash_args(input_path, details, start_segment, output_dir);
        }

        // Decision Logic for HLS
        let v_codec = if details.video_codec == "h264" { "copy" } else { &self.hw_encoder };
        let a_codec = if details.audio_codec == "aac" { "copy" } else { "aac" };

        let start_time = (start_segment * 10).to_string();

        let mut args = vec![
            "-loglevel".to_string(), "info".to_string(),
        ];

        // INJECT HW DECODER BEFORE -i
        if v_codec != "copy" {
            if let Some(hw_decoder) = crate::scanner::ffmpeg::FfmpegEngine::get_hw_decoder(&details.video_codec, &self.hw_decoders) {
                args.push("-c:v".to_string());
                args.push(hw_decoder);
            }
        }

        args.extend(vec![
            "-ss".to_string(), start_time,
            "-i".to_string(), input_path.to_string(),
            "-map".to_string(), "0:v:0".to_string(),
            "-map".to_string(), "0:a:0?".to_string(),
        ]);

        // Video codec and options
        args.push("-c:v".to_string());
        args.push(v_codec.to_string());
        if v_codec != "copy" {
            args.extend(vec![
                "-pix_fmt".to_string(), "yuv420p".to_string(),
                "-preset".to_string(), "ultrafast".to_string(),
                "-crf".to_string(), "26".to_string(),
            ]);

            // Hardware Scaling (Pi 4 Optimized)
            if self.hw_decoders.contains(&"h264_v4l2m2m".to_string()) && details.width > 1280 {
                args.push("-vf".to_string());
                args.push("scale_v4l2m2m=1280:-1".to_string());
            }
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

        let temp_segment_pattern = output_dir.join(FFMPEG_SEG_PATTERN);
        args.extend(vec![
            "-start_number".to_string(), start_segment.to_string(),
            "-hls_segment_type".to_string(), "mpegts".to_string(),
            "-hls_segment_filename".to_string(), temp_segment_pattern.to_string_lossy().to_string(),
            playlist_path.to_string_lossy().to_string(),
        ]);

        args
    }

    fn build_dash_args(
        &self,
        input_path: &str,
        details: &mediainfo::MediaDetails,
        start_segment: usize,
        output_dir: &Path,
    ) -> Vec<String> {
        let start_time = (start_segment * 10).to_string();
        let mut args = vec![
            "-loglevel".to_string(), "info".to_string(),
        ];

        // HW Decoder for VP9/Opus transcode
        if let Some(hw_decoder) = crate::scanner::ffmpeg::FfmpegEngine::get_hw_decoder(&details.video_codec, &self.hw_decoders) {
            args.push("-c:v".to_string());
            args.push(hw_decoder);
        }

        args.extend(vec![
            "-ss".to_string(), start_time,
            "-i".to_string(), input_path.to_string(),
        ]);

        // Video stream
        args.extend(vec![
            "-map".to_string(), "0:v:0".to_string(),
            "-c:v".to_string(), "libvpx-vp9".to_string(),
            "-s".to_string(), format!("{}x{}", details.width, details.height),
            "-keyint_min".to_string(), "150".to_string(),
            "-g".to_string(), "150".to_string(),
            "-tile-columns".to_string(), "4".to_string(),
            "-frame-parallel".to_string(), "1".to_string(),
            "-crf".to_string(), "30".to_string(),
            "-b:v".to_string(), "2000k".to_string(),
            "-f".to_string(), "webm_chunk".to_string(),
            "-header".to_string(), output_dir.join("init-stream0.webm").to_string_lossy().to_string(),
            "-chunk_start_index".to_string(), (start_segment + 1).to_string(),
            output_dir.join(".chunk-stream0-%05d.webm").to_string_lossy().to_string(),
        ]);

        // Audio stream
        args.extend(vec![
            "-map".to_string(), "0:a:0?".to_string(),
            "-c:a".to_string(), "libopus".to_string(),
            "-b:a".to_string(), "128k".to_string(),
            "-ac".to_string(), "2".to_string(),
            "-f".to_string(), "webm_chunk".to_string(),
            "-header".to_string(), output_dir.join("init-stream1.webm").to_string_lossy().to_string(),
            "-chunk_start_index".to_string(), (start_segment + 1).to_string(),
            output_dir.join(".chunk-stream1-%05d.webm").to_string_lossy().to_string(),
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
            false,
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
        manager.hw_decoders = vec![]; // Mock empty decoders for this test
        
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
            false,
        );

        // Check -hls_segment_filename
        let hls_seg_idx = args.iter().position(|r| r == "-hls_segment_filename").unwrap() + 1;
        assert!(args[hls_seg_idx].contains(".seg_"));

        // Check if codecs are not copy
        // Use rposition because we might have a decoder -c:v before -i
        let v_codec_idx = args.iter().rposition(|r| r == "-c:v").unwrap() + 1;
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

    #[tokio::test]
    async fn test_request_segment_logic() {
        let temp_dir = tempfile::tempdir().unwrap();
        let manager = StreamManager::new(temp_dir.path().to_path_buf());
        let id = "test_stream";
        let output_dir = temp_dir.path().join(id);
        std::fs::create_dir_all(&output_dir).unwrap();

        let process = if cfg!(windows) {
            tokio::process::Command::new("cmd")
                .args(&["/c", "exit 0"])
                .spawn()
                .unwrap()
        } else {
            tokio::process::Command::new("true")
                .spawn()
                .unwrap()
        };

        let (_tx, rx) = tokio::sync::watch::channel(None);
        {
            let mut sessions = manager.sessions.lock().await;
            sessions.insert(id.to_string(), StreamSession {
                process,
                output_dir: output_dir.clone(),
                input_path: PathBuf::from("input.mp4"),
                last_access: std::time::Instant::now(),
                start_segment: 0,
                last_requested_segment: 0,
                latest_segment_rx: rx,
                is_dash: false,
            });
        }

        let seg_path = manager.get_segment_path(id, 0);
        std::fs::write(&seg_path, "dummy data").unwrap();
        let result = manager.request_segment(id, Path::new("input.mp4"), 0).await;
        assert!(result.is_ok());

        let result = manager.request_segment(id, Path::new("input.mp4"), 1).await;
        assert!(result.is_err());

        let process = if cfg!(windows) {
            tokio::process::Command::new("cmd")
                .args(&["/c", "exit 0"])
                .spawn()
                .unwrap()
        } else {
            tokio::process::Command::new("true")
                .spawn()
                .unwrap()
        };
        let (_tx, rx) = tokio::sync::watch::channel(None);
        {
            let mut sessions = manager.sessions.lock().await;
            sessions.insert(id.to_string(), StreamSession {
                process,
                output_dir: output_dir.clone(),
                input_path: PathBuf::from("input.mp4"),
                last_access: std::time::Instant::now(),
                start_segment: 0,
                last_requested_segment: 0,
                latest_segment_rx: rx,
                is_dash: false,
            });
        }

        let temp_seg_path = manager.get_temp_segment_path(id, 1);
        std::fs::write(&temp_seg_path, "dummy data").unwrap();
        let result = manager.request_segment(id, Path::new("input.mp4"), 1).await;
        assert!(result.is_ok());

        let result = manager.request_segment(id, Path::new("input.mp4"), 2).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_transcoder_throttling() {
        let temp_dir = tempfile::tempdir().unwrap();
        let manager = StreamManager::new(temp_dir.path().to_path_buf());
        let id = "throttle_test";
        let output_dir = temp_dir.path().join(id);
        std::fs::create_dir_all(&output_dir).unwrap();

        let process = if cfg!(windows) {
            tokio::process::Command::new("cmd")
                .args(&["/c", "exit 0"])
                .spawn()
                .unwrap()
        } else {
            tokio::process::Command::new("true")
                .spawn()
                .unwrap()
        };

        let (tx, rx) = tokio::sync::watch::channel(None);
        {
            let mut sessions = manager.sessions.lock().await;
            sessions.insert(id.to_string(), StreamSession {
                process,
                output_dir: output_dir.clone(),
                input_path: PathBuf::from("input.mp4"),
                last_access: std::time::Instant::now(),
                start_segment: 0,
                last_requested_segment: 0,
                latest_segment_rx: rx,
                is_dash: false,
            });
        }

        tx.send(Some(10)).unwrap();
        manager.cleanup_stale_streams().await;
        {
            let sessions = manager.sessions.lock().await;
            assert!(sessions.contains_key(id));
        }

        tx.send(Some(31)).unwrap(); 
        manager.cleanup_stale_streams().await;
        {
            let sessions = manager.sessions.lock().await;
            assert!(!sessions.contains_key(id));
        }
    }

    #[test]
    fn test_build_ffmpeg_args_hw_accel_pi4() {
        let mut manager = StreamManager::new(PathBuf::from("tmp"));
        manager.hw_encoder = "h264_v4l2m2m".to_string();
        manager.hw_decoders = vec!["h264_v4l2m2m".to_string(), "hevc_v4l2m2m".to_string()];
        
        let details = MediaDetails {
            width: 1921, 
            height: 1081,
            video_codec: "hevc".to_string(),
            audio_codec: "mp3".to_string(),
            audio_channels: 2,
            size_bytes: 1000,
            duration_secs: 100,
        };

        let args = manager.build_ffmpeg_args(
            "input.mkv",
            &details,
            0,
            &PathBuf::from("playlist.m3u8"),
            &PathBuf::from("tmp"),
            false,
        );

        let i_idx = args.iter().position(|r| r == "-i").unwrap();
        let decoder_idx = args.iter().position(|r| r == "hevc_v4l2m2m").expect("Hardware decoder missing");
        assert!(decoder_idx < i_idx, "Hardware decoder must be before -i");
        assert_eq!(args[decoder_idx - 1], "-c:v");

        let v_codec_idx = args.iter().rposition(|r| r == "-c:v").unwrap() + 1;
        assert_eq!(args[v_codec_idx], "h264_v4l2m2m");

        let vf_idx = args.iter().position(|r| r == "-vf").expect("Scaling filter missing") + 1;
        assert_eq!(args[vf_idx], "scale_v4l2m2m=1280:-1");
    }
}
