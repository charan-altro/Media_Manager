-- 024_deduplicate_tv_shows.sql
-- This migration merges duplicate TV show entries that represent the same show
-- but were stored under slightly different raw names (e.g., "Better Call Saul" and
-- "Better.Call.Saul.S04.1080p.BluRay.x265-KONTRAST").
--
-- Strategy:
--   For each library, group shows that share the same normalized key
--   (lowercase, punctuation->space, year+quality tags stripped, whitespace collapsed).
--   Pick the "canonical" entry (shortest trimmed title = most human-readable).
--   Reassign all seasons belonging to the duplicate entries to the canonical entry,
--   fixing any season_number conflicts by merging episodes into the surviving season.
--   Finally delete the duplicate show rows.
--
-- Because SQLite does not support user-defined functions inside plain SQL, the actual
-- deduplication logic is implemented in the application startup code (tv_repo.rs).
-- This migration is intentionally a no-op SQL stub so that the migration runner
-- records it as applied; the Rust code in db::tv_repo::run_startup_deduplication()
-- performs the real work immediately after migrations complete.

SELECT 1; -- no-op placeholder
