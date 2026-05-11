// core/src/scraper/kyradb.rs
use reqwest::Client;
use crate::scraper::Result;

pub struct KyraDbClient {
    client: Client,
    api_key: String,
}

impl KyraDbClient {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }

    pub async fn get_artwork(&self, id: &str, media_type: &str) -> Result<serde_json::Value> {
        let url = format!("https://api.kyradb.com/v1/{}/{}", media_type, id);
        let resp = self.client.get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;
        
        Ok(resp.json().await?)
    }
}
