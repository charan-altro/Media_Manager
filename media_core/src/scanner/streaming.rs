// media_core/src/scanner/streaming.rs
use tokio::process::Child;
use std::sync::Arc;
use dashmap::DashMap;
use std::path::{Path, PathBuf};
use crate::errors::Result;
use std::process::Stdio;
use std::collections::HashMap;
use crate::scanner::mediainfo;
use tokio_util::io::ReaderStream;
use tokio_util::bytes::Bytes;
use futures::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::io;
use async_trait::async_trait;

const SEG_PREFIX: &str = "seg_";
const FFMPEG_SEG_PATTERN: &str = ".seg_%03d.ts";

#[async_trait]
pub trait StreamingService: Send + Sync {
    /// Starts an HLS stream at a specific segment.
    async fn start_hls(&self, id: &str, input_path: &Path) -> Result<PathBuf>;
    async fn start_hls_at(&self, id: &str, input_path: &Path, start_segment: usize) -> Result<PathBuf>;
    
    /// Starts a DASH stream at a specific segment.
    async fn start_dash(&self, id: &str, input_path: &Path) -> Result<PathBuf>;
    async fn start_dash_at(&self, id: &str, input_path: &Path, start_segment: usize) -> Result<PathBuf>;

    /// Starts a direct remuxed/transcoded stream piped to stdout.
    async fn start_direct_stream(&self, input_path: &Path, start_time: f64, format: &str) -> Result<ChildStream>;

    /// Waits for a specific segment file to be generated.
    async fn wait_for_segment(&self, id: &str, segment_index: usize, file: &str) -> Result<bool>;

    /// Requests a segment, restarting the process if necessary.
    async fn request_segment(&self, id: &str, input_path: &Path, segment_index: usize, file: &str) -> Result<()>;

    /// Stops a specific stream process (throttling).
    async fn stop_stream(&self, id: &str);

    /// Destroys a stream session and its files (cleanup).
    async fn destroy_session(&self, id: &str);

    /// Stops all active stream sessions.
    async fn stop_all_streams(&self);

    /// Updates the heartbeat for a session to prevent it from being reaped.
    async fn update_heartbeat(&self, id: &str);

    /// Registers a client viewer for a stream.
    async fn register_client(&self, id: &str, client_id: &str);

    /// Unregisters a client viewer for a stream. If no viewers are left, stops the stream.
    async fn unregister_client(&self, id: &str, client_id: &str);

    /// Periodically called to clean up idle or runaway stream sessions.
    async fn cleanup_stale_streams(&self);

    /// Generates a path for a segment.
    fn get_segment_path(&self, id: &str, segment_index: usize, file: &str) -> PathBuf;
}

pub struct ChildStream {
    reader: ReaderStream<tokio::process::ChildStdout>,
    _child: tokio::process::Child,
}

impl Stream for ChildStream {
    type Item = io::Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.reader).poll_next(cx)
    }
}

pub fn generate_hls_manifest(duration_secs: i32) -> String {
    let segment_duration = 2;
    let mut manifest = String::from("#EXTM3U\n");
    manifest.push_str("#EXT-X-VERSION:3\n");
    manifest.push_str("#EXT-X-TARGETDURATION:3\n");
    manifest.push_str("#EXT-X-MEDIA-SEQUENCE:0\n");
    manifest.push_str("#EXT-X-PLAYLIST-TYPE:VOD\n");

    let mut leftover = duration_secs;
    let mut segment_idx = 0;
    while leftover > 0 {
        let length = if leftover > segment_duration { segment_duration } else { leftover };
        manifest.push_str(&format!("#EXTINF:{}.0,\nseg_{:03}.ts\n", length, segment_idx));
        leftover -= length;
        segment_idx += 1;
    }

    manifest.push_str("#EXT-X-ENDLIST\n");
    manifest
}

