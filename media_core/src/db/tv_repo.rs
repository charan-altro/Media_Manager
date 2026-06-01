// media_core/src/db/tv_repo.rs
#![allow(async_fn_in_trait)]
use crate::models::{TVShow, TvShowId, Season, SeasonId, Episode, EpisodeId, LibraryId, Resolution, MediaStatus};
use crate::db::Result;
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;
use std::path::PathBuf;

// --- Reader interface ---
#[cfg_attr(test, mockall::automock)]
pub trait TvReader: Send + Sync {
    async fn find_all_shows(&self, library_id: Option<LibraryId>, genre: Option<String>, language: Option<String>) -> Result<Vec<TVShow>>;
    async fn find_show_by_id(&self, id: TvShowId) -> Result<Option<TVShow>>;
    async fn find_shows_by_ids(&self, ids: &[TvShowId]) -> Result<Vec<TVShow>>;
    async fn find_seasons_by_show_id(&self, show_id: TvShowId) -> Result<Vec<Season>>;
    async fn find_episodes_by_season_id(&self, season_id: SeasonId) -> Result<Vec<Episode>>;
    async fn find_episode_by_path(&self, path: &str) -> Result<Option<Episode>>;
    async fn find_episode_by_hash(&self, hash: &str) -> Result<Option<Episode>>;
    async fn find_episode_by_fingerprint(&self, fp: &str) -> Result<Option<Episode>>;
    async fn find_episode_by_id(&self, id: EpisodeId) -> Result<Option<Episode>>;
    async fn get_episode_full_path(&self, episode_id: EpisodeId) -> Result<Option<PathBuf>>;
}

// --- Writer interface ---
#[cfg_attr(test, mockall::automock)]
pub trait TvWriter: Send + Sync {
    async fn upsert_show(&self, library_id: LibraryId, title: &str) -> Result<TvShowId>;
    async fn update_show<'a>(
        &self, 
        id: TvShowId, 
        title: &str, 
        plot: Option<&'a str>, 
        rating: Option<f32>, 
        genres: Option<&'a str>,
        tagline: Option<&'a str>,
        runtime: Option<i32>,
        language: Option<&'a str>,
        trailer_url: Option<&'a str>,
    ) -> Result<()>;
    async fn update_show_metadata(
        &self,
        id: TvShowId,
        tmdb_id: Option<i32>,
        plot: Option<String>,
        rating: Option<f32>,
        genres: Option<String>,
        language: Option<String>,
        cast_list: Option<String>,
        poster_url: Option<String>,
        backdrop_url: Option<String>,
        trailer_url: Option<String>,
        status: MediaStatus,
    ) -> Result<()>;
    async fn upsert_season(&self, show_id: TvShowId, season_number: i32) -> Result<SeasonId>;
    async fn upsert_episode<'a>(
        &self, 
        season_id: SeasonId, 
        episode_number: i32, 
        file_path: &str, 
        original_name: &str, 
        size_bytes: i64,
        mtime: Option<i64>,
        resolution: Option<Resolution>,
        codec: Option<&'a str>,
        audio_codec: Option<&'a str>,
        duration_secs: Option<i32>,
        hash: Option<&'a str>,
        fingerprint: Option<&'a str>,
        title: Option<&'a str>,
        plot: Option<&'a str>,
    ) -> Result<EpisodeId>;
    async fn update_episode_path(&self, id: EpisodeId, new_path: &str) -> Result<()>;
    async fn update_episode_last_scanned(&self, id: EpisodeId) -> Result<()>;
    async fn update_episode_fingerprint(&self, id: EpisodeId, fingerprint: &str) -> Result<()>;
    async fn update_episode_duration(&self, id: EpisodeId, duration_secs: i32) -> Result<()>;
    async fn update_episode_metadata(&self, id: EpisodeId, duration_secs: i32, width: i32, height: i32) -> Result<()>;
    async fn update_episode_title_and_plot<'a>(&self, id: EpisodeId, title: &str, plot: Option<&'a str>) -> Result<()>;
    async fn update_episode_scraped_metadata(
        &self,
        id: EpisodeId,
        title: Option<String>,
        plot: Option<String>,
        rating: Option<f32>,
        thumbnail_path: Option<String>,
    ) -> Result<()>;
    async fn update_season_scraped_metadata(
        &self,
        id: SeasonId,
        name: Option<String>,
        plot: Option<String>,
        poster_url: Option<String>,
    ) -> Result<()>;
    async fn mark_missing_in_library(&self, library_id: LibraryId) -> Result<i32>;
    async fn delete_episode(&self, id: EpisodeId) -> Result<()>;
    async fn delete_show(&self, id: TvShowId) -> Result<()>;
}

