# Media Manager – Master Implementation Plan

> **Stack:** Rust (Tokio + Axum + Tauri) · React (Vite + TypeScript) · SQLite (SQLx)  
> **Phases:** 5 MVP phases + 3 post-MVP phases  
> **Approach:** Build `core` first. Wrap second. UI last.

---

## Phase 0: Repository Bootstrap (Day 1)

**Goal:** A compilable, empty workspace that proves the structure works.

### Tasks
- [ ] Create `Media_Manager/` directory
- [ ] Write root `Cargo.toml` workspace manifest
- [ ] `cargo new --lib core`
- [ ] `cargo new apps/server`
- [ ] `cargo new apps/desktop` (will become Tauri later)
- [ ] `npm create vite@latest frontend -- --template react-ts`
- [ ] Write `frontend/src/api/adapter.ts` (empty stubs)
- [ ] Verify `cargo build` succeeds across all crates

### Root `Cargo.toml`
```toml
[workspace]
members = ["core", "apps/server", "apps/desktop"]
resolver = "2"

[workspace.dependencies]
# Shared versions — defined once, used everywhere
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sqlx = { version = "0.7", features = ["sqlite", "runtime-tokio", "macros", "migrate"] }
reqwest = { version = "0.11", features = ["json"] }
walkdir = "2"
rayon = "1"
regex = "1"
once_cell = "1"
quick-xml = { version = "0.31", features = ["serialize"] }
anyhow = "1"
thiserror = "1"
uuid = { version = "1", features = ["v4"] }
tracing = "1"
tracing-subscriber = "1"
```

### `core/Cargo.toml`
```toml
[package]
name = "core"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
sqlx.workspace = true
reqwest.workspace = true
walkdir.workspace = true
rayon.workspace = true
regex.workspace = true
once_cell.workspace = true
quick-xml.workspace = true
anyhow.workspace = true
thiserror.workspace = true
uuid.workspace = true
tracing.workspace = true
```

---

## Phase 1: Database Layer & Models (Week 1)

**Goal:** SQLite schema is defined, migrations run, and basic CRUD is verified.

### Tasks
- [ ] Write `core/src/db/migrations/001_initial.sql` (full schema from architecture doc)
- [ ] Enable WAL mode in `PRAGMA` at connection time
- [ ] Write `core/src/models/` structs for `Library`, `Movie`, `MovieFile`, `TVShow`, `Season`, `Episode`, `Task`
- [ ] All structs implement `serde::Serialize + Deserialize`
- [ ] Write typed `sqlx::query_as!` functions in `core/src/db/queries.rs`:
  - `get_all_movies(pool, library_id)` → `Vec<Movie>`
  - `insert_movie(pool, title, year, library_id)` → `i64`
  - `get_all_libraries(pool)` → `Vec<Library>`
  - `insert_library(pool, name, path, media_type)` → `i64`
- [ ] Write unit tests for all queries using `sqlx::test`

### Key Design Decision: No ORM
SQLx's `query_as!` macro verifies SQL against a live SQLite file at compile time. This catches schema errors during `cargo build`, not in production.

---

## Phase 2: Smart Scanner & Parser (Week 1–2)

**Goal:** Given a folder path, discover all video files and populate the database.

### Tasks
- [ ] `core/src/parser/mod.rs`: regex engine with `MOVIE_RE` and `TV_RE`
  - [ ] `parse_filename(name: &str) -> ParsedMedia`
  - [ ] `clean_title(raw: &str) -> String` (replace dots/underscores with spaces)
  - [ ] Unit tests for 20+ messy filenames
- [ ] `core/src/scanner/worker.rs`:
  - [ ] `scan_library(pool, library, tx) -> Result<ScanResult>`
  - [ ] WalkDir traversal with skip list
  - [ ] `rayon::par_iter()` for parallel parse phase
  - [ ] Batch DB insert (50-item chunks)
  - [ ] `progress_tx.send()` after each batch
- [ ] `core/src/task_manager/mod.rs`:
  - [ ] `TaskManager` struct with `broadcast::Sender<TaskUpdate>`
  - [ ] `TaskUpdate` struct (task_id, status, progress, total, message)

### Supported Extensions (from Python `SUPPORTED_EXTS`)
```rust
const VIDEO_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "avi", "mov", "wmv", "m4v", "ts", "m2ts", "mts",
    "mpg", "mpeg", "vob", "divx", "xvid", "webm", "flv", "ogv", "iso",
];
```

---

## Phase 3: Scraper Chain (Week 2–3)

**Goal:** For every `status='unmatched'` item in the DB, fetch rich metadata from TMDB.

### Tasks
- [ ] `core/src/scraper/tmdb.rs`:
  - [ ] `TmdbClient::new(api_key)` — builds `reqwest::Client` with base URL
  - [ ] `search_movie(title, year) -> Vec<TmdbSearchResult>`
  - [ ] `get_movie_details(tmdb_id) -> TmdbMovieDetails`
  - [ ] Semaphore: `Arc<Semaphore::new(40)>` for rate limiting
- [ ] `core/src/scraper/omdb.rs`:
  - [ ] `OmdbClient::new(api_key)`
  - [ ] `get_ratings(imdb_id) -> OmdbRatings`
