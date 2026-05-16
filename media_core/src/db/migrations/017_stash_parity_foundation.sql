CREATE TABLE IF NOT EXISTS media_streams (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_hash TEXT NOT NULL,
    stream_index INTEGER NOT NULL,
    stream_type TEXT NOT NULL, -- 'video', 'audio', 'subtitle'
    codec TEXT,
    language TEXT,
    title TEXT,
    channels INTEGER,
    is_default BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(file_hash, stream_index)
);

CREATE TABLE IF NOT EXISTS generated_assets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_hash TEXT NOT NULL,
    asset_type TEXT NOT NULL, -- 'sprite', 'preview', 'thumb', 'vtt'
    path TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(file_hash, asset_type)
);

CREATE INDEX idx_media_streams_hash ON media_streams(file_hash);
CREATE INDEX idx_generated_assets_hash ON generated_assets(file_hash);
