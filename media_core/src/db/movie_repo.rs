// media_core/src/db/movie_repo.rs
#![allow(async_fn_in_trait)]
use crate::models::{Movie, MovieId, LibraryId, MovieFile, MovieFileId, Resolution, MediaStatus};
use crate::db::Result;
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;
use std::path::PathBuf;

// --- Reader interface ---
#[cfg_attr(test, mockall::automock)]
pub trait MovieReader: Send + Sync {
    async fn find_all(&self, library_id: Option<LibraryId>, genre: Option<String>, language: Option<String>) -> Result<Vec<Movie>>;
    async fn find_by_id(&self, id: MovieId) -> Result<Option<Movie>>;
    async fn find_by_ids(&self, ids: &[MovieId]) -> Result<Vec<Movie>>;
    async fn find_file_by_path(&self, path: &str) -> Result<Option<MovieFile>>;
    async fn find_file_by_hash(&self, hash: &str) -> Result<Option<MovieFile>>;
    async fn find_file_by_fingerprint(&self, fp: &str) -> Result<Option<MovieFile>>;
    async fn find_file_by_movie_id(&self, movie_id: MovieId) -> Result<Option<MovieFile>>;
    async fn get_full_path(&self, movie_id: MovieId) -> Result<Option<PathBuf>>;
}

// --- Writer interface ---
#[cfg_attr(test, mockall::automock)]
pub trait MovieWriter: Send + Sync {
    async fn upsert(&self, library_id: LibraryId, title: &str, year: Option<i32>) -> Result<MovieId>;
    async fn update<'a>(&self, id: MovieId, title: &str, year: Option<i32>, plot: Option<&'a str>, rating: Option<f32>, genres: Option<&'a str>) -> Result<()>;
    async fn update_metadata(
        &self,
        id: MovieId,
        tmdb_id: Option<i32>,
        imdb_id: Option<String>,
        status: MediaStatus,
        plot: Option<String>,
        rating: Option<f32>,
        tagline: Option<String>,
        runtime: Option<i32>,
        genres: Option<String>,
        language: Option<String>,
        cast_list: Option<String>,
        poster_url: Option<String>,
        backdrop_url: Option<String>,
    ) -> Result<()>;
    async fn upsert_file<'a>(
        &self, 
        movie_id: MovieId, 
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
    ) -> Result<MovieFileId>;
    async fn update_file_path(&self, id: MovieFileId, new_path: &str) -> Result<()>;
    async fn update_file_last_scanned(&self, id: MovieFileId) -> Result<()>;
    async fn update_file_fingerprint(&self, id: MovieFileId, fingerprint: &str) -> Result<()>;
    async fn update_file_resolution(&self, id: MovieFileId, resolution: Resolution) -> Result<()>;
    async fn update_file_duration(&self, id: MovieFileId, duration_secs: i32) -> Result<()>;
    async fn update_file_metadata(&self, id: MovieFileId, duration_secs: i32, width: i32, height: i32) -> Result<()>;
    async fn mark_missing_in_library(&self, library_id: LibraryId) -> Result<i32>;
}

// --- Combined ---
pub trait MovieReaderWriter: MovieReader + MovieWriter {}

// --- SQLite implementation ---
pub struct SqliteMovieRepository {
    base: super::base::SqliteBase,
}

impl SqliteMovieRepository {
    pub fn new(pool: Arc<SqlitePool>) -> Self {
        Self {
            base: super::base::SqliteBase::new(pool),
        }
    }
}

