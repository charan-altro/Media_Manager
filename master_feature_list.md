# 📘 Master Book: SelfHost Media Orchestrator Ecosystem

This document provides a comprehensive, non-technical overview of the core capabilities and the future technical roadmap for the Media Orchestrator ecosystem.

---

### 🚀 Master Book Highlights:
*   **Deep Metadata Harvesting**: Automatically extracts and persists comprehensive details including **Title**, **Year**, **Plot**, **Tagline**, **Runtime**, and both **IMDB & TMDB Ratings**.
*   **Local Metadata Persistence**: Automatically generates and saves industry-standard **.nfo files** into your media directories for 100% compatibility.
*   **Artwork & Cast Org**: Downloads high-res posters and backdrops, and creates a local **.actors folder** for offline cast photos.
*   **Smart Matching**: Uses **Levenshtein Scoring** to ensure name accuracy and **Rate-Limiting Semaphores** to stay within API bounds.
*   **Advanced UI**: Includes the new **Selection Mode** (Multi-select), **Bulk Ops**, and **Smart Dynamic Filters** (Genre/Language dropdowns).
*   **High-Speed Delivery**: Support for parallel downloads via **Range headers** while preserving **original disk filenames**.

---

### 1. Media Ingestion & Scanning
*   **Fast Directory Sync**: Rapid, non-blocking file discovery using high-performance concurrency (Rust Rayon) to populate the UI within seconds.
*   **Zero-Touch Ingestion (Watchdog)**: Real-time directory monitoring using the `notify` crate. Automatically triggers parsing, scraping, and artwork downloads the moment a new file is detected.
*   **Delayed Background Ingestion**: Initial fast sync followed by deep metadata extraction and artwork processing running asynchronously to ensure a fluid user experience.
*   **Intelligent Path Parsing**: Advanced regex-based engine that accurately extracts Title, Year, Season, and Episode numbers from complex or messy filenames.
*   **NFO Prioritization (Local-First)**: Prioritizes existing `.nfo` files for 100% metadata accuracy, bypassing the need for internet scraping when local data is present.

### 2. Metadata Extraction & Enrichment
*   **Full Metadata Harvesting**: Automatically extracts and persists comprehensive details including Genre, Language, and synchronized IMDB/TMDB scores.
*   **Standardized .nfo Generation**: Automatically creates and saves metadata into industry-standard `.nfo` files for maximum compatibility.
*   **Automated Artwork Management**: Detects and downloads high-resolution Posters and Backdrops (Fanart) directly to the media folders.
*   **Cast & Crew Enrichment**:
    *   Pulls full **Cast Lists** with roles and character names.
    *   Downloads **Actor Profile Images** into a dedicated `.actors` folder for offline display.
*   **Online Trailer Discovery**: Automatically fetches and stores links to movie and TV show trailers from TMDB.

### 3. Scraping & Smart Matching
*   **Multi-Source Scraper Chain**: Deep integration with **TMDB**, **OMDb**, and **TheTVDB (TVDB)** APIs for localized metadata.
*   **Levenshtein Matching Logic**: Intelligent string-distance algorithms that resolve naming collisions by scoring filename accuracy against global search results.
*   **Concurrent Scraper Engine**: High-speed parallel fetching with **Rate-Limiting Semaphores** to ensure maximum throughput without triggering API blocks.

### 4. Library & Bulk Management
*   **Selection Mode**: Intuitive multi-select support across the entire library (even in "All Media" view) for large-scale organization.
*   **Bulk Scrape & Cleanup**: One-click triggers for background metadata matching or cleaning orphan artwork and empty folders.
*   **Targeted Metadata Refresh**: Ability to force-refresh specific titles to sync the latest ratings and details from the cloud.
*   **Webhook Notifications**: Event-driven alerts via Discord or Telegram bots. Pings administrators when high-intensity tasks like "Bulk Scrape" finish or new media is ready.

### 5. Advanced Browsing & Filtering
*   **Smart Dynamic Filters**: Browse your library using **Genre** and **Language** dropdowns that are automatically generated based on the actual content of your collection.
*   **Live Netflix-Style Search**: Instant, high-performance filtering that updates the dashboard in real-time as you type.
*   **State Tracking & User Profiles**:
    *   **Resume Playback**: Remembers exactly where you paused.
    *   **Heartbeat Sync**: Real-time playback reporting that saves progress to SQLite every 5 seconds.
    *   **Multi-User Support**: Maintains separate watch histories for different household members.

### 6. Playback & Streaming
*   **Dual-Mode Playback Engine**:
    *   **Native Launch**: Direct integration to open media files in high-fidelity local players like **VLC** or **MPV** for the best local experience.
    *   **Adaptive HLS Streaming (Optional)**: On-the-fly transcoding using FFmpeg. Mimics Netflix by automatically adapting to network conditions, allowing heavy MKV/HEVC files to play on any device.
    *   **Hardware-Aware Transcoding**: Optimizes performance using hardware encoders (Apple VideoToolbox / NVENC) and uses "Direct Stream" to copy compatible streams without overhead.

### 7. System & Networking
*   **Edge-Ready Reverse Proxy Support**: Native awareness of `X-Forwarded-For` and `X-Real-IP` headers for secure deployment behind Traefik, Caddy, or Cloudflare tunnels.
*   **Real-Time Task Monitoring**: Uses **Server-Sent Events (SSE)** to provide live progress bars and status updates for all background tasks.
*   **Containerized Delivery**: Shipped as a lightweight, multi-architecture Docker image with the React frontend compiled directly into the Rust binary for one-command deployment.
*   **High-Speed File Access**: Support for `Range` headers to enable multi-stream, high-speed file transfers while preserving original disk filenames.
*   **Command Line Interface (CLI)**: Native CLI support for triggering library scans, batch scraping, and database maintenance from terminal scripts.
*   **Enhanced Media Analysis**: FFmpeg integration for **Aspect Ratio Detection** and technical stream analysis.
*   **External Tool Integration**: Ability to hook into external utilities like **yt-dlp** for trailer downloads or custom post-processing scripts.

---

### Ecosystem Architecture
*   **Frontend**: React with Zustand for ultra-responsive state management.
*   **Backend**: High-performance Rust/Axum or Go/Wails cores.
*   **Data**: SQLite for a portable, self-contained database that travels with your media.
