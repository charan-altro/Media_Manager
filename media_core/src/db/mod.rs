// media_core/src/db/mod.rs
pub mod queries;

use sqlx::sqlite::{SqlitePool, SqliteConnectOptions};
use std::str::FromStr;
use anyhow::Result;

pub async fn init_pool(database_url: &str) -> Result<SqlitePool> {
    tracing::info!("Connecting to database: {}", database_url);
    
    let opts = SqliteConnectOptions::from_str(database_url)?
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
