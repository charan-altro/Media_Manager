// core/src/scraper/moviemeter.rs
use serde::{Deserialize, Serialize};
use reqwest::Client;
use anyhow::Result;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MovieMeterSearchResult {
    pub id: i32,
    pub title: String,
    pub year: Option<i32>,
}

pub struct MovieMeterClient {
    client: Client,
    api_key: String,
}

impl MovieMeterClient {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<MovieMeterSearchResult>> {
        let url = format!("https://www.moviemeter.nl/api/movie/search/{}", query);
        let resp = self.client.get(&url)
            .query(&[("api_key", &self.api_key)])
            .send()
            .await?;
            
        if !resp.status().is_success() {
            return Ok(vec![]);
        }

        let data: Vec<serde_json::Value> = resp.json().await?;
        let results = data.into_iter().map(|v| MovieMeterSearchResult {
            id: v.get("id").and_then(|id| id.as_i64()).unwrap_or(0) as i32,
            title: v.get("title").and_then(|t| t.as_str()).unwrap_or_default().to_string(),
            year: v.get("year").and_then(|y| y.as_i64()).map(|y| y as i32),
        }).collect();

        Ok(results)
    }

    pub async fn get_details(&self, id: i32) -> Result<serde_json::Value> {
        let url = format!("https://www.moviemeter.nl/api/movie/{}", id);
        let resp = self.client.get(&url)
            .query(&[("api_key", &self.api_key)])
            .send()
            .await?;
        
        Ok(resp.json().await?)
    }
}
