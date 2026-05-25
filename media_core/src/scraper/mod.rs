// core/src/scraper/mod.rs
pub mod tmdb;
pub mod omdb;
pub mod fanart;
pub mod trakt;
pub mod tvdb;
pub mod anidb;
pub mod imdb;
pub mod moviemeter;
pub mod thesportsdb;
pub mod ofdb;
pub mod kyradb;
pub mod kodi;
pub mod mpdb;
pub mod tvmaze;
pub mod imdbapi;
pub mod service;

pub mod provider;
pub mod errors;
pub use errors::{ScraperError, Result};

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::db::{Repositories, SettingsRepository};

pub fn build_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScraperSettings {
    pub primary_movie_scraper: String, // "tmdb", "imdb", "universal", "kodi"
    pub primary_tv_scraper: String,    // "tmdb", "tvdb", "anidb"
    pub movie_title_source: String,
    pub movie_plot_source: String,
    pub movie_rating_source: String,
    pub movie_artwork_source: String,
    pub fallback_language: String,
    pub include_adult: bool,
}

impl Default for ScraperSettings {
    fn default() -> Self {
        Self {
            primary_movie_scraper: "tmdb".to_string(),
            primary_tv_scraper: "tmdb".to_string(),
            movie_title_source: "tmdb".to_string(),
            movie_plot_source: "tmdb".to_string(),
            movie_rating_source: "omdb".to_string(),
            movie_artwork_source: "fanart".to_string(),
            fallback_language: "en".to_string(),
            include_adult: false,
        }
    }
}

pub trait MediaScraper: Send + Sync {
    fn search_movie<'a>(&'a self, title: &'a str, year: Option<i32>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<crate::scraper::tmdb::TmdbSearchResult>>> + Send + 'a>>;
    fn get_movie_details<'a>(&'a self, id: i32) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<crate::scraper::tmdb::TmdbMovieDetails>> + Send + 'a>>;
    fn search_tv_show<'a>(&'a self, title: &'a str) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<crate::scraper::tmdb::TmdbTvSearchResult>>> + Send + 'a>>;
    fn get_tv_details<'a>(&'a self, id: i32) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<crate::scraper::tmdb::TmdbTvDetails>> + Send + 'a>>;
}

pub struct ScraperClients {
    pub tmdb: Arc<dyn provider::ScraperProvider>,
    pub omdb: omdb::OmdbClient,
    pub fanart: fanart::FanartClient,
    pub trakt: trakt::TraktClient,
    pub tvdb: tvdb::TvdbClient,
    pub anidb: anidb::AnidbClient,
    pub imdb: imdb::ImdbClient,
    pub moviemeter: moviemeter::MovieMeterClient,
    pub sportsdb: thesportsdb::TheSportsDbClient,
    pub ofdb: ofdb::OfdbClient,
    pub kyra: kyradb::KyraDbClient,
    pub mpdb: mpdb::MpdbClient,
    pub tvmaze: tvmaze::TvmazeClient,
    pub imdbapi: imdbapi::ImdbApiClient,
    pub rate_limiter: tokio::sync::Semaphore,
}

impl ScraperClients {
    pub async fn from_settings(repos: &Repositories) -> Self {
        let settings = repos.settings.get_all().await.unwrap_or_default();
        Self::new(
            std::env::var("TMDB_API_KEY").unwrap_or_else(|_| settings.get("tmdb_api_key").cloned().unwrap_or_default()),
            std::env::var("OMDB_API_KEY").unwrap_or_else(|_| settings.get("omdb_api_key").cloned().unwrap_or_default()),
            std::env::var("FANART_API_KEY").unwrap_or_else(|_| settings.get("fanart_api_key").cloned().unwrap_or_default()),
            std::env::var("TRAKT_API_KEY").unwrap_or_else(|_| settings.get("trakt_access_token").cloned().unwrap_or_default()), // Corrected key
            std::env::var("TVDB_API_KEY").unwrap_or_else(|_| settings.get("tvdb_api_key").cloned().unwrap_or_default()),
            std::env::var("MOVIEMETER_API_KEY").unwrap_or_else(|_| settings.get("moviemeter_api_key").cloned().unwrap_or_default()),
            std::env::var("SPORTSDB_API_KEY").unwrap_or_else(|_| settings.get("sportsdb_api_key").cloned().unwrap_or_default()),
            std::env::var("KYRADB_API_KEY").unwrap_or_else(|_| settings.get("kyradb_api_key").cloned().unwrap_or_default()),
            std::env::var("MPDB_API_KEY").unwrap_or_else(|_| settings.get("mpdb_api_key").cloned().unwrap_or_default()),
            std::env::var("IMDBAPI_API_KEY").unwrap_or_else(|_| settings.get("imdbapi_api_key").cloned().unwrap_or_default()),
        )
    }

    pub fn new(
        tmdb_key: String, 
        omdb_key: String, 
        fanart_key: String, 
        trakt_key: String, 
        tvdb_key: String,
        moviemeter_key: String,
        sportsdb_key: String,
        kyradb_key: String,
        mpdb_key: String,
        imdbapi_key: String,
    ) -> Self {
        Self {
            tmdb: Arc::new(tmdb::TmdbClient::new(tmdb_key)),
            omdb: omdb::OmdbClient::new(omdb_key),
            fanart: fanart::FanartClient::new(fanart_key),
            trakt: trakt::TraktClient::new(trakt_key),
            tvdb: tvdb::TvdbClient::new(tvdb_key),
            anidb: anidb::AnidbClient::new("MediaManager".to_string(), "0.1.0".to_string()),
            imdb: imdb::ImdbClient::new(),
            moviemeter: moviemeter::MovieMeterClient::new(moviemeter_key),
            sportsdb: thesportsdb::TheSportsDbClient::new(sportsdb_key),
            ofdb: ofdb::OfdbClient::new(),
            kyra: kyradb::KyraDbClient::new(kyradb_key),
            mpdb: mpdb::MpdbClient::new(mpdb_key, "default_user".to_string()),
            tvmaze: tvmaze::TvmazeClient::new(),
            imdbapi: imdbapi::ImdbApiClient::new(imdbapi_key),
            rate_limiter: tokio::sync::Semaphore::new(3), // Max 3 concurrent requests
        }
    }
}
