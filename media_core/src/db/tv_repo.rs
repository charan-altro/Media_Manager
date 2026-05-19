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
        fingerprint: Option<&'a str>
    ) -> Result<EpisodeId>;
    async fn update_episode_path(&self, id: EpisodeId, new_path: &str) -> Result<()>;
    async fn update_episode_last_scanned(&self, id: EpisodeId) -> Result<()>;
    async fn update_episode_fingerprint(&self, id: EpisodeId, fingerprint: &str) -> Result<()>;
    async fn update_episode_duration(&self, id: EpisodeId, duration_secs: i32) -> Result<()>;
    async fn update_episode_metadata(&self, id: EpisodeId, duration_secs: i32, width: i32, height: i32) -> Result<()>;
    async fn mark_missing_in_library(&self, library_id: LibraryId) -> Result<i32>;
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
}

impl TvWriter for SqliteTvRepository {
    #[tracing::instrument(skip(self), err)]
    async fn upsert_show(&self, library_id: LibraryId, title: &str) -> Result<TvShowId> {
        let row: (TvShowId,) = sqlx::query_as(
            r#"
            INSERT INTO tv_shows (library_id, title) 
            VALUES (?, ?) 
            ON CONFLICT(library_id, title) DO UPDATE SET updated_at = datetime('now')
            RETURNING id
            "#
        )
        .bind(library_id)
        .bind(title)
        .fetch_one(&*self.base.pool)
        .await?;

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
        .execute(&*self.base.pool)
        .await?;
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
        .execute(&*self.base.pool)
        .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn upsert_season(&self, show_id: TvShowId, season_number: i32) -> Result<SeasonId> {
        let row: (SeasonId,) = sqlx::query_as(
            r#"
            INSERT INTO seasons (show_id, season_number) 
            VALUES (?, ?) 
            ON CONFLICT(show_id, season_number) DO UPDATE SET updated_at = datetime('now')
            RETURNING id
            "#
        )
        .bind(show_id)
        .bind(season_number)
        .fetch_one(&*self.base.pool)
        .await?;

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
        fingerprint: Option<&str>
    ) -> Result<EpisodeId> {
        let normalized_path = crate::paths::normalize_slashes(file_path);
        let row: (EpisodeId,) = sqlx::query_as(
            r#"
            INSERT INTO episodes (season_id, episode_number, file_path, original_name, size_bytes, mtime, resolution, codec, audio_codec, duration_secs, hash, fingerprint, is_missing, last_scanned)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, datetime('now'))
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
                updated_at = datetime('now')
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
        .fetch_one(&*self.base.pool)
        .await?;

        Ok(row.0)
    }

    #[tracing::instrument(skip(self), err)]
    async fn update_episode_path(&self, id: EpisodeId, new_path: &str) -> Result<()> {
        let normalized = crate::paths::normalize_slashes(new_path);
        sqlx::query("UPDATE episodes SET file_path = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(normalized)
            .bind(id)
            .execute(&*self.base.pool)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn update_episode_last_scanned(&self, id: EpisodeId) -> Result<()> {
        sqlx::query("UPDATE episodes SET last_scanned = datetime('now'), is_missing = 0 WHERE id = ?")
            .bind(id)
            .execute(&*self.base.pool)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn update_episode_fingerprint(&self, id: EpisodeId, fingerprint: &str) -> Result<()> {
        sqlx::query("UPDATE episodes SET fingerprint = ? WHERE id = ?")
            .bind(fingerprint)
            .bind(id)
            .execute(&*self.base.pool)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn update_episode_duration(&self, id: EpisodeId, duration_secs: i32) -> Result<()> {
        sqlx::query("UPDATE episodes SET duration_secs = ? WHERE id = ?")
            .bind(duration_secs)
            .bind(id)
            .execute(&*self.base.pool)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn update_episode_metadata(&self, id: EpisodeId, duration_secs: i32, width: i32, height: i32) -> Result<()> {
        let res = Resolution::from_dimensions(width, height);
        sqlx::query("UPDATE episodes SET duration_secs = ?, resolution = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(duration_secs)
            .bind(res)
            .bind(id)
            .execute(&*self.base.pool)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn mark_missing_in_library(&self, library_id: LibraryId) -> Result<i32> {
        let rows = sqlx::query(
            r#"
            UPDATE episodes
            SET is_missing = 1
            WHERE season_id IN (SELECT s.id FROM seasons s JOIN tv_shows t ON s.show_id = t.id WHERE t.library_id = ?)
            AND last_scanned < datetime('now', '-1 minute')
            RETURNING id
            "#
        )
        .bind(library_id)
        .fetch_all(&*self.base.pool)
        .await?;
        Ok(rows.len() as i32)
    }
}

impl TvReaderWriter for SqliteTvRepository {}
