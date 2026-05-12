-- 014_identity_foundation.sql

-- Add identity columns to movie_files
ALTER TABLE movie_files ADD COLUMN fingerprint TEXT;
ALTER TABLE movie_files ADD COLUMN is_missing BOOLEAN DEFAULT FALSE;
ALTER TABLE movie_files ADD COLUMN last_scanned TIMESTAMP;

-- Add identity columns to episodes
ALTER TABLE episodes ADD COLUMN fingerprint TEXT;
ALTER TABLE episodes ADD COLUMN is_missing BOOLEAN DEFAULT FALSE;
ALTER TABLE episodes ADD COLUMN last_scanned TIMESTAMP;

-- Create unique indices for fingerprints
CREATE UNIQUE INDEX idx_movie_files_fingerprint ON movie_files(fingerprint);
CREATE UNIQUE INDEX idx_episodes_fingerprint ON episodes(fingerprint);

-- Index for path lookups (file_path is already unique, but an index is explicit)
CREATE INDEX IF NOT EXISTS idx_movie_files_path_lookup ON movie_files(file_path);
CREATE INDEX IF NOT EXISTS idx_episodes_path_lookup ON episodes(file_path);
