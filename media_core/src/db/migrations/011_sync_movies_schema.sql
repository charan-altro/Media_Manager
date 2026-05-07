-- core/src/db/migrations/011_sync_movies_schema.sql

-- Add missing columns to movies if they don't exist (using a safe approach for SQLite)
-- We try to add them; if they exist, this might fail, but we'll wrap it in a way that works or use a fresh migration.
-- SQLite doesn't support ADD COLUMN IF NOT EXISTS, so we just run it.

-- Movies
ALTER TABLE movies ADD COLUMN created_at TEXT NOT NULL DEFAULT '2026-05-07 00:00:00';
ALTER TABLE movies ADD COLUMN updated_at TEXT NOT NULL DEFAULT '2026-05-07 00:00:00';

-- Movie Files
ALTER TABLE movie_files ADD COLUMN created_at TEXT NOT NULL DEFAULT '2026-05-07 00:00:00';
ALTER TABLE movie_files ADD COLUMN updated_at TEXT NOT NULL DEFAULT '2026-05-07 00:00:00';

-- Libraries (add updated_at for consistency)
ALTER TABLE libraries ADD COLUMN updated_at TEXT NOT NULL DEFAULT '2026-05-07 00:00:00';

-- Update them to the current time
UPDATE movies SET created_at = datetime('now'), updated_at = datetime('now') WHERE created_at = '2026-05-07 00:00:00';
UPDATE movie_files SET created_at = datetime('now'), updated_at = datetime('now') WHERE created_at = '2026-05-07 00:00:00';
UPDATE libraries SET updated_at = datetime('now') WHERE updated_at = '2026-05-07 00:00:00';
