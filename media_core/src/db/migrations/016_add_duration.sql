-- Add duration_secs to movie_files and episodes
ALTER TABLE movie_files ADD COLUMN duration_secs INTEGER;
ALTER TABLE episodes ADD COLUMN duration_secs INTEGER;
