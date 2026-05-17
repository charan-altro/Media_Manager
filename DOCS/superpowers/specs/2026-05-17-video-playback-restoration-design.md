# Video Playback Restoration Design

## Goal
Restore reliable MKV direct play and seeking in the `Media_Manager-dev` branch by adopting the `ServeFile` logic from the `master` branch and the `stash_reference_project_GO` reference.

## Analysis
- **Master/Stash Logic**: Uses `ServeFile` (Axum/Tower in Rust, `http.ServeFile` in Go). This allows the browser to handle playback natively using HTTP Range requests.
- **Dev Logic**: Attempts JIT remuxing or defaults to HLS for non-MP4 files. This adds unnecessary overhead and breaks native browser seeking.

## Proposed Changes

### 1. Backend (`apps/server/src/routes/streaming.rs`)
- **Modify `serve_direct_movie` and `serve_direct_episode`**:
    - Remove custom JIT remuxing logic (`start_direct_stream`).
    - Use `tower_http::services::ServeFile` to serve the media file directly.
    - Ensure headers like `Accept-Ranges: bytes` are correctly handled (automatic with `ServeFile`).
- **Update `start_movie_stream` and `start_episode_stream`**:
    - Default to the `/api/stream/direct/...` URL.
    - Only use HLS/DASH if explicitly requested via protocol query parameter.

### 2. Frontend (`frontend/src/components/VideoPlayer.tsx`)
- **Fix Source Type Mapping**:
    - Ensure that URLs containing `/direct/` are mapped to `video/mp4` (standard hint for browsers).
    - Modern browsers (Chrome/Edge) handle MKV containers well when served via Range requests, even if the MIME type is hinted as MP4 or served generically.

## Verification Strategy
- **Direct Link Test**: Verify that `/api/stream/direct/movie/{id}` supports seeking in a standalone browser tab.
- **Player Integration**: Test playback of MKV and MP4 files in the integrated Video.js player.
- **Regression Check**: Ensure HLS streaming still works when explicitly requested.

---
Approved by reference to `master` and `stash_reference_project_GO`.
