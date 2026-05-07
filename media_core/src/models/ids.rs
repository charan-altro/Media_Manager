use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct LibraryId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct MovieId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct TvShowId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct SeasonId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct EpisodeId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct MovieFileId(pub i64);

macro_rules! impl_display_and_from {
    ($t:ident) => {
        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<i64> for $t {
            fn from(id: i64) -> Self {
                Self(id)
            }
        }

        impl From<$t> for i64 {
            fn from(id: $t) -> Self {
                id.0
            }
        }
    };
}

impl_display_and_from!(LibraryId);
impl_display_and_from!(MovieId);
impl_display_and_from!(TvShowId);
impl_display_and_from!(SeasonId);
impl_display_and_from!(EpisodeId);
impl_display_and_from!(MovieFileId);
