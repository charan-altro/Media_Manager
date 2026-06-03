# 🎯 Implementation Plan: Stash Feature Parity for Media Manager (Rust)

This implementation plan outlines how to port four high-value features from **Stash (Go)** to **Media Manager (Rust)** to improve duplicate detection, playback reliability, visual chapter/marker bookmarking, and local casting.

---

## 🛠️ Feature 1: Perceptual Hashing (pHash) Duplication Engine

### Problem Context
Currently, Media Manager only hashes files via MD5 / OSHash, which are cryptographic and fail if the container, resolution, or bitrate changes. Stash calculates a perceptual video hash by extracting screenshots at 25 timestamps, combining them into a 5x5 grid, and computing a DCT pHash.

### Proposed Changes

#### [NEW] [025_add_phash.sql](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/media_core/src/db/migrations/025_add_phash.sql)
- Create a `fingerprints` table to hold hashes, matching Stash's design:
  ```sql
  CREATE TABLE IF NOT EXISTS fingerprints (
      file_hash TEXT NOT NULL,
      type TEXT NOT NULL, -- 'phash'
      fingerprint TEXT NOT NULL, -- Hex-encoded pHash
      PRIMARY KEY (file_hash, type)
  );
  CREATE INDEX idx_fingerprints_fingerprint ON fingerprints(fingerprint);
  ```

#### [MODIFY] [Cargo.toml](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/media_core/Cargo.toml)
- Add dependencies for image compilation and DCT hashing:
  * `img_hash = "3.2"`
  * `image = "0.24"`

#### [NEW] [phash.rs](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/media_core/src/scanner/phash.rs)
- Implement pHash calculation logic:
  1. Determine 25 timestamp points evenly spaced throughout the video duration (excluding the first and last 5%).
  2. For each timestamp, invoke `ffmpeg` to extract a 160-pixel wide thumbnail image into memory (`std::process::Command` capturing stdout as BMP).
  3. Combine the 25 thumbnail buffers into a single 5x5 grid image using the `image` crate.
  4. Perform DCT perceptual hashing using the `img_hash` crate.
  5. Convert the hash bytes to a hexadecimal string.

#### [MODIFY] [service.rs](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/media_core/src/scanner/service.rs)
- Integrate pHash generation into the scan loop. After scanning a video file and calculating OSHash, run the `phash::generate` function in a background thread task.
- Store results in the `fingerprints` table.

#### [NEW] [duplicates.rs](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/media_core/src/services/duplicates.rs)
- Implement duplicate querying logic in the service layer:
  1. Load all video files and their `phash` strings from the database.
  2. Perform bitwise Hamming distance calculations between all hash pairs.
  3. Group video files where `distance <= 4` (or a configurable threshold).
  4. Expose a new Axum API endpoint `/api/media/duplicates` to retrieve duplicate groups.

---

## 🛠️ Feature 2: Seekbar Timeline Markers (Visual Markers)

### Problem Context
The database already has scene markers (seconds + title) via `020_add_scene_markers.sql`, and React has a sidebar bookmark list, but there is no seekbar integration. Hovering or clicking dots on the timeline is missing.

### Proposed Changes

#### [MODIFY] [VidstackPlayer.tsx](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/frontend/src/components/VidstackPlayer.tsx)
- Fetch scene markers for the active media file during load.
- Override Vidstack's default timeline layout to inject absolute-positioned marker dots:
  ```tsx
  {markers.map((marker) => (
    <div
      key={marker.id}
      className="absolute top-1/2 -translate-y-1/2 w-2.5 h-2.5 bg-yellow-400 border border-black rounded-full cursor-pointer hover:scale-125 z-10 group"
      style={{ left: `${(marker.seconds / duration) * 100}%` }}
      onClick={(e) => {
        e.stopPropagation();
        player.currentTime = marker.seconds;
      }}
    >
      <div className="absolute bottom-5 left-1/2 -translate-x-1/2 hidden group-hover:block bg-zinc-900 border border-zinc-700 text-xs px-2 py-1 rounded shadow-lg whitespace-nowrap">
        {marker.title} ({formatTime(marker.seconds)})
      </div>
    </div>
  ))}
  ```
- Support overlapping or highly dense clusters of markers by displaying a stacked list on tooltip hover.

---

## 🛠️ Feature 3: Marker Media Preview Assets

### Problem Context
Stash renders scene markers with looping WebP clips. Currently, Media Manager only stores the title and timestamp of scene markers textually in SQLite.

