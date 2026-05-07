# Python → Rust Migration Plan

> **Source:** `SelfHost_Media_Orchestrator_PYTHON`  
> **Target:** `Media_Manager` (Rust + Tauri + Axum)  
> **Strategy:** Feature-parity migration with architectural improvements

---

## Migration Philosophy

This is **not** a line-for-line port. It is a principled re-architecture that:
1. Preserves 100% of the Python app's **features** and **behaviors**
2. Replaces Python's concurrency model (`threading` + `asyncio` mix) with Rust's unified `tokio` async runtime
3. Eliminates the Docker-on-Windows SQLite locking bugs by using WAL mode from the start
4. Delivers Phase 14 of the Python roadmap (native Windows `.exe`) as a **first-class target**, not an afterthought

---

## Module-by-Module Translation Table

| Python File | Lines | Rust Target | Priority |
|---|---|---|---|
| `services/scanner.py` | 197 | `core/src/scanner/worker.rs` | 🔴 MVP Critical |
| `services/parser.py` | ~80 | `core/src/parser/mod.rs` | 🔴 MVP Critical |
| `services/nfo_reader.py` | 183 | `core/src/nfo/reader.rs` | 🔴 MVP Critical |
| `services/nfo.py` | ~300 | `core/src/nfo/writer.rs` | 🟡 Phase 2 |
| `services/scraper/tmdb.py` | ~130 | `core/src/scraper/tmdb.rs` | 🔴 MVP Critical |
| `services/scraper/omdb.py` | ~55 | `core/src/scraper/omdb.rs` | 🟡 Phase 2 |
| `services/scraper/chain.py` | ~210 | `core/src/scraper/mod.rs` | 🔴 MVP Critical |
| `services/cleanup.py` | 550 | `core/src/cleanup/mod.rs` | 🟡 Phase 2 |
| `services/renamer.py` | ~145 | `core/src/renamer/mod.rs` | 🟡 Phase 2 |
| `services/artwork.py` | ~135 | `core/src/scraper/artwork.rs` | 🟡 Phase 2 |
| `services/subtitles.py` | ~100 | `core/src/subtitles/mod.rs` | 🟢 Phase 3 |
| `services/exporter.py` | ~165 | `core/src/exporter/mod.rs` | 🟢 Phase 3 |
| `services/mediainfo.py` | ~55 | `core/src/scanner/mediainfo.rs` | 🟡 Phase 2 |
| `core/db.py` (SQLAlchemy) | — | `core/src/db/` (SQLx) | 🔴 MVP Critical |
| `core/task_manager.py` | — | `core/src/task_manager/mod.rs` | 🔴 MVP Critical |
| `models/media.py` | — | `core/src/models/` | 🔴 MVP Critical |
| `api/movies.py` | 460 | `apps/server/src/routes/movies.rs` | 🔴 MVP Critical |
| `api/libraries.py` | — | `apps/server/src/routes/libraries.rs` | 🔴 MVP Critical |
| `api/tvshows.py` | 340 | `apps/server/src/routes/tv.rs` | 🔴 MVP Critical |
| `api/tasks_api.py` | — | `apps/server/src/routes/tasks.rs` | 🔴 MVP Critical |
| `api/nfo_api.py` | — | `apps/server/src/routes/nfo.rs` | 🟡 Phase 2 |
| `api/artwork_api.py` | — | `apps/server/src/routes/artwork.rs` | 🟡 Phase 2 |
| `api/export_api.py` | — | `apps/server/src/routes/export.rs` | 🟢 Phase 3 |
| `api/settings_api.py` | — | `apps/server/src/routes/settings.rs` | 🟡 Phase 2 |
| `api/media_extras.py` | — | `apps/server/src/routes/extras.rs` | 🟡 Phase 2 |

---

## API Surface Mapping

All existing Python FastAPI endpoints must be preserved for frontend compatibility.

