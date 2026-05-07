// core/src/models/library.rs
use serde::{Deserialize, Serialize};
use sqlx::Type;

use crate::models::LibraryId;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Library {
    pub id: LibraryId,
    pub name: String,
    pub path: String,
    pub media_type: MediaType,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum MediaType {
    Movie,
    Tv,
}
