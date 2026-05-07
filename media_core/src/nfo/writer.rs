// core/src/nfo/writer.rs
use crate::models::{Movie, TVShow, Episode};
use anyhow::Result;
use quick_xml::se::to_string;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "movie")]
pub struct MovieNfo {
    pub title: String,
    pub originaltitle: Option<String>,
    pub sorttitle: Option<String>,
    pub year: Option<i32>,
    pub rating: Option<f32>,
    pub plot: Option<String>,
    pub outline: Option<String>,
    pub tagline: Option<String>,
    pub runtime: Option<i32>,
    pub thumb: Option<String>,
    pub fanart: Option<String>,
    pub mpau: Option<String>,
    pub id: Option<String>, // IMDB ID
    pub tmdbid: Option<i32>,
    pub genre: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "tvshow")]
pub struct TvShowNfo {
    pub title: String,
    pub originaltitle: Option<String>,
    pub sorttitle: Option<String>,
    pub year: Option<i32>,
    pub rating: Option<f32>,
    pub plot: Option<String>,
    pub id: Option<String>, // TVDB/IMDB ID
    pub tmdbid: Option<i32>,
    pub genre: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "episodedetails")]
pub struct EpisodeNfo {
    pub title: String,
    pub showtitle: Option<String>,
    pub season: i32,
    pub episode: i32,
    pub plot: Option<String>,
    pub rating: Option<f32>,
    pub thumb: Option<String>,
    pub runtime: Option<i32>,
}

pub struct NfoWriter;

impl NfoWriter {
    pub async fn write_movie_nfo(movie: &Movie, dest_path: &Path) -> Result<()> {
        let genres: Vec<String> = if let Some(g_json) = &movie.genres {
            serde_json::from_str(g_json).unwrap_or_default()
        } else {
            vec![]
        };

        let nfo = MovieNfo {
            title: movie.title.clone(),
            originaltitle: Some(movie.title.clone()),
            sorttitle: Some(movie.title.clone()),
            year: movie.year,
            rating: movie.rating,
            plot: movie.plot.clone(),
            outline: movie.plot.clone(),
            tagline: movie.tagline.clone(),
            runtime: movie.runtime,
            thumb: movie.poster_url.clone(),
            fanart: movie.backdrop_url.clone(),
            mpau: None,
            id: movie.imdb_id.clone(),
            tmdbid: movie.tmdb_id,
            genre: genres,
        };

        let xml = to_string(&nfo)?;
        fs::write(dest_path, format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\" ?>\n{}", xml)).await?;
        Ok(())
    }

    pub async fn write_tvshow_nfo(show: &TVShow, dest_path: &Path) -> Result<()> {
        let genres: Vec<String> = if let Some(g_json) = &show.genres {
            serde_json::from_str(g_json).unwrap_or_default()
        } else {
            vec![]
        };

        let nfo = TvShowNfo {
            title: show.title.clone(),
            originaltitle: Some(show.title.clone()),
            sorttitle: Some(show.title.clone()),
            year: None,
            rating: show.rating,
            plot: show.plot.clone(),
            id: show.imdb_id.clone(),
            tmdbid: show.tmdb_id,
            genre: genres,
        };

        let xml = to_string(&nfo)?;
        fs::write(dest_path, format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\" ?>\n{}", xml)).await?;
        Ok(())
    }

    pub async fn write_episode_nfo(episode: &Episode, season_number: i32, dest_path: &Path) -> Result<()> {
        let title = episode.title.clone().unwrap_or_else(|| episode.original_name.clone());
        let nfo = EpisodeNfo {
            title,
            showtitle: None,
            season: season_number,
            episode: episode.episode_number,
            plot: None,
            rating: None,
            thumb: episode.thumbnail_path.clone(),
            runtime: None,
        };

        let xml = to_string(&nfo)?;
        fs::write(dest_path, format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\" ?>\n{}", xml)).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_write_movie_nfo() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("movie_nfo.test");
        
        let movie = Movie {
            id: 1,
            library_id: 1,
            title: "Test Movie".to_string(),
            year: Some(2023),
            tmdb_id: Some(12345),
            imdb_id: Some("tt12345".to_string()),
            status: crate::models::MediaStatus::Matched,
            plot: Some("A test movie plot".to_string()),
            rating: Some(8.5),
            poster_url: Some("http://example.com/poster.jpg".to_string()),
            backdrop_url: Some("http://example.com/backdrop.jpg".to_string()),
            tagline: Some("A test tagline".to_string()),
            runtime: Some(120),
            genres: Some("[\"Action\", \"Sci-Fi\"]".to_string()),
            language: Some("en".to_string()),
            cast_list: None,
            nfo_path: None,
            created_at: "2023-01-01".to_string(),
            updated_at: "2023-01-01".to_string(),
        };

        let res = NfoWriter::write_movie_nfo(&movie, &path).await;
        assert!(res.is_ok());

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\" ?>"));
        assert!(content.contains("<title>Test Movie</title>"));
        assert!(content.contains("<year>2023</year>"));
        assert!(content.contains("<id>tt12345</id>"));
        assert!(content.contains("<tmdbid>12345</tmdbid>"));
        assert!(content.contains("<genre>Action</genre>"));
        assert!(content.contains("<genre>Sci-Fi</genre>"));
        
        let _ = std::fs::remove_file(path);
    }
}
