# High-Performance Streaming Implementation Plan (Stash-Parity)

## Objective
Transform the current HLS-only streaming engine into a "Buffer-Free" JIT (Just-In-Time) system matching Stash's performance levels.

---

## Phase 1: The "Smart Remux" Decision Engine
*Goal: Avoid unnecessary transcoding to save CPU.*

- [ ] **Task S-1.1: Codec Compatibility Matrix**
  - Define a struct/enum in `media_core/src/scanner/ffmpeg.rs` that maps browser-supported codecs (H264, AAC, VP9, Opus).
- [ ] **Task S-1.2: The `StreamStrategy` Resolver**
  - Implement a function `get_stream_strategy(details: &MediaDetails) -> StreamStrategy`.
  - **Strategies:**
    - `DirectCopy`: Container and Codec both OK.
    - `SmartRemux`: Video OK, Audio/Container need change (e.g., MKV -> MP4).
    - `FullTranscode`: Codec incompatible.
- [ ] **Task S-1.3: FFmpeg Command Builder Refactor**
  - Update `build_ffmpeg_args` to support `-c:v copy` and `-c:a copy` dynamically based on the strategy.

---

## Phase 2: Fragmented MP4 (fMP4) Pipeline
*Goal: Achieve <500ms Time-to-First-Frame (TTFF).*

- [ ] **Task S-2.1: Axum `StreamDirect` Route**
  - Create `GET /api/stream/direct/:id` in `apps/server/src/main.rs`.
- [ ] **Task S-2.2: FFmpeg fMP4 Flags**
  - Implement the "Instant Start" command:
    - `-movflags frag_keyframe+empty_moov+default_base_moof`
    - `-f mp4`
- [ ] **Task S-2.3: Non-Blocking Pipe (Stdout)**
  - Use `tokio::process::Command` with `Stdio::piped()`.
  - Convert `stdout` into a `tokio_util::io::ReaderStream`.
  - Return `axum::body::StreamBody` for zero-buffer transmission.

---

## Phase 3: Reactive Process Lifecycle
*Goal: Immediate CPU recovery on tab close.*

- [ ] **Task S-3.1: Token-Based Cancellation**
  - Pass a `tokio_util::sync::CancellationToken` to the streaming task.
- [ ] **Task S-3.2: Drop-Signal Handling**
  - Ensure that when the Axum `StreamBody` is dropped (connection closed), the `CancellationToken` is triggered.
- [ ] **Task S-3.3: Immediate Process Reaper**
  - Catch the cancellation signal and send `SIGKILL` (or `taskkill` on Windows) to the FFmpeg child process immediately.

---

## Phase 4: Instant Seeking (Keyframe Alignment)
*Goal: Seek anywhere in the video in <1s.*

- [ ] **Task S-4.1: Input-Seeking Implementation**
  - Ensure the `-ss {time}` flag is placed **before** the `-i` flag in all streaming commands (Input Seeking is significantly faster than Output Seeking).
- [ ] **Task S-4.2: Keyframe Injection**
  - For `FullTranscode` strategies, add `-force_key_frames expr:gte(t,n_forced*2)` to ensure 2-second seeking granularity.

---

## Phase 5: Hardware Acceleration (Pi 4 Optimized)
*Goal: Stable 1080p transcoding on Raspberry Pi.*

- [ ] **Task S-5.1: V4L2M2M Integration**
  - Verify and enable `h264_v4l2m2m` as the primary encoder for Linux/ARM.
- [ ] **Task S-5.2: Memory Management**
  - Ensure all temporary fragments are written to `/dev/shm` (RAM disk) instead of the SD card.
