-- Migration: 008_add_advanced_media_info.sql
-- Add aspect_ratio and thumbnail_path to movie_files
ALTER TABLE movie_files ADD COLUMN aspect_ratio TEXT;
ALTER TABLE movie_files ADD COLUMN thumbnail_path TEXT;

-- Add aspect_ratio and thumbnail_path to episodes
ALTER TABLE episodes ADD COLUMN aspect_ratio TEXT;
ALTER TABLE episodes ADD COLUMN thumbnail_path TEXT;
