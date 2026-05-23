-- 019_add_performance_indexes.sql

CREATE INDEX IF NOT EXISTS idx_movies_library_id ON movies(library_id);
CREATE INDEX IF NOT EXISTS idx_movie_files_movie_id ON movie_files(movie_id);
CREATE INDEX IF NOT EXISTS idx_tv_shows_library_id ON tv_shows(library_id);
CREATE INDEX IF NOT EXISTS idx_seasons_show_id ON seasons(show_id);
CREATE INDEX IF NOT EXISTS idx_episodes_season_id ON episodes(season_id);
