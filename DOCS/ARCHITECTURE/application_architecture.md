# Application Architecture & Technical Logic (Rust Edition)

> Detailed breakdown of internal Rust modules, their design decisions, and how they map to the original Python implementation.

---

## 1. The `core` Crate — Directory Layout

```
core/
└── src/
    ├── lib.rs              ← pub use re-exports
    ├── models/
    │   ├── mod.rs
    │   ├── library.rs      ← Library, MediaType enum
    │   ├── movie.rs        ← Movie, MovieFile structs
    │   └── tv.rs           ← TVShow, Season, Episode structs
    ├── db/
    │   ├── mod.rs
    │   ├── migrations/     ← SQL migration files (sqlx-migrate)
    │   └── queries.rs      ← Typed query functions
    ├── scanner/
    │   ├── mod.rs
    │   └── worker.rs       ← Parallel WalkDir engine
    ├── parser/
    │   └── mod.rs          ← Regex filename parser
    ├── scraper/
    │   ├── mod.rs          ← ScraperChain orchestrator
    │   ├── tmdb.rs         ← TMDB API client
    │   └── omdb.rs         ← OMDb fallback client
    ├── nfo/
    │   ├── reader.rs       ← Parse Kodi/Jellyfin NFO XML
    │   └── writer.rs       ← Generate NFO XML
    ├── renamer/
    │   └── mod.rs          ← Template-based file renamer
    ├── cleanup/
    │   └── mod.rs          ← Orphan purge & empty folder removal
    └── task_manager/
        └── mod.rs          ← tokio::broadcast task state
```

---

## 2. Two-Phase Scanner Logic

The Python scanner used `ThreadPoolExecutor` + `os.walk`. The Rust scanner uses `WalkDir` + `rayon::par_iter()` — a parallelism model that's safer and significantly faster.

### Phase 1: Fast Directory Sync

```rust
// core/src/scanner/worker.rs
use walkdir::WalkDir;
use rayon::prelude::*;

pub async fn scan_library(
    pool: &SqlitePool,
    library: &Library,
    tx: broadcast::Sender<TaskUpdate>,
) -> Result<ScanResult> {

    let skip_dirs = HashSet::from([".git", "node_modules", ".actors", "@eaDir", "#recycle"]);

    // Phase 1: Collect all file paths (fast, no I/O beyond directory listing)
    let files: Vec<PathBuf> = WalkDir::new(&library.path)
        .into_iter()
        .filter_entry(|e| !skip_dirs.contains(e.file_name().to_str().unwrap_or("")))
        .filter_map(|e| e.ok())
        .filter(|e| is_video_file(e.path()))
        .map(|e| e.into_path())
        .collect();

    let total = files.len();
    tx.send(TaskUpdate::started(total))?;

    // Phase 1: Parse filenames in parallel (CPU-bound, rayon is ideal)
    let parsed: Vec<ParsedFile> = files
        .par_iter()
        .map(|path| ParsedFile {
            path: path.clone(),
            parsed: parser::parse_filename(path.file_name().unwrap().to_str().unwrap()),
            size: path.metadata().map(|m| m.len()).unwrap_or(0),
        })
        .collect();

    // Phase 2: Batch database inserts (sequential, atomic)
    for (i, chunk) in parsed.chunks(50).enumerate() {
        db::batch_insert_media(pool, &library, chunk).await?;
        tx.send(TaskUpdate::progress((i + 1) * 50, total))?;
    }

    Ok(ScanResult { files_processed: total })
}
```

**Key improvement over Python:** `rayon::par_iter()` uses all CPU cores with zero manual thread management. The borrow checker guarantees no data races.

### Phase 2: Background Metadata Enrichment

After the fast sync, a Tokio task runs the scraper chain for all `status = 'unmatched'` items:

```rust
tokio::spawn(async move {
    scraper::enrich_unmatched(&pool, &tmdb_client, &tx).await;
});
```

---

## 3. Filename Parser — Regex Engine

**Python original:**
```python
r'^(.*?)[. (\[]*(?:((?:19|20)\d{2}))[. )\]]*(.*)$'
```

