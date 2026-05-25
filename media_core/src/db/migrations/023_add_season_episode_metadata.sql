-- Migration: 023_add_season_episode_metadata.sql
-- Add metadata columns to seasons
ALTER TABLE seasons ADD COLUMN name TEXT;
ALTER TABLE seasons ADD COLUMN plot TEXT;
ALTER TABLE seasons ADD COLUMN poster_url TEXT;

-- Add metadata columns to episodes
ALTER TABLE episodes ADD COLUMN plot TEXT;
ALTER TABLE episodes ADD COLUMN rating REAL;
