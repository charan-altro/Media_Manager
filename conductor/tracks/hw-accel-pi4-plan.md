# Task 3: Full-Path Hardware Acceleration (Pi 4 Optimized) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Offload video decoding and scaling to the GPU on Raspberry Pi 4 to save CPU cycles during HLS transcoding.

**Architecture:** Extend `FfmpegEngine` to detect hardware decoders and map them to input codecs. Update `StreamManager` to inject these decoders into the FFmpeg command line before the input flag and use hardware-optimized scaling filters.

**Tech Stack:** Rust, FFmpeg, V4L2M2M (Pi 4 HW Accel)

---

### Task 1: Extend `FfmpegEngine` with Hardware Decoder Support

**Files:**
- Modify: `media_core/src/scanner/ffmpeg.rs`

- [ ] **Step 1: Add `probe_hw_decoders` to `FfmpegEngine`**

```rust
    pub fn probe_hw_decoders() -> Vec<String> {
        let mut supported = Vec::new();
        let decoders_to_test = ["h264_v4l2m2m", "hevc_v4l2m2m", "h264_cuvid", "hevc_cuvid", "h264_qsv", "hevc_qsv"];
        
        for decoder in decoders_to_test {
            let output = Command::new(crate::config::get_ffmpeg_path())
                .args(&[
                    "-v", "error",
                    "-decoders"
                ])
                .output();
                
            if let Ok(out) = output {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if stdout.contains(decoder) {
                    supported.push(decoder.to_string());
                }
            }
        }
        supported
    }
```

- [ ] **Step 2: Add `get_hw_decoder` to `FfmpegEngine`**

```rust
    pub fn get_hw_decoder(source_codec: &str, supported_decoders: &[String]) -> Option<String> {
        match source_codec {
            "h264" => {
                if supported_decoders.contains(&"h264_v4l2m2m".to_string()) {
                    Some("h264_v4l2m2m".to_string())
                } else if supported_decoders.contains(&"h264_cuvid".to_string()) {
                    Some("h264_cuvid".to_string())
                } else {
                    None
                }
            },
            "hevc" => {
                if supported_decoders.contains(&"hevc_v4l2m2m".to_string()) {
                    Some("hevc_v4l2m2m".to_string())
                } else if supported_decoders.contains(&"hevc_cuvid".to_string()) {
                    Some("hevc_cuvid".to_string())
                } else {
                    None
                }
            },
            _ => None
        }
    }
```

- [ ] **Step 3: Commit**

### Task 2: Update `StreamManager` to support Hardware Decoders

**Files:**
- Modify: `media_core/src/scanner/streaming.rs`

- [ ] **Step 1: Add `hw_decoders` field to `StreamManager`**

```rust
pub struct StreamManager {
    sessions: Arc<TokioMutex<HashMap<String, StreamSession>>>,
    pending_restarts: Arc<TokioMutex<HashMap<String, usize>>>,
    base_output_dir: PathBuf,
    hw_encoder: String,
    hw_decoders: Vec<String>, // NEW
}
```

- [ ] **Step 2: Initialize `hw_decoders` in `StreamManager::new`**

```rust
    pub fn new(base_output_dir: PathBuf) -> Self {
        // ...
        let hw_decoders = crate::scanner::ffmpeg::FfmpegEngine::probe_hw_decoders();
        // ...
        Self {
            sessions: Arc::new(TokioMutex::new(HashMap::new())),
            pending_restarts: Arc::new(TokioMutex::new(HashMap::new())),
            base_output_dir,
            hw_encoder,
            hw_decoders,
        }
    }
```

- [ ] **Step 3: Commit**

### Task 3: Implement HW-Accelerated Decoding and Scaling in `build_ffmpeg_args`

**Files:**
- Modify: `media_core/src/scanner/streaming.rs`

- [ ] **Step 1: Update `build_ffmpeg_args` to use HW decoder and scaling**

```rust
    fn build_ffmpeg_args(
        &self,
        input_path: &str,
        details: &mediainfo::MediaDetails,
        start_segment: usize,
        playlist_path: &Path,
        output_dir: &Path,
    ) -> Vec<String> {
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

        // ... scaling logic ...
        if v_codec != "copy" {
            // Check if we need scaling (e.g. if we want to force 720p or something)
            // For now let's assume if we are on Pi 4 and transcoding, we might want to use scale_v4l2m2m if we added scaling
            // The task says: "If scaling is needed (e.g., transcoding to 720p), use hardware-aware filters like -vf scale_v4l2m2m=1280:720"
            // We don't have a specific target resolution in MediaDetails yet, but we can add a check.
        }
        // ...
```

- [ ] **Step 2: Commit**

### Task 4: Verification and Unit Tests

**Files:**
- Modify: `media_core/src/scanner/streaming.rs` (tests)

- [ ] **Step 1: Add test for HW Decoder placement**
- [ ] **Step 2: Add test for HW Scaling (if applicable)**
- [ ] **Step 3: Run all tests**
- [ ] **Step 4: Commit**
