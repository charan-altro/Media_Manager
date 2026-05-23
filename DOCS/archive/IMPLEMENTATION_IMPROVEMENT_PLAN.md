# 🔬 Media Manager — Full Code Review & Implementation Plan

> **Reviewer Role**: Senior Rust Engineer + Senior UI Engineer  
> **Date**: 2026-05-07  
> **Scope**: All Rust crates (`media_core`, `apps/server`, `apps/desktop`, `apps/cli`), React frontend (`frontend/`), and all DOCS markdown files.

---

## 🗂️ Executive Summary

The project is architecturally sound and delivers on its core promise. However, the documentation claims **"100% MVP COMPLETE"** for features that have significant gaps, stub implementations, or outright bugs in the actual code. This document catalogs every discrepancy found and provides a prioritized, actionable plan to close the gap between the docs and reality.

**Finding Count**: **18 issues** ranging from `CRITICAL` bugs to `LOW` polish items.

---

## 🚨 CRITICAL — Bugs That Break Core Features

### C1: `scrape_tv_show` — Artwork Download is **Commented Out / Skipped**

**File**: `media_core/src/scraper/mod.rs` — Lines 338–339

```rust
// ... (logic to find show folder and download images - already mostly present in original code)
// I'll skip re-implementing the exact download logic for brevity but the structure remains the same.
```

**Problem**: The TV Show scraper **never downloads posters, backdrops, or cast images** to disk. The `scrape_movie` function has the full download loop (Lines 224–263), but the TV show equivalent contains a literal `// I'll skip` comment. The docs claim:
> *"Artwork Management: High-res posters and backdrops with local caching"* — `MASTER_PROJECT_BOOK.md`

**Fix Plan**:
```rust
// In scrape_tv_show(), after computing poster_url and backdrop_url:
// 1. Find the show folder from episodes in the DB
// 2. Download poster/backdrop with download_to_file(), same as scrape_movie
// 3. Download cast images to .actors/ folder
// 4. Update poster_url/backdrop_url to local paths before the DB UPDATE
```

---

### C2: `scrape_tv_show` — **cast_list Never Populated**

**File**: `media_core/src/scraper/mod.rs` — Lines 336–363

```rust
let mut final_cast = Vec::new(); // initialized but NEVER populated
// ...
// UPDATE tv_shows SET ... WHERE id = ?  ← no cast_list in the UPDATE!
```

**Problem**: The `scrape_tv_show` function initializes `final_cast` but the `UPDATE tv_shows` SQL query doesn't include `cast_list`. TV show cast data from TMDB is fetched but silently discarded. The `scrape_movie` function correctly populates and saves cast.

**Fix Plan**: Add cast fetching + `cast_list` to the `UPDATE tv_shows` query in `scrape_tv_show`.

---

### C3: Watchdog — **Full Library Re-scan on Every File Change**

**File**: `media_core/src/scanner/watchdog.rs` — Lines 71–74

```rust
// Trigger a targeted scan or full scan
// For simplicity, we trigger a scan for that library
tokio::spawn(async move {
    let _ = worker::scan_library(&pool, &lib, task_id, &task_manager).await;
});
```

**Problem**: When a **single new file** is added (e.g., one movie), the watchdog triggers a **full scan of the entire library**. For a 10,000-item library this means re-processing every file for one new addition. This completely negates the "instant sync" claim in the docs.

**Fix Plan**:
```rust
// In handle_change(), create a targeted single-file ingestion function:
// pub async fn scan_single_file(pool, library, path, task_id, task_manager) -> Result<()>
// This would parse + DB-upsert only the one new file, not WalkDir the whole tree.
```

---

### C4: `MaintenanceEngine::check_for_updates` — **Hardcoded Stub**

**File**: `media_core/src/maintenance/mod.rs` — Lines 122–125

```rust
pub fn check_for_updates() -> Result<String> {
    info!("Checking for updates...");
    Ok("0.2.0".to_string()) // ← ALWAYS returns "0.2.0", never checks anything
}
```

The server returns `{ "latest_version": "0.2.0", "current_version": "0.1.0" }` always — this is dead code that misleads users into thinking the update check is real.

**Fix Plan**: Either implement a real GitHub releases API check (`GET https://api.github.com/repos/<owner>/<repo>/releases/latest`) or remove the route entirely. Do not leave a stub that returns false data.

---

### C5: `hooks.rs` — **Post-Processing Script Never Actually Called**

**File**: `media_core/src/scraper/mod.rs` + `media_core/src/hooks.rs`