// --- Combined ---
pub trait TvReaderWriter: TvReader + TvWriter {}

// --- SQLite implementation ---
pub struct SqliteTvRepository {
    base: super::base::SqliteBase,
}

impl SqliteTvRepository {
    pub fn new(pool: Arc<SqlitePool>) -> Self {
        Self {
            base: super::base::SqliteBase::new(pool),
        }
    }
}

impl TvReader for SqliteTvRepository {
    #[tracing::instrument(skip(self), err)]
    async fn find_all_shows(&self, library_id: Option<LibraryId>, genre: Option<String>, language: Option<String>) -> Result<Vec<TVShow>> {
        let mut query = String::from(r#"
            SELECT t.*, 
            (SELECT e.preview_path FROM episodes e JOIN seasons s ON e.season_id = s.id WHERE s.show_id = t.id AND e.preview_path IS NOT NULL LIMIT 1) as preview_path
            FROM tv_shows t WHERE 1=1
        "#);
        if library_id.is_some() {
            query.push_str(" AND t.library_id = ?");
        }
        
        let genre_active = genre.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
        if genre_active {
            query.push_str(" AND t.genres LIKE ?");
        }
        
        let language_active = language.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
        if language_active {
            query.push_str(" AND t.language = ?");
        }
        query.push_str(" ORDER BY t.title ASC");

        let mut args = sqlx::sqlite::SqliteArguments::default();
        if let Some(id) = library_id {
            sqlx::Arguments::add(&mut args, id);
        }
        if genre_active {
            if let Some(ref g) = genre {
                sqlx::Arguments::add(&mut args, format!("%\"{}\"%", g));
            }
        }
        if language_active {
            if let Some(ref l) = language {
                sqlx::Arguments::add(&mut args, l);
            }
        }

        self.base.fetch_all(&*self.base.pool, &query, args).await
    }

    #[tracing::instrument(skip(self), err)]
    async fn find_show_by_id(&self, id: TvShowId) -> Result<Option<TVShow>> {
        let sql = r#"
            SELECT t.*, 
            (SELECT e.preview_path FROM episodes e JOIN seasons s ON e.season_id = s.id WHERE s.show_id = t.id AND e.preview_path IS NOT NULL LIMIT 1) as preview_path
            FROM tv_shows t WHERE id = ?
        "#;
        let mut args = sqlx::sqlite::SqliteArguments::default();
        sqlx::Arguments::add(&mut args, id);
        self.base.fetch_optional(&*self.base.pool, sql, args).await
    }

    #[tracing::instrument(skip(self), err)]
    async fn find_shows_by_ids(&self, ids: &[TvShowId]) -> Result<Vec<TVShow>> {
        if ids.is_empty() { return Ok(vec![]); }
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(r#"
            SELECT t.*, 
            (SELECT e.preview_path FROM episodes e JOIN seasons s ON e.season_id = s.id WHERE s.show_id = t.id AND e.preview_path IS NOT NULL LIMIT 1) as preview_path
            FROM tv_shows t WHERE id IN ({})
        "#, placeholders);
        
        let mut args = sqlx::sqlite::SqliteArguments::default();
        for id in ids {
            sqlx::Arguments::add(&mut args, *id);
        }
        self.base.fetch_all(&*self.base.pool, &query, args).await
    }

    #[tracing::instrument(skip(self), err)]
    async fn find_seasons_by_show_id(&self, show_id: TvShowId) -> Result<Vec<Season>> {
        let mut args = sqlx::sqlite::SqliteArguments::default();
        sqlx::Arguments::add(&mut args, show_id);
        self.base.fetch_all(&*self.base.pool, "SELECT * FROM seasons WHERE show_id = ? ORDER BY season_number ASC", args).await
    }

    #[tracing::instrument(skip(self), err)]
    async fn find_episodes_by_season_id(&self, season_id: SeasonId) -> Result<Vec<Episode>> {
        let mut args = sqlx::sqlite::SqliteArguments::default();
        sqlx::Arguments::add(&mut args, season_id);
        self.base.fetch_all(&*self.base.pool, "SELECT *, codec, codec as video_codec FROM episodes WHERE season_id = ? ORDER BY episode_number ASC", args).await
    }

    #[tracing::instrument(skip(self), err)]
    async fn find_episode_by_path(&self, path: &str) -> Result<Option<Episode>> {
        let normalized = crate::paths::normalize_slashes(path);
        let mut args = sqlx::sqlite::SqliteArguments::default();
        sqlx::Arguments::add(&mut args, normalized);
        self.base.fetch_optional(&*self.base.pool, "SELECT *, codec, codec as video_codec FROM episodes WHERE file_path = ?", args).await
    }

    #[tracing::instrument(skip(self), err)]
    async fn find_episode_by_hash(&self, hash: &str) -> Result<Option<Episode>> {
        let mut args = sqlx::sqlite::SqliteArguments::default();
        sqlx::Arguments::add(&mut args, hash);
        self.base.fetch_optional(&*self.base.pool, "SELECT *, codec, codec as video_codec FROM episodes WHERE hash = ?", args).await
    }

    #[tracing::instrument(skip(self), err)]
    async fn find_episode_by_fingerprint(&self, fp: &str) -> Result<Option<Episode>> {
        let mut args = sqlx::sqlite::SqliteArguments::default();
        sqlx::Arguments::add(&mut args, fp);
        self.base.fetch_optional(&*self.base.pool, "SELECT *, codec, codec as video_codec FROM episodes WHERE fingerprint = ?", args).await
    }

    #[tracing::instrument(skip(self), err)]
    async fn get_episode_full_path(&self, episode_id: EpisodeId) -> Result<Option<PathBuf>> {
        let mut args = sqlx::sqlite::SqliteArguments::default();
        sqlx::Arguments::add(&mut args, episode_id);
        let row: Option<(String, String)> = self.base.fetch_optional(&*self.base.pool, 
            r#"
            SELECT l.path, e.file_path 
            FROM episodes e 
            JOIN seasons s ON e.season_id = s.id 
            JOIN tv_shows t ON s.show_id = t.id 
            JOIN libraries l ON t.library_id = l.id 
            WHERE e.id = ?
            "#, args).await?;

        if let Some((lib_path, rel_path)) = row {
            Ok(Some(crate::paths::make_absolute(&rel_path, std::path::Path::new(&lib_path))))
        } else {
            Ok(None)
        }
    }

    #[tracing::instrument(skip(self), err)]
    async fn find_episode_by_id(&self, id: EpisodeId) -> Result<Option<Episode>> {
        let mut args = sqlx::sqlite::SqliteArguments::default();
        sqlx::Arguments::add(&mut args, id);
        self.base.fetch_optional(&*self.base.pool, "SELECT *, codec, codec as video_codec FROM episodes WHERE id = ?", args).await
    }
}

fn extract_year(s: &str) -> Option<i32> {
    static RE_YEAR: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"\b((?:19|20)\d{2})\b").unwrap()
    });
    RE_YEAR.captures(s)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<i32>().ok())
}

