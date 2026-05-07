// core/src/scraper/tmdb.rs
use serde::{Deserialize, Serialize};
use reqwest::Client;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Semaphore;
use once_cell::sync::Lazy;

static TMDB_SEMAPHORE: Lazy<Arc<Semaphore>> = Lazy::new(|| Arc::new(Semaphore::new(40)));

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TmdbSearchResult {
    pub id: i32,
    pub title: String,
    pub release_date: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub vote_average: f32,
    pub original_language: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TmdbSearchResponse {
    pub results: Vec<TmdbSearchResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TmdbMovieDetails {
    pub id: i32,
    pub imdb_id: Option<String>,
    pub title: String,
    pub overview: Option<String>,
    pub tagline: Option<String>,
    pub runtime: Option<i32>,
    pub release_date: Option<String>,
    pub vote_average: f32,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub genres: Vec<TmdbGenre>,
    pub credits: TmdbCredits,
    pub videos: TmdbVideos,
    pub spoken_languages: Vec<TmdbLanguage>,
    pub original_language: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TmdbVideos {
    pub results: Vec<TmdbVideo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TmdbVideo {
    pub key: String,
    pub site: String,
    #[serde(rename = "type")]
    pub video_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TmdbLanguage {
    pub iso_639_1: String,
    pub name: String,
    pub english_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TmdbCredits {
    pub cast: Vec<TmdbCastMember>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TmdbCastMember {
    pub name: String,
    pub character: String,
    pub profile_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TmdbGenre {
    pub id: i32,
    pub name: String,
}

pub struct TmdbClient {
    client: Client,
    api_key: String,
    base_url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TmdbTvSearchResult {
    pub id: i32,
    pub name: String,
    pub first_air_date: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub vote_average: f32,
    pub original_language: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TmdbTvSearchResponse {
    pub results: Vec<TmdbTvSearchResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TmdbTvDetails {
    pub id: i32,
    pub name: String,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub vote_average: f32,
    pub genres: Vec<TmdbGenre>,
    pub credits: TmdbCredits,
    pub videos: TmdbVideos,
    pub spoken_languages: Vec<TmdbLanguage>,
    pub original_language: Option<String>,
}

impl TmdbClient {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.trim().to_string(),
            base_url: "https://api.themoviedb.org/3".to_string(),
        }
    }

    fn build_request(&self, method: reqwest::Method, endpoint: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, endpoint);
        let mut req = self.client.request(method, url).header("Accept", "application/json");

        if self.api_key.len() > 50 {
            // It's a v4 Bearer token
            req = req.bearer_auth(&self.api_key);
        } else {
            // It's a v3 API key
            req = req.query(&[("api_key", &self.api_key)]);
        }
        
        req
    }
}

impl crate::scraper::MediaScraper for TmdbClient {
    fn search_movie<'a>(&'a self, title: &'a str, year: Option<i32>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<TmdbSearchResult>>> + Send + 'a>> {
        Box::pin(async move {
            let _permit = TMDB_SEMAPHORE.acquire().await?;
            
            let mut req = self.build_request(reqwest::Method::GET, "/search/movie")
                .query(&[("query", title)]);

            if let Some(y) = year {
                req = req.query(&[("primary_release_year", &y.to_string())]);
            }

            let resp = req.send().await?;
            if !resp.status().is_success() {
                tracing::error!("TMDB search_movie error {}: {}", resp.status(), resp.text().await?);
                return Ok(vec![]);
            }
            
            let data = resp.json::<TmdbSearchResponse>().await?;
            Ok(data.results)
        })
    }

    fn get_movie_details<'a>(&'a self, tmdb_id: i32) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TmdbMovieDetails>> + Send + 'a>> {
        Box::pin(async move {
            let _permit = TMDB_SEMAPHORE.acquire().await?;

            let req = self.build_request(reqwest::Method::GET, &format!("/movie/{}", tmdb_id))
                .query(&[("append_to_response", "credits,release_dates,images,videos")]);

            let resp = req.send().await?;
            if !resp.status().is_success() {
                let status = resp.status();
                let err_text = resp.text().await?;
                return Err(anyhow::anyhow!("TMDB get_movie_details error {}: {}", status, err_text));
            }

            Ok(resp.json::<TmdbMovieDetails>().await?)
        })
    }

    fn search_tv_show<'a>(&'a self, title: &'a str) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<TmdbTvSearchResult>>> + Send + 'a>> {
        Box::pin(async move {
            let _permit = TMDB_SEMAPHORE.acquire().await?;
            let resp = self.build_request(reqwest::Method::GET, "/search/tv")
                .query(&[("query", title)])
                .send()
                .await?
                .json::<TmdbTvSearchResponse>()
                .await?;
            Ok(resp.results)
        })
    }

    fn get_tv_details<'a>(&'a self, tmdb_id: i32) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TmdbTvDetails>> + Send + 'a>> {
        Box::pin(async move {
            let _permit = TMDB_SEMAPHORE.acquire().await?;
            let resp = self.build_request(reqwest::Method::GET, &format!("/tv/{}", tmdb_id))
                .query(&[("append_to_response", "credits,videos")])
                .send()
                .await?
                .json::<TmdbTvDetails>()
                .await?;
            Ok(resp)
        })
    }
}