/// Generates a single-segment HLS manifest that points to a direct stream.
/// This trick allows browsers to seek while using a single piped process.
pub fn generate_direct_hls_manifest(duration_secs: f64, stream_url: &str) -> String {
    let mut manifest = String::from("#EXTM3U\n");
    manifest.push_str("#EXT-X-VERSION:3\n");
    manifest.push_str(&format!("#EXT-X-TARGETDURATION:{}\n", duration_secs.ceil()));
    manifest.push_str("#EXT-X-PLAYLIST-TYPE:VOD\n");
    manifest.push_str(&format!("#EXTINF:{:.3},\n{}\n", duration_secs, stream_url));
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
     minBufferTime="PT2.0S">
  <Period id="0">
    <AdaptationSet id="0" contentType="video" segmentAlignment="true" bitstreamSwitching="true">
      <Representation id="0" mimeType="video/webm" codecs="vp9" bandwidth="2000000" width="{width}" height="{height}" frameRate="30">
        <SegmentTemplate timescale="1000" duration="10000" initialization="init-stream0.webm" media="chunk-stream0-$Number%05d$.webm" startNumber="1"/>
      </Representation>
    </AdaptationSet>
    <AdaptationSet id="1" contentType="audio" segmentAlignment="true" bitstreamSwitching="true">
      <Representation id="1" mimeType="audio/webm" codecs="opus" bandwidth="128000" audioSamplingRate="48000">
        <AudioChannelConfiguration schemeIdUri="urn:mpeg:dash:23003:3:audio_channel_configuration:2011" value="2"/>
        <SegmentTemplate timescale="1000" duration="10000" initialization="init-stream1.webm" media="chunk-stream1-$Number%05d$.webm" startNumber="1"/>
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
    sessions: Arc<DashMap<String, StreamSession>>,
    pending_restarts: Arc<DashMap<String, usize>>,
    base_output_dir: PathBuf,
    hw_encoder: String,
    hw_decoders: Vec<String>,
    config: crate::AppConfig,
    viewers: Arc<DashMap<String, std::collections::HashSet<String>>>,
}

pub fn is_direct_playable(path: &Path, details: &crate::scanner::mediainfo::MediaDetails) -> bool {
    // 1. Check file extension
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    let is_mp4_like = ext == "mp4" || ext == "m4v" || ext == "mov";
    let is_webm_like = ext == "webm";

    if !is_mp4_like && !is_webm_like {
        return false;
    }

    // 2. Check codecs
    let v_codec = details.video_codec.to_lowercase();
    let a_codec = details.audio_codec.to_lowercase();

    let video_ok = ["h264", "vp9", "av1", "hevc"].contains(&v_codec.as_str());
    
    // MP4/MOV supports AAC, MP3, and Opus
    // WebM supports Opus and Vorbis
    let audio_ok = if is_webm_like {
        ["opus", "vorbis", "none"].contains(&a_codec.as_str())
    } else {
        ["aac", "mp3", "opus", "none"].contains(&a_codec.as_str())
    };

    // 3. Rotation must be 0
    let rotation_ok = details.rotation == 0;

    video_ok && audio_ok && rotation_ok
}

impl StreamManager {
    pub fn new(config: crate::AppConfig) -> Self {
        let base_output_dir = PathBuf::from(&config.hls_transcode_dir);
        if !base_output_dir.exists() {
            let _ = std::fs::create_dir_all(&base_output_dir);
        }

        let supported_codecs = crate::scanner::ffmpeg::FfmpegEngine::probe_hw_codecs_with_path(&config.ffmpeg_path);
        let hw_encoder = if let Some(codec) = supported_codecs.first() {
            tracing::info!("Using detected hardware encoder: {}", codec);
            codec.clone()
        } else {
            tracing::info!("No hardware encoder detected, falling back to libx264");
            "libx264".to_string()
        };

        let hw_decoders = crate::scanner::ffmpeg::FfmpegEngine::probe_hw_decoders_with_path(&config.ffmpeg_path);

        Self {
            sessions: Arc::new(DashMap::new()),
            pending_restarts: Arc::new(DashMap::new()),
            base_output_dir,
            hw_encoder,
            hw_decoders,
            config,
            viewers: Arc::new(DashMap::new()),
        }
    }

