# Stash-Inspired Implementation Plan: Media Manager

This plan outlines the integration of high-value features from Stash into Media Manager, prioritizing stability, compatibility, and visual polish.

## Phase 1: MVP - The Foundation (Digital Fingerprinting)
**Goal:** Move from "Path-based identity" to "Content-based identity".

1.  **OSHash Implementation**: Port the OpenSubtitles Hash algorithm to `media_core`. It's extremely fast and provides a unique signature for video files.
2.  **Schema Update**:
    *   Add `hash` column to `movie_files` and `episode_files`.
    *   Add `checksum` (MD5/SHA256) for smaller files/images.
3.  **Smart Ingestion**:
    *   Modify `worker.rs` to calculate hashes during scan.
    *   Update `upsert` logic: If a file is moved, find it by hash and update the path instead of creating a new entry.

## Phase 2: Playback & Streaming (The "Play Anywhere" Engine)
**Goal:** 100% browser compatibility for all media.

1.  **FFmpeg Transcoding Engine**:
    *   Implement an HLS (HTTP Live Streaming) segmenter in Rust.
    *   Add a `StreamManager` to handle active `ffmpeg` processes.
2.  **On-the-Fly Logic**:
    *   Detect if source codec is browser-native (H.264/AAC in MP4).
    *   If not, start transcoding to a temporary HLS directory.
3.  **Frontend Player**:
    *   Integrate `Hls.js` or `Video.js` into the React frontend.

## Phase 3: Visual Polish (The Discovery Engine)
**Goal:** High-end UX with instant previews.

1.  **Screenshot Generator**: Auto-capture frames at 20%, 50%, and 80% of duration.
2.  **Seek-Bar Sprites**:
    *   Generate a sprite sheet (grid of thumbnails).
    *   Generate a WebVTT file mapping timestamps to sprite coordinates.
3.  **Hover Previews**:
    *   Generate a 5-10 second low-bitrate WebP/MP4 "preview clip".
    *   Play on hover in the library grid.

## Phase 4: Architectural Upgrade (Optional)
1.  **GraphQL Integration**: Transition from REST to `async-graphql` for complex data relationships.
2.  **Plugin Scrapers**: Allow external scripts (Python/JS) to act as scrapers, mimicking Stash's community-driven model.