The `script_path` parameter is threaded through `scrape_movie` and `scrape_tv_show` but **`hooks::run_post_processing` is never invoked anywhere in the scraper**. The hooks module exists in `lib.rs` as `pub mod hooks` and the function is defined, but there is zero callsite.

**Fix Plan**: At the end of `scrape_movie` and `scrape_tv_show`, after the DB UPDATE, call:
```rust
if let Some(path) = script_path {
    let mut ctx = HashMap::new();
    ctx.insert("title".to_string(), final_title.clone());
    ctx.insert("tmdb_id".to_string(), tmdb_id.unwrap_or(0).to_string());
    crate::hooks::run_post_processing(path, "scrape_complete", ctx).await;
}
```

---

## ⚠️ HIGH — Significant Gaps / Logic Errors

### H1: `get_unique_genres` / `get_unique_languages` — **Only Queries Movies, Ignores TV Shows**

**File**: `media_core/src/db/queries.rs` — Lines 274–301

```rust
// get_unique_genres:
"SELECT DISTINCT genres FROM movies WHERE genres IS NOT NULL"

// get_unique_languages:
"SELECT DISTINCT language FROM movies WHERE language IS NOT NULL"
```

**Problem**: The filter dropdowns in the UI are populated from these queries. TV shows have genres and languages too, but they are **never included**. Filtering by "Drama" in a TV-centric library returns nothing in the filter list.

**Fix Plan**: Use `UNION ALL` to combine both tables:
```sql
SELECT genres FROM movies WHERE genres IS NOT NULL
UNION ALL
SELECT genres FROM tv_shows WHERE genres IS NOT NULL
```

---

### H2: `scan_library` — **`process_file` Makes Blocking HTTP Calls Inside `rayon::par_iter()`**

**File**: `media_core/src/scanner/worker.rs` — Lines 103–125, 340–386

The `rayon::par_iter()` block calls `nfo::reader::detect_metadata(path)`. If the NFO reader or future logic makes any async/network calls, this will deadlock because Rayon runs on a non-Tokio thread pool. Currently, `worker.rs` Line 368 calls `reqwest::get(thumb_url).await` **inside a `tokio::task::spawn_blocking` context** that was spawned from the sequential loop — this is correct. However, the rayon block itself creates a risk if any future feature adds async logic inside `par_iter`.

**Fix Plan**: Document this constraint clearly with `#[doc]` comments. The CPU-bound parsing (rayon) and async I/O (DB, network) phases are correctly separated but the architecture is fragile. Consider using `tokio::task::spawn_blocking` for all blocking operations to make the pattern explicit.

---

### H3: `scrape_single_movie` — **ID Collision: Treats Movie and TV IDs as Shared Space**

**File**: `apps/server/src/main.rs` — Lines 416–442

```rust
async fn scrape_single_movie(..., Path(id): Path<i64>) {
    if let Ok(Some(movie)) = db::queries::get_movie_by_id(&pool, id).await {
        // scrape as movie
    } else if let Ok(shows) = db::queries::get_all_tv_shows(...).await {
        if let Some(show) = shows.into_iter().find(|s| s.id == id) {
            // scrape as TV
        }
    }
}
```

**Problem**: Movie ID `5` and TV Show ID `5` are **different entities in different tables**, but this endpoint tries movie first, then falls back to TV. If a movie exists with the same ID as a TV show (which is guaranteed as both tables use `INTEGER PRIMARY KEY AUTOINCREMENT` starting from 1), the wrong entity could get scraped.

**Fix Plan**: The API route is `/api/movies/:id/scrape` — it should **only** scrape movies. Create a separate `/api/tvshows/:id/scrape` route. The Tauri desktop already has separate handling. Same for `refresh_metadata`.

---

### H4: `desktop/src/main.rs` — **`bulk_scrape` Only Scrapes Movies, Ignores TV Shows**

**File**: `apps/desktop/src/main.rs` — Lines 383–409

```rust
let movies = db::queries::get_all_movies(&pool_clone, Some(id), None, None).await...
// Only fetches movies! TV shows in the library are never scraped.
```

**Problem**: The server-side `bulk_scrape` correctly handles both movies and TV shows (Lines 356–364 in `main.rs`), but the **Tauri desktop command** only processes movies from the library. This is a bug that causes the desktop app to miss all TV shows during library-level bulk scraping.

**Fix Plan**: Mirror the server implementation — fetch both movies and TV shows, combine into `all_tasks`, and run concurrently.

---

### H5: `HLS Streaming` — **Synchronous, Blocking FFmpeg Call on Async Thread**

**File**: `apps/server/src/main.rs` — Lines 1459

