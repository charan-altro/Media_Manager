// core/src/scraper/omdb.rs
use serde::{Deserialize, Serialize};
use reqwest::Client;
use crate::scraper::Result;

#[derive(Debug, Serialize, Deserialize)]
pub struct OmdbRatings {
    #[serde(rename = "Ratings")]
    pub ratings: Vec<OmdbRating>,
    #[serde(rename = "imdbRating")]
    pub imdb_rating: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OmdbRating {
    #[serde(rename = "Source")]
    pub source: String,
    #[serde(rename = "Value")]
    pub value: String,
}

pub struct OmdbClient {
    client: Client,
    api_key: String,
}

impl OmdbClient {
    pub fn new(api_key: String) -> Self {
        Self {
            client: crate::scraper::build_http_client(),
            api_key,
        }
    }

    pub async fn get_ratings(&self, imdb_id: &str) -> Result<OmdbRatings> {
        let resp = self.client
            .get("http://www.omdbapi.com/")
            .query(&[("apikey", self.api_key.as_str()), ("i", imdb_id)])
            .send()
            .await?
            .json::<OmdbRatings>()
            .await?;

        Ok(resp)
    }
}