| Python Endpoint | Method | Rust Axum Route |
|---|---|---|
| `/api/health` | GET | `GET /api/health` |
| `/api/libraries` | GET/POST/PATCH/DELETE | `Router` on `/api/libraries` |
| `/api/libraries/{id}/scan` | POST | `POST /api/libraries/:id/scan` |
| `/api/movies` | GET | `GET /api/movies` |
| `/api/movies/{id}` | GET/PATCH/DELETE | `/api/movies/:id` |
| `/api/movies/{id}/scrape` | POST | `POST /api/movies/:id/scrape` |
| `/api/tvshows` | GET | `GET /api/tvshows` |
| `/api/tvshows/{id}` | GET/PATCH | `/api/tvshows/:id` |
| `/api/scan` | POST | `POST /api/scan` |
| `/api/scrape/bulk` | POST | `POST /api/scrape/bulk` |
| `/api/tasks` | GET/DELETE | `/api/tasks` |
| `/api/tasks/stream` | GET (SSE) | `GET /api/tasks/stream` |
| `/api/export` | GET | `GET /api/export` |
| `/api/media/download/movie/{id}` | GET | `GET /api/media/download/movie/:id` |
| `/api/nfo/generate` | POST | `POST /api/nfo/generate` |
| `/api/artwork` | GET/POST | `/api/artwork` |

---

## Concurrency Model Translation

### Python Pattern → Rust Pattern

```
Python:                          Rust:
-------                          -----
asyncio.gather()              →  tokio::join!()
asyncio.Semaphore(40)         →  tokio::sync::Semaphore::new(40)
ThreadPoolExecutor(4)         →  rayon::par_iter()  (for CPU-bound)
threading.Lock()              →  tokio::sync::Mutex<T>  (for async)
asyncio.Queue()               →  tokio::sync::mpsc::channel()
in-memory dict (task state)   →  tokio::sync::broadcast + SQLite tasks table
StreamingResponse (SSE)       →  axum_extra::TypedHeader + Sse<Stream>
FastAPI BackgroundTasks       →  tokio::spawn()
```

---

## Database Migration Strategy

### Python: SQLAlchemy ORM + Alembic

```python
class Movie(Base):
    __tablename__ = "movies"
    id = Column(Integer, primary_key=True)
    title = Column(String, nullable=False)
    ...
```

### Rust: SQLx with compile-time verified queries

```rust
// No ORM macros needed for simple queries
let movies = sqlx::query_as!(
    Movie,
    "SELECT * FROM movies WHERE library_id = ? ORDER BY title",
    library_id
)
.fetch_all(pool)
.await?;
```

### Schema Compatibility

The SQLite schema is **intentionally kept compatible** with the Python version during migration. The same `.db` file can be opened by both the old Python app and the new Rust app. This enables:

1. **Zero data loss migration**: Users run the Rust app pointing to their existing `orchestrator.db`
2. **Rollback safety**: If the Rust app has issues, users can revert to Python without losing data

**Only additive migrations** are permitted during Phase 1 and Phase 2.

---

## Frontend Migration

The React frontend from the Python project can be reused **with minimal changes**. The only required change is adding the API Adapter:

```typescript
// NEW: frontend/src/api/adapter.ts
const isTauri = () => typeof window !== 'undefined' && '__TAURI__' in window;

export const api = {
    getMovies: async (): Promise<Movie[]> => {
        if (isTauri()) return invoke('get_movies');
        return fetch('/api/movies').then(r => r.json());
    },
    startScan: async (libraryId: number) => {
        if (isTauri()) return invoke('start_scan', { libraryId });
        return fetch('/api/scan', { method: 'POST', body: JSON.stringify({ library_id: libraryId }) });
    },
    // ... all other API calls
};
```

Existing Zustand stores, React components, and Tailwind styles migrate **unchanged**.

---

## Risk Register

| Risk | Severity | Mitigation |
|---|---|---|
| `pymediainfo` has no direct Rust equivalent | Medium | Use `ffprobe` via `std::process::Command` in Phase 2 |
| Cinemagoer Python library (IMDB fallback) | Low | OMDb API covers this use case; Cinemagoer was last-resort |
| Subtitle hash algorithm (OpenSubtitles) | Medium | Port the OSDb hash function (well-documented algorithm) |
| Windows path separators in SQLite | Low | Normalize all paths to forward slashes at ingestion |
| Tauri WebView2 availability on old Windows | Low | WebView2 installs silently via Tauri's bootstrapper |
| Cross-compilation for Linux from Windows | Medium | Use GitHub Actions CI with `cross` crate for Linux builds |

---

## Success Criteria

The migration is complete when:
- [ ] All 50,000+ item library scans complete in < 30 seconds (vs. Python's ~90s)
- [ ] Docker image size < 60MB (vs. Python's ~400MB)
- [ ] Windows `.exe` runs with zero runtime dependencies
- [ ] All Python API endpoints return identical JSON responses
- [ ] Existing `orchestrator.db` files load without schema changes
- [ ] Real-time SSE progress updates work in both Docker and desktop mode
