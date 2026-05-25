// core/src/scraper/trakt.rs
use serde::{Deserialize, Serialize};
use reqwest::Client;
use crate::scraper::{Result, ScraperError};

#[derive(Debug, Serialize, Deserialize)]
pub struct TraktIds {
    pub trakt: i32,
    pub slug: Option<String>,
    pub imdb: Option<String>,
    pub tmdb: Option<i32>,
    pub tvdb: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TraktMovie {
    pub title: String,
    pub year: i32,
    pub ids: TraktIds,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TraktSearchResult {
    pub score: f32,
    pub movie: Option<TraktMovie>,
    pub show: Option<TraktMovie>,
}

/// Represents a Trakt collection/watchlist item
#[derive(Debug, Serialize, Deserialize)]
pub struct TraktCollectionItem {
    pub collected_at: Option<String>,
    pub listed_at: Option<String>,
    pub movie: Option<TraktMovie>,
    pub show: Option<TraktMovie>,
}

/// Represents a watched history item
#[derive(Debug, Serialize, Deserialize)]
pub struct TraktHistoryItem {
    pub id: i64,
    pub watched_at: String,
    pub action: String,
    pub movie: Option<TraktMovie>,
    pub show: Option<TraktMovie>,
}

pub struct TraktClient {
    client: Client,
    client_id: String,
    base_url: String,
}

impl TraktClient {
    pub fn new(client_id: String) -> Self {
        Self {
            client: crate::scraper::build_http_client(),
            client_id: client_id.trim().to_string(),
            base_url: "https://api.trakt.tv".to_string(),
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.client_id.is_empty()
    }

    fn headers(&self) -> Vec<(&'static str, String)> {
        vec![
            ("trakt-api-version", "2".to_string()),
            ("trakt-api-key", self.client_id.clone()),
            ("Content-Type", "application/json".to_string()),
        ]
    }

    // ── Search ──────────────────────────────────────────────────

    pub async fn search_movie(&self, query: &str) -> Result<Vec<TraktSearchResult>> {
        let mut req = self.client.get(format!("{}/search/movie", self.base_url))
            .query(&[("query", query)]);
        for (k, v) in self.headers() { req = req.header(k, v); }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(ScraperError::Internal(format!("Trakt error: {}", resp.status())));
        }
        Ok(resp.json::<Vec<TraktSearchResult>>().await?)
    }

    pub async fn search_show(&self, query: &str) -> Result<Vec<TraktSearchResult>> {
        let mut req = self.client.get(format!("{}/search/show", self.base_url))
            .query(&[("query", query)]);
        for (k, v) in self.headers() { req = req.header(k, v); }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(ScraperError::Internal(format!("Trakt error: {}", resp.status())));
        }
        Ok(resp.json::<Vec<TraktSearchResult>>().await?)
    }

    /// Multi-type search (movies + shows + episodes)
    pub async fn search_all(&self, query: &str) -> Result<Vec<TraktSearchResult>> {
        let mut req = self.client.get(format!("{}/search/movie,show", self.base_url))
            .query(&[("query", query)]);
        for (k, v) in self.headers() { req = req.header(k, v); }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(ScraperError::Internal(format!("Trakt error: {}", resp.status())));
        }
        Ok(resp.json::<Vec<TraktSearchResult>>().await?)
    }

    // ── Library Sync (requires OAuth access_token) ──────────────