fn normalize_for_comparison(s: &str) -> String {
    let mut normalized = s.to_lowercase()
        .replace('.', " ")
        .replace('_', " ")
        .replace('-', " ")
        .replace('(', " ")
        .replace(')', " ")
        .replace('[', " ")
        .replace(']', " ");
    
    // Strip year
    static RE_YEAR: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"\b(?:19|20)\d{2}\b").unwrap()
    });
    normalized = RE_YEAR.replace_all(&normalized, "").to_string();
    
    // Collapse whitespace
    static RE_SPACES: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"\s+").unwrap()
    });
    normalized = RE_SPACES.replace_all(&normalized, " ").trim().to_string();
    normalized
}

impl TvWriter for SqliteTvRepository {
    #[tracing::instrument(skip(self), err)]
    async fn upsert_show(&self, library_id: LibraryId, title: &str) -> Result<TvShowId> {
        // Fetch all shows in this library to check for a normalized title match
        let existing_shows: Vec<(TvShowId, String)> = crate::fetch_all_db!(
            &*self.base.pool,
            sqlx::query_as("SELECT id, title FROM tv_shows WHERE library_id = ?")
                .bind(library_id)
        ).await?;

        let current_normalized = normalize_for_comparison(title);
        let current_year = extract_year(title);

        for (id, existing_title) in &existing_shows {
            let existing_normalized = normalize_for_comparison(existing_title);
            let existing_year = extract_year(existing_title);
            
            // If both have years and they are different, treat as different shows
            if let (Some(cy), Some(ey)) = (current_year, existing_year) {
                if cy != ey {
                    continue;
                }
            }
            
            if existing_normalized == current_normalized {
                // If the incoming title is shorter/cleaner, update the stored title
                // so the DB converges to the most human-readable version over time.
                if title.len() < existing_title.len() {
                    let _ = sqlx::query(
                        "UPDATE tv_shows SET title = ?, updated_at = datetime('now') WHERE id = ?"
                    )
                    .bind(title)
                    .bind(*id)
                    .execute(&*self.base.pool)
                    .await;
                }
                return Ok(*id);
            }
        }

        let row: (TvShowId,) = crate::fetch_one_db!(
            &*self.base.pool,
            sqlx::query_as(
                r#"
                INSERT INTO tv_shows (library_id, title) 
                VALUES (?, ?) 
                ON CONFLICT(library_id, title) DO UPDATE SET updated_at = datetime('now')
                RETURNING id
                "#
            )
            .bind(library_id)
            .bind(title)
        ).await?;

        Ok(row.0)
    }

