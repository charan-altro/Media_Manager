# Stash-Parity Scanner Implementation TODO

- [x] **Task 1: Database Migration for Streams and Assets**
  - Create migration `017_stash_parity_foundation.sql`.
  - Add `MediaStream` and `GeneratedAsset` models.
  - Implement upsert queries in `queries.rs`.
- [x] **Task 2: Refactor Deep Probe (`mediainfo.rs`)**
  - Update `MediaDetails` struct.
  - Refactor `get_media_info` for full stream extraction.
- [x] **Task 3: Hash-First Scanner Refactor (`worker.rs`)**
  - Implement Identity Resolution in `process_file`.
- [x] **Task 4: Centralized Asset Management (`ffmpeg.rs` & `streaming.rs`)**
  - Update `ffmpeg.rs` to use `data/generated/<hash>/`.
  - Serve generated folder in Axum.
  - Add asset retrieval route.
- [x] **Task 5: Final Verification**
  - Write integration test `stash_parity_integration_test.rs`.
