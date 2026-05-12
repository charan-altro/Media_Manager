# Stash-Inspired Implementation Plan: Media Manager

This document serves as the technical blueprint for evolving Media_Manager into a high-performance media vault. It leverages **Rust's** performance (multithreading/safety) and **Stash's** proven UX patterns.

---

## 1. Architectural Comparison

| Feature | Stash (Mature / Go) | Media_Manager (In Dev / Rust) | Architect's Verdict |
| :--- | :--- | :--- | :--- |
| **API Paradigm** | **GraphQL (GQLGen)** | REST (Axum) | **Stash Wins on Flexibility.** GraphQL handles deep relations (Movie <-> Actor <-> Studio) without "Under-fetching" or "God Endpoints." |
| **Identity Logic** | **Fingerprint-First** (Hash) | Path-First (Relative Path) | **Stash Wins on Stability.** Using Hash as the "Anchor" makes the DB resilient to manual file moves or renames. |
| **Streaming** | **Smart Remuxing** (HLS-Copy) | Transcode-Only (HLS x264) | **Stash Wins on Efficiency.** Native files use 0% CPU via remuxing; only non-native files trigger the transcode pipeline. |
| **Scraping** | **Plugin-based** (YML/Python) | Hardcoded (Rust Modules) | **Stash Wins on Extensibility.** Community scrapers allow updates without needing a full binary recompile. |

---

## 2. Core Logics & The "Rust Advantage"

### A. The "Fingerprint-as-Anchor" Logic (Resilient Identity)
*   **The Logic**: The Hash (OSHash/SHA256) is the **Primary Identity**. The Path is just an attribute.
*   **Rust Advantage**: Use **`rayon`** for multithreaded hashing. On a multi-core system (like a RPi 4), we can hash new files in parallel during the scan, significantly outperforming Go's sequential approach.
*   **Identity Recovery**: If a file is moved, the scanner finds the hash match, updates the path, and preserves all metadata/tags/watched-status.

### B. The "Hybrid Streaming" Engine (CPU Efficiency)
To ensure 100% playback compatibility with 0% wasted CPU:
1.  **Probe**: Use `ffprobe` to check if container and codecs are browser-native (H.264/AAC).
2.  **Remux (The Sweet Spot)**: If the video is H.264 but the container is MKV, don't transcode. Use `ffmpeg -c:v copy -c:a copy`. This uses ~1% CPU.
3.  **Transcode (Last Resort)**: If codecs are incompatible (HEVC/DTS), trigger the HLS pipeline with `-c:v libx264 -preset ultrafast`.
4.  **HLS Advantage**: Enables instant seeking and adaptive bitrate for remote viewing.

### C. The "Visual Discovery" Engine
*   **Sprite Sheets**: Pre-generated 10x10 grids for instant seek-bar thumbnails.
*   **Hover Clips**: 5-second silent WebP/MP4 previews to make the library feel "alive."
*   **VTT Metadata**: Text files mapping timestamps to sprite coordinates for the frontend player.

---

## 3. Implementation Roadmap

### MVP 1.1: Foundation & Fingerprinting (The Identity Layer) [COMPLETED]
**Goal:** Establish the unique "Fingerprint" for every media file.
1.  **Database Schema Update**: Add `fingerprint`, `is_missing`, and `last_scanned`.
    ```sql
    ALTER TABLE movie_files ADD COLUMN fingerprint TEXT UNIQUE;
    ALTER TABLE movie_files ADD COLUMN is_missing BOOLEAN DEFAULT FALSE;
    ALTER TABLE movie_files ADD COLUMN last_scanned TIMESTAMP DEFAULT CURRENT_TIMESTAMP;
    CREATE INDEX idx_file_path ON movie_files(file_path);
    ```
2.  **Advanced Path Normalization**: Implement NFC (Normalization Form Canonical Composition) and force forward-slashes (`/`) in `paths.rs`.
3.  **FastHash Implementation**: Integrate OSHash (File Size + First/Last 64KB) as the primary fingerprint.
**Testing & Validation:**
*   **Automated**: `cargo test -p media_core --test identity`. Verifies OSHash generation and link healing.
*   **Manual**: Rename a folder containing a movie; run a scan; verify the database `file_path` updates while keeping the same metadata.

