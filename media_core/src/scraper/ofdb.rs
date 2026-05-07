// core/src/scraper/ofdb.rs
use reqwest::Client;
use anyhow::{Result, anyhow};
use scraper::{Html, Selector};

pub struct OfdbClient {
    client: Client,
}

impl OfdbClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("Mozilla/5.0")
                .build()
                .unwrap(),
        }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<(String, String)>> {
        let url = format!("https://www.ofdb.de/view.php?page=suchergebnis&SText={}", query);
        let resp = self.client.get(&url).send().await?;
        let html = resp.text().await?;
        let document = Html::parse_document(&html);
        
        let mut results = Vec::new();
        // OFDb search results are usually in a table or list
        // This is a simplified scraper
        let selector = Selector::parse("a[href*='film/']").unwrap();
        for element in document.select(&selector) {
            let title = element.text().collect::<Vec<_>>().join(" ");
            let href = element.value().attr("href").unwrap_or_default().to_string();
            if !title.is_empty() && href.contains("film/") {
                results.push((title, href));
            }
        }
        Ok(results)
    }
}
