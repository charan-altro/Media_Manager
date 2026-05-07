-- core/src/db/migrations/010_sync_tv_shows_schema.sql

-- Add missing columns to tv_shows to align with TVShow struct
-- SQLite ALTER TABLE ADD COLUMN does not support non-constant defaults like datetime('now')
-- We use a constant string first, then update it.
ALTER TABLE tv_shows ADD COLUMN imdb_id TEXT;
ALTER TABLE tv_shows ADD COLUMN tagline TEXT;
ALTER TABLE tv_shows ADD COLUMN runtime INTEGER;
-- Note: 'language' was already added in 006
-- Note: 'cast_list' was already added in 005
ALTER TABLE tv_shows ADD COLUMN nfo_path TEXT;
ALTER TABLE tv_shows ADD COLUMN created_at TEXT NOT NULL DEFAULT '2026-05-07 00:00:00';
ALTER TABLE tv_shows ADD COLUMN updated_at TEXT NOT NULL DEFAULT '2026-05-07 00:00:00';

UPDATE tv_shows SET created_at = datetime('now'), updated_at = datetime('now');

-- Add missing columns to seasons
ALTER TABLE seasons ADD COLUMN created_at TEXT NOT NULL DEFAULT '2026-05-07 00:00:00';
ALTER TABLE seasons ADD COLUMN updated_at TEXT NOT NULL DEFAULT '2026-05-07 00:00:00';

UPDATE seasons SET created_at = datetime('now'), updated_at = datetime('now');

-- Add missing columns to episodes
ALTER TABLE episodes ADD COLUMN resolution TEXT;
ALTER TABLE episodes ADD COLUMN codec TEXT;
-- Note: 'aspect_ratio' and 'thumbnail_path' were added in 008
ALTER TABLE episodes ADD COLUMN created_at TEXT NOT NULL DEFAULT '2026-05-07 00:00:00';
ALTER TABLE episodes ADD COLUMN updated_at TEXT NOT NULL DEFAULT '2026-05-07 00:00:00';

UPDATE episodes SET created_at = datetime('now'), updated_at = datetime('now');
