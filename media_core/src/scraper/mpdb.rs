// core/src/scraper/mpdb.rs
// MPDb.TV - French metadata provider (private, requires abo key + username)
use serde::{Deserialize, Serialize};
use reqwest::Client;
use anyhow::{Result, anyhow};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MpdbSearchResult {
    pub id: i32,
    pub title: String,
    pub year: Option<i32>,
    pub poster_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MpdbMovieDetails {
    pub id: i32,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i32>,
    pub plot: Option<String>,
    pub genres: Vec<String>,
    pub runtime: Option<i32>,
    pub rating: Option<f32>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub director: Option<String>,
    pub actors: Vec<String>,
}

pub struct MpdbClient {
    client: Client,
    abo_key: String,
    username: String,
}

impl MpdbClient {
    pub fn new(abo_key: String, username: String) -> Self {
        Self {
            client: Client::builder()
                .user_agent("MediaManager/0.2.0")
                .build()
                .unwrap(),
            abo_key,
            username,
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.abo_key.is_empty() && !self.username.is_empty()
    }

    pub async fn search(&self, query: &str) -> Result<Vec<MpdbSearchResult>> {
        if !self.is_configured() {
            return Err(anyhow!("MPDb requires abo_key and username to be configured"));
        }

        let url = format!(
            "https://www.mpdb.tv/api/search/movie?q={}&abo_key={}&username={}",
            urlencoding::encode(query),
            urlencoding::encode(&self.abo_key),
            urlencoding::encode(&self.username)
        );

        let resp = self.client.get(&url).send().await?;

        if !resp.status().is_success() {
            return Ok(vec![]);
        }

        let data: Vec<serde_json::Value> = resp.json().await.unwrap_or_default();
        let results = data.into_iter().map(|v| MpdbSearchResult {
            id: v.get("id").and_then(|i| i.as_i64()).unwrap_or(0) as i32,
            title: v.get("title").and_then(|t| t.as_str()).unwrap_or_default().to_string(),
            year: v.get("year").and_then(|y| y.as_i64()).map(|y| y as i32),
            poster_url: v.get("poster").and_then(|p| p.as_str()).map(|s| s.to_string()),
        }).collect();

        Ok(results)
    }

    pub async fn get_details(&self, id: i32) -> Result<MpdbMovieDetails> {
        if !self.is_configured() {
            return Err(anyhow!("MPDb requires abo_key and username to be configured"));
        }

        let url = format!(
            "https://www.mpdb.tv/api/movie/{}?abo_key={}&username={}",
            id,
            urlencoding::encode(&self.abo_key),
            urlencoding::encode(&self.username)
        );

        let resp = self.client.get(&url).send().await?;

        if !resp.status().is_success() {
            return Err(anyhow!("MPDb returned {}", resp.status()));
        }

        let v: serde_json::Value = resp.json().await?;

        Ok(MpdbMovieDetails {
            id,
            title: v.get("title").and_then(|t| t.as_str()).unwrap_or_default().to_string(),
            original_title: v.get("original_title").and_then(|t| t.as_str()).map(|s| s.to_string()),
            year: v.get("year").and_then(|y| y.as_i64()).map(|y| y as i32),
            plot: v.get("plot").and_then(|p| p.as_str()).map(|s| s.to_string()),
            genres: v.get("genres")
                .and_then(|g| g.as_array())
                .map(|arr| arr.iter().filter_map(|g| g.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default(),
            runtime: v.get("runtime").and_then(|r| r.as_i64()).map(|r| r as i32),
            rating: v.get("rating").and_then(|r| r.as_f64()).map(|r| r as f32),
            poster_url: v.get("poster").and_then(|p| p.as_str()).map(|s| s.to_string()),
            backdrop_url: v.get("backdrop").and_then(|b| b.as_str()).map(|s| s.to_string()),
            director: v.get("director").and_then(|d| d.as_str()).map(|s| s.to_string()),
            actors: v.get("actors")
                .and_then(|a| a.as_array())
                .map(|arr| arr.iter().filter_map(|a| a.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default(),
        })
    }
}
