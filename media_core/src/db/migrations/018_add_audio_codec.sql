-- core/src/db/migrations/018_add_audio_codec.sql

ALTER TABLE movie_files ADD COLUMN audio_codec TEXT;
ALTER TABLE episodes ADD COLUMN audio_codec TEXT;
