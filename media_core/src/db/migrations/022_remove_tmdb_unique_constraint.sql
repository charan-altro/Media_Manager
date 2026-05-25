-- Disable foreign key checks temporarily
PRAGMA foreign_keys=OFF;

-- 1. TV SHOWS TABLE
-- Create a new tv_shows table without the UNIQUE constraint on tmdb_id
CREATE TABLE tv_shows_new (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id    INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    title         TEXT    NOT NULL,
    tmdb_id       INTEGER, -- Removed UNIQUE
    status        TEXT    NOT NULL DEFAULT 'unmatched',
    plot          TEXT,
    rating        REAL,
    poster_url    TEXT,
    backdrop_url  TEXT,
    genres        TEXT,
    language      TEXT,
    cast_list     TEXT,
    imdb_id       TEXT,
    tagline       TEXT,
    runtime       INTEGER,
    nfo_path      TEXT,
    trailer_url   TEXT,
    created_at    TEXT NOT NULL DEFAULT '2026-05-07 00:00:00',
    updated_at    TEXT NOT NULL DEFAULT '2026-05-07 00:00:00'
);

-- Copy data from old to new
INSERT INTO tv_shows_new (
    id, library_id, title, tmdb_id, status, plot, rating, poster_url,
    backdrop_url, genres, language, cast_list, imdb_id, tagline,
    runtime, nfo_path, trailer_url, created_at, updated_at
)
SELECT 
    id, library_id, title, tmdb_id, status, plot, rating, poster_url,
    backdrop_url, genres, language, cast_list, imdb_id, tagline,
    runtime, nfo_path, trailer_url, created_at, updated_at
FROM tv_shows;

-- Drop old table
DROP TABLE tv_shows;

-- Rename new table
ALTER TABLE tv_shows_new RENAME TO tv_shows;

-- Re-create indexes for tv_shows
CREATE UNIQUE INDEX IF NOT EXISTS idx_tv_shows_unique_identity ON tv_shows (library_id, title);
CREATE INDEX IF NOT EXISTS idx_tv_shows_library_id ON tv_shows(library_id);


-- 2. MOVIES TABLE
-- Create a new movies table without the UNIQUE constraint on tmdb_id
CREATE TABLE movies_new (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id    INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    title         TEXT    NOT NULL,
    year          INTEGER,
    tmdb_id       INTEGER, -- Removed UNIQUE
    imdb_id       TEXT,
    status        TEXT    NOT NULL DEFAULT 'unmatched',
    plot          TEXT,
    rating        REAL,
    poster_url    TEXT,
    backdrop_url  TEXT,
    nfo_path      TEXT,
    tagline       TEXT,
    runtime       INTEGER,
    genres        TEXT,
    language      TEXT,
    cast_list     TEXT,
    trailer_url   TEXT,
    created_at    TEXT NOT NULL DEFAULT '2026-05-07 00:00:00',
    updated_at    TEXT NOT NULL DEFAULT '2026-05-07 00:00:00'
);

-- Copy data from old to new
INSERT INTO movies_new (
    id, library_id, title, year, tmdb_id, imdb_id, status, plot, rating,
    poster_url, backdrop_url, nfo_path, tagline, runtime, genres,
    language, cast_list, trailer_url, created_at, updated_at
)
SELECT 
    id, library_id, title, year, tmdb_id, imdb_id, status, plot, rating,
    poster_url, backdrop_url, nfo_path, tagline, runtime, genres,
    language, cast_list, trailer_url, created_at, updated_at
FROM movies;

-- Drop old table
DROP TABLE movies;

-- Rename new table
ALTER TABLE movies_new RENAME TO movies;

-- Re-create indexes for movies
CREATE UNIQUE INDEX IF NOT EXISTS idx_movies_unique_identity ON movies (library_id, title, IFNULL(year, 0));
CREATE INDEX IF NOT EXISTS idx_movies_library_id ON movies(library_id);

-- Re-enable foreign key checks
PRAGMA foreign_keys=ON;