    #[tracing::instrument(skip(self), err)]
    async fn update_show(
        &self, 
        id: TvShowId, 
        title: &str, 
        plot: Option<&str>, 
        rating: Option<f32>, 
        genres: Option<&str>,
        tagline: Option<&str>,
        runtime: Option<i32>,
        language: Option<&str>,
        trailer_url: Option<&str>,
    ) -> Result<()> {
        crate::execute_db!(
            &*self.base.pool,
            sqlx::query(
                r#"
                UPDATE tv_shows 
                SET title = ?, plot = ?, rating = ?, genres = ?, tagline = ?, runtime = ?, language = ?, trailer_url = ?, updated_at = datetime('now')
                WHERE id = ?
                "#
            )
            .bind(title)
            .bind(plot)
            .bind(rating)
            .bind(genres)
            .bind(tagline)
            .bind(runtime)
            .bind(language)
            .bind(trailer_url)
            .bind(id)
        ).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn update_show_metadata(
        &self,
        id: TvShowId,
        tmdb_id: Option<i32>,
        plot: Option<String>,
        rating: Option<f32>,
        genres: Option<String>,
        language: Option<String>,
        cast_list: Option<String>,
        poster_url: Option<String>,
        backdrop_url: Option<String>,
        trailer_url: Option<String>,
        status: MediaStatus,
    ) -> Result<()> {
        crate::execute_db!(
            &*self.base.pool,
            sqlx::query(
                r#"
                UPDATE tv_shows
                SET tmdb_id = COALESCE(?, tmdb_id),
                    plot = COALESCE(?, plot),
                    rating = COALESCE(?, rating),
                    genres = COALESCE(?, genres),
                    language = COALESCE(?, language),
                    cast_list = COALESCE(?, cast_list),
                    poster_url = COALESCE(?, poster_url),
                    backdrop_url = COALESCE(?, backdrop_url),
                    trailer_url = COALESCE(?, trailer_url),
                    status = ?
                WHERE id = ?
                "#
            )
            .bind(tmdb_id)
            .bind(plot)
            .bind(rating)
            .bind(genres)
            .bind(language)
            .bind(cast_list)
            .bind(poster_url)
            .bind(backdrop_url)
            .bind(trailer_url)
            .bind(status)
            .bind(id)
        ).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn upsert_season(&self, show_id: TvShowId, season_number: i32) -> Result<SeasonId> {
        let row: (SeasonId,) = crate::fetch_one_db!(
            &*self.base.pool,
            sqlx::query_as(
                r#"
                INSERT INTO seasons (show_id, season_number) 
                VALUES (?, ?) 
                ON CONFLICT(show_id, season_number) DO UPDATE SET updated_at = datetime('now')
                RETURNING id
                "#
            )
            .bind(show_id)
            .bind(season_number)
        ).await?;

        Ok(row.0)
    }

    #[tracing::instrument(skip(self), err)]
    async fn upsert_episode(
        &self, 
        season_id: SeasonId, 
        episode_number: i32, 
        file_path: &str, 
        original_name: &str, 
        size_bytes: i64,
        mtime: Option<i64>,
        resolution: Option<Resolution>,
        codec: Option<&str>,
        audio_codec: Option<&str>,
        duration_secs: Option<i32>,
        hash: Option<&str>,
        fingerprint: Option<&str>,
        title: Option<&str>,
        plot: Option<&str>,
    ) -> Result<EpisodeId> {
        let normalized_path = crate::paths::normalize_slashes(file_path);
        let row: (EpisodeId,) = crate::fetch_one_db!(
            &*self.base.pool,
            sqlx::query_as(
                r#"
                INSERT INTO episodes (season_id, episode_number, file_path, original_name, size_bytes, mtime, resolution, codec, audio_codec, duration_secs, hash, fingerprint, is_missing, last_scanned, title, plot)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, datetime('now'), ?, ?)
                ON CONFLICT(file_path) DO UPDATE SET 
                    size_bytes = excluded.size_bytes,
                    mtime = excluded.mtime,
                    resolution = excluded.resolution,
                    codec = excluded.codec,
                    audio_codec = excluded.audio_codec,
                    duration_secs = excluded.duration_secs,
                    hash = excluded.hash,
                    fingerprint = excluded.fingerprint,
                    is_missing = 0,
                    last_scanned = datetime('now'),
                    updated_at = datetime('now'),
                    title = COALESCE(excluded.title, episodes.title),
                    plot = COALESCE(excluded.plot, episodes.plot)
                RETURNING id
                "#
            )
            .bind(season_id)
            .bind(episode_number)
            .bind(&normalized_path)
            .bind(original_name)
            .bind(size_bytes)
            .bind(mtime)
            .bind(resolution)
            .bind(codec)
            .bind(audio_codec)
            .bind(duration_secs)
            .bind(hash)
            .bind(fingerprint)
            .bind(title)
            .bind(plot)
        ).await?;

        Ok(row.0)
    }

    #[tracing::instrument(skip(self), err)]
    async fn update_episode_path(&self, id: EpisodeId, new_path: &str) -> Result<()> {
        let normalized = crate::paths::normalize_slashes(new_path);
        crate::execute_db!(
            &*self.base.pool,
            sqlx::query("UPDATE episodes SET file_path = ?, updated_at = datetime('now') WHERE id = ?")
                .bind(normalized)
                .bind(id)
        ).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn update_episode_last_scanned(&self, id: EpisodeId) -> Result<()> {
        crate::execute_db!(
            &*self.base.pool,
            sqlx::query("UPDATE episodes SET last_scanned = datetime('now'), is_missing = 0 WHERE id = ?")
                .bind(id)
        ).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn update_episode_fingerprint(&self, id: EpisodeId, fingerprint: &str) -> Result<()> {
        crate::execute_db!(
            &*self.base.pool,
            sqlx::query("UPDATE episodes SET fingerprint = ? WHERE id = ?")
                .bind(fingerprint)
                .bind(id)
        ).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn update_episode_duration(&self, id: EpisodeId, duration_secs: i32) -> Result<()> {
        crate::execute_db!(
            &*self.base.pool,
            sqlx::query("UPDATE episodes SET duration_secs = ? WHERE id = ?")
                .bind(duration_secs)
                .bind(id)
        ).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn update_episode_metadata(&self, id: EpisodeId, duration_secs: i32, width: i32, height: i32) -> Result<()> {
        let res = Resolution::from_dimensions(width, height);
        crate::execute_db!(
            &*self.base.pool,
            sqlx::query("UPDATE episodes SET duration_secs = ?, resolution = ?, updated_at = datetime('now') WHERE id = ?")
                .bind(duration_secs)
                .bind(res)
                .bind(id)
        ).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn update_episode_title_and_plot<'a>(&self, id: EpisodeId, title: &str, plot: Option<&'a str>) -> Result<()> {
        crate::execute_db!(
            &*self.base.pool,
            sqlx::query("UPDATE episodes SET title = ?, plot = ?, updated_at = datetime('now') WHERE id = ?")
                .bind(title)
                .bind(plot)
                .bind(id)
        ).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn update_episode_scraped_metadata(
        &self,
        id: EpisodeId,
        title: Option<String>,
        plot: Option<String>,
        rating: Option<f32>,
        thumbnail_path: Option<String>,
    ) -> Result<()> {
        crate::execute_db!(
            &*self.base.pool,
            sqlx::query("UPDATE episodes SET title = ?, plot = ?, rating = ?, thumbnail_path = ?, updated_at = datetime('now') WHERE id = ?")
                .bind(title)
                .bind(plot)
                .bind(rating)
                .bind(thumbnail_path)
                .bind(id)
        ).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn update_season_scraped_metadata(
        &self,
        id: SeasonId,
        name: Option<String>,
        plot: Option<String>,
        poster_url: Option<String>,
    ) -> Result<()> {
        crate::execute_db!(
            &*self.base.pool,
            sqlx::query("UPDATE seasons SET name = ?, plot = ?, poster_url = ?, updated_at = datetime('now') WHERE id = ?")
                .bind(name)
                .bind(plot)
                .bind(poster_url)
                .bind(id)
        ).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn mark_missing_in_library(&self, library_id: LibraryId) -> Result<i32> {
        let rows: Vec<(EpisodeId,)> = crate::fetch_all_db!(
            &*self.base.pool,
            sqlx::query_as(
                r#"
                UPDATE episodes
                SET is_missing = 1
                WHERE season_id IN (SELECT s.id FROM seasons s JOIN tv_shows t ON s.show_id = t.id WHERE t.library_id = ?)
                AND last_scanned < datetime('now', '-1 minute')
                RETURNING id
                "#
            )
            .bind(library_id)
        ).await?;
        Ok(rows.len() as i32)
    }

    #[tracing::instrument(skip(self), err)]
    async fn delete_episode(&self, id: EpisodeId) -> Result<()> {
        crate::execute_db!(
            &*self.base.pool,
            sqlx::query("DELETE FROM episodes WHERE id = ?").bind(id)
        ).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn delete_show(&self, id: TvShowId) -> Result<()> {
        crate::execute_db!(
            &*self.base.pool,
            sqlx::query("DELETE FROM tv_shows WHERE id = ?").bind(id)
        ).await?;
        Ok(())
    }
}

impl TvReaderWriter for SqliteTvRepository {}

// ---------------------------------------------------------------------------
// Startup deduplication: merge TV shows that are the same show but were
// stored under slightly different raw folder/file names.
// ---------------------------------------------------------------------------

/// Normalise a show title for grouping purposes.
/// This matches `normalize_for_comparison` but is kept as a standalone fn
/// so it can be used outside the impl block.
fn normalize_title_key(s: &str) -> String {
    // Replace common separators with spaces
    let mut out = s.to_lowercase()
        .replace('.', " ")
        .replace('_', " ")
        .replace('-', " ")
        .replace('(', " ")
        .replace(')', " ")
        .replace('[', " ")
        .replace(']', " ");

    // Strip release-group / quality tokens
    static RE_QUALITY_KEY: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(
            r"(?i)\b(2160p|1080p|720p|480p|576p|x264|x265|h264|h265|10bit|bluray|web[- ]?dl|webrip|hdtv|brrip|hdrip|proper|repack|hevc|avc|aac|ddp|nf|amzn|complete|galaxy|tv|mkvcage|eztv|yts|rarbg|1337x|tgx|psarips|kontrast|minx|memento)\b.*"
        ).unwrap()
    });
    out = RE_QUALITY_KEY.replace(&out, "").to_string();

    // Strip SxxExx markers
    static RE_SEP_KEY: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"(?i)\bs\d{1,2}(?:e\d{1,2})?\b.*").unwrap()
    });
    out = RE_SEP_KEY.replace(&out, "").to_string();

    // Strip year
    static RE_YEAR_KEY: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"\b(?:19|20)\d{2}\b").unwrap()
    });
    out = RE_YEAR_KEY.replace_all(&out, "").to_string();

    // Strip torrent-site prefixes
    static RE_SITE_KEY: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"(?i)^(?:www\s+)?(?:torrenting|eztv|yts|rarbg)\s+com\s*").unwrap()
    });
    out = RE_SITE_KEY.replace(&out, "").to_string();

    // Collapse whitespace
    static RE_WS_KEY: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"\s+").unwrap()
    });
    RE_WS_KEY.replace_all(&out, " ").trim().to_string()
}

