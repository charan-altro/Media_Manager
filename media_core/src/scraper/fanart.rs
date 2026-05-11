// core/src/scraper/fanart.rs
use serde::{Deserialize, Serialize};
use reqwest::Client;
use crate::scraper::{Result, ScraperError};

#[derive(Debug, Serialize, Deserialize)]
pub struct FanartImages {
    pub name: String,
    pub tmdb_id: String,
    pub movieposter: Option<Vec<FanartImage>>,
    pub moviebackground: Option<Vec<FanartImage>>,
    pub movielogo: Option<Vec<FanartImage>>,
    pub moviebanner: Option<Vec<FanartImage>>,
    pub moviethumb: Option<Vec<FanartImage>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FanartImage {
    pub id: String,
    pub url: String,
    pub likes: String,
}

pub struct FanartClient {
    client: Client,
    api_key: String,
    base_url: String,
}

impl FanartClient {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.trim().to_string(),
            base_url: "https://webservice.fanart.tv/v3".to_string(),
        }
    }

    pub async fn get_movie_images(&self, tmdb_id: i32) -> Result<FanartImages> {
        let url = format!("{}/movies/{}", self.base_url, tmdb_id);
        let resp = self.client.get(url)
            .query(&[("api_key", &self.api_key)])
            .send()
            .await?;
            
        if !resp.status().is_success() {
            return Err(ScraperError::Internal(format!("Fanart.tv error: {}", resp.status())));
        }
        
        Ok(resp.json::<FanartImages>().await?)
    }
}
