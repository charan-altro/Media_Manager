// core/src/scraper/tvmaze.rs
use serde::{Deserialize, Serialize};
use reqwest::Client;
use anyhow::Result;

#[derive(Debug, Serialize, Deserialize)]
pub struct TvmazeShow {
    pub id: i32,
    pub name: String,
    pub summary: Option<String>,
    pub premiered: Option<String>,
    pub externals: TvmazeExternals,
    pub image: Option<TvmazeImage>,
    pub runtime: Option<i32>,
    pub rating: Option<TvmazeRating>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TvmazeExternals {
    pub tvrage: Option<i32>,
    pub thetvdb: Option<i32>,
    pub imdb: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TvmazeImage {
    pub medium: Option<String>,
    pub original: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TvmazeRating {
    pub average: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TvmazeSearchResult {
    pub score: f32,
    pub show: TvmazeShow,
}

pub struct TvmazeClient {
    client: Client,
}

impl TvmazeClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    pub async fn search_show(&self, query: &str) -> Result<Vec<TvmazeSearchResult>> {
        let url = format!("https://api.tvmaze.com/search/shows?q={}", urlencoding::encode(query));
        let resp = self.client.get(&url).send().await?;

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("TVMaze error: {}", resp.status()));
        }

        Ok(resp.json::<Vec<TvmazeSearchResult>>().await?)
    }

    pub async fn get_show_details(&self, id: i32) -> Result<TvmazeShow> {
        let url = format!("https://api.tvmaze.com/shows/{}", id);
        let resp = self.client.get(&url).send().await?;

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("TVMaze error: {}", resp.status()));
        }

        Ok(resp.json::<TvmazeShow>().await?)
    }
}
