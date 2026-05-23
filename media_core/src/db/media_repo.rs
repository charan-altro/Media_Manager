// media_core/src/db/media_repo.rs
#![allow(async_fn_in_trait)]
use crate::models::{MediaStream, PlaybackState};
use crate::db::Result;
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;

// --- MediaRepository trait (streams/assets/genres/playback) ---
#[cfg_attr(test, mockall::automock)]
pub trait MediaRepository: Send + Sync {
    async fn upsert_stream(&self, stream: &MediaStream) -> Result<()>;
    async fn upsert_generated_asset(&self, hash: &str, asset_type: &str, path: &str) -> Result<()>;
    async fn get_unique_genres(&self) -> Result<Vec<String>>;
    async fn get_unique_languages(&self) -> Result<Vec<String>>;
    
    // Playback State (Resume)
    async fn get_playback_status(&self, media_id: i64, media_type: &str) -> Result<Option<PlaybackState>>;
    async fn update_playback_status(&self, media_id: i64, media_type: &str, position_ms: i32, duration_ms: i32, is_finished: bool) -> Result<()>;
}

// --- SQLite implementation ---
pub struct SqliteMediaRepository {
    base: super::base::SqliteBase,
}

impl SqliteMediaRepository {
    pub fn new(pool: Arc<SqlitePool>) -> Self {
        Self {
            base: super::base::SqliteBase::new(pool),
        }
    }
}

impl MediaRepository for SqliteMediaRepository {
    #[tracing::instrument(skip(self), err)]
    async fn upsert_stream(&self, stream: &MediaStream) -> Result<()> {
        crate::execute_db!(
            &*self.base.pool,
            sqlx::query(
                r#"
                INSERT INTO media_streams (file_hash, stream_index, stream_type, codec, language, title, channels, is_default)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(file_hash, stream_index) DO UPDATE SET
                    codec = excluded.codec,
                    language = excluded.language,
                    title = excluded.title,
                    channels = excluded.channels,
                    is_default = excluded.is_default
                "#
            )
            .bind(&stream.file_hash)
            .bind(stream.stream_index)
            .bind(&stream.stream_type)
            .bind(&stream.codec)
            .bind(&stream.language)
            .bind(&stream.title)
            .bind(stream.channels)
            .bind(stream.is_default)
        ).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn upsert_generated_asset(&self, hash: &str, asset_type: &str, path: &str) -> Result<()> {
        crate::execute_db!(
            &*self.base.pool,
            sqlx::query(
                "INSERT INTO generated_assets (file_hash, asset_type, path) VALUES (?, ?, ?) ON CONFLICT(file_hash, asset_type) DO UPDATE SET path=excluded.path"
            )
            .bind(hash)
            .bind(asset_type)
            .bind(path)
        ).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    async fn get_unique_genres(&self) -> Result<Vec<String>> {
        let rows: Vec<(Option<String>,)> = sqlx::query_as(
            "SELECT genres FROM movies WHERE genres IS NOT NULL
             UNION ALL
             SELECT genres FROM tv_shows WHERE genres IS NOT NULL"
        )
        .fetch_all(&*self.base.pool)
        .await?;
        
        let mut genres = std::collections::HashSet::new();
        for (row,) in rows {
            if let Some(json_str) = row {
                if let Ok(list) = serde_json::from_str::<Vec<String>>(&json_str) {
                    for g in list { genres.insert(g); }
                }
            }
        }
        
        let mut result: Vec<String> = genres.into_iter().collect();
        result.sort();
        Ok(result)
    }

    #[tracing::instrument(skip(self), err)]
    async fn get_unique_languages(&self) -> Result<Vec<String>> {
        let rows: Vec<(Option<String>,)> = sqlx::query_as(
            "SELECT language FROM movies WHERE language IS NOT NULL
             UNION ALL
             SELECT language FROM tv_shows WHERE language IS NOT NULL"
        )
        .fetch_all(&*self.base.pool)
        .await?;
        
        let langs: std::collections::HashSet<String> = rows.into_iter().filter_map(|(l,)| l).collect();
        let mut result: Vec<String> = langs.into_iter().collect();
        result.sort();
        Ok(result)
    }

    #[tracing::instrument(skip(self), err)]
    async fn get_playback_status(&self, media_id: i64, media_type: &str) -> Result<Option<PlaybackState>> {
        let mut args = sqlx::sqlite::SqliteArguments::default();
        sqlx::Arguments::add(&mut args, media_id);
        sqlx::Arguments::add(&mut args, media_type);
        self.base.fetch_optional(&*self.base.pool, "SELECT media_id, media_type, position_ms, duration_ms, is_finished FROM playback_state WHERE media_id = ? AND media_type = ?", args).await
    }

    #[tracing::instrument(skip(self), err)]
    async fn update_playback_status(&self, media_id: i64, media_type: &str, position_ms: i32, duration_ms: i32, is_finished: bool) -> Result<()> {
        crate::execute_db!(
            &*self.base.pool,
            sqlx::query(
                r#"
                INSERT INTO playback_state (media_id, media_type, position_ms, duration_ms, is_finished, updated_at)
                VALUES (?, ?, ?, ?, ?, datetime('now'))
                ON CONFLICT(media_id, media_type) DO UPDATE SET
                    position_ms = excluded.position_ms,
                    duration_ms = excluded.duration_ms,
                    is_finished = excluded.is_finished,
                    updated_at = datetime('now')
                "#
            )
            .bind(media_id)
            .bind(media_type)
            .bind(position_ms)
            .bind(duration_ms)
            .bind(is_finished)
        ).await?;
        Ok(())
    }
}
