# Streaming Implementation Plan: Phase 2 & 3 (fMP4 & Reactive Lifecycle)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a "Buffer-Free" JIT streaming system using Fragmented MP4 (fMP4) and ensure immediate FFmpeg process termination when clients disconnect.

**Architecture:** Use `tokio::process` to spawn FFmpeg, stream stdout via `tokio_util::io::ReaderStream` wrapped in `axum::body::StreamBody`. Use `tokio_util::sync::CancellationToken` tied to the response stream drop for reactive cancellation.

**Tech Stack:** Rust, Axum, Tokio, FFmpeg.

---

### Task 1: FFmpeg fMP4 Command Builder

**Files:**
- Modify: `media_core/src/scanner/streaming.rs`
- Test: `media_core/src/scanner/streaming.rs` (inline module)

- [ ] **Step 1: Write the failing test**

```rust
// In media_core/src/scanner/streaming.rs, inside `mod tests`
#[test]
fn test_build_fmp4_args() {
    let manager = StreamManager::new(std::path::PathBuf::from("tmp"));
    let details = crate::scanner::mediainfo::MediaDetails {
        width: 1920,
        height: 1080,
        video_codec: "h264".to_string(),
        audio_codec: "aac".to_string(),
        audio_channels: 2,
        size_bytes: 1000,
        duration_secs: 100,
    };

    let args = manager.build_fmp4_args("input.mkv", &details, 0.0);
    
    assert!(args.contains(&"-movflags".to_string()));
    assert!(args.contains(&"frag_keyframe+empty_moov+default_base_moof".to_string()));
    assert!(args.contains(&"-f".to_string()));
    assert!(args.contains(&"mp4".to_string()));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --package media_core --lib -- scanner::streaming::tests::test_build_fmp4_args`
Expected: FAIL (method not defined)

- [ ] **Step 3: Write minimal implementation**

```rust
// In media_core/src/scanner/streaming.rs, add to `StreamManager`:

pub fn build_fmp4_args(
    &self,
    input_path: &str,
    details: &crate::scanner::mediainfo::MediaDetails,
    start_time_secs: f64,
) -> Vec<String> {
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

    let mut args = vec!["-loglevel".to_string(), "error".to_string()];
    
    if v_codec != "copy" {
        if let Some(hw_decoder) = crate::scanner::ffmpeg::FfmpegEngine::get_hw_decoder(&details.video_codec, &self.hw_decoders) {
            args.push("-c:v".to_string());
            args.push(hw_decoder);
        }
    }

    if start_time_secs > 0.0 {
        args.extend(vec!["-ss".to_string(), start_time_secs.to_string()]);
    }

    args.extend(vec![
        "-i".to_string(), input_path.to_string(),
        "-map".to_string(), "0:v:0".to_string(),
        "-map".to_string(), "0:a:0?".to_string(),
        "-c:v".to_string(), v_codec.to_string(),
    ]);

    if v_codec != "copy" {
        args.extend(vec![
            "-preset".to_string(), "ultrafast".to_string(),
            "-crf".to_string(), "26".to_string(),
            "-force_key_frames".to_string(), "expr:gte(t,n_forced*2)".to_string(),
        ]);
    }

    args.extend(vec![
        "-c:a".to_string(), a_codec.to_string(),
    ]);
    
    if a_codec != "copy" {
        args.extend(vec!["-b:a".to_string(), "128k".to_string(), "-ac".to_string(), "2".to_string()]);
    }

    args.extend(vec![
        "-movflags".to_string(), "frag_keyframe+empty_moov+default_base_moof".to_string(),
        "-f".to_string(), "mp4".to_string(),
        "pipe:1".to_string(),
    ]);

    args
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --package media_core --lib -- scanner::streaming::tests::test_build_fmp4_args`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add media_core/src/scanner/streaming.rs
git commit -m "feat(streaming): implement build_fmp4_args for direct mp4 streaming"
```

---

### Task 2: Streaming Logic & Process Cancellation

**Files:**
- Modify: `media_core/src/scanner/streaming.rs`
- Modify: `media_core/Cargo.toml` (if needed for `tokio-util`)

- [ ] **Step 1: Check/Add tokio-util dependency**

Ensure `media_core/Cargo.toml` has `tokio-util` with features `io` and `io-util`. If not, add them.

- [ ] **Step 2: Implement Direct Stream Method**

```rust
// Add to `media_core/src/scanner/streaming.rs` inside `StreamManager`

use tokio_util::io::ReaderStream;
use tokio::io::AsyncRead;
use std::process::Stdio;