impl MovieReader for SqliteMovieRepository {
    #[tracing::instrument(skip(self), err)]
    async fn find_all(&self, library_id: Option<LibraryId>, genre: Option<String>, language: Option<String>) -> Result<Vec<Movie>> {
        let mut query = String::from("SELECT m.*, mf.preview_path, mf.codec as video_codec, mf.audio_codec FROM movies m LEFT JOIN movie_files mf ON m.id = mf.movie_id WHERE 1=1");
        if library_id.is_some() {
            query.push_str(" AND m.library_id = ?");
        }
        
        let genre_active = genre.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
        if genre_active {
            query.push_str(" AND m.genres LIKE ?");
        }
        
        let language_active = language.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
        if language_active {
            query.push_str(" AND m.language = ?");
        }
        query.push_str(" GROUP BY m.id ORDER BY m.title ASC");

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
    async fn find_by_id(&self, id: MovieId) -> Result<Option<Movie>> {
        let mut args = sqlx::sqlite::SqliteArguments::default();
        sqlx::Arguments::add(&mut args, id);
        self.base.fetch_optional(&*self.base.pool, "SELECT m.*, mf.codec as video_codec, mf.audio_codec FROM movies m LEFT JOIN movie_files mf ON m.id = mf.movie_id WHERE m.id = ?", args).await
    }

    #[tracing::instrument(skip(self), err)]
    async fn find_by_ids(&self, ids: &[MovieId]) -> Result<Vec<Movie>> {
        if ids.is_empty() { return Ok(vec![]); }
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!("SELECT m.*, mf.preview_path, mf.codec as video_codec, mf.audio_codec FROM movies m LEFT JOIN movie_files mf ON m.id = mf.movie_id WHERE m.id IN ({})", placeholders);
        
        let mut args = sqlx::sqlite::SqliteArguments::default();
        for id in ids {
            sqlx::Arguments::add(&mut args, *id);
        }
        self.base.fetch_all(&*self.base.pool, &query, args).await
    }

    #[tracing::instrument(skip(self), err)]
    async fn find_file_by_path(&self, path: &str) -> Result<Option<MovieFile>> {
        let normalized = crate::paths::normalize_slashes(path);
        let mut args = sqlx::sqlite::SqliteArguments::default();
        sqlx::Arguments::add(&mut args, normalized);
        self.base.fetch_optional(&*self.base.pool, "SELECT * FROM movie_files WHERE file_path = ?", args).await
    }

    #[tracing::instrument(skip(self), err)]
    async fn find_file_by_hash(&self, hash: &str) -> Result<Option<MovieFile>> {
        let mut args = sqlx::sqlite::SqliteArguments::default();
        sqlx::Arguments::add(&mut args, hash);
        self.base.fetch_optional(&*self.base.pool, "SELECT * FROM movie_files WHERE hash = ?", args).await
    }

    #[tracing::instrument(skip(self), err)]
    async fn find_file_by_fingerprint(&self, fp: &str) -> Result<Option<MovieFile>> {
        let mut args = sqlx::sqlite::SqliteArguments::default();
        sqlx::Arguments::add(&mut args, fp);
        self.base.fetch_optional(&*self.base.pool, "SELECT * FROM movie_files WHERE fingerprint = ?", args).await
    }

    #[tracing::instrument(skip(self), err)]
    async fn find_file_by_movie_id(&self, movie_id: MovieId) -> Result<Option<MovieFile>> {
        let mut args = sqlx::sqlite::SqliteArguments::default();
        sqlx::Arguments::add(&mut args, movie_id);
        self.base.fetch_optional(&*self.base.pool, "SELECT * FROM movie_files WHERE movie_id = ? LIMIT 1", args).await
    }

    #[tracing::instrument(skip(self), err)]
    async fn get_full_path(&self, movie_id: MovieId) -> Result<Option<PathBuf>> {
        let mut args = sqlx::sqlite::SqliteArguments::default();
        sqlx::Arguments::add(&mut args, movie_id);
        let row: Option<(String, String)> = self.base.fetch_optional(&*self.base.pool, 
            r#"
            SELECT l.path, mf.file_path 
            FROM movie_files mf 
            JOIN movies m ON mf.movie_id = m.id 
            JOIN libraries l ON m.library_id = l.id 
            WHERE m.id = ? 
            LIMIT 1
            "#, args).await?;

        if let Some((lib_path, rel_path)) = row {
            Ok(Some(crate::paths::make_absolute(&rel_path, std::path::Path::new(&lib_path))))
        } else {
            Ok(None)
        }
    }
}

impl MovieWriter for SqliteMovieRepository {
    #[tracing::instrument(skip(self), err)]
    async fn upsert(&self, library_id: LibraryId, title: &str, year: Option<i32>) -> Result<MovieId> {
        let row: (MovieId,) = sqlx::query_as(
            r#"
            INSERT INTO movies (library_id, title, year)
            VALUES (?, ?, ?)
            ON CONFLICT(library_id, title, IFNULL(year, 0)) DO UPDATE SET updated_at = datetime('now')
            RETURNING id
            "#
        )
        .bind(library_id)
        .bind(title)
        .bind(year)
        .fetch_one(&*self.base.pool)
        .await?;

        Ok(row.0)
    }

    #[tracing::instrument(skip(self), err)]
    async fn update(&self, id: MovieId, title: &str, year: Option<i32>, plot: Option<&str>, rating: Option<f32>, genres: Option<&str>) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE movies 
            SET title = ?, year = ?, plot = ?, rating = ?, genres = ?, updated_at = datetime('now')
            WHERE id = ?
            "#
        )
        .bind(title)
        .bind(year)
        .bind(plot)
        .bind(rating)
        .bind(genres)
        .bind(id)
        .execute(&*self.base.pool)
        .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn update_metadata(
        &self,
        id: MovieId,
        tmdb_id: Option<i32>,
        imdb_id: Option<String>,
        status: MediaStatus,
        plot: Option<String>,
        rating: Option<f32>,
        tagline: Option<String>,
        runtime: Option<i32>,
        genres: Option<String>,
        language: Option<String>,
        cast_list: Option<String>,
        poster_url: Option<String>,
        backdrop_url: Option<String>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE movies
            SET tmdb_id = COALESCE(?, tmdb_id),
                imdb_id = COALESCE(?, imdb_id),
                status   = ?,
                plot     = COALESCE(?, plot),
                rating   = COALESCE(?, rating),
                tagline  = COALESCE(?, tagline),
                runtime  = COALESCE(?, runtime),
                genres   = COALESCE(?, genres),
                language = COALESCE(?, language),
                cast_list = COALESCE(?, cast_list),
                poster_url   = COALESCE(?, poster_url),
                backdrop_url = COALESCE(?, backdrop_url),
                updated_at   = datetime('now')
            WHERE id = ?
            "#
        )
        .bind(tmdb_id)
        .bind(imdb_id)
        .bind(status)
        .bind(plot)
        .bind(rating)
        .bind(tagline)
        .bind(runtime)
        .bind(genres)
        .bind(language)
        .bind(cast_list)
        .bind(poster_url)
        .bind(backdrop_url)
        .bind(id)
        .execute(&*self.base.pool)
        .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn upsert_file(
        &self, 
        movie_id: MovieId, 
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
    ) -> Result<MovieFileId> {
        let normalized_path = crate::paths::normalize_slashes(file_path);
        let row: (MovieFileId,) = sqlx::query_as(
            r#"
            INSERT INTO movie_files (movie_id, file_path, original_name, size_bytes, mtime, resolution, codec, audio_codec, duration_secs, hash, fingerprint, is_missing, last_scanned)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, datetime('now'))
            ON CONFLICT(file_path) DO UPDATE SET 
                size_bytes=excluded.size_bytes,
                mtime=excluded.mtime,
                resolution=excluded.resolution,
                codec=excluded.codec,
                audio_codec=excluded.audio_codec,
                duration_secs=excluded.duration_secs,
                hash=excluded.hash,
                fingerprint=excluded.fingerprint,
                is_missing=0,
                last_scanned=datetime('now'),
                updated_at=datetime('now')
            RETURNING id
            "#
        )
        .bind(movie_id)
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
    async fn update_file_path(&self, id: MovieFileId, new_path: &str) -> Result<()> {
        let normalized = crate::paths::normalize_slashes(new_path);
        sqlx::query("UPDATE movie_files SET file_path = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(normalized)
            .bind(id)
            .execute(&*self.base.pool)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn update_file_last_scanned(&self, id: MovieFileId) -> Result<()> {
        sqlx::query("UPDATE movie_files SET last_scanned = datetime('now'), is_missing = 0 WHERE id = ?")
            .bind(id)
            .execute(&*self.base.pool)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn update_file_fingerprint(&self, id: MovieFileId, fingerprint: &str) -> Result<()> {
        sqlx::query("UPDATE movie_files SET fingerprint = ? WHERE id = ?")
            .bind(fingerprint)
            .bind(id)
            .execute(&*self.base.pool)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn update_file_resolution(&self, id: MovieFileId, resolution: Resolution) -> Result<()> {
        sqlx::query("UPDATE movie_files SET resolution = ? WHERE id = ?")
            .bind(resolution)
            .bind(id)
            .execute(&*self.base.pool)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn update_file_duration(&self, id: MovieFileId, duration_secs: i32) -> Result<()> {
        sqlx::query("UPDATE movie_files SET duration_secs = ? WHERE id = ?")
            .bind(duration_secs)
            .bind(id)
            .execute(&*self.base.pool)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn update_file_metadata(&self, id: MovieFileId, duration_secs: i32, width: i32, height: i32) -> Result<()> {
        let res = Resolution::from_dimensions(width, height);
        sqlx::query("UPDATE movie_files SET duration_secs = ?, resolution = ?, updated_at = datetime('now') WHERE id = ?")
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
            UPDATE movie_files
            SET is_missing = 1
            WHERE movie_id IN (SELECT id FROM movies WHERE library_id = ?)
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

impl MovieReaderWriter for SqliteMovieRepository {}
