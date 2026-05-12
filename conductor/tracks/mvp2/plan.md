# MVP 2: Reliable Streaming Implementation Plan

## Background & Motivation
The current HLS streaming implementation uses synchronous `std::process::Command` calls, which block the async executor during startup. Furthermore, streams are cleaned up based on a naive 5-minute timeout, and transcoded segments are written directly to the current working directory, which causes excessive wear on SD cards (like on Raspberry Pi 4). 

MVP 2 addresses these issues to ensure reliable, non-blocking streaming that protects hardware and cleans up gracefully.

## Scope & Impact
- **Process Management**: Refactoring `StreamManager` and `FfmpegEngine` to use `tokio::process::Command` for true async execution.
- **Hardware Protection (RAM Disk)**: Allowing the transcode directory to be configured via an environment variable (`HLS_TRANSCODE_DIR`), enabling the use of a `tmpfs` RAM disk.
- **Heartbeat & Reaper**: Implementing a heartbeat API and an aggressive 120-second timeout to kill orphaned FFmpeg processes.
- **Intelligent Polling**: Using the `notify` crate to actively watch for the creation of `playlist.m3u8` instead of synchronous waiting or fixed delays.

## Implementation Steps

### 1. Configuration & RAM Disk Support
- Add `HLS_TRANSCODE_DIR` to the configuration (fallback to `./transcodes`).
- Update `StreamManager` initialization to use this configured directory.

### 2. Async Process Management (`tokio::process`)
- Modify `media_core::scanner::streaming::StreamSession` to store `tokio::process::Child`.
- Update `FfmpegEngine::create_hls_stream` (if it exists) or inline the command spawning in `StreamManager::start_hls` to use `tokio::process::Command::new(...).spawn()`.
- Ensure the Tokio Child process handles `SIGKILL` cleanly when dropped or explicitly killed.

### 3. File Watcher (Playlist Polling)
- Add the `notify` crate to `media_core/Cargo.toml`.
- In `StreamManager::start_hls`, after spawning FFmpeg, initialize a `notify` watcher on the specific stream's output directory.
- Await an event indicating `playlist.m3u8` has been created or modified (and `len > 0`) before returning `Ok(playlist_path)`. Include a global timeout (e.g., 15 seconds) to prevent infinite hanging.

### 4. Heartbeat API & The Reaper
- The API route `/api/playback/heartbeat` already exists but currently updates `playback_state` in the DB.
- Update `StreamManager` to track `last_access` for each stream session.
- Add an API to ping the `StreamManager` explicitly to update `last_access`, OR hook into the existing `/api/playback/heartbeat` to update the session in memory.
- Adjust the background cleanup loop in `server/src/main.rs` to run more frequently (e.g., every 30s) and reduce the timeout threshold to 120s.

## Verification & Testing
- **Async Verification**: Ensure no Tokio executor blocks occur during stream initialization.
- **Polling Verification**: Start a stream and observe the log. It should wait exactly until `playlist.m3u8` is populated before returning.
- **Reaper Verification**: Start a stream, close the player, and verify the FFmpeg process is killed within ~120 seconds.
- **RAM Disk Validation**: Start the server with `HLS_TRANSCODE_DIR=/tmp` and verify segments are written there.