-- core/src/db/migrations/009_add_watch_history_and_trailers.sql

-- Add trailer_url to movies and tv_shows
ALTER TABLE movies ADD COLUMN trailer_url TEXT;
ALTER TABLE tv_shows ADD COLUMN trailer_url TEXT;

-- Create playback_state table for Resume Playback
CREATE TABLE IF NOT EXISTS playback_state (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    media_id    INTEGER NOT NULL,
    media_type  TEXT NOT NULL CHECK(media_type IN ('movie','episode')),
    position_ms INTEGER NOT NULL DEFAULT 0,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    is_finished BOOLEAN NOT NULL DEFAULT 0,
    updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(media_id, media_type)
);

-- Index for fast lookup
CREATE INDEX IF NOT EXISTS idx_playback_media ON playback_state(media_id, media_type);
