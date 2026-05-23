# Stash Parity MVP Implementation Plan (Vidstack / Windows)

## Objective
To bring the existing Vidstack-based React frontend and Rust backend closer to Stash-like feature parity, focusing on the highest-value features for a Minimum Viable Product (MVP) running on Windows, while leveraging Vidstack's native capabilities.

## Scope & Priorities
Based on a review of `stash_feature_map.md` and current implementation, the following features form the MVP:
1.  **Sidecar Subtitles:** Automatic discovery of local `.srt` files and on-the-fly conversion to WebVTT.
2.  **Scene Markers:** Database support and UI overlays on the Vidstack timeline to mark and jump to points of interest.
3.  **Enhanced Player Hotkeys:** 0-9 percentage-based seeking (e.g., press '5' to jump to 50%).
4.  **Native Integration:** Enable Vidstack's native `storage` (volume/settings persistence) and `mediaSession` metadata.

## 1. Backend Architecture (Rust)

### 1.1 Sidecar Subtitles (Discovery & Conversion)
- **File:** `media_core/src/subtitles/sidecar.rs` (New)
- **Logic:**
  - Given a media file path (e.g., `C:\movies\Avatar.mp4`), scan the parent directory for matching subtitle files (e.g., `Avatar.en.srt`).
  - Read the `.srt` file and implement a fast `srt_to_vtt` converter.
- **Endpoints:**
  - `GET /movies/:id/subtitles/:lang`
  - Serve the converted WebVTT string with `text/vtt` content type.

### 1.2 Scene Markers (Database)
- **Migration:** `media_core/src/db/migrations/020_scene_markers.sql`
- **Logic:** Store `media_id`, `media_type`, `seconds`, and `title`.
- **Endpoints:**
  - `GET /media/:type/:id/markers`
  - `POST /media/:type/:id/markers`

## 2. Frontend Implementation (React/Vidstack)

### 2.1 Subtitle Integration
- **File:** `frontend/src/components/VidstackPlayer.tsx`
- **Logic:**
  - On mount, call `api.getSubtitles(mediaId, mediaType)`.
  - For each track, add a `<Track>` component inside the `<MediaPlayer>`.

### 2.2 Scene Markers UI
- **File:** `frontend/src/components/VidstackPlayer.tsx`
- **Logic:**
  - Render markers as dots over the timeline.
  - *Implementation Detail:* Use CSS to position dots relative to the progress bar container, or customize the Vidstack `<TimeSlider>`.

### 2.3 Native & UX Refinements
- **Media Session:** Pass `title`, `artist`, and `poster` to `<MediaPlayer>` to enable OS-level controls.
- **Persistence:** Add `storage="media-manager-player"` to `<MediaPlayer>` to automatically save/restore volume and muted state.
- **0-9 Hotkeys:** Add a `keydown` listener to `remote.seek((duration * key) / 10)`.

## 3. Verification
- **Subtitles:** Place an `.srt` file next to a video and verify it renders in Vidstack.
- **Markers:** Create a marker and verify the dot appears on the timeline.
- **Persistence:** Change volume, refresh, and verify the volume level is restored.
