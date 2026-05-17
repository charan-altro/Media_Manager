use thiserror::Error;

#[derive(Error, Debug)]
pub enum ScraperError {
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("Rate limit exceeded")]
    RateLimited(#[from] tokio::sync::AcquireError),

    #[error("Metadata not found: {0}")]
    NotFound(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Repository error: {0}")]
    RepoError(#[from] crate::db::DatabaseError),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("API Key missing: {0}")]
    MissingApiKey(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("XML error: {0}")]
    XmlError(#[from] quick_xml::DeError),
}

pub type Result<T> = std::result::Result<T, ScraperError>;
