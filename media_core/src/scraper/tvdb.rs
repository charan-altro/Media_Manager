// core/src/scraper/tvdb.rs
use serde::{Deserialize, Serialize};
use reqwest::Client;
use crate::scraper::{Result, ScraperError};
use tokio::sync::RwLock;
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub struct TvdbResponse<T> {
    pub status: String,
    pub data: T,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TvdbSeries {
    pub id: i32,
    pub name: String,
    pub overview: Option<String>,
    pub status: Option<TvdbStatus>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TvdbStatus {
    pub name: Option<String>,
}

pub struct TvdbClient {
    client: Client,
    api_key: String,
    token: Arc<RwLock<Option<String>>>,
    base_url: String,
}

impl TvdbClient {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.trim().to_string(),
            token: Arc::new(RwLock::new(None)),
            base_url: "https://api4.thetvdb.com/v4".to_string(),
        }
    }

    async fn get_token(&self) -> Result<String> {
        {
            let token_guard = self.token.read().await;
            if let Some(ref t) = *token_guard {
                return Ok(t.clone());
            }
        }

        let mut lock = self.token.write().await;
        // Check again in case another thread updated it
        if let Some(ref t) = *lock {
            return Ok(t.clone());
        }

        let resp = self.client.post(format!("{}/login", self.base_url))
            .json(&serde_json::json!({ "apikey": self.api_key }))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(ScraperError::Internal(format!("TVDB login failed: {}", resp.status())));
        }

        let data: TvdbResponse<serde_json::Value> = resp.json().await?;
        let token = data.data["token"].as_str()
            .ok_or_else(|| ScraperError::Internal("No token in TVDB response".to_string()))?
            .to_string();

        *lock = Some(token.clone());
        Ok(token)
    }

    pub async fn search_series(&self, query: &str) -> Result<Vec<TvdbSeries>> {
        let token = self.get_token().await?;
        let resp = self.client.get(format!("{}/search", self.base_url))
            .query(&[("query", query), ("type", "series")])
            .bearer_auth(token)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(ScraperError::Internal(format!("TVDB search failed: {}", resp.status())));
        }

        let data: TvdbResponse<Vec<TvdbSeries>> = resp.json().await?;
        Ok(data.data)
    }
}
