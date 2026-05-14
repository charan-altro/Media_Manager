# Design: HLS Fix & Local Playback

**Date**: 2026-05-13
**Status**: Approved
**Topic**: Fixing HLS streaming timeout and adding a Local Playback button

## 1. Goal
Resolve the 'playlist timeout' error during HLS stream generation and introduce a feature allowing users to open media in their default local desktop media player (e.g., VLC).

## 2. HLS Timeout Fix
- **Path Normalization**: The backend currently passes mixed slash paths (e.g., `F:/dir\\file.mp4`) to FFmpeg, which can cause silent failures on Windows. We will normalize the path using `crate::paths::normalize_slashes` before spawning the process in `media_core/src/scanner/streaming.rs`.
- **Timeout Increase**: Increase the playlist watcher timeout from 15s to 30s in `StreamManager::start_hls`.

## 3. Local Playback Feature
- **Backend**: The routes `/api/movies/:id/play` and `/api/episodes/:id/play` already exist and use the `opener` crate.
- **Frontend (`DetailModal.tsx`)**: 
  - **Movies**: Rename the current "Start Playback" button to "Stream (Browser)" and add a secondary "Play Locally" button below it that calls `api.playMovie`.
  - **TV Shows**: Add a new icon button (e.g., Monitor or PlaySquare icon) next to the Download icon on episode rows that calls `api.playEpisode`.