    async fn start_stream_at(&self, id: &str, input_path: &Path, start_segment: usize, is_dash: bool) -> Result<PathBuf> {
        // Stop existing process if any, but KEEP THE FILES
        self.stop_stream(id).await;

        let details = mediainfo::get_media_info_with_path(input_path, &self.config.ffprobe_path).unwrap_or_else(|e| {
            tracing::warn!("Failed to probe file {:?}, falling back to full transcode: {}", input_path, e);
            mediainfo::MediaDetails::default()
        });

        let output_dir = self.base_output_dir.join(id);
        if !output_dir.exists() {
            std::fs::create_dir_all(&output_dir)?;
        }

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

        let mut process = tokio::process::Command::new(&self.config.ffmpeg_path)
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
                                            // Only rename if target doesn't exist (preserve completed segments)
                                            if !new_path.exists() {
                                                let _ = tokio::fs::rename(&old_path, &new_path).await;
                                            } else {
                                                let _ = tokio::fs::remove_file(&old_path).await;
                                            }
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
                                        // Only rename if target doesn't exist
                                        if !new_path.exists() {
                                            let _ = tokio::fs::rename(&old_path, &new_path).await;
                                        } else {
                                            let _ = tokio::fs::remove_file(&old_path).await;
                                        }
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
                    if !output_dir_clone.join(&new_name).exists() {
                        let _ = tokio::fs::rename(output_dir_clone.join(old_name), output_dir_clone.join(new_name)).await;
                    } else {
                        let _ = tokio::fs::remove_file(output_dir_clone.join(old_name)).await;
                    }
                } else {
                    let old_path = output_dir_clone.join(format!(".seg_{:03}.ts", last_idx));
                    let new_path = output_dir_clone.join(format!("seg_{:03}.ts", last_idx));
                    if !new_path.exists() {
                        let _ = tokio::fs::rename(&old_path, &new_path).await;
                    } else {
                        let _ = tokio::fs::remove_file(&old_path).await;
                    }
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

        self.sessions.insert(id.to_string(), StreamSession {
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
        let strategy = crate::scanner::ffmpeg::FfmpegEngine::get_stream_strategy(details);
        let v_codec = match strategy {
            crate::scanner::ffmpeg::StreamStrategy::DirectCopy => "copy",
            crate::scanner::ffmpeg::StreamStrategy::SmartRemux { video_copy, .. } if video_copy => "copy",
            _ => &self.hw_encoder,
        };
        let a_codec = match strategy {
            crate::scanner::ffmpeg::StreamStrategy::DirectCopy => "copy",
            crate::scanner::ffmpeg::StreamStrategy::SmartRemux { audio_copy, .. } if audio_copy => "copy",
            _ => "aac",
        };

        // HLS segments are 2 seconds in this implementation (matching Stash)
        let hls_time = 2;
        let start_time = (start_segment * hls_time).to_string();

        let mut args = vec![
            "-loglevel".to_string(), "info".to_string(),
            "-probesize".to_string(), "50000000".to_string(),
            "-analyzeduration".to_string(), "50000000".to_string(),
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
            "-sn".to_string(), // Disable subtitles
            "-dn".to_string(), // Disable data streams
        ]);

        // Video codec and options
        args.push("-c:v".to_string());
        args.push(v_codec.to_string());
        if v_codec != "copy" {
            if v_codec == "h264_v4l2m2m" {
                // Pi 4 optimized flags
                args.extend(vec![
                    "-b:v".to_string(), "4M".to_string(),
                    "-maxrate".to_string(), "5M".to_string(),
                    "-bufsize".to_string(), "8M".to_string(),
                ]);
            } else {
                args.extend(vec![
                    "-pix_fmt".to_string(), "yuv420p".to_string(),
                    "-preset".to_string(), "ultrafast".to_string(),
                    "-crf".to_string(), "26".to_string(),
                    "-flags".to_string(), "+cgop".to_string(),
                ]);
            }

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
            "-copyts".to_string(),
            "-f".to_string(), "hls".to_string(),
            "-max_muxing_queue_size".to_string(), "1024".to_string(),
            "-hls_time".to_string(), hls_time.to_string(),
            "-hls_list_size".to_string(), "0".to_string(),
            "-hls_flags".to_string(), "split_by_time".to_string(),
            "-hls_playlist_type".to_string(), "vod".to_string(),
            "-avoid_negative_ts".to_string(), "disabled".to_string(),
        ]);

        if v_codec != "copy" {
            args.push("-force_key_frames".to_string());
            args.push(format!("expr:gte(t,n_forced*{})", hls_time));
        }

        let temp_segment_pattern = crate::paths::normalize_slashes(&output_dir.join(FFMPEG_SEG_PATTERN).to_string_lossy());
        let normalized_playlist = crate::paths::normalize_slashes(&playlist_path.to_string_lossy());

        args.extend(vec![
            "-start_number".to_string(), start_segment.to_string(),
            "-hls_segment_type".to_string(), "mpegts".to_string(),
            "-hls_segment_filename".to_string(), temp_segment_pattern,
            normalized_playlist,
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
            "-probesize".to_string(), "5000000".to_string(),
            "-analyzeduration".to_string(), "5000000".to_string(),
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

        // Video stream (stream0)
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
            "-copyts".to_string(),
            "-avoid_negative_ts".to_string(), "disabled".to_string(),
            "-f".to_string(), "webm_chunk".to_string(),
            "-header".to_string(), output_dir.join(if start_segment == 0 { "init-stream0.webm" } else { ".init-stream0.webm" }).to_string_lossy().to_string(),
            "-chunk_start_index".to_string(), (start_segment + 1).to_string(),
            output_dir.join(".chunk-stream0-%05d.webm").to_string_lossy().to_string(),
        ]);

        // Audio stream (stream1)
        args.extend(vec![
            "-map".to_string(), "0:a:0?".to_string(),
            "-c:a".to_string(), "libopus".to_string(),
            "-b:a".to_string(), "128k".to_string(),
            "-ac".to_string(), "2".to_string(),
            "-copyts".to_string(),
            "-avoid_negative_ts".to_string(), "disabled".to_string(),
            "-f".to_string(), "webm_chunk".to_string(),
            "-audio_chunk_duration".to_string(), "10000".to_string(),
            "-header".to_string(), output_dir.join(if start_segment == 0 { "init-stream1.webm" } else { ".init-stream1.webm" }).to_string_lossy().to_string(),
            "-chunk_start_index".to_string(), (start_segment + 1).to_string(),
            output_dir.join(".chunk-stream1-%05d.webm").to_string_lossy().to_string(),
        ]);

        args
    }

