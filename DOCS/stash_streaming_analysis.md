# Stash Streaming Architecture Analysis & Benchmarking

## Overview
This document analyzes the high-performance video delivery techniques used by Stash (Go) and provides a blueprint for implementing similar "buffer-free" capabilities in the Rust-based Media Manager.

## 1. Stash's Core "Buffer-Free" Techniques

### A. Fragmented MP4 (fMP4) Pipeline
Stash avoids the "header-wait" problem of standard MP4s by using fragmented MP4.
*   **Technique:** Emits small, self-contained fragments that the browser can decode immediately.
*   **FFmpeg Flags:** `-movflags frag_keyframe+empty_moov+default_base_moof`
*   **Impact:** Reduces "Time to First Frame" (TTFF) by up to 80%.

### B. Reactive Process Management (The "Ghost" Buffer)
Stash manages a sliding window of segments and kills idle processes.
*   **Algorithm:** Generates ~15 segments ahead of the user (`maxSegmentBuffer`).
*   **Seek Handling:** Immediately terminates the current FFmpeg process on seek and spawns a new one at the exact `-ss` timestamp.
*   **Idle Cleanup:** Kills processes after 30 seconds of inactivity to save CPU.

### C. Just-In-Time (JIT) Smart Remuxing
Stash performs a codec-audit before deciding how to stream.
*   **Direct:** Byte-copy for compatible codecs (H.264/AAC).
*   **Smart Remux:** `-c:v copy -c:a aac` (Video copy, Audio transcode) for container mismatches (MKV -> MP4).
*   **Transcode:** Full re-encode only as a last resort using `-preset veryfast`.

---

## 2. Media_Manager Implementation Blueprint (Rust)

### A. Async Streaming with Tokio
Leverage `tokio::process` and `tokio_util::io::ReaderStream` for non-blocking I/O.

### B. Reactive Body Stream
Use `axum::body::StreamBody` (or equivalent) to pipe FFmpeg's `stdout` directly to the network socket. This ensures that if the user closes the tab, the Rust `Drop` implementation can catch the signal and kill the child process.

### C. Keyframe-Aligned Seeking
Force I-frames at the start of every segment (e.g., 2 seconds) to ensure seeks are "snappy" and don't require the backend to scan through the bitstream.
*   **Flag:** `-force_key_frames expr:gte(t,n_forced*2)`

---

## 3. Benchmarking Requirements
*   **TTFF (Time to First Frame):** Should be < 500ms on local network.
*   **Seek Latency:** Should be < 1s.
*   **CPU Overhead:** Should be < 5% for Smart Remuxing on a single 1080p stream.

2nd time analysis

# Stash Streaming Architecture Analysis — Complete Reference
> Full analysis in: `C:\Users\chara\.gemini\antigravity\brain\61f04872-64e9-4f56-8f57-a1a2ddebae56\stash_streaming_analysis.md`

## Quick-Reference: What Stash Does That We Must Port

### 1. Three Streaming Modes
| Mode | Endpoint | Technique |
|------|----------|-----------|
| Direct | `/stream` | `http.ServeFile` — OS sendfile, zero CPU |
| Pipe Transcode | `/stream.mp4` `.webm` `.mkv` | FFmpeg stdout → `io.Copy` → socket |
| Segmented Cache | `/stream.m3u8` `.mpd` | StreamManager + 200ms monitor goroutine |

### 2. The 5 Must-Use FFmpeg Flags
```
-movflags frag_keyframe+empty_moov   # fMP4: browser plays without full file
-flags +cgop                          # closed GOPs: seek-safe HLS
-force_key_frames expr:gte(t,n_forced*2)  # I-frame at every segment boundary
-copyts -avoid_negative_ts disabled   # correct timestamp maths for seeks
-preset veryfast -crf 25              # fast CPU transcode
```

### 3. Seek Before Input
```
ffmpeg -ss {timestamp} -i {file}    ← CORRECT (fast)
ffmpeg -i {file} -ss {timestamp}    ← WRONG  (slow, decodes from start)
```

### 4. Segment Buffer Constants
```
segmentLength   = 2s      maxSegmentBuffer = 15   maxSegmentGap = 5
maxIdleTime     = 30s     monitorInterval  = 200ms maxSegmentWait = 15s
```

### 5. HW Codec Priority
N264H (NVENC HQ) > N264 (NVENC) > I264 (QSV) > V264 (VAAPI) > R264 (V4L2M2M/Pi) > RK264 (Rockchip) > M264 (VideoToolbox)

### 6. Rust Key Patterns
- `kill_on_drop(true)` on `tokio::process::Child` → replaces Go's context-cancel
- `ReaderStream::new(stdout)` → `Body::from_stream()` → replaces `io.Copy`
- Always `tokio::spawn` stderr drain to prevent FFmpeg deadlock
- `Arc<Mutex<HashMap>>` + `tokio::time::interval(200ms)` → replaces Go's StreamManager goroutine
