# Stash-Parity Scanner & Centralized Asset Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a hash-first media scanner with a relational stream registry and centralized asset storage for seek-previews and sprites, matching StashApp's architecture.

**Architecture:** We will use `oshash` as the primary identity for media files, decouple assets from filenames by storing them in a centralized `generated/` folder, and add relational tables to manage multi-track audio/subtitles.

**Tech Stack:** Rust, SQLx (SQLite), FFmpeg, Axum.

---

### Task 1: Database Migration for Streams and Assets

**Files:**
- Create: `media_core/src/db/migrations/017_stash_parity_foundation.sql`
- Modify: `media_core/src/models/mod.rs`
- Modify: `media_core/src/db/queries.rs`

- [ ] **Step 1: Create migration file**
Create `media_core/src/db/migrations/017_stash_parity_foundation.sql` with `media_streams` and `generated_assets` tables.

- [ ] **Step 2: Add models**
Add `MediaStream` and `GeneratedAsset` structs to `media_core/src/models/mod.rs`.

- [ ] **Step 3: Implement upsert queries**
Add `upsert_media_stream` and `upsert_generated_asset` to `media_core/src/db/queries.rs`.

- [ ] **Step 4: Run migrations**
Run `cargo test` or similar to trigger migrations.

---

### Task 2: Refactor Deep Probe (`mediainfo.rs`)

**Files:**
- Modify: `media_core/src/scanner/mediainfo.rs`

- [ ] **Step 1: Update `MediaDetails` struct**
Include `streams: Vec<MediaStreamInfo>`, `rotation: i32`, and `bit_depth: i32`.

- [ ] **Step 2: Update `get_media_info`**
Refactor the parsing logic to extract all streams and metadata tags (rotate, bits_per_raw_sample).

- [ ] **Step 3: Test track extraction**
Verify with an MKV file containing multiple tracks.

---

### Task 3: Hash-First Scanner Refactor (`worker.rs`)

**Files:**
- Modify: `media_core/src/scanner/worker.rs`

- [ ] **Step 1: Implement Identity Resolution**
Update `process_file` to check for fingerprints (hash) before paths. If hash exists at a different path, update the path instead of adding a new entry.

- [ ] **Step 2: Test move detection**
Rename a file and verify the database record updates without creating a duplicate.

---

### Task 4: Centralized Asset Management (`ffmpeg.rs` & `streaming.rs`)

**Files:**
- Modify: `media_core/src/scanner/ffmpeg.rs`
- Modify: `apps/server/src/main.rs`

- [ ] **Step 1: Update `ffmpeg.rs` asset generation**
Change `extract_thumbnail`, `generate_preview`, and `generate_sprite_sheet` to save to a centralized `data/generated/<hash>/` directory.

- [ ] **Step 2: Serve generated folder in Axum**
Add `.nest_service("/api/generated", ...)` to `apps/server/src/main.rs`.

- [ ] **Step 3: Add asset retrieval route**
Implement `/api/assets/:hash/:type` to return the correct file from the central store.

---

### Task 5: Final Verification

**Files:**
- Test: `media_core/tests/stash_parity_integration_test.rs`

- [ ] **Step 1: Integration Test**
Write a test that scans a file, renames it, rescans, and verifies generated assets are still correctly linked and accessible.