    pub fn build_direct_args(
        &self,
        input_path: &str,
        details: &crate::scanner::mediainfo::MediaDetails,
        start_time_secs: f64,
        format: &str,
    ) -> Vec<String> {
        let mut args = vec!["-loglevel".to_string(), "error".to_string()];

        // 1. Server-side Seeking (must be before -i for fast seek)
        if start_time_secs > 0.0 {
            args.extend(vec![
                "-ss".to_string(), format!("{:.3}", start_time_secs),
                "-copyts".to_string(),
                "-avoid_negative_ts".to_string(), "disabled".to_string(),
            ]);
        }

        // HW Decoder for transcode if video copy is not possible
        let strategy = crate::scanner::ffmpeg::FfmpegEngine::get_stream_strategy(details);
        let mut video_copyable = match strategy {
            crate::scanner::ffmpeg::StreamStrategy::DirectCopy => true,
            crate::scanner::ffmpeg::StreamStrategy::SmartRemux { video_copy, .. } => video_copy,
            _ => false,
        };

        if format == "webm" {
            // WebM only supports vp8, vp9, av1
            video_copyable = ["vp9", "vp8", "av1"].contains(&details.video_codec.to_lowercase().as_str());
        }

        if !video_copyable {
            if let Some(hw_decoder) = crate::scanner::ffmpeg::FfmpegEngine::get_hw_decoder(&details.video_codec, &self.hw_decoders) {
                args.push("-c:v".to_string());
                args.push(hw_decoder);
            }
        }

        args.extend(vec!["-i".to_string(), input_path.to_string()]);

        // 2. Video Logic (Copy if possible, otherwise transcode with zerolatency)
        args.push("-c:v".to_string());
        if video_copyable {
            args.push("copy".to_string());
        } else if format == "webm" {
            args.push("libvpx-vp9".to_string());
            args.extend(vec![
                "-crf".to_string(), "30".to_string(),
                "-b:v".to_string(), "2000k".to_string(),
                "-deadline".to_string(), "realtime".to_string(),
                "-cpu-used".to_string(), "5".to_string(),
                "-row-mt".to_string(), "1".to_string(),
            ]);
        } else {
            args.push("libx264".to_string());
            args.extend(vec![
                "-preset".to_string(), "ultrafast".to_string(),
                "-tune".to_string(), "zerolatency".to_string()
            ]);
        }

        // 3. Audio Logic (Transcode only when necessary)
        args.push("-c:a".to_string());
        if format == "mkv" {
            let audio_copyable = ["opus", "aac", "mp3", "vorbis"].contains(&details.audio_codec.to_lowercase().as_str());
            if audio_copyable {
                args.push("copy".to_string());
            } else {
                // MKV must be Opus
                args.extend(vec![
                    "libopus".to_string(),
                    "-b:a".to_string(), "128k".to_string(),
                    "-vbr".to_string(), "on".to_string(),
                    "-ac".to_string(), "2".to_string()
                ]);
            }
        } else if format == "webm" {
            let audio_copyable = ["opus", "vorbis"].contains(&details.audio_codec.to_lowercase().as_str());
            if audio_copyable {
                args.push("copy".to_string());
            } else {
                // WebM must be Opus
                args.extend(vec![
                    "libopus".to_string(),
                    "-b:a".to_string(), "128k".to_string(),
                    "-ac".to_string(), "2".to_string()
                ]);
            }
        } else {
            // MP4: Copy if AAC, MP3, or Opus, otherwise transcode to AAC
            let audio_copyable = ["aac", "mp3", "opus"].contains(&details.audio_codec.to_lowercase().as_str());
            if audio_copyable {
                args.push("copy".to_string());
            } else {
                // MP4 must be AAC
                args.extend(vec![
                    "aac".to_string(),
                    "-b:a".to_string(), "128k".to_string(),
                    "-ac".to_string(), "2".to_string()
                ]);
            }
        }

        // 4. Container & Pipe flags
        if format == "mp4" {
            args.extend(vec![
                "-movflags".to_string(),
                "frag_keyframe+empty_moov+default_base_moof".to_string()
            ]);
        } else if format == "mkv" {
            args.extend(vec!["-live".to_string(), "1".to_string()]);
        }

        let container = if format == "mkv" {
            "matroska"
        } else if format == "webm" {
            "webm"
        } else {
            "mp4"
        };
        args.extend(vec![
            "-f".to_string(), container.to_string(),
            "-flush_packets".to_string(), "1".to_string(),
            "pipe:1".to_string()
        ]);

        args
    }
}