    /// Get the user's movie collection (requires OAuth)
    pub async fn get_collection_movies(&self, access_token: &str) -> Result<Vec<TraktCollectionItem>> {
        let mut req = self.client.get(format!("{}/sync/collection/movies", self.base_url))
            .bearer_auth(access_token);
        for (k, v) in self.headers() { req = req.header(k, v); }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(ScraperError::Internal(format!("Trakt operation error: {}", resp.status())));
        }
        Ok(resp.json().await?)
    }

    /// Get the user's show collection (requires OAuth)
    pub async fn get_collection_shows(&self, access_token: &str) -> Result<Vec<TraktCollectionItem>> {
        let mut req = self.client.get(format!("{}/sync/collection/shows", self.base_url))
            .bearer_auth(access_token);
        for (k, v) in self.headers() { req = req.header(k, v); }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(ScraperError::Internal(format!("Trakt operation error: {}", resp.status())));
        }
        Ok(resp.json().await?)
    }

    /// Get the user's movie watchlist (requires OAuth)
    pub async fn get_watchlist_movies(&self, access_token: &str) -> Result<Vec<TraktCollectionItem>> {
        let mut req = self.client.get(format!("{}/sync/watchlist/movies", self.base_url))
            .bearer_auth(access_token);
        for (k, v) in self.headers() { req = req.header(k, v); }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(ScraperError::Internal(format!("Trakt API error: {}", resp.status())));
        }
        Ok(resp.json().await?)
    }

    /// Get the user's show watchlist (requires OAuth)
    pub async fn get_watchlist_shows(&self, access_token: &str) -> Result<Vec<TraktCollectionItem>> {
        let mut req = self.client.get(format!("{}/sync/watchlist/shows", self.base_url))
            .bearer_auth(access_token);
        for (k, v) in self.headers() { req = req.header(k, v); }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(ScraperError::Internal(format!("Trakt API error: {}", resp.status())));
        }
        Ok(resp.json().await?)
    }

    /// Get the user's watch history (requires OAuth)
    pub async fn get_history(&self, access_token: &str, media_type: &str, limit: u32) -> Result<Vec<TraktHistoryItem>> {
        let mut req = self.client.get(format!("{}/sync/history/{}", self.base_url, media_type))
            .query(&[("limit", limit.to_string())])
            .bearer_auth(access_token);
        for (k, v) in self.headers() { req = req.header(k, v); }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(ScraperError::Internal(format!("Trakt history error: {}", resp.status())));
        }
        Ok(resp.json().await?)
    }

    /// Add movies to the user's collection (requires OAuth)
    pub async fn add_to_collection(&self, access_token: &str, movies: Vec<serde_json::Value>) -> Result<serde_json::Value> {
        let body = serde_json::json!({ "movies": movies });
        let mut req = self.client.post(format!("{}/sync/collection", self.base_url))
            .bearer_auth(access_token)
            .json(&body);
        for (k, v) in self.headers() { req = req.header(k, v); }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(ScraperError::Internal(format!("Trakt collection error: {}", resp.status())));
        }
        Ok(resp.json().await?)
    }

    // ── OAuth Token Exchange ────────────────────────────────────

    /// Exchange an authorization code for an access token (OAuth2 flow)
    pub async fn exchange_code(&self, code: &str, client_secret: &str, redirect_uri: &str) -> Result<serde_json::Value> {
        let body = serde_json::json!({
            "code": code,
            "client_id": self.client_id,
            "client_secret": client_secret,
            "redirect_uri": redirect_uri,
            "grant_type": "authorization_code"
        });

        let resp = self.client.post(format!("{}/oauth/token", self.base_url))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(ScraperError::Internal(format!("Trakt OAuth error: {}", resp.status())));
        }
        Ok(resp.json().await?)
    }

    /// Refresh an expired access token
    pub async fn refresh_token(&self, refresh_token: &str, client_secret: &str, redirect_uri: &str) -> Result<serde_json::Value> {
        let body = serde_json::json!({
            "refresh_token": refresh_token,
            "client_id": self.client_id,
            "client_secret": client_secret,
            "redirect_uri": redirect_uri,
            "grant_type": "refresh_token"
        });

        let resp = self.client.post(format!("{}/oauth/token", self.base_url))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(ScraperError::Internal(format!("Trakt token refresh error: {}", resp.status())));
        }
        Ok(resp.json().await?)
    }

    /// Build the OAuth authorization URL that users visit in their browser
    pub fn get_auth_url(&self, redirect_uri: &str) -> String {
        format!(
            "https://trakt.tv/oauth/authorize?response_type=code&client_id={}&redirect_uri={}",
            self.client_id,
            urlencoding::encode(redirect_uri)
        )
    }
}