As a Software Architect, I have analyzed both Stash (a mature, feature-rich Go/GraphQL media vault) and Media_Manager (your emerging Rust-based
  orchestrator).

  While Stash is highly specialized for specific content types (e.g., performers, studios), its technical core for media handling is world-class.
  Media_Manager, being built in Rust, has the potential to outperform Stash in speed and resource efficiency.

  Here is an architectural comparison and a strategic "Best Logic" blueprint to borrow from Stash.

  ---

  1. Architectural Comparison

  ┌──────────────┬──────────────────────────┬─────────────────────────┬─────────────────────────────────────────────────────────────────────────────────┐
  │ Feature      │ Stash (Mature / Go)      │ Media_Manager (In Dev / │ Architect's Verdict                                                             │
  │              │                          │ Rust)                   │                                                                                 │
  ├──────────────┼──────────────────────────┼─────────────────────────┼─────────────────────────────────────────────────────────────────────────────────┤
  │ API Paradigm │ GraphQL (GQLGen)         │ REST (Axum)             │ Stash Wins on Flexibility. GQL is superior for media where relationships (Actor │
  │              │                          │                         │ <-> Movie) are deep.                                                            │
  │ Identity     │ Hash-First               │ Path-First (Relative    │ Stash Wins on Stability. If you rename a file in Media_Manager, it loses        │
  │ Logic        │ (OSHash/Checksum)        │ Path)                   │ metadata. In Stash, it just updates the path.                                   │
  │ Streaming    │ HLS/DASH/MP4 Transcoding │ Direct Play / Commented │ Stash Wins on Compatibility. Stash can play any codec in a browser via          │
  │              │                          │ HLS                     │ on-the-fly transcoding.                                                         │
  │ UX Polish    │ Sprites, VTT, WebP       │ Posters & Backdrops     │ Stash Wins on Discovery. Hover-previews and seek-bar thumbnails make a library  │
  │              │ Previews                 │                         │ feel "alive."                                                                   │
  │ Scraping     │ Scraper CD (Community    │ Hardcoded (TMDB/OMDB)   │ Stash Wins on Breadth. Stash uses a plugin system for scrapers, making it       │
  │              │ Driven)                  │                         │ resilient to site changes.                                                      │
  └──────────────┴──────────────────────────┴─────────────────────────┴─────────────────────────────────────────────────────────────────────────────────┘
  ---

  2. "Best Logic" to Borrow from Stash

  To make Media_Manager a top-tier tool, I suggest implementing these three "Core Logics" immediately:

  A. The "Digital Fingerprint" Logic (Identification)
   * The Problem: Currently, Media_Manager identifies movies by title and year. If you fix a typo in a folder name, the DB entry breaks.
   * The Stash Logic: During the scan, calculate an OSHash (a very fast hash of the first/last 64kb) and a Checksum.
   * Rust Implementation:
       * Add a file_hash column to your movie_files table.
       * When scanning, check if the hash exists. If it does, and the path changed, just update the path. Never lose your "Watched" status or manual edits
         again.

  B. The "Lazy-Transcoding" Pipeline
   * The Problem: High-quality 4K HEVC or MKV files often won't play in Chrome/Safari without stuttering or "Format Not Supported" errors.
   * The Stash Logic: A dedicated Stream Manager.
   * Rust Implementation:
       1. When a user hits "Play", use ffprobe (via your mediainfo.rs) to check if the browser supports the codecs.
       2. If it doesn't, spawn a background ffmpeg process to convert to HLS (h.264/AAC) on-the-fly.
       3. Serve the .m3u8 manifest to the frontend. This allows "Instant Play" even for 50GB files.

  C. The "Visual Discovery" Engine (Previews)
   * The Problem: Scrolling through static posters feels like a database, not a media center.
   * The Stash Logic: Pre-generated Sprites and Clips.
   * Rust Implementation:
       * Sprites: Every 10 seconds of video, grab a tiny frame. Combine them into one large JPEG (a sprite sheet) and a .vtt file.
       * Frontend: The React UI uses the VTT to show "Seek Previews" when the user hovers over the progress bar (exactly like YouTube).
       * Clips: Generate a 5-second low-res WebP or MP4. Play this on the poster when the user hovers.

  ---

  3. Strategic Recommendations for Media_Manager

  1. Transition to GraphQL (async-graphql)
  Since you are using Rust and Axum, I strongly recommend moving to GraphQL.
   * Why? In your React frontend, you'll often want to say: "Get this movie, its actors, and other movies those actors were in."
   * In REST, that's 3 API calls or one "fat" messy endpoint. In GraphQL, it's one clean query.

  2. Granular Background Tasks
  Stash separates "Scanning" (finding files) from "Generating" (making previews/hashes).
   * Your current TaskManager should be split into priorities:
       * Priority 1: Scan & Scrape (Get the text data in the DB fast).
       * Priority 2: Hashing (Identify duplicates).
       * Priority 3: Preview Generation (Heavy CPU work, done in background).

  3. Content-Based Deduplication
  Stash uses Perceptual Hashing (pHash). This allows it to find two different files that are actually the same movie (e.g., a 1080p copy and a 720p copy).
   * Use the img_hash crate in Rust to generate pHashes for your posters and video frames. This will help you find "Library Duplicates" even if the
     filenames are completely different.

  Final Summary for implementation:
   1. Adopt the Hash-as-Identity model to prevent data loss on file moves.
   2. Build a robust FFmpeg wrapper for HLS streaming to ensure 100% playback compatibility.
   3. Invest in "Secondary Processing" (Sprites/Clips) to give your UI a premium feel.

  By using Rust for these heavy-lifting tasks (FFmpeg wrapping and hashing), Media_Manager will be significantly faster and more stable than Stash while
  offering the same "magic" user experience.