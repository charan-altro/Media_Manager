# 🎯 Media Manager — Stash & TinyMediaManager Parity Master Plan

This plan outlines the strategic roadmap to implement key features from both **Stash** (adult organizer) and **TinyMediaManager** (general media manager), combining their strengths into our unified Rust-powered Media Manager.

---

## 🏗️ Ecosystem Architecture Recap

Our project uses a high-performance Rust core that powers multiple delivery mechanisms:
1. **`core` (media_core)**: Shared library containing DB (SQLx/SQLite), scrapers, scanning engine, renamer, and streaming/transcoding pipelines.
2. **`apps/server`**: Axum-based web server hosting the API and streaming endpoints.
3. **`apps/desktop`**: Tauri-based native wrapper for desktop experiences.
4. **`frontend`**: React-based cinematic dashboard using Vidstack player.

---

## 📊 Feature Status & Roadmap

| Feature | Source | Status | Next Steps / Actions |
| :--- | :--- | :--- | :--- |
| **Directory Sync** | Both | ✅ Completed | Fully optimized with Rayon parallel walks. |
| **NFO Import/Export** | TinyMediaManager | ✅ Completed | Read/write Kodi-compatible NFOs via `quick-xml`. |
| **Metadata Scrapers** | TinyMediaManager | ✅ Completed | Scrapers for TMDB, TVDB, OMDb, Trakt, AniDB, etc. are written. |
| **Sidecar Subtitles** | Both | ⏳ Planned | Auto-discover local `.srt` sidecars; convert on-the-fly to WebVTT. |
| **Scene Markers** | Stash | ⏳ Planned | DB migration, REST APIs, and Vidstack player timeline dots. |
| **Video Scrub Previews** | Stash | ⏳ Planned | Generate storyboard sprite sheets and VTT indices using FFmpeg. |
| **Smart Web Streaming** | Stash | ✅ Completed | On-the-fly HLS, DASH, progressive MP4, and direct remuxing. |
| **Hardware Acceleration** | Stash | ✅ Completed | NVENC, QSV, VAAPI, Apple VT support exists in backend. |
| **Player Hotkeys & Settings**| Both | ⏳ Planned | Percentage-based seeks (0-9), space/arrows hotkeys, volume persistence. |
| **Data Export (CSV/HTML)**| TinyMediaManager | ✅ Completed | Export library data successfully. |
| **Trakt.tv Sync** | TinyMediaManager | ⏳ Planned | Deep sync watch status / collection with Trakt accounts. |

---

## 🚀 Execution Tracks

We will implement the remaining high-value features sequentially. Here are the detailed specifications for each phase.

---

### Track 1: Scene Markers (Stash Parity)
**Goal:** Allow users to bookmark points of interest (markers) in videos, tag them, and jump directly to them via the player timeline.

#### 1.1 Backend Implementation (Rust)
* **Migration:** Create `media_core/src/db/migrations/020_scene_markers.sql`
  ```sql
  CREATE TABLE IF NOT EXISTS scene_markers (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      media_id INTEGER NOT NULL,
      media_type TEXT NOT NULL, -- 'movie' or 'episode'
      seconds REAL NOT NULL,
      title TEXT NOT NULL,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
  );
  CREATE INDEX idx_scene_markers_media ON scene_markers(media_id, media_type);
  ```
* **Database Repository:** Implement CRUD in `media_core/src/db/media_repo.rs` or `media_core/src/db/base.rs`.
* **Axum Endpoints in `apps/server/src/routes/streaming.rs`:**
  * `GET /api/media/:type/:id/markers` -> Returns list of markers for a video.
  * `POST /api/media/:type/:id/markers` -> Creates a new scene marker.
  * `DELETE /api/media/markers/:marker_id` -> Deletes a scene marker.

#### 1.2 Frontend Implementation (React/Vidstack)
* **API Adapter:** Add marker endpoints to `frontend/src/api/adapter.ts`.
* **UI Overlay on Player:**
  * Render markers as clickable dots over the Vidstack progress bar (`<TimeSlider>`).
  * Add a marker list side-panel or drawer to click and jump (`player.seek(seconds)`).
  * Add a "Mark Scene" hotkey (e.g., press `M`) to create a marker at current playtime.

---

