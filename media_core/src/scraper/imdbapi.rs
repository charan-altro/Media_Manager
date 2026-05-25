// core/src/scraper/imdbapi.rs
use serde::{Deserialize, Serialize};
use reqwest::Client;
use crate::scraper::{Result, ScraperError};

#[derive(Debug, Serialize, Deserialize)]
pub struct ImdbApiSearchResult {
    pub id: String,
    pub title: String,
    pub year: Option<i32>,
    pub r#type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImdbApiDetails {
    pub id: String,
    pub title: String,
    pub year: Option<i32>,
    pub r#type: Option<String>,
    pub plot: Option<String>,
    pub rating: Option<f32>,
    pub poster: Option<String>,
    pub genres: Option<Vec<String>>,
}

pub struct ImdbApiClient {
    client: Client,
    api_key: String,
}

impl ImdbApiClient {
    pub fn new(api_key: String) -> Self {
        Self {
            client: crate::scraper::build_http_client(),
            api_key,
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.api_key.is_empty()
    }

    pub async fn search(&self, query: &str) -> Result<Vec<ImdbApiSearchResult>> {
        if !self.is_configured() {
            return Err(ScraperError::MissingApiKey("IMDbAPI requires an API key".to_string()));
        }

        let url = format!("https://imdb-api.projects.abhisavisa.com/Search/{}/{}", self.api_key, urlencoding::encode(query));
        let resp = self.client.get(&url).send().await?;

        if !resp.status().is_success() {
            return Err(ScraperError::Internal(format!("IMDbAPI error: {}", resp.status())));
        }

        let v: serde_json::Value = resp.json().await?;
        if let Some(results) = v.get("results").and_then(|r| r.as_array()) {
            let mut items = Vec::new();
            for item in results {
                items.push(ImdbApiSearchResult {
                    id: item.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string(),
                    title: item.get("title").and_then(|t| t.as_str()).unwrap_or_default().to_string(),
                    year: item.get("description").and_then(|d| d.as_str()).and_then(|d| d.chars().filter(|c| c.is_ascii_digit()).collect::<String>().parse().ok()),
                    r#type: item.get("resultType").and_then(|t| t.as_str()).map(|s| s.to_string()),
                });
            }
            return Ok(items);
        }

        Ok(vec![])
    }
}
