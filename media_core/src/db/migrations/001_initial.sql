-- core/src/db/migrations/001_initial.sql

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

-- WAL mode for concurrent access
PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;
