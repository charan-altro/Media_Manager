// core/src/scraper/thesportsdb.rs
use serde::{Deserialize, Serialize};
use reqwest::Client;
use anyhow::{Result, anyhow};

pub struct TheSportsDbClient {
    client: Client,
    api_key: String,
}

impl TheSportsDbClient {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key: if api_key.is_empty() { "3".to_string() } else { api_key }, // '3' is often a free test key
        }
    }

    pub async fn search_league(&self, name: &str) -> Result<serde_json::Value> {
        let url = format!("https://www.thesportsdb.com/api/v1/json/{}/search_all_leagues.php?s={}", self.api_key, name);
        let resp = self.client.get(&url).send().await?;
        Ok(resp.json().await?)
    }

    pub async fn get_events_by_season(&self, league_id: &str, season: &str) -> Result<serde_json::Value> {
        let url = format!("https://www.thesportsdb.com/api/v1/json/{}/eventsseason.php?id={}&s={}", self.api_key, league_id, season);
        let resp = self.client.get(&url).send().await?;
        Ok(resp.json().await?)
    }
}