/// Score a title — lower is better (shorter, no dots, no quality tags).
fn title_score(t: &str) -> usize {
    t.len()
}

/// Sanitize a raw show title stored in the database into a clean human-readable name.
/// E.g. `Better.Call.Saul.S04.1080p.BluRay.x265-KONTRAST` → `Better Call Saul`
fn sanitize_db_show_title(raw: &str) -> String {
    use once_cell::sync::Lazy;

    // Replace dots/underscores with spaces
    let spaced = raw.replace('.', " ").replace('_', " ");

    // Strip from the first Sxx / SxxExx marker onwards
    static RE_SEASON: Lazy<regex::Regex> = Lazy::new(|| {
        regex::Regex::new(r"(?i)\s*\b[Ss]\d{1,2}(?:[Ee]\d{1,2})?\b.*").unwrap()
    });
    let stripped = RE_SEASON.replace(&spaced, "");

    // Strip quality/release noise tokens
    static RE_QUALITY: Lazy<regex::Regex> = Lazy::new(|| {
        regex::Regex::new(
            r"(?i)\s+\b(2160p|1080p|720p|480p|576p|x264|x265|h264|h265|10bit|hdtv|web[- ]?dl|webdl|bluray|brrip|hdrip|webrip|hevc|avc|complete|galaxy|tv|mkvcage|eztv|yts|rarbg|1337x|tgx|psarips|kontrast|minx|memento)\b.*"
        ).unwrap()
    });
    let stripped = RE_QUALITY.replace(&stripped, "");

    // Collapse whitespace
    static RE_SPACES: Lazy<regex::Regex> = Lazy::new(|| regex::Regex::new(r"\s+").unwrap());
    RE_SPACES.replace_all(stripped.trim(), " ").trim().to_string()
}