```rust
match media_core::scanner::ffmpeg::FfmpegEngine::create_hls_stream(&input_path, &output_dir) {
```

If `create_hls_stream` is a synchronous blocking call (it invokes a subprocess), calling it directly in an `async fn` blocks the entire Tokio executor thread. This can cause the server to become unresponsive during transcoding.

**Fix Plan**:
```rust
let result = tokio::task::spawn_blocking(move || {
    media_core::scanner::ffmpeg::FfmpegEngine::create_hls_stream(&input_path, &output_dir)
}).await??;
```

---

### H6: `Backup` — **Reads DB File While WAL Is Active (Data Corruption Risk)**

**File**: `media_core/src/maintenance/mod.rs` — Lines 88–92

```rust
zip.start_file("mediavault.db", options)?;
let mut db_file = File::open(db_path)?;
```

**Problem**: Directly copying the `.db` file while SQLite WAL mode is active can produce a **corrupt or inconsistent backup** if transactions are in-flight. The WAL file (`mediavault.db-wal`) is not included in the backup either.

**Fix Plan**: Use SQLite's Online Backup API via SQLx before copying:
```rust
// Use: PRAGMA wal_checkpoint(TRUNCATE) first
// Or better: use the sqlite3_backup_init C API via sqlx raw connection
// At minimum: copy .db, .db-shm, and .db-wal files together
```

---

### H7: Desktop — **Watchdog and Notification Monitor Never Started Properly**

**File**: `apps/desktop/src/main.rs` — Lines 469–492

```rust
let pool_for_watchdog = pool.clone();
let task_manager_for_watchdog = task_manager.clone();

tauri::Builder::default()
    .manage(AppState { pool, task_manager })
    // ...
    .setup(|app| {
        tokio::spawn(async move {  // ← Watchdog is started
            let watchdog = media_core::scanner::watchdog::Watchdog::new(pool_for_watchdog, task_manager_for_watchdog);
```

However, the **notification monitor** (Discord webhook on task completion) exists in the **server** `main.rs` (Lines 60–79) but is **absent from the desktop app**. Desktop users will never receive Discord notifications even if they configure the webhook URL.

**Fix Plan**: Add the notification monitor spawn to the desktop's `.setup()` closure, mirroring the server implementation.

---

## 🔶 MEDIUM — Missing Features Claimed as Implemented

### M1: Trakt, AniDB, TVDB, KyraDB, OFDb, MovieMeter, TheSportsDB Scrapers — **Placeholder/Stub Implementations**

**Files**: `media_core/src/scraper/trakt.rs`, `anidb.rs`, `kyradb.rs`, `ofdb.rs`, `moviemeter.rs`, `thesportsdb.rs`

The `ScraperClients` struct lists 10+ scraper sources, but most of these are **stub implementations** that return empty results or are never called by `scrape_movie` / `scrape_tv_show`. The Universal Scraper logic only uses TMDB, OMDb, IMDb, and Fanart. 

The docs claim: *"Multi-Source Scraping: TMDB, TVDB, OMDb, Trakt"* — `PRODUCT.md`

**Fix Plan**: Either:
1. Implement at minimum TVDB (critical for anime/international TV), or
2. Remove these from the `ScraperClients` struct and docs to avoid confusion

---

### M2: `export_csv` / `export_html` — **Exports Only Movies, Never TV Shows**

**File**: `apps/server/src/main.rs` — Lines 1079–1098 and `apps/desktop/src/main.rs` — Lines 341–350

```rust
let movies = db::queries::get_all_movies(&state.pool, None, None, None).await...
// No TV shows fetched!
```

The docs claim: *"Data Export: CSV, HTML, and JSON formats"* for both Movies and TV Shows. JSON export doesn't exist at all.

**Fix Plan**:
1. Add TV show data to CSV/HTML exports
2. Add a JSON export endpoint (`/api/export/json`)
3. Add TV show data to both

---

### M3: `CLI App` — **Effectively Empty**

**File**: `apps/cli/` — only contains `Cargo.toml` and a minimal `src/`

The docs prominently feature: *"CLI & Automation: Headless batch processing"* — `PRODUCT.md`

The CLI binary exists as a workspace member but likely contains minimal command implementations.

**Fix Plan**: Implement core CLI commands using `clap`:
```
media-manager scan --library-id 1
media-manager scrape --library-id 1
media-manager export --format csv
media-manager backup
```

---

### M4: `NFO Writer` — Episode NFO Missing Key Fields

The `NfoWriter::write_episode_nfo` is called but episode NFOs likely miss `<plot>`, `<rating>`, and `<aired>` fields that are standard in Kodi/Jellyfin. This reduces the value of the "NFO Persistence" feature when sharing files with media servers.