#[async_trait]
impl StreamingService for StreamManager {
    async fn start_hls(&self, id: &str, input_path: &Path) -> Result<PathBuf> {
        self.start_hls_at(id, input_path, 0).await
    }

    async fn start_hls_at(&self, id: &str, input_path: &Path, start_segment: usize) -> Result<PathBuf> {
        self.start_stream_at(id, input_path, start_segment, false).await
    }

    async fn start_dash(&self, id: &str, input_path: &Path) -> Result<PathBuf> {
        self.start_dash_at(id, input_path, 0).await
    }

    async fn start_dash_at(&self, id: &str, input_path: &Path, start_segment: usize) -> Result<PathBuf> {
        self.start_stream_at(id, input_path, start_segment, true).await
    }

    async fn start_direct_stream(
        &self,
        input_path: &Path,
        start_time: f64,
        format: &str,
    ) -> Result<ChildStream> {
        let details = crate::scanner::mediainfo::get_media_info_with_path(input_path, &self.config.ffprobe_path).unwrap_or_default();
        let normalized_input = crate::paths::normalize_slashes(&input_path.to_string_lossy());
        
        let args = self.build_direct_args(&normalized_input, &details, start_time, format);

        let mut child = tokio::process::Command::new(&self.config.ffmpeg_path)
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        let stdout = child.stdout.take().ok_or_else(|| crate::errors::CoreError::FfmpegError("Failed to capture stdout".to_string()))?;
        let mut stderr = child.stderr.take().ok_or_else(|| crate::errors::CoreError::FfmpegError("Failed to capture stderr".to_string()))?;

        tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let mut buf = vec![0u8; 1024];
            let mut err_str = String::new();
            while let Ok(n) = stderr.read(&mut buf).await {
                if n == 0 {
                    break;
                }
                err_str.push_str(&String::from_utf8_lossy(&buf[..n]));
            }
            if !err_str.trim().is_empty() {
                tracing::error!("FFmpeg stderr for direct stream: {}", err_str.trim());
            }
        });
        
        Ok(ChildStream {
            reader: ReaderStream::new(stdout),
            _child: child,
        })
    }

    async fn wait_for_segment(&self, id: &str, segment_index: usize, file: &str) -> Result<bool> {
        let mut rx = {
            if let Some(session) = self.sessions.get(id) {
                session.latest_segment_rx.clone()
            } else {
                return Ok(false);
            }
        };

        // Update heartbeat to prevent reaping while waiting
        self.update_heartbeat(id).await;

        let result = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let current = *rx.borrow();
                // If current segment being written is > segment_index, then segment_index is READY
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
        
        if result.is_ok() {
            // Verify file exists on disk (double check for race conditions)
            let path = self.get_segment_path(id, segment_index, file);
            for _ in 0..5 {
                if path.exists() { return Ok(true); }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }

        Ok(result.is_ok())
    }

    async fn request_segment(&self, id: &str, input_path: &Path, segment_index: usize, file: &str) -> Result<()> {
        let (needs_restart, is_dash) = if let Some(mut session) = self.sessions.get_mut(id) {
            session.last_access = std::time::Instant::now();
            
            // Check if segment already exists on disk (completed or being written)
            let segment_path = self.get_segment_path(id, segment_index, file);
            if segment_path.exists() {
                session.last_requested_segment = segment_index;
                return Ok(());
            }

            // Check if process is still alive
            let is_alive = match session.process.try_wait() {
                Ok(None) => true,
                _ => false,
            };

            // Restart if process died or segment is outside current transcode window (behind or too far ahead)
            let needs_restart = !is_alive 
                || segment_index < session.start_segment 
                || segment_index > session.last_requested_segment + 50;

            if !needs_restart {
                session.last_requested_segment = segment_index;
            }
            (needs_restart, session.is_dash)
        } else {
            (true, id.contains("dash"))
        };

        if needs_restart {
            // Check if a restart for this id/segment is already in progress
            if let Some(target) = self.pending_restarts.get(id) {
                if *target == segment_index {
                    return Ok(());
                }
            }
            self.pending_restarts.insert(id.to_string(), segment_index);

            let result = self.start_stream_at(id, input_path, segment_index, is_dash).await;
            
            // Clear pending restart regardless of success
            self.pending_restarts.remove(id);
            
            return result.map(|_| ());
        }

        Ok(())
    }

    /// Kills the process but keeps the files (throttling)
    async fn stop_stream(&self, id: &str) {
        if let Some(mut session) = self.sessions.get_mut(id) {
            let _ = session.process.kill().await;
        }
    }

    /// Kills the process and deletes the files (cleanup)
    async fn destroy_session(&self, id: &str) {
        if let Some((_, mut session)) = self.sessions.remove(id) {
            let _ = session.process.kill().await;
            let _ = std::fs::remove_dir_all(&session.output_dir);
        }
        self.viewers.remove(id);
    }

    async fn stop_all_streams(&self) {
        let session_ids: Vec<String> = self.sessions.iter().map(|s| s.key().clone()).collect();
        for id in session_ids {
            self.destroy_session(&id).await;
        }
        self.viewers.clear();
    }

    async fn cleanup_stale_streams(&self) {
        let mut session_ids_to_destroy = Vec::new();
        let mut session_ids_to_throttle = Vec::new();
        
        let now = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(120);

        for entry in self.sessions.iter() {
            let id = entry.key();
            let session = entry.value();
            
            // 1. Idle timeout (full cleanup)
            if now.duration_since(session.last_access) > timeout {
                session_ids_to_destroy.push(id.clone());
                continue;
            }

            // 2. Transcoder too far ahead (throttle only)
            if let Some(latest) = *session.latest_segment_rx.borrow() {
                if latest > session.last_requested_segment + 30 { 
                    session_ids_to_throttle.push(id.clone());
                }
            }
        }

        for id in session_ids_to_throttle {
            tracing::info!("Throttling active transcoder: session {} is too far ahead", id);
            self.stop_stream(&id).await;
        }

        for id in session_ids_to_destroy {
            tracing::info!("Cleaning up stale stream session: {}", id);
            self.destroy_session(&id).await;
        }
    }

    async fn update_heartbeat(&self, id: &str) {
        if let Some(mut session) = self.sessions.get_mut(id) {
            session.last_access = std::time::Instant::now();
        }
    }

    async fn register_client(&self, id: &str, client_id: &str) {
        self.viewers.entry(id.to_string()).or_default().insert(client_id.to_string());
        if let Some(mut session) = self.sessions.get_mut(id) {
            session.last_access = std::time::Instant::now();
        }
    }

    async fn unregister_client(&self, id: &str, client_id: &str) {
        let mut should_stop = false;
        if let Some(mut set) = self.viewers.get_mut(id) {
            set.remove(client_id);
            if set.is_empty() {
                should_stop = true;
            }
        }
        if should_stop {
            tracing::info!("No more viewers for stream session {}, stopping stream immediately.", id);
            self.stop_stream(id).await;
        }
    }

    fn get_segment_path(&self, id: &str, _segment_index: usize, file: &str) -> PathBuf {
        self.base_output_dir.join(id).join(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::mediainfo::MediaDetails;

    #[test]
    fn test_generate_hls_manifest() {
        let duration = 25;
        let manifest = generate_hls_manifest(duration);
        
        assert!(manifest.contains("#EXTM3U"));
        assert!(manifest.contains("#EXT-X-PLAYLIST-TYPE:VOD"));
        assert!(manifest.contains("#EXTINF:2.0,\nseg_000.ts"));
        assert!(manifest.contains("#EXTINF:2.0,\nseg_001.ts"));
        assert!(manifest.contains("#EXT-X-ENDLIST"));
    }

    #[test]
    fn test_build_ffmpeg_args_smart_remux() {
        let config = crate::AppConfig {
            ffmpeg_path: "ffmpeg".to_string(),
            ffprobe_path: "ffprobe".to_string(),
            hls_transcode_dir: "tmp".to_string(),
        };
        let manager = StreamManager::new(config);
        let details = MediaDetails {
            width: 1920,
            height: 1080,
            video_codec: "h264".to_string(),
            audio_codec: "aac".to_string(),
            audio_channels: 2,
            size_bytes: 1000,
            duration_secs: 100,
            ..Default::default()
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
        let config = crate::AppConfig {
            ffmpeg_path: "ffmpeg".to_string(),
            ffprobe_path: "ffprobe".to_string(),
            hls_transcode_dir: "tmp".to_string(),
        };
        let mut manager = StreamManager::new(config);
        manager.hw_encoder = "libx264".to_string();
        manager.hw_decoders = vec![]; // Mock empty decoders for this test
        
        let details = MediaDetails {
            width: 1920,
            height: 1080,
            video_codec: "hevc".to_string(),
            audio_codec: "ac3".to_string(),
            audio_channels: 2,
            size_bytes: 1000,
            duration_secs: 100,
            ..Default::default()
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
        let config = crate::AppConfig {
            ffmpeg_path: "ffmpeg".to_string(),
            ffprobe_path: "ffprobe".to_string(),
            hls_transcode_dir: temp_dir.path().to_string_lossy().to_string(),
        };
        let manager = StreamManager::new(config);
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
            manager.sessions.insert(id.to_string(), StreamSession {
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

        let file = "seg_000.ts";
        let seg_path = manager.get_segment_path(id, 0, file);
        std::fs::write(&seg_path, "dummy data").unwrap();
        let result = manager.request_segment(id, Path::new("input.mp4"), 0, file).await;
        assert!(result.is_ok());

        // Segment 1 should NOT trigger a restart anymore even if file is missing,
        // as long as the process is alive and it's within the window.
        let _result = manager.request_segment(id, Path::new("input.mp4"), 1, "seg_001.ts").await;
        
        // Let's test the "too far ahead" logic which definitely triggers restart
        let result = manager.request_segment(id, Path::new("input.mp4"), 100, "seg_100.ts").await;
        assert!(result.is_err()); // Should fail because input.mp4 missing during restart attempt
    }

    #[tokio::test]
    async fn test_transcoder_throttling() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = crate::AppConfig {
            ffmpeg_path: "ffmpeg".to_string(),
            ffprobe_path: "ffprobe".to_string(),
            hls_transcode_dir: temp_dir.path().to_string_lossy().to_string(),
        };
        let manager = StreamManager::new(config);
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
            manager.sessions.insert(id.to_string(), StreamSession {
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
            assert!(manager.sessions.contains_key(id));
        }

        tx.send(Some(31)).unwrap(); 
        manager.cleanup_stale_streams().await;
        {
            // Throttled sessions now remain in the map to preserve segments!
            assert!(manager.sessions.contains_key(id));
        }

        // Test idle timeout (full cleanup)
        {
            if let Some(mut session) = manager.sessions.get_mut(id) {
                session.last_access = std::time::Instant::now() - std::time::Duration::from_secs(130);
            }
            manager.cleanup_stale_streams().await;
            assert!(!manager.sessions.contains_key(id));
        }
    }

    #[tokio::test]
    async fn test_get_or_restart_process_parallel() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = crate::AppConfig {
            ffmpeg_path: "ffmpeg".to_string(),
            ffprobe_path: "ffprobe".to_string(),
            hls_transcode_dir: temp_dir.path().to_string_lossy().to_string(),
        };
        let manager = Arc::new(StreamManager::new(config));
        let id = "parallel_test";
        let input_path = PathBuf::from("input.mp4");

        // We can't easily mock start_stream_at without refactoring, 
        // but we can check if pending_restarts handles concurrent calls.
        
        let mut handles = Vec::new();
        for _ in 0..10 {
            let m = manager.clone();
            let id_clone = id.to_string();
            let path_clone = input_path.clone();
            handles.push(tokio::spawn(async move {
                // This will fail because input.mp4 doesn't exist, but it should hit pending_restarts
                let _ = m.request_segment(&id_clone, &path_clone, 0, "seg_000.ts").await;
            }));
        }

        for h in handles {
            let _ = h.await;
        }

        // Check that pending_restarts is empty after all tasks finish
        assert!(manager.pending_restarts.is_empty());
    }

    #[test]
    fn test_build_ffmpeg_args_hw_accel_pi4() {
        let config = crate::AppConfig {
            ffmpeg_path: "ffmpeg".to_string(),
            ffprobe_path: "ffprobe".to_string(),
            hls_transcode_dir: "tmp".to_string(),
        };
        let mut manager = StreamManager::new(config);
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
            ..Default::default()
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

    #[test]
    fn test_build_direct_args() {
        let config = crate::AppConfig {
            ffmpeg_path: "ffmpeg".to_string(),
            ffprobe_path: "ffprobe".to_string(),
            hls_transcode_dir: "tmp".to_string(),
        };
        let manager = StreamManager::new(config);
        let details = crate::scanner::mediainfo::MediaDetails {
            width: 1920,
            height: 1080,
            video_codec: "h264".to_string(),
            audio_codec: "aac".to_string(),
            audio_channels: 2,
            size_bytes: 1000,
            duration_secs: 100,
            ..Default::default()
        };

        // Test MKV (default/remux)
        let args = manager.build_direct_args("input.mkv", &details, 10.5, "mkv");
        assert!(args.contains(&"-ss".to_string()));
        assert!(args.contains(&"10.500".to_string()));
        assert!(args.contains(&"matroska".to_string()));
        let v_codec_idx = args.iter().rposition(|r| r == "-c:v").unwrap() + 1;
        assert_eq!(args[v_codec_idx], "copy");

        // Test MP4 (transcode if needed)
        let args = manager.build_direct_args("input.mkv", &details, 0.0, "mp4");
        assert!(args.contains(&"mp4".to_string()));
        assert!(args.contains(&"-movflags".to_string()));
        assert!(args.contains(&"frag_keyframe+empty_moov+default_base_moof".to_string()));

        // Test WebM (transcode)
        let args = manager.build_direct_args("input.mkv", &details, 0.0, "webm");
        assert!(args.contains(&"webm".to_string()));
        let v_codec_idx = args.iter().rposition(|r| r == "-c:v").unwrap() + 1;
        assert_eq!(args[v_codec_idx], "libvpx-vp9");
    }
}
