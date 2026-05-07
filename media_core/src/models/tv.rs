// core/src/models/tv.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TVShow {
    pub id: i64,
    pub library_id: i64,
    pub title: String,
    pub tmdb_id: Option<i32>,
    pub imdb_id: Option<String>,
    pub status: crate::models::MediaStatus,
    pub plot: Option<String>,
    pub rating: Option<f32>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub tagline: Option<String>,
    pub runtime: Option<i32>,
    pub genres: Option<String>,
    pub language: Option<String>,
    pub cast_list: Option<String>,
    pub trailer_url: Option<String>,
    pub nfo_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Season {
    pub id: i64,
    pub show_id: i64,
    pub season_number: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Episode {
    pub id: i64,
    pub season_id: i64,
    pub episode_number: i32,
    pub title: Option<String>,
    pub file_path: String,
    pub original_name: String,
    pub size_bytes: i64,
    pub resolution: Option<crate::models::Resolution>,
    pub codec: Option<String>,
    pub aspect_ratio: Option<String>,
    pub thumbnail_path: Option<String>,
}
