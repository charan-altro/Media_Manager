-- 020_add_scene_markers.sql

CREATE TABLE IF NOT EXISTS scene_markers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    media_id INTEGER NOT NULL,
    media_type TEXT NOT NULL, -- 'movie' or 'episode'
    seconds REAL NOT NULL,
    title TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_scene_markers_media ON scene_markers(media_id, media_type);
