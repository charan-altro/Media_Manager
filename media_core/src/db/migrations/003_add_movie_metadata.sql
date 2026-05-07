-- core/src/db/migrations/003_add_movie_metadata.sql

-- Add extra metadata columns to movies
ALTER TABLE movies ADD COLUMN tagline TEXT;
ALTER TABLE movies ADD COLUMN runtime INTEGER;
ALTER TABLE movies ADD COLUMN genres TEXT; -- JSON array of strings
