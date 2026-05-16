# Spec: Stash-Parity Scanner & Centralized Asset Pipeline

**Status:** Approved  
**Date:** 2026-05-16  
**Author:** Gemini CLI (Senior Rust Engineer)

## 1. Objective
Transform the existing "Fast Scanner" into a "Stash-Identical" hybrid pipeline. This ensures that renamed or moved files retain their metadata and generated assets (sprites/previews) while maintaining Kodi-compatible NFO/Artwork in the media folders.

## 2. Architecture: The Hybrid Model

### 2.1 Schema Extensions
We will introduce a relational track registry and an asset lookup table.

**Table: `media_streams`**
| Column | Type | Description |
| :--- | :--- | :--- |
| `id` | INTEGER PRIMARY KEY | |
| `media_hash` | TEXT | Linked to `movie_files.fingerprint` or `episodes.fingerprint` |
| `stream_index` | INTEGER | The FFmpeg stream index (e.g., 1 for audio) |
| `stream_type` | TEXT | "video", "audio", "subtitle" |
| `codec` | TEXT | e.g., "h264", "aac", "subrip" |
| `language` | TEXT | ISO 639-2 (e.g., "eng", "jpn") |
| `title` | TEXT | Track title from metadata |
| `channels` | INTEGER | Audio channel count |
| `is_default` | BOOLEAN | |

**Table: `generated_assets`**
| Column | Type | Description |
| :--- | :--- | :--- |
| `id` | INTEGER PRIMARY KEY | |
| `file_hash` | TEXT | The `oshash` (fingerprint) of the media |
| `asset_type` | TEXT | "sprite", "preview", "thumb", "vtt" |
| `path` | TEXT | Relative path in the central `generated/` directory |

### 2.2 Centralized Blob Store
Internal technical assets will be moved from media folders to a centralized store.
- **Root:** `data/generated/` (Configurable via `.env`)
- **Structure:** `data/generated/<hash>/<asset_type>.<ext>`
- **Example:** `data/generated/a1b2c3d4/sprite.webp`

## 3. Core Logic Enhancements

### 3.1 Hash-First Scanner (`worker.rs`)
1. **Discovery:** Walk filesystem, check `mtime`/`size`.
2. **Fingerprinting:** If `mtime`/`size` changed or file is new, calculate `oshash`.
3. **Identity Resolution:**
   - Search DB for `oshash`.
   - If found: Update `file_path`. All linked metadata, streams, and generated assets stay connected.
   - If not found: Create new record.

### 3.2 Deep Probing (`mediainfo.rs`)
Upgrade `get_media_info` to:
- Parse every stream in the `streams` array from `ffprobe`.
- Detect video `rotate` tags for orientation correction.
- Detect HDR/10-bit color space (BT.2020) for future JIT Tone Mapping.

### 3.3 Advanced Analysis Pipeline (`ffmpeg.rs`)
A unified "Generation" task:
1. **Probe:** Populate `media_streams`.
2. **Sprite:** Generate 10x10 tile grid + `.vtt` file for seek previews.
3. **Preview:** Generate 10s hover clip (no audio).
4. **Cleanup:** Ensure old assets for the same hash are overwritten or updated.

## 4. User Experience & API

### 4.1 Player Integration
The frontend player will:
1. Fetch `/api/streams/:id` to populate Audio/Subtitle selectors.
2. Load the VTT/Sprite from `/api/assets/:hash/sprite` for scrubbing.

### 4.2 Asset Serving
The server will nest a new service:
`app.nest_service("/generated", tower_http::services::ServeDir::new(&generated_dir))`

## 5. Success Criteria
- [ ] Renaming a folder in Windows Explorer does not break thumbnails in the UI.
- [ ] MKV files with multiple audio tracks show all tracks in the player.
- [ ] Seek bar shows visual previews (sprites) on hover.
- [ ] 4K HDR videos are correctly identified in the database.