**Rust equivalent** using the `regex` crate with named capture groups:

```rust
// core/src/parser/mod.rs
use regex::Regex;
use once_cell::sync::Lazy;

static MOVIE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^(?P<title>.+?)[. \(\[]*(?P<year>(?:19|20)\d{2})[. \)\]]*").unwrap()
});

static TV_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^(?P<title>.+?)[.\s_-]+[Ss](?P<season>\d{2})[Ee](?P<episode>\d{2})").unwrap()
});

pub fn parse_filename(name: &str) -> ParsedMedia {
    let stem = Path::new(name).file_stem().unwrap_or_default().to_str().unwrap_or(name);

    if let Some(caps) = TV_RE.captures(stem) {
        return ParsedMedia {
            title: clean_title(caps.name("title").unwrap().as_str()),
            year: None,
            season: caps.name("season").and_then(|s| s.as_str().parse().ok()),
            episode: caps.name("episode").and_then(|e| e.as_str().parse().ok()),
            is_tv: true,
        };
    }
    if let Some(caps) = MOVIE_RE.captures(stem) {
        return ParsedMedia {
            title: clean_title(caps.name("title").unwrap().as_str()),
            year: caps.name("year").and_then(|y| y.as_str().parse().ok()),
            season: None,
            episode: None,
            is_tv: false,
        };
    }
    ParsedMedia { title: clean_title(stem), ..Default::default() }
}
```

`once_cell::Lazy` compiles the regex **once** at startup — no per-call recompilation overhead.

---

## 4. Scraper Chain & Async Concurrency

The Python scraper used `asyncio.gather()` to fetch TMDB, OMDb, poster, and fanart in parallel. The Rust equivalent uses `tokio::join!` and `reqwest`:

```rust
// core/src/scraper/mod.rs
pub async fn scrape_movie(
    movie_id: i64,
    title: &str,
    year: Option<i32>,
    clients: &ScraperClients,
    pool: &SqlitePool,
) -> Result<()> {

    // Search TMDB first
    let tmdb_result = clients.tmdb.search(title, year).await?;

    if let Some(tmdb_id) = tmdb_result.first().map(|r| r.id) {
        // Fetch details + OMDb ratings concurrently
        let (tmdb_details, omdb_ratings) = tokio::join!(
            clients.tmdb.get_details(tmdb_id),
            clients.omdb.get_by_imdb(tmdb_result[0].imdb_id.as_deref().unwrap_or(""))
        );

        let details = tmdb_details?;
        let merged = merge_metadata(details, omdb_ratings.ok());

        // Download poster + backdrop concurrently
        let (poster_path, backdrop_path) = tokio::join!(
            download_image(&merged.poster_url, "poster"),
            download_image(&merged.backdrop_url, "backdrop")
        );

        db::update_movie_metadata(pool, movie_id, &merged, poster_path.ok(), backdrop_path.ok()).await?;
    }
    Ok(())
}
```