/// After migrations, deduplicate TV shows within each library.
///
/// Phase 1: Sanitize every show title in the DB (e.g. `Better.Call.Saul.S04...` →
///          `Better Call Saul`) so the grouping key is computed on the clean title.
/// Phase 2: Group shows by (library_id, normalised_key) and merge duplicates into the
///          entry with the best (shortest / cleanest) title.
pub async fn deduplicate_shows(pool: &sqlx::sqlite::SqlitePool) -> Result<()> {
    // Fetch all shows: (id, library_id, title)
    let all_shows: Vec<(i64, i64, String)> =
        sqlx::query_as("SELECT id, library_id, title FROM tv_shows ORDER BY id ASC")
            .fetch_all(pool)
            .await?;

    if all_shows.is_empty() {
        return Ok(());
    }

    // --- Phase 1: sanitize titles ---
    // Build a mapping from (id, old_title) → cleaned_title and batch-update the DB.
    let mut title_updates: Vec<(i64, String)> = Vec::new();
    for (id, _lib_id, title) in &all_shows {
        let cleaned = sanitize_db_show_title(title);
        // Only update if the title actually changed and the clean version is non-empty
        if !cleaned.is_empty() && &cleaned != title {
            title_updates.push((*id, cleaned));
        }
    }

    if !title_updates.is_empty() {
        tracing::info!("Sanitizing {} show title(s) in database…", title_updates.len());
        for (id, new_title) in &title_updates {
            sqlx::query(
                "UPDATE tv_shows SET title = ?, updated_at = datetime('now') WHERE id = ?"
            )
            .bind(new_title)
            .bind(id)
            .execute(pool)
            .await?;
            tracing::debug!("  Sanitized show id={} → '{}'", id, new_title);
        }
    }

    // Reload after sanitization so grouping uses the updated titles
    let all_shows: Vec<(i64, i64, String)> =
        sqlx::query_as("SELECT id, library_id, title FROM tv_shows ORDER BY id ASC")
            .fetch_all(pool)
            .await?;

    // --- Phase 2: group and merge ---
    let mut groups: std::collections::HashMap<(i64, String), Vec<(i64, String)>> =
        std::collections::HashMap::new();

    for (id, lib_id, title) in &all_shows {
        let key = normalize_title_key(title);
        if key.is_empty() {
            continue; // can't reliably group, leave alone
        }
        groups.entry((*lib_id, key)).or_default().push((*id, title.clone()));
    }

    let mut total_merged = 0usize;

    for ((_lib_id, _key), mut group) in groups {
        if group.len() < 2 {
            continue; // nothing to merge
        }

        // Sort: lowest score (shortest/cleanest title) first → that's the canonical winner
        group.sort_by_key(|(_, t)| title_score(t));

        let (canonical_id, canonical_title) = group[0].clone();
        let duplicates = &group[1..];

        tracing::info!(
            "Deduplicating show '{}' (id={}) — merging {} duplicate(s)",
            canonical_title, canonical_id, duplicates.len()
        );

        for (dup_id, dup_title) in duplicates {
            // Move seasons from dup_id → canonical_id, handling conflicts
            let dup_seasons: Vec<(i64, i32)> =
                sqlx::query_as("SELECT id, season_number FROM seasons WHERE show_id = ?")
                    .bind(dup_id)
                    .fetch_all(pool)
                    .await?;

            for (dup_season_id, season_number) in &dup_seasons {
                // Check if canonical already has a season with this number
                let canon_season: Option<(i64,)> = sqlx::query_as(
                    "SELECT id FROM seasons WHERE show_id = ? AND season_number = ?",
                )
                .bind(canonical_id)
                .bind(season_number)
                .fetch_optional(pool)
                .await?;

                let target_season_id = if let Some((existing_season_id,)) = canon_season {
                    // Merge episodes into the existing season
                    sqlx::query(
                        "UPDATE OR IGNORE episodes SET season_id = ? WHERE season_id = ?",
                    )
                    .bind(existing_season_id)
                    .bind(dup_season_id)
                    .execute(pool)
                    .await?;
                    // Delete any leftover episodes that conflicted (same file_path)
                    sqlx::query("DELETE FROM episodes WHERE season_id = ?")
                        .bind(dup_season_id)
                        .execute(pool)
                        .await?;
                    existing_season_id
                } else {
                    // No conflict — just re-parent the season
                    sqlx::query(
                        "UPDATE seasons SET show_id = ? WHERE id = ?",
                    )
                    .bind(canonical_id)
                    .bind(dup_season_id)
                    .execute(pool)
                    .await?;
                    *dup_season_id
                };

                let _ = target_season_id; // used above
            }

            // Delete the duplicate show (cascades to leftover empty seasons)
            sqlx::query("DELETE FROM seasons WHERE show_id = ?")
                .bind(dup_id)
                .execute(pool)
                .await?;
            sqlx::query("DELETE FROM tv_shows WHERE id = ?")
                .bind(dup_id)
                .execute(pool)
                .await?;

            tracing::info!("  Removed duplicate show '{}' (id={})", dup_title, dup_id);
            total_merged += 1;
        }
    }

    if total_merged > 0 {
        tracing::info!("TV show deduplication complete: merged {} duplicate entries", total_merged);
    } else {
        tracing::info!("TV show deduplication complete: no duplicates found");
    }

    Ok(())
}