### MVP 1.1.1: Fast-Skip Scanning (Performance Patch)
**Goal:** Achieve instant rescans (< 5s for 300+ files) by ignoring unchanged media.
1.  **Database Schema Update**: Add `mtime` (last modified time) to `movie_files` and `episodes`.
    ```sql
    ALTER TABLE movie_files ADD COLUMN mtime BIGINT;
    ALTER TABLE episodes ADD COLUMN mtime BIGINT;
    ```
2.  **Differential Logic**:
    *   During `WalkDir`, capture each file's `Size` and `mtime`.
    *   Compare against DB: If `Path + Size + mtime` matches exactly, **Skip** hashing and FFprobe.
    *   Only process files that are brand new or have a newer `mtime`.

**Testing & Validation:**
*   **Manual**: Run a scan twice. The second scan should complete in under 5 seconds and the logs should show "Skipping unchanged file" for all items.
*   **Benchmarking**: Compare "Initial Scan" time vs "Rescan" time on the Pi 4.

**Technical Improvements & Logic Added:**
*   **Existence-Aware Healing**: The scanner now performs a disk existence check on the *old* path before healing a record. This prevents database "flipping" in libraries with duplicate files (identical content at different paths).
*   **Small File Safety (MD5 Fallback)**: For files smaller than 128KB (previews/clips), the system uses a full **MD5 hash** instead of OSHash to prevent fingerprint collisions.
*   **Parallel Metadata Extraction**: Moved technical info extraction (`ffprobe`) into the **Rayon parallel parsing phase**. This allows the Raspberry Pi 4 to utilize all CPU cores to analyze multiple files simultaneously, reducing scan times from minutes to seconds.
*   **Unicode NFC Normalization**: Integrated the `unicode-normalization` crate to ensure that paths like `café.mp4` are stored identically regardless of whether they were created on macOS, Linux, or Windows.

### MVP 1.2: Structured Communication (The Progress Layer)
**Goal:** Enable the UI to see "Healed" vs "New" files in real-time.
1.  **Task Messaging Schema**:
    *   **TaskStatus Enum**: `Scanning`, `Hashing`, `Healing`, `Scraping`, `Complete`.
    *   **TaskProgress Struct**: Track `files_processed`, `files_healed` (moved), `files_new`, and `files_missing`.
2.  **SSE Integration**: Update the Axum SSE stream to push these structured enums instead of generic strings.
**Testing & Validation:**
*   **Automated**: Unit test the `TaskUpdate::to_json()` output to ensure it matches the React frontend expectations.
*   **Manual**: Open the "Tasks" page in the browser; trigger a scan; observe the real-time counters for "Healed" and "New" items.

### MVP 1.3: The Healing Scanner (The Logic Layer)
**Goal:** Implement the non-destructive "Reconciliation" workflow. Optimized for Raspberry Pi 4.
1.  **The "Four-Case" Reconciliation Algorithm**:
    *   **Case A (Match)**: Path & Hash match. Increment `files_processed`.
    *   **Case B (Heal)**: Hash matches but Path differs. **Action**: Update DB Path. Increment `files_healed`.
    *   **Case C (New)**: Hash not in DB. **Action**: Insert Record. Increment `files_new`.
    *   **Case D (Missing)**: DB entry not on disk. **Action**: Set `is_missing = true`. Increment `files_missing`.
2.  **Throttled Parallelism**: Use `rayon` for concurrent processing, but wrap IO in a `tokio::sync::Semaphore` (limit: 2) to protect Pi 4 disk/USB bus thrashing.

**Testing & Validation:**
*   **Automated**: Integration test simulating a disconnected drive (Setting `is_missing=true`) and then reconnecting it.
*   **Manual**: Run a scan while monitoring the Pi 4's CPU/IO with `htop`; ensure the scanner doesn't lock up the system.

### MVP 2: Reliable Streaming (Lifecycle & Storage)
**Goal:** Fix "stuck" streams and protect hardware.
1.  **Process Management**: Use `tokio::process::Command` to manage FFmpeg as a detached background child.
2.  **The Reaper**: A background task that kills any FFmpeg process if no heartbeat is received from the player for 120s.
3.  **RAM Disk (SD Card Protection)**: Map the HLS transcode directory to a **tmpfs (RAM Disk)**.
    *   **Why**: Protects the Pi 4's SD card from thousands of tiny read/write operations and ensures instant delivery.
