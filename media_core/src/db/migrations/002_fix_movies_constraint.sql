-- core/src/db/migrations/002_fix_movies_constraint.sql

-- Add a unique constraint to movies to prevent duplicates on rescans
-- title + year + library_id should be unique
CREATE UNIQUE INDEX IF NOT EXISTS idx_movies_unique_identity ON movies (library_id, title, year);
