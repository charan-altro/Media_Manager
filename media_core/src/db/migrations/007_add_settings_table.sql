CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Insert default placeholder settings
INSERT OR IGNORE INTO settings (key, value) VALUES ('fanart_api_key', '');
INSERT OR IGNORE INTO settings (key, value) VALUES ('trakt_api_key', '');
INSERT OR IGNORE INTO settings (key, value) VALUES ('tvdb_api_key', '');
INSERT OR IGNORE INTO settings (key, value) VALUES ('tmdb_api_key', '');
INSERT OR IGNORE INTO settings (key, value) VALUES ('omdb_api_key', '');
INSERT OR IGNORE INTO settings (key, value) VALUES ('post_processing_script', '');
