// core/src/scraper/trakt.rs
use serde::{Deserialize, Serialize};
use reqwest::Client;
use anyhow::Result;

#[derive(Debug, Serialize, Deserialize)]
pub struct TraktIds {
    pub trakt: i32,
    pub slug: Option<String>,
    pub imdb: Option<String>,
    pub tmdb: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TraktMovie {
    pub title: String,
    pub year: i32,
    pub ids: TraktIds,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TraktSearchResult {
    pub score: f32,
    pub movie: Option<TraktMovie>,
    pub show: Option<TraktMovie>, // Re-using TraktMovie for simplicity if structure is same
}

pub struct TraktClient {
    client: Client,
    client_id: String,
    base_url: String,
}

impl TraktClient {
    pub fn new(client_id: String) -> Self {
        Self {
            client: Client::new(),
            client_id: client_id.trim().to_string(),
            base_url: "https://api.trakt.tv".to_string(),
        }
    }

    pub async fn search_movie(&self, query: &str) -> Result<Vec<TraktSearchResult>> {
        let resp = self.client.get(format!("{}/search/movie", self.base_url))
            .query(&[("query", query)])
            .header("trakt-api-version", "2")
            .header("trakt-api-key", &self.client_id)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("Trakt error: {}", resp.status()));
        }

        Ok(resp.json::<Vec<TraktSearchResult>>().await?)
    }
}
