use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Database query failed: {0}")]
    SqlxError(#[from] sqlx::Error),

    #[error("Migration failed: {0}")]
    MigrationError(#[from] sqlx::migrate::MigrateError),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("NotFound: {0}")]
    NotFound(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, DatabaseError>;