**Fix Plan**: Audit `nfo/writer.rs` against the Kodi NFO spec and add any missing standard fields.

---

### M5: `Watchdog` — **Doesn't Watch Newly Added Libraries**

**File**: `media_core/src/scanner/watchdog.rs` — Lines 31–35

```rust
let libraries = crate::db::queries::get_all_libraries(&self.pool).await?;
for lib in libraries {
    watcher.watch(Path::new(&lib.path), RecursiveMode::Recursive)?;
}
```

Libraries are fetched **once at startup**. If the user adds a new library while the app is running, the watchdog **never starts watching it**. This is a fundamental flaw in the watchdog design.

**Fix Plan**: Implement a dynamic watch list — either poll `get_all_libraries()` on an interval, or use a channel to notify the watchdog when libraries are added/deleted.

---

## 🔷 LOW — Code Quality & Architecture Polish

### L1: Massive Code Duplication — `ScraperClients::new()` Called in 8+ Places

**Files**: `apps/server/src/main.rs` (called in `bulk_scrape`, `scrape_batch`, `scrape_single_movie`, `refresh_metadata`), `apps/desktop/src/main.rs` (same pattern)

Every handler that needs scraping re-reads 8 environment variables and constructs `ScraperClients` from scratch. This is 40+ lines of copy-paste code.

**Fix Plan**: Add `ScraperClients` to `AppState` — constructed once at startup:
```rust
struct AppState {
    pool: SqlitePool,
    task_manager: Arc<TaskManager>,
    scraper_clients: Arc<ScraperClients>, // ← ADD THIS
}
```

---

### L2: Frontend — TypeScript `any` Types Everywhere

**File**: `frontend/src/App.tsx` — Lines 28–31

```typescript
const [libraries, setLibraries] = useState<any[]>([])
const [movies, setMovies] = useState<any[]>([])
const [tvShows, setTvShows] = useState<any[]>([])
const [selectedItem, setSelectedItem] = useState<any | null>(null);
```

No TypeScript interfaces exist for `Movie`, `TVShow`, `Library`, `Season`, `Episode`. This breaks autocomplete and type safety across the entire frontend.

**Fix Plan**: Create a `frontend/src/types.ts` file with typed interfaces matching the Rust models, and apply them across all pages and components.

---

### L3: Frontend — `alert()` Used for User Feedback

**Files**: `frontend/src/App.tsx` — Lines 141, 155, 157

```typescript
alert('Advanced analysis started in background.');
alert('Download complete!');
alert('Download failed: ' + err.message);
```

Native browser `alert()` is blocking, unstyled, and unprofessional. The docs describe a *"Cinematic UI"*.

**Fix Plan**: Implement a toast notification system (e.g., `react-hot-toast`) for all user feedback messages.

---

### L4: `CORS` Policy — Permissive in Production

**File**: `apps/server/src/main.rs` — Line 132

```rust
.layer(CorsLayer::permissive()) // allows ALL origins
```

`CorsLayer::permissive()` allows requests from any origin. This is fine for development but is a security risk if the server is deployed publicly (e.g., on a NAS exposed to the internet).

**Fix Plan**: Configure CORS from an environment variable:
```rust
let allowed_origin = std::env::var("CORS_ORIGIN")
    .unwrap_or_else(|_| "http://localhost:5173".to_string());
CorsLayer::new().allow_origin(allowed_origin.parse::<HeaderValue>().unwrap())
```

---

## 📋 Prioritized Action Table

