// core/src/scraper/anidb.rs
use serde::{Deserialize, Serialize};
use reqwest::Client;
use anyhow::Result;
use quick_xml::de::from_str;

#[derive(Debug, Serialize, Deserialize)]
pub struct AnidbAnime {
    pub aid: i32,
    #[serde(rename = "type")]
    pub anime_type: Option<String>,
    pub episodecount: Option<i32>,
    pub startdate: Option<String>,
    pub enddate: Option<String>,
    pub titles: Option<AnidbTitles>,
    pub description: Option<String>,
    pub picture: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnidbTitles {
    #[serde(rename = "title")]
    pub title: Vec<AnidbTitle>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnidbTitle {
    #[serde(rename = "@type")]
    pub title_type: String,
    #[serde(rename = "$value")]
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "anime")]
pub struct AnidbResponse {
    pub id: i32,
    #[serde(rename = "type")]
    pub anime_type: Option<String>,
    pub episodecount: Option<i32>,
    pub startdate: Option<String>,
    pub enddate: Option<String>,
    pub titles: Option<AnidbTitles>,
    pub description: Option<String>,
    pub picture: Option<String>,
}

pub struct AnidbClient {
    client: Client,
    client_name: String,
    client_version: String,
    base_url: String,
}

impl AnidbClient {
    pub fn new(client_name: String, client_version: String) -> Self {
        Self {
            client: Client::new(),
            client_name: client_name.trim().to_string(),
            client_version: client_version.trim().to_string(),
            base_url: "http://api.anidb.net:9001/httpapi".to_string(),
        }
    }

    pub async fn get_anime_details(&self, aid: i32) -> Result<AnidbResponse> {
        let url = format!(
            "{}?client={}&clientver={}&protover=1&request=anime&aid={}",
            self.base_url, self.client_name, self.client_version, aid
        );
        
        let resp = self.client.get(url).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("AniDB error: {}", resp.status()));
        }
        
        let xml_text = resp.text().await?;
        let data: AnidbResponse = from_str(&xml_text)?;
        Ok(data)
    }

    // AniDB doesn't have a direct "search" in their HTTP API easily.
    // Usually people use the animetitles.xml file. 
    // For now, we'll provide the structural support and detail fetching.
}
