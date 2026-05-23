// media_core/src/scraper/provider.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScrapedMovieSearchResult {
    pub id: String,
    pub title: String,
    pub release_date: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub vote_average: f32,
    pub original_language: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScrapedMovie {
    pub id: String,
    pub imdb_id: Option<String>,
    pub title: String,
    pub overview: Option<String>,
    pub tagline: Option<String>,
    pub runtime: Option<i32>,
    pub release_date: Option<String>,
    pub vote_average: f32,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub genres: Vec<String>,
    pub cast: Vec<ScrapedCastMember>,
    pub videos: Vec<ScrapedVideo>,
    pub original_language: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScrapedCastMember {
    pub name: String,
    pub character: String,
    pub profile_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScrapedVideo {
    pub key: String,
    pub site: String,
    pub video_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScrapedTvSearchResult {
    pub id: String,
    pub name: String,
    pub first_air_date: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub vote_average: f32,
    pub original_language: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScrapedTvShow {
    pub id: String,
    pub name: String,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub vote_average: f32,
    pub genres: Vec<String>,
    pub cast: Vec<ScrapedCastMember>,
    pub videos: Vec<ScrapedVideo>,
    pub original_language: Option<String>,
}

pub trait ScraperProvider: Send + Sync {
    fn name(&self) -> &str;

    fn search_movie<'a>(&'a self, title: &'a str, year: Option<i32>) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::scraper::Result<Vec<ScrapedMovieSearchResult>>> + Send + 'a>>;

    fn get_movie_details<'a>(&'a self, id: &'a str) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::scraper::Result<ScrapedMovie>> + Send + 'a>>;

    fn search_tv_show<'a>(&'a self, title: &'a str) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::scraper::Result<Vec<ScrapedTvSearchResult>>> + Send + 'a>>;

    fn get_tv_details<'a>(&'a self, id: &'a str) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::scraper::Result<ScrapedTvShow>> + Send + 'a>>;
}