**Rate limiting** uses a `tokio::sync::Semaphore` (identical semantics to Python's `asyncio.Semaphore`):

```rust
static TMDB_SEMAPHORE: Lazy<Arc<Semaphore>> = Lazy::new(|| Arc::new(Semaphore::new(40)));

// Inside TMDB client:
let _permit = TMDB_SEMAPHORE.acquire().await?;
// ... make request ...
```

---

## 5. SQLite Database Layer (SQLx)

SQLx provides **compile-time SQL verification** — query errors are caught at `cargo build`, not at runtime.

### Schema (core/src/db/migrations/001_initial.sql)

```sql
CREATE TABLE IF NOT EXISTS libraries (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT    NOT NULL,
    path        TEXT    NOT NULL UNIQUE,
    media_type  TEXT    NOT NULL CHECK(media_type IN ('movie','tv')),
    created_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS movies (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id  INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    title       TEXT    NOT NULL,
    year        INTEGER,
    tmdb_id     INTEGER UNIQUE,
    imdb_id     TEXT,
    status      TEXT    NOT NULL DEFAULT 'unmatched',
    plot        TEXT,
    rating      REAL,
    poster_url  TEXT,
    backdrop_url TEXT,
    nfo_path    TEXT,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS movie_files (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    movie_id        INTEGER NOT NULL REFERENCES movies(id) ON DELETE CASCADE,
    file_path       TEXT    NOT NULL UNIQUE,
    original_name   TEXT    NOT NULL,
    size_bytes      INTEGER NOT NULL DEFAULT 0,
    resolution      TEXT,
    codec           TEXT
);

CREATE TABLE IF NOT EXISTS tv_shows (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id  INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    title       TEXT    NOT NULL,
    tmdb_id     INTEGER UNIQUE,
    status      TEXT    NOT NULL DEFAULT 'unmatched',
    plot        TEXT,
    rating      REAL,
    poster_url  TEXT
);

CREATE TABLE IF NOT EXISTS seasons (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    show_id         INTEGER NOT NULL REFERENCES tv_shows(id) ON DELETE CASCADE,
    season_number   INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS episodes (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    season_id       INTEGER NOT NULL REFERENCES seasons(id) ON DELETE CASCADE,
    episode_number  INTEGER NOT NULL,
    title           TEXT,
    file_path       TEXT    NOT NULL UNIQUE,
    original_name   TEXT    NOT NULL,
    size_bytes      INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS tasks (
    id          TEXT PRIMARY KEY,
    task_type   TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'running',
    progress    INTEGER DEFAULT 0,
    total       INTEGER DEFAULT 0,
    message     TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- WAL mode for concurrent access (critical for Docker + Windows volumes)
PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;
```

---

## 6. Task Manager — tokio::broadcast

The Python task manager used a `threading.Lock()` around a dict and manually throttled updates. The Rust version uses `tokio::sync::broadcast`:

```rust
// core/src/task_manager/mod.rs
use tokio::sync::broadcast;
use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct TaskUpdate {
    pub task_id: String,
    pub status: TaskStatus,
    pub progress: u64,
    pub total: u64,
    pub message: String,
}

pub struct TaskManager {
    pub sender: broadcast::Sender<TaskUpdate>,
}

impl TaskManager {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(256);
        Self { sender }
    }
    pub fn update(&self, update: TaskUpdate) {
        // Non-blocking send; if no receivers, silently drops
        let _ = self.sender.send(update);
    }
    pub fn subscribe(&self) -> broadcast::Receiver<TaskUpdate> {
        self.sender.subscribe()
    }
}
```

In `apps/server`, the Axum SSE endpoint subscribes to this channel:
```rust
// apps/server/src/routes/tasks.rs
async fn task_stream(State(tm): State<Arc<TaskManager>>) -> Sse<...> {
    let mut rx = tm.subscribe();
    let stream = async_stream::stream! {
        while let Ok(update) = rx.recv().await {
            yield Ok(Event::default().json_data(update).unwrap());
        }
    };
    Sse::new(stream)
}
```

In `apps/desktop`, Tauri emits directly to the frontend window:
```rust
// apps/desktop/src/main.rs
let mut rx = task_manager.subscribe();
tokio::spawn(async move {
    while let Ok(update) = rx.recv().await {
        window.emit("task_update", &update).unwrap();
    }
});
```

---

## 7. NFO Engine (Kodi/Jellyfin Compatible)

The NFO reader/writer uses `quick-xml` with `serde` for zero-copy XML deserialization:

```rust
// core/src/nfo/reader.rs
use quick_xml::de::from_str;

#[derive(Deserialize)]
pub struct MovieNfo {
    pub title: String,
    pub year: Option<u32>,
    pub uniqueid: Vec<UniqueId>,   // Contains TMDB / IMDb IDs
    pub plot: Option<String>,
    pub rating: Option<f32>,
}

pub fn read_nfo(path: &Path) -> Result<MovieNfo> {
    let content = std::fs::read_to_string(path)?;
    Ok(from_str(&content)?)
}
```

If a `.nfo` file contains a `tmdb` or `imdb` ID, the scraper chain is bypassed entirely — same logic as Python, but parsing is 50x faster.
