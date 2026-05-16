// core/src/models/mod.rs
pub mod library;
pub mod movie;
pub mod tv;
pub mod task;
pub mod ids;

pub use library::*;
pub use movie::*;
pub use tv::*;
pub use task::*;
pub use ids::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub enum MediaStatus {
    Unmatched,
    Matched,
}

impl std::fmt::Display for MediaStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unmatched => write!(f, "unmatched"),
            Self::Matched => write!(f, "matched"),
        }
    }
}

impl Default for MediaStatus {
    fn default() -> Self {
        Self::Unmatched
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
pub enum Resolution {
    #[serde(rename = "2160p")]
    #[sqlx(rename = "2160p")]
    R2160p,
    #[serde(rename = "1080p")]
    #[sqlx(rename = "1080p")]
    R1080p,
    #[serde(rename = "720p")]
    #[sqlx(rename = "720p")]
    R720p,
    #[serde(rename = "576p")]
    #[sqlx(rename = "576p")]
    R576p,
    #[serde(rename = "480p")]
    #[sqlx(rename = "480p")]
    R480p,
    #[serde(rename = "imax")]
    #[sqlx(rename = "imax")]
    Imax,
}

impl std::str::FromStr for Resolution {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "2160p" => Ok(Self::R2160p),
            "1080p" => Ok(Self::R1080p),
            "720p" => Ok(Self::R720p),
            "576p" => Ok(Self::R576p),
            "480p" => Ok(Self::R480p),
            "imax" => Ok(Self::Imax),
            _ => Err(()),
        }
    }
}

impl Resolution {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::R2160p => "2160p",
            Self::R1080p => "1080p",
            Self::R720p => "720p",
            Self::R576p => "576p",
            Self::R480p => "480p",
            Self::Imax => "imax",
        }
    }

    pub fn from_dimensions(width: i32, height: i32) -> Self {
        if width >= 3840 || height >= 2160 { Self::R2160p }
        else if width >= 1920 || height >= 1080 { Self::R1080p }
        else if width >= 1280 || height >= 720 { Self::R720p }
        else if width >= 1024 || height >= 576 { Self::R576p }
        else { Self::R480p }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MediaStream {
    pub id: i64,
    pub file_hash: String,
    pub stream_index: i32,
    pub stream_type: String,
    pub codec: Option<String>,
    pub language: Option<String>,
    pub title: Option<String>,
    pub channels: Option<i32>,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GeneratedAsset {
    pub id: i64,
    pub file_hash: String,
    pub asset_type: String,
    pub path: String,
}
