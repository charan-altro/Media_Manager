# Product Scope & Feature Matrix

This document defines the core product vision, MVP scope, and the detailed feature matrix for the Media Manager ecosystem.

---

## 1. Product Vision
A "Netflix-like" experience over local media files. Scan your media library, fetch gorgeous metadata and artwork, and manage your collection in a cinematic React dashboard. Available as a standalone Windows app and a portable Docker container.

---

## 2. Core Feature Matrix

| Feature Description | Movies | TV Shows | Status |
|:--- |:---:|:---:|:--- |
| **Rapid Scanning** | ✅ | ✅ | Rayon-powered parallel scan |
| **NFO Support** | ✅ | ✅ | Kodi/Jellyfin XML compatibility |
| **Metadata Editing** | ✅ | ✅ | Manual UI overrides |
| **Renaming Engine** | ✅ | ✅ | Template-based reorganization |
| **CLI & Automation** | ✅ | ✅ | Headless batch processing |
| **REST API** | ✅ | ✅ | Full Axum-based interface |
| **Media Analysis** | ✅ | ✅ | FFmpeg/ffprobe integration |
| **Multi-Source Scraping** | ✅ | ✅ | TMDB, TVDB, OMDb, Trakt |
| **Subtitle Download** | ✅ | ✅ | Automated OpenSubtitles fetching |
| **Dual Playback** | ✅ | ✅ | Native Launch + HLS Streaming |
| **Data Export** | ✅ | ✅ | CSV, HTML, and JSON formats |

---

## 3. Implemented MVP Scope

### Deployment Targets
- **Tauri Desktop (Windows)**: Single `.exe` with zero runtime dependencies.
- **Docker (Linux/NAS)**: Optimized image (< 50MB) with Axum API and static frontend.

### Library Management
- **Multi-library support**: Named collections with path and media-type isolation.
- **Watchdog**: Real-time directory monitoring for instant sync.
- **Two-phase ingestion**: Fast registration followed by background enrichment.

### Metadata & UI
- **Cinematic Dashboard**: Poster grid with hover effects and detailed hero views.
- **TV Hierarchy**: Full Season/Episode browsing and metadata.
- **Selection Mode**: Intuitive multi-select for bulk scrapes and cleanup.
- **Task Monitor**: Real-time progress tracking via SSE or Tauri Events.

---

## 4. Stability & Safety
- **SQLite WAL Mode**: Ensures database integrity during concurrent read/write operations.
- **Safe Renaming**: `fs_extra` powered moves prevent data loss during reorganization.
- **NFO Persistence**: Metadata travels with the media, ensuring portability and backup safety.