4.  **Playlist Polling**: API must wait for `playlist.m3u8` to be ready before returning success.

**Testing & Validation:**
*   **Automated**: Test the "Reaper" by spawning a mock FFmpeg process and verifying it's killed after heartbeat timeout.
*   **Manual**: Start a movie; close browser; verify `ffmpeg` process dies after 2 minutes.


### MVP 3: Smart Hybrid Streaming (Hardware-Aware)
**Goal:** 0% CPU usage for native files and optimized transcoding.
1.  **Logic Switch**: Implement the Probe -> Remux -> Transcode decision tree in `streaming.rs`.
2.  **Hardware-Aware Scaling**: Use modular environment variables for encoders:
    *   **Pi 4 (Broadcom)**: Use `h264_v4l2m2m` (encoding) and `hevc_v4l2m2m` (decoding).
    *   **Apple Silicon (M1-M4)**: Flip to `h264_videotoolbox`.
3.  **Validation**: Verify < 5% CPU usage when streaming native files and manageable usage during Pi 4 hardware transcoding.


**Testing & Validation:**
*   **Automated**: Unit test the "Probe" logic with sample MKV/MP4 files to ensure correct codec/container detection.
*   **Manual**: Play an H.264 MP4; verify CPU usage < 5%. Play HEVC; verify logs show hardware encoder usage.


### MVP 4: Visual Polish - Seek-Bar Previews
**Goal:** YouTube-style previews on hover.
1.  **VTT Generation**: Update `FfmpegEngine` to generate `.vtt` metadata alongside sprite sheets.
2.  **Idle-Only Execution**: This task only runs if **CPU < 30%** and **Active Streams == 0**.
3.  **Process Control**: The Rust backend must be able to `SIGSTOP` (pause) sprite generation if a user starts a stream, and `SIGCONT` (resume) when finished.

**Testing & Validation:**
*   **Automated**: Verify `.vtt` file syntax and timestamp accuracy.
*   **Manual**: Hover over the player seek bar; verify thumbnail appears at the correct timestamp.

### MVP 5: Visual Polish - Hover Previews
**Goal:** Animate library posters on mouse hover.
1.  **Low-Priority Generator**: Move silent clip generation to the "Idle-Only" queue.
2.  **Lazy-Load UI**: Update React `MovieCard` to only play previews after a 500ms hover delay to save network bandwidth.


**Testing & Validation:**
*   **Automated**: Verify clip file sizes and durations.
*   **Manual**: Quickly scroll movie grid; ensure clips only play after intentional hover delay.

### MVP 6: Advanced Deduplication (Optimized)
**Goal:** Clean up quality duplicates without heavy CPU load.
1.  **Hash Grouping**: UI view to identify identical files (grouped by OSHash).
2.  **Poster-Based pHash**: To save CPU, only run the `img_hash` crate on **Poster images** or a single extracted middle frame.
**Testing & Validation:**
*   **Automated**: Test pHash similarity on different resolutions of the same image.
*   **Manual**: Check "Cleanup" settings; verify it correctly identifies a 1080p and 720p version of the same film.
---

## 4. The "Architectural Jump" (Long-Term)

### Transition to GraphQL (`async-graphql` + Axum)
*   **Lookaheads**: Use GraphQL lookaheads in Rust to only perform SQL joins for requested fields, preventing the N+1 problem.
*   **Type Safety**: End-to-end type safety between Rust structs and TypeScript frontend.
*   **Relational Depth**: Handle the "Movie -> Actors -> Other Movies" graph naturally.

### Plugin Scrapers
*   **External Logic**: Use `Rhai` or WASM to load scrapers as external scripts.
*   **Stability**: Update site scrapers without needing to recompile or redeploy the main Rust binary.

---

## 5. Process Lifecycle Management
To ensure the server remains stable, all external processes (FFmpeg, FFprobe) must be managed via:
*   **Tokio Channels**: Use channels to send "Kill" signals to active stream tasks.
*   **Zombie Protection**: Use `tokio::process` to ensure child processes are properly reaped and cleaned up even if the server crashes.
*   **Heartbeat API**: The frontend must send a `POST /api/playback/heartbeat` every 30s to keep the HLS session alive.