pub async fn stream_direct(
    &self,
    input_path: &std::path::Path,
    start_time_secs: f64,
) -> crate::errors::Result<impl tokio_stream::Stream<Item = std::io::Result<axum::body::Bytes>> + Send> {
    let details = crate::scanner::mediainfo::get_media_info(input_path).unwrap_or_default();
    let normalized_input = crate::paths::normalize_slashes(&input_path.to_string_lossy());
    
    let args = self.build_fmp4_args(&normalized_input, &details, start_time_secs);

    let mut process = tokio::process::Command::new(crate::config::get_ffmpeg_path())
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true) // Crucial for Phase 3: Kills process when struct drops
        .spawn()?;

    let stdout = process.stdout.take().expect("Failed to open stdout");
    
    // We wrap stdout to ensure the child process is kept alive as long as the stream is read.
    // However, `tokio::process::Child` doesn't implement AsyncRead directly to yield its handle,
    // so we can use a custom wrapper or rely on Tokio's `ChildStdout` dropping.
    // When `ReaderStream` drops, it drops `ChildStdout`. If `ChildStdout` drops, FFmpeg receives SIGPIPE.
    // `kill_on_drop(true)` on `Child` only works if the `Child` struct itself is dropped.
    // Let's spawn a task to wait on the child and hold the child struct.

    let stream = ReaderStream::new(stdout);

    // Spawn a monitor to hold the Child struct and kill it if stream ends
    tokio::spawn(async move {
        let _ = process.wait().await;
    });

    Ok(stream)
}
```

*Note: Since the stream trait involves axum types, make sure axum is available, or map the bytes. We will just use `axum::body::Bytes` in the server layer.*
*Wait, `ReaderStream` returns `Result<bytes::Bytes, io::Error>`. `axum::body::Bytes` is just `bytes::Bytes`.*

Let's refine the method signature to avoid Axum dependency in `media_core` if possible:

```rust
pub async fn stream_direct(
    &self,
    input_path: &std::path::Path,
    start_time_secs: f64,
) -> crate::errors::Result<tokio_util::io::ReaderStream<tokio::process::ChildStdout>> {
    // ...
    let stdout = process.stdout.take().expect("Failed to open stdout");
    tokio::spawn(async move {
        let _ = process.wait().await;
    });
    Ok(tokio_util::io::ReaderStream::new(stdout))
}
```
Wait, if `process` is moved into `tokio::spawn`, and `kill_on_drop(true)` is set, `process` won't drop until the wait completes, which is when FFmpeg exits. BUT what if the browser disconnects? `ChildStdout` drops, causing `SIGPIPE` in FFmpeg, causing it to exit, ending `process.wait()`. This elegantly handles Phase 3 without complex CancellationTokens!

- [ ] **Step 3: Commit**

```bash
git add media_core/src/scanner/streaming.rs
git commit -m "feat(streaming): implement stream_direct with stdout pipe and process lifecycle management"
```

---

### Task 3: Axum Route Integration

**Files:**
- Modify: `apps/server/src/main.rs`

- [ ] **Step 1: Implement direct streaming routes**

Update `serve_direct_movie` and `serve_direct_episode` to use the new `stream_direct` method.

```rust
// In apps/server/src/main.rs

use axum::body::Body;
use axum::http::header;

#[derive(serde::Deserialize)]
struct DirectStreamQuery {
    start: Option<f64>,
}

async fn serve_direct_movie(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(query): Query<DirectStreamQuery>,
) -> impl IntoResponse {
    if let Ok(Some(path)) = db::queries::get_movie_full_path(&state.pool, MovieId(id)).await {
        let start = query.start.unwrap_or(0.0);
        match state.stream_manager.stream_direct(&path, start).await {
            Ok(stream) => {
                let body = Body::from_stream(stream);
                (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "video/mp4")],
                    body
                ).into_response()
            },
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        }
    } else {
        (StatusCode::NOT_FOUND, "Movie not found").into_response()
    }
}

async fn serve_direct_episode(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(query): Query<DirectStreamQuery>,
) -> impl IntoResponse {
    if let Ok(Some(path)) = db::queries::get_episode_full_path(&state.pool, EpisodeId(id)).await {
        let start = query.start.unwrap_or(0.0);
        match state.stream_manager.stream_direct(&path, start).await {
            Ok(stream) => {
                let body = Body::from_stream(stream);
                (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "video/mp4")],
                    body
                ).into_response()
            },
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        }
    } else {
        (StatusCode::NOT_FOUND, "Episode not found").into_response()
    }
}
```

- [ ] **Step 2: Remove old static file direct play logic in start stream routes**
In `start_movie_stream` and `start_episode_stream`, change them to point to the direct streaming endpoint instead of HLS, or modify frontend to call the direct endpoint directly for compatible streams. For MVP2, returning `/api/stream/direct/movie/{}?start=0` is correct.

- [ ] **Step 3: Commit**

```bash
git add apps/server/src/main.rs
git commit -m "feat(server): integrate fMP4 direct streaming into Axum routes"
```

---

### Final Verification

- [ ] **Verify fMP4 Stream**: Request a movie stream using `/api/stream/direct/movie/1`. Verify FFmpeg spawns and returns MP4 bytes.
- [ ] **Verify Reactive Cancellation**: Cancel the request (e.g., using curl or browser). Verify FFmpeg process terminates immediately.