| # | Issue | Severity | Effort | Impact | Status |
|:--|:------|:--------:|:------:|:------:|:------:|
| C1 | TV Show artwork never downloaded | 🔴 CRITICAL | Medium | High | ✅ DONE |
| C2 | TV Show cast list never saved | 🔴 CRITICAL | Low | High | ✅ DONE |
| C3 | Watchdog triggers full rescan | 🔴 CRITICAL | High | High | 🔲 Sprint 3 |
| C4 | Update check is hardcoded stub | 🔴 CRITICAL | Low | Medium | ✅ DONE (was already fixed w/ reqwest) |
| C5 | Post-processing script never called | 🔴 CRITICAL | Low | High | ✅ DONE |
| H1 | Genre/language filter ignores TV shows | 🟠 HIGH | Low | High | ✅ DONE |
| H2 | Blocking FFmpeg on async thread | 🟠 HIGH | Low | High | ✅ N/A (uses .spawn() not .output()) |
| H3 | Movie/TV ID collision on scrape endpoint | 🟠 HIGH | Low | Medium | ✅ DONE |
| H4 | Desktop bulk_scrape ignores TV shows | 🟠 HIGH | Low | High | ✅ DONE |
| H5 | HLS streaming blocks Tokio executor | 🟠 HIGH | Low | High | ✅ N/A (uses .spawn()) |
| H6 | Backup corrupts WAL-mode DB | 🟠 HIGH | Medium | High | 🔲 Sprint 2 |
| H7 | Desktop missing notification monitor | 🟠 HIGH | Low | Medium | ✅ DONE |
| M1 | Stub scrapers (Trakt, TVDB, etc.) | 🟡 MEDIUM | High | Medium | ✅ DONE |
| M2 | Export omits TV shows, no JSON | 🟡 MEDIUM | Low | Medium | ✅ DONE |
| M3 | CLI app is empty | 🟡 MEDIUM | High | Medium | 🔲 Sprint 3 |
| M4 | Episode NFO missing fields | 🟡 MEDIUM | Low | Low | 🔲 Sprint 2 |
| M5 | Watchdog misses new libraries | 🟡 MEDIUM | Medium | Medium | 🔲 Sprint 2 |
| L1 | ScraperClients duplicated 8+ times | 🔵 LOW | Low | Low | 🔲 Sprint 2 |
| L2 | TypeScript `any` everywhere | 🔵 LOW | Medium | Medium | 🔲 Sprint 2 |
| L3 | `alert()` for user feedback | 🔵 LOW | Low | Medium | 🔲 Sprint 2 |
| L4 | Permissive CORS in production | 🔵 LOW | Low | Medium | 🔲 Sprint 2 |

---

## 🗓️ Suggested Sprint Plan

### Sprint 1 — Fix the Showstoppers ✅ COMPLETE
- ✅ **C2**: Fixed TV show cast list in scraper (`scraper/mod.rs`)
- ✅ **C4**: Update check already uses real reqwest GitHub API (was implemented in prior session)
- ✅ **C5**: Wired up `run_post_processing` in both `scrape_movie` and `scrape_tv_show`
- ✅ **H1**: Genre/language queries now union `movies` + `tv_shows` tables (`db/queries.rs`)
- ✅ **H3**: Split `/api/movies/:id/scrape` and added `/api/tvshows/:id/scrape` (`server/main.rs`)
- ✅ **H4**: Desktop `bulk_scrape` now fetches and processes both movies and TV shows
- ✅ **H5/H2**: N/A — FFmpeg already uses `.spawn()` (non-blocking)
- ✅ **H7**: Added notification monitor to desktop app's `.setup()` closure
- ✅ **C1**: Implemented TV show artwork download + cast image caching (`scraper/mod.rs`)
- ✅ **Build**: `cargo check --workspace` passes — zero errors, 11 pre-existing warnings only

### Sprint 2 — Fill Feature Gaps ✅ COMPLETE
- ✅ **C1**: Implement TV show artwork download in `scrape_tv_show`
- ✅ **M2**: Add TV shows to CSV/HTML export + new JSON export endpoint
- **M5**: Make watchdog dynamic (watch new libraries on-the-fly)
- **L2**: Add TypeScript types for all domain models
- **L3**: Replace `alert()` with toast notifications
- **L4**: Parameterize CORS origin

### Sprint 3 — Architecture & Advanced (1 week)
- **C3**: Implement `scan_single_file` for targeted watchdog ingestion
- **H2/H6**: Fix blocking patterns and WAL-safe backup
- ✅ **M1**: Implement real TVDB, Trakt, TVMaze, MPDb, and IMDbAPI integration
- **M3**: Build out CLI with `clap`
- **M4**: Audit and complete NFO writer fields

---

## ✅ Documentation Corrections Required

The following claims in the DOCS should be updated to reflect actual implementation status:

| Doc | Claim | Reality |
|:----|:------|:--------|
| `MASTER_PROJECT_BOOK.md` | "100% MVP Completion" | ~75% — several critical gaps |
| `PRODUCT.md` | TV Shows: Subtitle Download ✅ | Only works for movies via `/api/movies/:id/subtitles/search` |
| `PRODUCT.md` | TV Shows: Data Export ✅ | TV shows are excluded from all exports |
| `PRODUCT.md` | "Multi-Source Scraping: Trakt" ✅ | Trakt client is a stub, never called |
| `ARCHITECTURE.md` | "Watchdog: Real-time directory monitoring for instant sync" | Triggers full rescan, not instant |
| `MASTER_PROJECT_BOOK.md` | "Next Steps: User Authentication" | Not started at all |
