# Design: MVP 1 & 2 Completion (Progress & Persistence)

**Date**: 2026-05-12  
**Status**: Approved  
**Topic**: Completing the Stash-inspired reconciliation and streaming persistence.

---

## 1. Goal
Complete the missing functional and visual components of MVP 1 (Identity/Scanning) and MVP 2 (Streaming/Persistence) to ensure a robust user experience on the Raspberry Pi 4.

## 2. MVP 1.2/1.3: Enhanced Scan Progress & Reconciliation

### 2.1 Backend: Data Model Update
Update `TaskUpdate` in `media_core/src/models/task.rs` to include reconciliation counters.

```rust
pub struct TaskUpdate {
    pub task_id: String,
    pub status: String,
    pub progress: i32,
    pub total: i32,
    pub message: String,
    // Reconciliation Counters
    pub files_new: Option<i32>,
    pub files_healed: Option<i32>,
    pub files_missing: Option<i32>,
    pub started_at: Option<u64>,
    pub finished_at: Option<u64>,
    pub debug_info: Option<String>,
}
```

### 2.2 Backend: Missing File Pass
Add a final step to `scan_library_internal` in `worker.rs`:
1.  Query all `file_path`s for the library from the DB.
2.  Compare against the set of paths visited during `WalkDir`.
3.  Any path in DB but NOT visited is flagged as `is_missing = true`.
4.  Update `files_missing` counter in the final `TaskUpdate`.

### 2.3 Frontend: Pro Stats UI
Update `TasksPage.tsx` to display three color-coded badges for active/completed scans:
- **New** (Blue): Files added to the DB.
- **Healed** (Green): Files whose paths were updated via fingerprint matching.
- **Missing** (Red): Files no longer found on disk.

---

## 3. MVP 2: Streaming Heartbeat & Resume Logic

### 3.1 Frontend: Heartbeat Mechanism
Modify `VideoPlayer.tsx`:
- Start a `setInterval` (30s) when the video starts playing.
- Call `POST /api/playback/heartbeat` with `{ media_id, media_type, position_ms, duration_ms }`.
- Clear interval on `onClose` or component unmount.

### 3.2 Frontend: Playback Persistence
- **Resume Check**: When a user clicks "Play", first call `GET /api/playback/status/:type/:id`.
- **UI Prompt**: If progress exists (> 0), show a modal: "Resume from [Time]?" or "Start Over".
- **Seek on Load**: If Resuming, seek the `<video>` element to the saved position once HLS is attached.

---

## 4. Success Criteria
1.  **Scanner**: A scan shows exactly how many files were healed vs. new.
2.  **Streaming**: FFmpeg processes are NOT killed while the video is playing (due to heartbeat).
3.  **Streaming**: FFmpeg processes ARE killed 120s after the browser tab is closed.
4.  **Resume**: Refreshing the page and clicking "Play" allows the user to resume their movie.
