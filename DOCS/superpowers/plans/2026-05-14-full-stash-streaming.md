# Full Stash On-Demand Streaming Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement an on-demand, continuous HLS streaming architecture that provides instant playback and efficient seeking while preserving Raspberry Pi 4 SD card life.

**Architecture:** 
1. **In-Memory Manifest:** Generate `.m3u8` instantly from database metadata.
2. **Continuous Transcoder:** Start a single FFmpeg process that generates segments sequentially.
3. **RAM Disk Storage:** Write segments to `tmpfs` (/dev/shm).
4. **Long Polling:** Backend waits for segments to appear before responding to player requests.

**Tech Stack:** Rust (Axum, Tokio, DashMap, sqlx), React, Video.js, FFmpeg (h264_v4l2m2m).

---

### Task 1: Database & Metadata Foundation

**Files:**
- Create: `media_core/src/db/migrations/016_add_duration.sql`
- Modify: `media_core/src/scanner/mediainfo.rs`
- Modify: `media_core/src/models/movie.rs`
- Modify: `media_core/src/models/tv.rs`
- Modify: `media_core/src/db/queries.rs` (if duration updates needed)

- [ ] **Step 1: Create migration for duration column**
```sql
-- Migration: 016_add_duration.sql
ALTER TABLE movie_files ADD COLUMN duration_secs INTEGER;
ALTER TABLE episodes ADD COLUMN duration_secs INTEGER;
```

- [ ] **Step 2: Update MediaDetails and ffprobe extraction**
Modify `media_core/src/scanner/mediainfo.rs` to include `duration_secs`:
```rust
pub struct MediaDetails {
    // ... existing
    pub duration_secs: i32,
}
// In get_media_info:
let duration_secs = json["format"]["duration"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0) as i32;
```

- [ ] **Step 3: Update Rust Models**
Add `duration_secs: Option<i32>` to `MovieFile` in `movie.rs` and `Episode` in `tv.rs`.

- [ ] **Step 4: Commit Phase 1**
```bash
git add .
git commit -m "feat: add duration_secs to media models and database"
```

---

### Task 2: In-Memory Manifest Generation

**Files:**
- Modify: `apps/server/src/main.rs`

- [ ] **Step 1: Implement Manifest Builder**
Create a helper function to generate the M3U8 string.
```rust
fn generate_m3u8(id: &str, duration_secs: i32) -> String {
    let segment_length = 10;
    let target_duration = 10;
    let mut manifest = String::from("#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:10\n#EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-PLAYLIST-TYPE:VOD\n");
    
    let mut leftover = duration_secs;
    let mut segment_idx = 0;
    while leftover > 0 {
        let length = if leftover > segment_length { segment_length } else { leftover };
        manifest.push_str(&format!("#EXTINF:{}.0,\nseg_{:03}.ts\n", length, segment_idx));
        leftover -= length;
        segment_idx += 1;
    }
    manifest.push_str("#EXT-X-ENDLIST\n");
    manifest
}
```

- [ ] **Step 2: Update Streaming Routes**
Update `start_movie_stream` and `start_episode_stream` to return the manifest instantly if duration is known.

- [ ] **Step 3: Commit Phase 2**
```bash
git add .
git commit -m "feat: implement instant in-memory HLS manifest generation"
```

---

### Task 3: Continuous Transcoder & Segment Serving

**Files:**
- Modify: `media_core/src/scanner/streaming.rs`
- Modify: `media_core/src/scanner/ffmpeg.rs`
- Modify: `apps/server/src/main.rs`

- [ ] **Step 1: Refactor StreamManager for Continuous Transcoding**
Update `StreamManager` to track `active_processes` using `DashMap`.
Implement `get_or_restart_process(id, start_time)`.

- [ ] **Step 2: Implement Long Polling for Segments**
In the segment serving route, if the file is missing, wait for up to 10 seconds.
```rust
// In apps/server/src/main.rs
async fn serve_hls_segment(...) {
    let mut attempts = 0;
    while attempts < 20 { // 20 * 500ms = 10s
        if file_path.exists() {
             return serve_file(file_path).await;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        attempts += 1;
    }
    StatusCode::NOT_FOUND
}
```

- [ ] **Step 3: Update FFmpeg Flags**
Use hardware-aware flags in `ffmpeg.rs`:
```bash
# Pi 4 Example
-c:v h264_v4l2m2m -b:v 4M -maxrate 5M -bufsize 8M -force_key_frames "expr:gte(t,n_forced*10)" -hls_flags independent_segments
```

- [ ] **Step 4: Commit Phase 3**
```bash
git add .
git commit -m "feat: implement continuous transcoding and segment long-polling"
```

---

### Task 4: Frontend Video.js Migration

**Files:**
- Modify: `frontend/package.json`
- Modify: `frontend/src/components/VideoPlayer.tsx`

- [ ] **Step 1: Install Video.js**
Run `npm install video.js @types/video.js` in `frontend/`.

- [ ] **Step 2: Re-implement VideoPlayer.tsx**
Use the `videojs` library to wrap a `<video>` element. Ensure `vhs` and `liveui` options are configured.

- [ ] **Step 3: Commit Phase 4**
```bash
git add .
git commit -m "feat: migrate frontend to Video.js for resilient HLS playback"
```

---

### Task 5: Verification & RAM Disk Setup

- [ ] **Step 1: Verify tmpfs on Linux**
Ensure `transcodes/` is symlinked to `/dev/shm/mediavault_transcodes` or similar.

- [ ] **Step 2: End-to-End Test**
Play a movie, seek to the middle, verify FFmpeg restarts correctly and playback resumes.
Verify CPU usage on Pi 4 is stable during continuous transcoding.
