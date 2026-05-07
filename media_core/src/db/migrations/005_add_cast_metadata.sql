-- core/src/db/migrations/005_add_cast_metadata.sql

-- Add cast_list to movies and tv_shows
ALTER TABLE movies ADD COLUMN cast_list TEXT;
ALTER TABLE tv_shows ADD COLUMN cast_list TEXT;

-- Cleanup existing duplicates before creating the unique index
-- Keeps the row with the lowest ID for each title/year/library combination
DELETE FROM movies 
WHERE id NOT IN (
    SELECT MIN(id)
    FROM movies
    GROUP BY library_id, title, IFNULL(year, 0)
);

-- Redefine the movies unique index to treat NULL years as 0
DROP INDEX IF EXISTS idx_movies_unique_identity;
CREATE UNIQUE INDEX idx_movies_unique_identity ON movies (library_id, title, IFNULL(year, 0));
