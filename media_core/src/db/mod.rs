// media_core/src/db/mod.rs
pub mod errors;
pub(super) mod base;
pub mod library_repo;
pub mod movie_repo;
pub mod tv_repo;
pub mod media_repo;
pub mod settings_repo;

use sqlx::sqlite::{SqlitePool, SqliteConnectOptions};
use std::str::FromStr;
use std::sync::Arc;
pub use errors::{DatabaseError, Result};

pub use library_repo::*;
pub use movie_repo::*;
pub use tv_repo::*;
pub use media_repo::*;
pub use settings_repo::*;

pub struct Repositories {
    pub library:  Arc<SqliteLibraryRepository>,
    pub movie:    Arc<SqliteMovieRepository>,
    pub tv:       Arc<SqliteTvRepository>,
    pub media:    Arc<SqliteMediaRepository>,
    pub settings: Arc<SqliteSettingsRepository>,
    pub pool:     SqlitePool,
}

impl Repositories {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            library:  Arc::new(SqliteLibraryRepository::new(Arc::new(pool.clone()))),
            movie:    Arc::new(SqliteMovieRepository::new(Arc::new(pool.clone()))),
            tv:       Arc::new(SqliteTvRepository::new(Arc::new(pool.clone()))),
            media:    Arc::new(SqliteMediaRepository::new(Arc::new(pool.clone()))),
            settings: Arc::new(SqliteSettingsRepository::new(Arc::new(pool.clone()))),
            pool,
        }
    }

    /// Run a closure inside a SQLite transaction.
    /// Mirrors Stash's `repo.WithTxn(ctx, fn)` pattern.
    pub async fn with_txn<F, T>(&self, f: F) -> crate::errors::Result<T>
    where
        F: for<'c> FnOnce(&mut sqlx::Transaction<'c, sqlx::Sqlite>) 
            -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::errors::Result<T>> + Send + 'c>>,
        T: Send,
    {
        let mut txn = self.pool.begin().await
            .map_err(|e| DatabaseError::SqlxError(e))?;
        let result = f(&mut txn).await?;
        txn.commit().await
            .map_err(|e| DatabaseError::SqlxError(e))?;
        Ok(result)
    }
}

pub async fn init_pool(database_url: &str) -> Result<SqlitePool> {
    tracing::info!("Connecting to database: {}", database_url);
    
    let opts = SqliteConnectOptions::from_str(database_url)
        .map_err(|e| DatabaseError::ConfigError(e.to_string()))?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true);

    let pool = SqlitePool::connect_with(opts).await?;
    
    // Run migrations - path is relative to this file
    tracing::info!("Running migrations...");
    sqlx::migrate!("src/db/migrations")
        .run(&pool)
        .await?;
        
    tracing::info!("Database initialized successfully");
    Ok(pool)
}
