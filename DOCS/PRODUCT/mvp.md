# Media Manager – MVP Scope (Rust / Tauri Edition)

> **MVP Target:** Feature parity with Python v1.0 + native Windows desktop packaging

---

## Core Product Vision

A "Netflix-like" experience over local files — scan your media drive, fetch gorgeous posters from TMDB, and manage your entire collection in a cinematic React dashboard. Available as both a **standalone Windows `.exe`** and a **Docker container**.

---

## 1. IN-SCOPE: MVP Features

### Deployment Targets (Both Required)
- **Tauri Desktop App**: Single `.exe` for Windows. No Node.js, no Python, no Rust required by end users. Installs via `.msi` or `.nsis` installer.
- **Docker Container**: Axum REST API + compiled React frontend. Single `docker compose up` deployment for Linux/NAS servers.

### Library Management
- **Multi-library support**: Define named libraries with a path and type (Movie / TV)
- **Automated directory scanning**: WalkDir-powered scan of directories with 10,000+ items
- **Two-phase ingestion**: Instant file registration → background metadata enrichment
- **Duplicate detection**: Group files by byte size before expensive hashing

### Metadata & Scraping
- **TMDB integration**: Title search, full details fetch (plot, rating, cast, genres)
- **OMDb ratings**: IMDb/Rotten Tomatoes/Metacritic score overlay
- **NFO prioritization**: If a `.nfo` file exists with a TMDB/IMDb ID, skip internet scraping
- **Poster + backdrop download**: Concurrent image fetching with local caching

### UI (React Frontend — reused from Python project)
- **Cinematic movie grid**: Poster cards with hover animations, ratings overlay
- **Movie hero/detail view**: Full-screen backdrop, cast carousel, plot, ratings
- **TV shows**: Season/episode hierarchy with episode cards
- **Real-time scan progress**: Live progress bar via SSE (Docker) or Tauri events (Desktop)
- **Library settings**: Add/edit/remove library paths

### Data Portability
- **NFO generation**: Kodi/Jellyfin-compatible XML files written next to media files
- **SQLite compatibility**: New app opens existing `orchestrator.db` from Python version

---

## 2. OUT-OF-SCOPE: Post-MVP (v2.0+)

| Feature | Reason |
|---|---|
| File renamer (template-based) | Non-trivial to make safe; Phase 2 |
| Subtitle downloader (OpenSubtitles) | Hash algorithm port needed; Phase 2 |
| Media info extraction (resolution/codec) | Requires `ffprobe` subprocess; Phase 2 |
| Library cleanup / orphan purge | Port of 550-line `cleanup.py`; Phase 2 |
| Artwork manager (custom posters) | Phase 2 |
| Export (JSON/CSV/HTML) | Phase 3 |
| Hybrid video playback (streaming) | Phase 3 |
| Real-time directory monitoring (notify crate) | Phase 3 |
| Multi-tenant user accounts | Out of scope |
| On-the-fly transcoding | Out of scope (delegate to Tdarr) |

---

## 3. Performance Targets

| Metric | Python Baseline | Rust MVP Target |
|---|---|---|
| Scan speed (10,000 files) | ~90 seconds | < 10 seconds |
| Docker image size | ~400 MB | < 60 MB |
| Memory at idle (Docker) | ~180 MB | < 30 MB |
| Windows binary size | N/A (PyInstaller ~80MB) | < 15 MB `.exe` |
| Cold start time | ~3 seconds | < 0.5 seconds |
| DB query (list 10k movies) | ~200ms | < 10ms |
