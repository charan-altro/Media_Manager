-- 015_fast_skip_scanning.sql

-- Add mtime column to movie_files
ALTER TABLE movie_files ADD COLUMN mtime INTEGER;

-- Add mtime column to episodes
ALTER TABLE episodes ADD COLUMN mtime INTEGER;

-- Update existing records to have a default mtime if needed (0 is fine)
UPDATE movie_files SET mtime = 0 WHERE mtime IS NULL;
UPDATE episodes SET mtime = 0 WHERE mtime IS NULL;
