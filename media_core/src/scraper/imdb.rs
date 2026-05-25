// core/src/scraper/imdb.rs
use serde::{Deserialize, Serialize};
use reqwest::Client;
use crate::scraper::{Result, ScraperError};
use scraper::{Html, Selector};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImdbSearchResult {
    pub id: String,
    pub title: String,
    pub year: Option<i32>,
    pub image_url: Option<String>,
}

pub struct ImdbClient {
    client: Client,
}

impl ImdbClient {
    pub fn new() -> Self {
        Self {
            client: crate::scraper::build_http_client(),
        }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<ImdbSearchResult>> {
        // Using IMDb's suggestion API (publicly accessible JSON)
        let clean_query = query.to_lowercase().replace(' ', "_");
        let first_char = clean_query.chars().next().unwrap_or('a');
        let url = format!("https://v3.sg.media-imdb.com/suggestion/{}/{}.json", first_char, clean_query);

        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(ScraperError::Internal(format!("IMDb suggestion API error: {}", resp.status())));
        }

        let data: serde_json::Value = resp.json().await?;
        let mut results = Vec::new();

        if let Some(entries) = data.get("d") {
            if let Some(entries_array) = entries.as_array() {
                for entry in entries_array {
                    let _q = entry.get("q").and_then(|v| v.as_str());
                    // q: "feature" for movies, "tvSeries" for shows
                    let id = entry.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                    let title = entry.get("l").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                    let year = entry.get("y").and_then(|v| v.as_i64()).map(|v| v as i32);
                    let image = entry.get("i").and_then(|v| v.get("imageUrl")).and_then(|v| v.as_str()).map(|v| v.to_string());

                    if id.starts_with("tt") {
                        results.push(ImdbSearchResult {
                            id,
                            title,
                            year,
                            image_url: image,
                        });
                    }
                }
            }
        }

        Ok(results)
    }

    pub async fn get_details(&self, imdb_id: &str) -> Result<serde_json::Value> {
        // For full details, we have to scrape the HTML or use a wrapper
        // Scraping IMDb HTML is fragile but often the only way for free
        let url = format!("https://www.imdb.com/title/{}/", imdb_id);
        let resp = self.client.get(&url).send().await?;
        let html_content = resp.text().await?;
        
        let document = Html::parse_document(&html_content);
        
        // IMDb uses JSON-LD for many details which is much easier to parse
        let selector = Selector::parse("script[type='application/ld+json']").unwrap();
        if let Some(script) = document.select(&selector).next() {
            let json_text = script.inner_html();
            let json_data: serde_json::Value = serde_json::from_str(&json_text)?;
            return Ok(json_data);
        }

        Err(ScraperError::Internal("Could not find JSON-LD in IMDb page".to_string()))
    }
}
