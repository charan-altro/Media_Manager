-- 012_add_file_hashes.sql

-- Add hash column to movie_files
ALTER TABLE movie_files ADD COLUMN hash TEXT;
CREATE INDEX idx_movie_files_hash ON movie_files(hash);

-- Add hash column to episodes
ALTER TABLE episodes ADD COLUMN hash TEXT;
CREATE INDEX idx_episodes_hash ON episodes(hash);
