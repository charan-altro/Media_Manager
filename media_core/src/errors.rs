use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Path error: {0}")]
    PathError(String),

    #[error("FFmpeg error: {0}")]
    FfmpegError(String),

    #[error("MediaInfo error: {0}")]
    MediaInfoError(String),

    #[error("Database error: {0}")]
    Database(#[from] crate::db::DatabaseError),

    #[error("Scraper error: {0}")]
    Scraper(#[from] crate::scraper::ScraperError),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Notification error: {0}")]
    NotificationError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Parse error: {0}")]
    ParseFloatError(#[from] std::num::ParseFloatError),

    #[error("SQLx error: {0}")]
    SqlxError(#[from] sqlx::Error),
}

pub type Result<T> = std::result::Result<T, CoreError>;
