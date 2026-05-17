// media_core/src/db/settings_repo.rs
#![allow(async_fn_in_trait)]
use crate::db::Result;
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;
use std::collections::HashMap;

// --- SettingsRepository trait ---
#[cfg_attr(test, mockall::automock)]
pub trait SettingsRepository: Send + Sync {
    async fn get_all(&self) -> Result<HashMap<String, String>>;
    async fn set(&self, key: &str, value: &str) -> Result<()>;
}

// --- SQLite implementation ---
pub struct SqliteSettingsRepository {
    base: super::base::SqliteBase,
}

impl SqliteSettingsRepository {
    pub fn new(pool: Arc<SqlitePool>) -> Self {
        Self {
            base: super::base::SqliteBase::new(pool),
        }
    }
}

impl SettingsRepository for SqliteSettingsRepository {
    #[tracing::instrument(skip(self), err)]
    async fn get_all(&self) -> Result<HashMap<String, String>> {
        let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM settings")
            .fetch_all(&*self.base.pool)
            .await?;
        Ok(rows.into_iter().collect())
    }

    #[tracing::instrument(skip(self), err)]
    async fn set(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=datetime('now')")
            .bind(key)
            .bind(value)
            .execute(&*self.base.pool)
            .await?;
        Ok(())
    }
}
