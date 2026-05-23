// core/src/notifications.rs
use serde::{Deserialize, Serialize};
use reqwest::Client;
use crate::errors::{CoreError, Result};
use crate::models::TaskUpdate;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WebhookPayload {
    pub content: String,
    pub embeds: Option<Vec<DiscordEmbed>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiscordEmbed {
    pub title: String,
    pub description: String,
    pub color: i32,
}

pub struct Notifier {
    client: Client,
}

impl Notifier {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    pub async fn send_discord_webhook(&self, url: &str, task: &TaskUpdate) -> Result<()> {
        let color = match task.status.as_str() {
            "completed" => 0x00FF00, // Green
            "error" => 0xFF0000,     // Red
            _ => 0x0000FF,          // Blue
        };

        let payload = WebhookPayload {
            content: format!("Task Update: {}", task.task_id),
            embeds: Some(vec![DiscordEmbed {
                title: format!("Status: {}", task.status),
                description: format!("Message: {}\nProgress: {}/{}", task.message, task.progress, task.total),
                color,
            }]),
        };

        let resp = self.client.post(url)
            .json(&payload)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(CoreError::NotificationError(format!("Webhook failed: {}", resp.status())));
        }

        Ok(())
    }
}