### Proposed Changes

#### [MODIFY] [017_stash_parity_foundation.sql](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/media_core/src/db/migrations/017_stash_parity_foundation.sql)
- Ensure the `generated_assets` table is fully supported in our schema models.

#### [MODIFY] [media_repo.rs](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/media_core/src/db/media_repo.rs)
- Implement queries to save/retrieve `generated_assets` mapped by file hash and asset type (`preview`, `sprite`, `thumb`, `vtt`).

#### [MODIFY] [ffmpeg.rs](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/media_core/src/scanner/ffmpeg.rs)
- Create a function to crop a 3-second segment centered around a specific timestamp and encode it to WebP:
  ```bash
  ffmpeg -ss <marker_time - 1.5> -t 3 -i <input> -vf "scale=160:-1" -c:v libwebp -loop 0 -an -y <output_path.webp>
  ```

#### [MODIFY] [assets.rs](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/apps/server/src/routes/assets.rs)
- Add a route `/api/assets/marker-preview/<marker_id>` to serve the generated `.webp` file inline from the transcodes directory.
- Update the bookmarks component to show the looping WebP preview on hover.

---

## 🛠️ Feature 4: Multi-Source Streaming Fallback & Smart Buffering

### Problem Context
Chrome, Firefox, Safari, and TV devices support different video codecs. Offering a single direct URL can cause playback failures. In addition, transcoder processes shouldn't run indefinitely if the client isn't actively watching.

### Proposed Changes

#### [MODIFY] [streaming.rs](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/apps/server/src/routes/streaming.rs)
- Expose a `/api/streaming/sources/<media_id>` route returning an ordered JSON array of source streams:
  ```json
  [
    { "src": "/api/streaming/direct/<media_id>", "type": "video/mp4" },
    { "src": "/api/streaming/hls/<media_id>/index.m3u8", "type": "application/x-mpegURL" },
    { "src": "/api/streaming/transcode/<media_id>/stream.webm", "type": "video/webm" }
  ]
  ```

#### [MODIFY] [VidstackPlayer.tsx](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/frontend/src/components/VidstackPlayer.tsx)
- Feed this list of sources to the Vidstack `<MediaPlayer>` component.
- Add an error listener hook `onPlayError` to detect decoding issues and auto-switch to HLS or transcoded fallback URLs if direct play fails.

#### [MODIFY] [streaming.rs](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/media_core/src/scanner/streaming.rs)
- Refactor the segmented transcoding pipeline:
  1. Add buffer thresholds checking: pause the spawned `ffmpeg` process when the HLS generator has written `maxSegmentBuffer = 15` segments ahead of the player request pointer.
  2. Implement seek reset: if a client requests a segment index that is >5 segments away from the current transcode point, terminate the running process and respawn `ffmpeg` starting at the new offset using `-ss <segment_time>`.

---

## 🔍 Open Questions & Design Decisions

> [!IMPORTANT]
> 1. **Image Crate Overhead**: Generating a 5x5 sprite grid in Rust requires decoding 25 separate frames and stitching them. Does the host machine have enough memory/CPU to run this during background scans? (Yes, we can spawn these at low process priority or limit concurrency to 2 threads).
> 2. **Hamming Distance calculations**: Should we perform pairwise calculations in-memory in Rust or compile an SQLite extension module to calculate distance in SQL? (In-memory is simpler and scales well for up to ~10,000 files, taking <10ms in Rust).

---

## 🧪 Verification Plan

### Automated Tests
- Build verification:
  ```powershell
  cargo check --workspace
  ```
- Run scanning tests:
  ```powershell
  cargo test --package media_core --lib scanner
  ```

### Manual Verification
1. **Perceptual Hashing**:
   * Duplicate a movie file in your library folder, re-encode it to a lower resolution or different container format (e.g. MP4 to WebM), and run a Library Scan.
   * Verify that both files show up as duplicates under the `/api/media/duplicates` dashboard.
2. **Seekbar Markers**:
   * Add a scene marker (e.g. at 30 seconds) on a movie page.
   * Open the player and confirm a yellow marker dot is visible on the timeline. Hover over it to check the title and click to verify it seeks correctly.
3. **Multi-source Fallback**:
   * Play a video using a codec not natively supported by your browser (e.g. HEVC on standard Linux Chrome). Verify the player intercepts the error and seamlessly transitions to the transcoded HLS fallback.
