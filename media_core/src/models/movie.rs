// core/src/models/movie.rs
use serde::{Deserialize, Serialize};
use crate::models::{MovieId, LibraryId, MovieFileId};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Movie {
    pub id: MovieId,
    pub library_id: LibraryId,
    pub title: String,
    pub year: Option<i32>,
    pub tmdb_id: Option<i32>,
    pub imdb_id: Option<String>,
    pub status: crate::models::MediaStatus,
    pub plot: Option<String>,
    pub rating: Option<f32>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub tagline: Option<String>,
    pub runtime: Option<i32>,
    pub genres: Option<String>, // JSON string
    pub language: Option<String>,
    pub cast_list: Option<String>, // JSON string
    pub preview_path: Option<String>,
    pub nfo_path: Option<String>,
    pub codec: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub hash: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CastMember {
    pub name: String,
    pub role: Option<String>,
    pub image: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MovieFile {
    pub id: MovieFileId,
    pub movie_id: MovieId,
    pub file_path: String,
    pub original_name: String,
    pub size_bytes: i64,
    pub resolution: Option<crate::models::Resolution>,
    pub codec: Option<String>,
    pub audio_codec: Option<String>,
    pub hash: Option<String>,
    pub fingerprint: Option<String>,
    pub is_missing: bool,
    pub last_scanned: Option<String>,
    pub preview_path: Option<String>,
    pub aspect_ratio: Option<String>,
    pub duration_secs: Option<i32>,
    pub thumbnail_path: Option<String>,
    pub mtime: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}
