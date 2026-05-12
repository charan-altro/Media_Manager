-- 013_add_preview_path.sql

-- Add preview_path column to movie_files
ALTER TABLE movie_files ADD COLUMN preview_path TEXT;

-- Add preview_path column to episodes
ALTER TABLE episodes ADD COLUMN preview_path TEXT;
