# Implementation Plan — Video.js v10 Migration & Subtitle Integration (DEFERRED)

> [!NOTE]
> **Deferred / Archived**: This migration proposal was deferred. The project continues to use the **Vidstack** player library, which is fully implemented and optimized.

Migrate the Media Manager player from Vidstack to Video.js v10 (`@videojs/react` and `@videojs/core`) and implement a complete backend/frontend subtitle management system for both Server (web) and Desktop (Tauri) environments.

## User Review Required

> [!IMPORTANT]
> **Complete player replacement:** This plan proposes replacing the current `@vidstack/react` player component entirely with a custom, lightweight, and modern `@videojs/react` player component.
> 
> **SRT to WebVTT conversion on-the-fly:** Subtitle files downloaded (e.g. sidecar `.srt` files next to media files) will be converted on-the-fly to the web-standard WebVTT format by the Rust backend. This keeps client-side rendering fast and standard-compliant across desktop and web browser views.

## Proposed Changes

---

### Backend (media_core & apps)

We will implement sidecar subtitle discovery and SRT-to-WebVTT conversion logic in `media_core`, and then expose them via server endpoints and Tauri commands.

#### [MODIFY] [subtitles/mod.rs](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/media_core/src/subtitles/mod.rs)
- Add a new helper `discover_sidecar_subtitles(media_path: &Path) -> Vec<SubtitleTrack>` that scans the parent folder for sidecar `.srt` or `.vtt` files matching the media file stem (e.g., `movie.en.srt`, `movie.vtt`).
- Add a utility function `convert_srt_to_vtt(srt_content: &str) -> String` that converts SubRip subtitles to WebVTT on-the-fly (replacing `,` with `.` in timestamp lines and adding `WEBVTT\n\n` header).
- Add a helper `load_subtitle_as_vtt(path: &Path) -> Result<String, std::io::Error>` to load, parse, and encode the subtitle correctly.

#### [MODIFY] [routes/streaming.rs](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/apps/server/src/routes/streaming.rs)
- Expose four new server endpoints:
  - `GET /movies/:id/subtitles` -> list discovered movie subtitles.
  - `GET /episodes/:id/subtitles` -> list discovered episode subtitles.
  - `GET /movies/:id/subtitles/:lang` -> serve the subtitle file for movie `id` and language `lang` converted to WebVTT.
  - `GET /episodes/:id/subtitles/:lang` -> serve the subtitle file for episode `id` and language `lang` converted to WebVTT.

#### [MODIFY] [main.rs](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/apps/desktop/src/main.rs)
- Expose a new Tauri command:
  - `get_subtitles(id: i64, media_type: String)` -> discovers subtitles and returns a list of tracks. If they are `.srt`, it converts them to `.vtt` and saves/caches them as a sidecar `.en.vtt` or in the cache directory, returning the local `convertFileSrc` path so the Tauri Webview can load them directly.
- Add `get_subtitles` to the list of handlers in `generate_handler!`.

---

### Frontend (API & Component)

#### [MODIFY] [api/adapter.ts](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/frontend/src/api/adapter.ts)
- Add a new API interface:
  ```typescript
  export interface SubtitleTrack {
    label: string;
    lang: string;
    src: string;
  }
  ```
- Add the `getSubtitles(id: number, type: 'movie' | 'episode') -> Promise<SubtitleTrack[]>` API endpoint in the `api` namespace, supporting both Tauri IPC and Web Server HTTP modes.

#### [NEW] [VideoJsPlayer.tsx](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/frontend/src/components/VideoJsPlayer.tsx)
- Create a modern Video.js v10 React Player component using `@videojs/react`.
- Include out-of-the-box controls like `<PlaybackRateButton>`, `<VolumeSlider>`, `<MuteButton>`, `<FullscreenButton>`, `<SeekButton>`, `<TimeSlider>`.
- Implement a custom source selector overlay dropdown in React (switches between direct download MP4 / HLS streams).
- Implement a custom A-B Loop hook `useAbLoop` in React to handle loops programmatically (since the legacy `videojs-abloop` plugin is incompatible).
- Fetch available subtitles on load using `api.getSubtitles` and map them to standard `<track>` elements nested inside `<Video>`.
- Hook into the player's time updates to report playback progress (heartbeat) to `api.updatePlaybackProgress` every 10 seconds.
- Persist player volume settings inside `localStorage` for future sessions.

#### [MODIFY] [DetailModal.tsx](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/frontend/src/components/DetailModal.tsx)
- Replace imports of `VidstackPlayer` with the new `VideoJsPlayer`.
- Ensure all parameters (like `resumePosition`, `onClose`, etc.) map correctly to the new component.

---

## Verification Plan

### Automated Tests
- Run `cargo test` in `media_core` to verify subtitle discovery and parsing logic.
- Run frontend typechecks and compilation via `npm run build` inside `frontend/`.

### Manual Verification
1. Play a movie or episode that has a downloaded `.srt` subtitle.
2. Confirm the player shows the subtitle track selection button.
3. Toggle the subtitle track and verify cues are rendered correctly.
4. Test changing play speed, volume (verify persistence on reload), and toggle A-B loops (verify loop points repeat).
5. Verify playback progress continues to update in the dashboard (heartbeat).
