// core/src/models/scene_marker.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SceneMarker {
    pub id: i64,
    pub media_id: i64,
    pub media_type: String, // "movie" or "episode"
    pub seconds: f64,
    pub title: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSceneMarker {
    pub media_id: i64,
    pub media_type: String, // "movie" or "episode"
    pub seconds: f64,
    pub title: String,
}