- [ ] `core/src/scraper/mod.rs` (Chain orchestrator):
  - [ ] `enrich_unmatched(pool, clients, tx)` — background worker
  - [ ] `tokio::join!` for parallel TMDB details + OMDb ratings
  - [ ] Image download with `reqwest::get()` → write to `config/artwork/`
- [ ] `core/src/nfo/reader.rs`:
  - [ ] `read_nfo(path) -> Option<NfoData>` — skip scraping if TMDB ID found

---

## Phase 4: API Layer (Week 3–4)

**Goal:** Expose all core functions via Axum (Docker) and Tauri commands (Desktop).

### `apps/server` (Axum)

#### `apps/server/Cargo.toml`
```toml
[package]
name = "server"
version = "0.1.0"
edition = "2021"

[dependencies]
core = { path = "../../core" }
axum = { version = "0.7", features = ["macros"] }
axum-extra = { version = "0.9", features = ["typed-header"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["fs", "cors", "compression-gzip"] }
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
sqlx.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
```

#### Routes to implement
```
GET  /api/health
GET  /api/libraries          → db::get_all_libraries()
POST /api/libraries          → db::insert_library()
GET  /api/movies             → db::get_all_movies() [with ?limit=&offset=&search=]
GET  /api/movies/:id         → db::get_movie_by_id()
GET  /api/tvshows            → db::get_all_shows()
POST /api/libraries/:id/scan → tokio::spawn(scanner::scan_library())
POST /api/scrape/bulk        → tokio::spawn(scraper::enrich_unmatched())
GET  /api/tasks              → db::get_tasks()
GET  /api/tasks/stream       → SSE stream from TaskManager
```

### `apps/desktop` (Tauri)

#### Setup
```bash
cargo install tauri-cli
cargo tauri init  # in apps/desktop/
```

#### Tauri commands to implement
```rust
#[tauri::command]
async fn get_movies(state: State<'_, AppState>) -> Result<Vec<Movie>, String> {
    db::get_all_movies(&state.pool, None).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_scan(library_id: i64, window: Window, state: State<'_, AppState>) -> Result<(), String> {
    let pool = state.pool.clone();
    let tx = state.task_manager.sender.clone();
    tokio::spawn(async move {
        // ... send events to window
    });
    Ok(())
}
```

---

## Phase 5: Frontend Integration & Packaging (Week 4–5)

**Goal:** The React UI works in both browser (Docker) and Tauri WebView. Both packaging targets produce valid artifacts.

### Frontend Tasks
- [ ] Add `@tauri-apps/api` npm dependency
- [ ] Implement `frontend/src/api/adapter.ts` (complete)
- [ ] Replace all direct `fetch('/api/...')` calls with adapter calls
- [ ] Test in browser (Vite dev server proxied to Axum) — all features work
- [ ] Test in Tauri dev mode (`cargo tauri dev`) — all features work

### Docker Packaging
- [ ] Write multi-stage `Dockerfile` (see architecture doc)
- [ ] Write `docker-compose.yml` with volume mappings
- [ ] `docker build .` → verify image < 60MB
- [ ] `docker compose up` → access at `http://localhost:7878`

### Tauri / Windows Packaging
- [ ] Configure `tauri.conf.json` (app name, identifier, window size)
- [ ] `cargo tauri build` → generates `.msi` and `setup.exe`
- [ ] Test `.msi` install on clean Windows VM
- [ ] Verify app runs with zero external dependencies

---

## Phase 6 (Post-MVP): Management Tools

- [ ] `core/src/renamer/mod.rs` — template-based file renamer
- [ ] `core/src/cleanup/mod.rs` — orphan purge, empty folder removal
- [ ] `core/src/scanner/mediainfo.rs` — `ffprobe` subprocess for resolution/codec
- [ ] API routes for all of the above

## Phase 7 (Post-MVP): Subtitle & Export

- [ ] `core/src/subtitles/mod.rs` — OpenSubtitles hash + download
- [ ] `core/src/exporter/mod.rs` — JSON/CSV/HTML export

## Phase 8 (Post-MVP): Playback & Monitoring

- [ ] Hybrid playback (native `open` command / HTTP stream)
- [ ] Real-time directory monitoring via `notify` crate
- [ ] Tdarr + Radarr/Sonarr webhook receivers

---

## Tech Stack Reference Card

| Layer | Technology | Version | Replaces |
|---|---|---|---|
| Async runtime | Tokio | 1.x | asyncio + threading |
| HTTP server | Axum | 0.7 | FastAPI |
| Desktop wrapper | Tauri | 2.x | PyWebView + PyInstaller |
| Database driver | SQLx | 0.7 | SQLAlchemy |
| DB migrations | sqlx-migrate | 0.7 | Alembic |
| File traversal | WalkDir | 2.x | os.walk |
| CPU parallelism | Rayon | 1.x | ThreadPoolExecutor |
| HTTP client | reqwest | 0.11 | httpx |
| Regex | regex crate | 1.x | re module |
| XML (NFO) | quick-xml | 0.31 | xml.etree |
| Serialization | serde + serde_json | 1.x | pydantic + json |
| Logging | tracing | 0.1 | logging module |
| Frontend build | Vite 5 + React 18 | latest | same (reused) |
| UI state | Zustand | 4.x | same (reused) |
| Styling | Tailwind CSS | 3.x | same (reused) |
| Animations | Framer Motion | 11.x | same (reused) |
| Data fetching | TanStack Query | 5.x | same (reused) |
