-- core/src/db/migrations/004_fix_tv_shows_and_unique.sql

-- Add missing columns to tv_shows
ALTER TABLE tv_shows ADD COLUMN backdrop_url TEXT;
ALTER TABLE tv_shows ADD COLUMN genres TEXT; -- JSON array

-- Add unique constraint to prevent duplicates on rescan
CREATE UNIQUE INDEX IF NOT EXISTS idx_tv_shows_unique_identity ON tv_shows (library_id, title);
