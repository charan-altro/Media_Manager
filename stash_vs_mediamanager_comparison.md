# 🎯 Feature Comparison & Parity Guide: Stash (Go) vs. Media Manager (Rust)

This document analyzes the differences between **Stash** (a self-hosted media organizer in Go) and **Media Manager** (a media manager written in Rust/Tauri/Axum). It highlights mature features from Stash's architecture and outlines how we can port them into the Rust-based **Media Manager**.

---

## 📊 Feature Comparison & Feasibility Matrix

| Feature / Subsystem | Stash (Go) Implementation | Media Manager (Rust) Status | Porting Feasibility to Rust | Recommendations & Target Tech Stack |
| :--- | :--- | :--- | :--- | :--- |
| **Perceptual Hashing (pHash) Duplication Engine** | Generates a 5x5 sprite grid from 25 video thumbnails, hashes it via DCT-based `goimagehash`, and runs database distance checks. | Standard cryptographic `MD5` / `OSHash` (first/last 64KB + size) only. | **High** | Generate 5x5 BMP sprites using `ffmpeg` inside Rust. Hash sprites using the [`img_hash`](https://crates.io/crates/img_hash) crate (DCT algorithm) and save as `u64` in SQLite. Use Hamming distance checks in SQL. |
| **Timeline Scene Markers** | Renders markers as hoverable, clickable dots on the player seekbar. Organizes overlapping ranges using Interval Scheduling. | Stores marker text/seconds in database. React sidebar exists, but no seekbar timeline integration. | **High** | Replace default Vidstack layout with custom player components. Overlay absolute-positioned dots on the Vidstack `<media-time-slider>` or container. |
| **Marker Clip Previews** | Extracts short WebP/MP4 preview clips and thumbnails at marker timestamps using background FFmpeg tasks. | No asset generation for markers; text-only bookmark titles in SQLite. | **Medium** | Spawn background FFmpeg tasks using Tokio `tokio::process::Command` in Rust to extract 3-second `.webm`/`.mp4` clips and thumbnails, saving to the transcodes directory. |
| **Multi-Source Player Fallback** | Video.js player falls back to alternate streams (direct file, HLS, WebM, fragmented MP4 transcode) on decode error. | Vidstack plays a single resolved `directUrl` source. Playback crashes on decode error. | **High** | Fetch an array of streams from the server (Direct, HLS, Transcode). In Vidstack, bind to `onError` or supply a multi-source array for automatic browser codec fallback. |
| **Smart Transcoder Buffering** | Stops FFmpeg when segment generation is 15 segments ahead. Restarts `-ss` transcode if player seeks >5 segments away. | Transcodes files, but lacks advanced dynamic pause/seek buffering. | **Medium** | Implement a task-monitoring loop in Rust's Axum streaming server, pausing/resuming or restarting the spawned FFmpeg process based on client segment requests. |
| **DLNA / UPnP Media Server** | Built-in DLNA/UPnP server enabling casting to Smart TVs and discovery by local media players (VLC, etc.). | No local network casting protocol support. | **Medium** | Add a lightweight UPnP/DLNA service in Axum using Rust libraries like [`rupnp`](https://crates.io/crates/rupnp) or [`hyper`](https://crates.io/crates/hyper) to broadcast SSDP discovery. |
| **Wake Lock & OS Media Session** | Prevents system sleep during active playback. Binds hardware/headset media keys to player. | Standard web view. No native screen wake-lock or media overlay bindings. | **High** | Implement the standard browser **Screen Wake Lock API** and **Media Session API** in `VidstackPlayer.tsx` to handle OS media buttons. |
| **VR / Panoramic Playback** | 180° and 360° video projection filters inside the web player. | Standard 2D flat video player. | **Low** | Use WebGL/Three.js projection layers inside the React player. (Only needed if VR content is present). |

---

## 🔍 Deep Dive: How We Can Port These Features to Rust

### 1. Perceptual Hashing (pHash) Video Duplication Engine
Stash uses pHash to find duplicate videos that have different formats, resolutions, or bitrates. It does this by creating a **Sprite Grid** and hashing that grid.

#### How Stash does it:
1. Spacially samples 25 thumbnails (`columns = 5`, `rows = 5`) throughout the video (skipping the first/last 5% to avoid intros/outros).
2. Generates a composite sprite sheet of all 25 images.
3. Computes a single 64-bit perceptual hash (DCT phash) of the sprite sheet using `goimagehash`.
4. Saves this `u64` in SQLite as `fingerprints_phash`.
5. Compares images by querying hashes and grouping those with low Hamming distance (e.g. `distance <= 4`).

#### Rust Implementation Strategy:
* **Sprite Generation**: Spawn `ffmpeg` inside Rust to generate the thumbnails as a sprite:
  ```rust
  // Extract 25 screenshots at calculated interval timestamps into BMP buffers
  // Combine using the `image` crate's canvas operations into a single combined sprite
  ```
* **Hashing**: Add the [`img_hash`](https://crates.io/crates/img_hash) crate to your `Cargo.toml`.
  ```rust
  use img_hash::{HasherConfig, HashAlg};
  
  let hasher = HasherConfig::new()
      .hash_alg(HashAlg::DoubleGradient) // or DCT
      .hash_width(8)
      .hash_height(8)
      .to_hasher();
      
  let hash = hasher.hash_image(&sprite_image);
  let hash_u64 = hash.to_base64(); // or serialize as bytes/u64
  ```
* **Distance Matching**: In SQLite, we can load all hashes into memory and perform bitwise XOR (`^`) comparison, or define a custom SQLite function in Rust using `sqlx` or `rusqlite` to compute Hamming distance on-the-fly.

---

### 2. Seekbar Scene Markers (Visual Timeline Markers)
In Stash, scene markers (bookmarks) appear as dots or ranges directly on the player timeline. Hovering over a dot shows a thumbnail and description. Clicking jumps the playhead to that second.

```
Timeline: [========●=================●============]
                   |                 |
            "Intro Ends" (02:14)   "Epic Battle" (15:40)
```

#### How Stash does it:
* Passes the list of markers (timestamps) to the player component.
* Uses absolute CSS positioning relative to the seek bar wrapper: `left: (marker.seconds / duration) * 100%`.
* Renders colored circles (`div`s with `border-radius: 50%`) that sit slightly above the seekbar.

#### Rust + React Implementation Strategy:
* Since Media Manager uses **Vidstack**, we can create a custom seekbar. Vidstack allows adding custom timeline elements inside the `<media-time-slider>`:
  ```tsx
  import { TimeSlider } from '@vidstack/react';
  
  // Custom marker component overlaying the seek bar
  const SeekbarMarkers = ({ markers, duration }) => {
    return (
      <div className="absolute inset-0 pointer-events-none">
        {markers.map((marker) => (
          <div
            key={marker.id}
            className="absolute top-1/2 -translate-y-1/2 w-2 h-2 rounded-full bg-indigo-500 cursor-pointer pointer-events-auto group"
            style={{ left: `${(marker.seconds / duration) * 100}%` }}
            onClick={() => player.currentTime = marker.seconds}
          >
            {/* Hover Tooltip */}
            <span className="hidden group-hover:block absolute bottom-4 left-1/2 -translate-x-1/2 bg-slate-900 text-xs text-white p-1 rounded whitespace-nowrap">
              {marker.title} ({formatTime(marker.seconds)})
            </span>
          </div>
        ))}
      </div>
    );
  };
  ```

---

### 3. Marker Media Clip Previews
Rather than a static text list, Stash renders markers as short looping video/image clips.

#### How Stash does it:
* Stash runs a background FFmpeg task:
  `ffmpeg -ss <timestamp-1.5s> -t 3 -i <video> -filter_complex "scale=160:-1" -c:v libwebp -loop 0 <output.webp>`
* This creates a 3-second looping WebP thumbnail centered around the bookmark timestamp.

#### Rust Implementation Strategy:
* Implement an FFmpeg command builder in `media_core/src/scanner/ffmpeg.rs`:
  ```rust
  pub fn generate_marker_preview(video_path: &Path, time: f64, out_path: &Path) -> Result<()> {
      let start_time = (time - 1.5).max(0.0);
      std::process::Command::new("ffmpeg")
          .args(&[
              "-ss", &start_time.to_string(),
              "-t", "3.0",
              "-i", video_path.to_str().unwrap(),
              "-vf", "scale=160:-1",
              "-c:v", "libwebp",
              "-loop", "0",
              "-y",
              out_path.to_str().unwrap()
          ])
          .output()?;
      Ok(())
  }
  ```
* Save generated preview file paths to the `generated_assets` table (referenced in migration `017_stash_parity_foundation.sql`).

---

### 4. Multi-Source Streaming Fallback
Web browsers support different video containers and codecs. A video stream in H.264 MP4 might work everywhere, but an HEVC/H.265 stream might crash on Chrome while working on Safari/Edge. Stash resolves this by sending *alternative* stream urls.

```
Stream Array: [
  { src: "/api/stream/direct", type: "video/mp4" },
  { src: "/api/stream/hls/manifest.m3u8", type: "application/x-mpegURL" },
  { src: "/api/stream/transcode/webm", type: "video/webm" }
]
```

#### How Stash does it:
* Serves a list of endpoints ordered by performance/compatibility (Direct Play -> Pre-Transcoded Cache -> Live HLS -> Live WebM Transcode).
* Video.js tries to play the first. If an error is caught, it falls back to the next.

#### Rust + React Implementation Strategy:
* **Backend**: Update `apps/server/src/routes/streaming.rs` to return a JSON array containing multiple streaming options depending on browser capabilities.
* **Frontend**: Feed the source array directly to Vidstack's provider, letting Vidstack natively attempt to decode the best format:
  ```tsx
  <MediaPlayer src={[
    { src: '/api/stream/direct', type: 'video/mp4' },
    { src: '/api/stream/hls/index.m3u8', type: 'application/x-mpegURL' },
  ]}>
    <MediaProvider />
  </MediaPlayer>
  ```
* Additionally, listen to player error hooks to switch stream types if a high-resolution direct stream fails to play.

---

### 5. Smart Transcoder Pausing & Seeking (Tokio-FFmpeg coordination)
When running live transcoding (especially for HLS streaming), generating the entire video is expensive and wastes CPU if the user stops watching or pauses.

#### How Stash does it:
* It keeps the FFmpeg transcode running but monitors client segment requests.
* If the transcoder is 15 segments ahead of the client's position, it **pauses** the FFmpeg process (using OS signals like `SIGSTOP` on Unix or suspending threads on Windows).
* If the user seeks forward past a buffer limit (> 5 segments gap), it kills the FFmpeg process and respawns it using `-ss <new_timestamp>`.

#### Rust Implementation Strategy:
* Leverage Tokio's asynchronous sub-process engine (`tokio::process::Command`).
* Keep a registry of active transcoding processes in an Axum state wrapper (similar to `running_streams.go` in Stash).
* Use Windows/Unix process signals or handle reader stream throttling: when the socket stream is read slowly, the stdout pipe fills up, naturally applying backpressure to the FFmpeg encoder.

---

## 🛠️ Summary & Priority Roadmap for Media Manager

To make the Rust project **Media Manager** as feature-rich and resilient as Stash, we should implement these features in phases:

1. **🔴 High Priority: Seekbar Timeline Markers & Multi-Source Fallback**
   * *Why:* Directly fixes player stability and exposes existing bookmark data visually on the timeline.
   * *Tasks:* Edit `VidstackPlayer.tsx` to mount custom marker nodes on the slider. Bind to player load errors to fallback from Direct playback to HLS.

2. **🟡 Medium Priority: Perceptual Hashing (pHash) Duplication Engine**
   * *Why:* Allows the scanner to group duplicates automatically, cleaning up directories.
   * *Tasks:* Implement sprite-sheet compilation inside `media_core/src/scanner` and add DCT hashing via the `img_hash` crate.

3. **🟡 Medium Priority: Marker WebP Looping Clips**
   * *Why:* Greatly enhances the UI visuals, allowing users to browse bookmark cards with moving clips.
   * *Tasks:* Use Tokio process invocation of FFmpeg in `media_core` to extract short `.webp` loops at marker points.

4. **🟢 Low Priority: Wake Lock & Media Keys Integration**
   * *Why:* Simple UX polish (prevents screen dimming; allows keyboard play/pause to work).
   * *Tasks:* Add standard browser WakeLock / MediaSession hooks in React.