### Track 2: Sidecar Subtitles (Discovery & On-the-fly WebVTT)
**Goal:** Automatically detect local `.srt` subtitle files next to the video and serve them to the browser player as WebVTT.

#### 2.1 Backend Implementation (Rust)
* **Subtitle Discovery Engine:** In `media_core/src/subtitles/sidecar.rs` (New)
  * Given a media file path (e.g. `/movies/Avatar.mp4`), search its directory for matching sidecar subtitles: `/movies/Avatar.*.srt`.
* **On-the-fly SRT-to-WebVTT Converter:**
  * Write a lightweight, fast text parser converting `.srt` format to `.vtt`.
* **Axum Routing:** In `apps/server/src/routes/streaming.rs`
  * `GET /api/media/:type/:id/subtitles` -> Returns listed sidecar subtitles found.
  * `GET /api/media/:type/:id/subtitles/:lang` -> Serves the converted WebVTT track with `Content-Type: text/vtt`.

#### 2.2 Frontend Implementation (React/Vidstack)
* Fetch discovered sidecars on player mount.
* Inject `<Track src="/api/media/movie/12/subtitles/en" label="English" kind="subtitles" default />` into `<MediaPlayer>`.

---

### Track 3: Storyboard Preview Sprites (Stash Feature)
**Goal:** Hovering over the seek timeline displays a small thumbnail preview frame of that exact moment.

#### 3.1 Backend Generation (FFmpeg)
* **Storyboard Generator:** In `media_core/src/scanner/sprites.rs`
  * Use FFmpeg to extract frames at regular intervals (e.g. every 5 seconds or every 1% of duration).
  * Tile them into a grid sprite sheet (e.g. 5x5 images, 150px width per thumb) to save HTTP requests.
  * Write a WebVTT storyboard index map referencing the sprite sheet offset coordinates:
    ```vtt
    00:00:00.000 --> 00:00:05.000
    /api/assets/sprite_123.jpg#xywh=0,0,150,84

    00:00:05.000 --> 00:00:10.000
    /api/assets/sprite_123.jpg#xywh=150,0,150,84
    ```
* **Asset Servicing:** Save the generated sprite sheets and WebVTT indices under local cache directories and serve them via `apps/server/src/routes/assets.rs`.

#### 3.2 Frontend Integration
* Inject the `.vtt` preview index into Vidstack:
  ```tsx
  <MediaPlayer>
    <MediaProvider />
    <Track src="/api/assets/preview_123.vtt" kind="chapters" />
    <TimeSlider>
      <SliderPreview>
        <SliderThumbnail track="/api/assets/preview_123.vtt" />
      </SliderPreview>
    </TimeSlider>
  </MediaPlayer>
  ```

---

### Track 4: UX & Settings Persistence (Hotkeys & Storage)
**Goal:** Enable fluid, high-fidelity player interactions.

#### 4.1 Volume & Playback Persistence
* Configure Vidstack's built-in `<MediaPlayer>` attributes:
  * `storage="media-manager-player-settings"` -> Auto-saves volume level, audio track, and subtitles preferences.
  * `resume-position` -> Saves and resumes playback position from cookies/local-storage or backend heartbeat logs.

#### 4.2 Desktop & Web Hotkeys
* Listen to keydowns on player view:
  * `0-9` Keys -> Instant percentage seeks: `5` jumps to 50% duration, `9` to 90%, etc.
  * `Space` -> Play/Pause.
  * `Left/Right Arrows` -> Skip back/forward 5 seconds.
  * `Up/Down Arrows` -> Volume control.

---

### Track 5: Trakt.tv Integration (TinyMediaManager Feature)
**Goal:** Sync watched history, ratings, and media collection lists back and forth with a user's Trakt.tv account.

#### 5.1 Backend Sync Manager
* **OAuth Authentication:** Standard Trakt OAuth flow integration.
* **Sync Engine:** Sync local DB state (movies/episodes watched timestamps) with Trakt API `sync/history` and `sync/collection` endpoints.

---

## 🎯 Verification and Test-Driven Process

1. We compile the workspace on the Windows host using `powershell.exe -Command "Set-Location ... ; cargo check"`.
2. We run cargo test using powershell.
3. We run the frontend using npm.
4. We verify the build pipelines before proposing changes.
