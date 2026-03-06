use crate::channels::Channel;
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use tracing::{info, warn, error};

pub struct HttpWebhookChannel {
    url: String,
    method: String,
    client: Client,
}

impl Default for HttpWebhookChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpWebhookChannel {
    pub fn new() -> Self {
        Self {
            url: std::env::var("WEBHOOK_URL").unwrap_or_default(),
            method: std::env::var("WEBHOOK_METHOD").unwrap_or_else(|_| "POST".to_string()),
            client: Client::new(),
        }
    }
}

#[async_trait]
impl Channel for HttpWebhookChannel {
    async fn send(&self, to: &str, content: &str) -> Result<()> {
        if self.url.is_empty() {
            error!("WEBHOOK_URL not set");
            return Ok(());
        }
        
        let payload = json!({
            "to": to,
            "content": content,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });
        
        let builder = match self.method.to_uppercase().as_str() {
            "GET" => self.client.get(&self.url),
            "PUT" => self.client.put(&self.url),
            _ => self.client.post(&self.url),
        };
        
        let res = builder.json(&payload).send().await?;
        
        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            error!("Webhook send failed: {} - {}", status, text);
        }
        Ok(())
    }

    async fn start(&self) -> Result<()> {
        info!("HttpWebhook channel initialized.");
        if self.url.is_empty() {
            warn!("WEBHOOK_URL not set.");
        }
        Ok(())
    }
    
    fn name(&self) -> &str {
        "webhook"
    }
}
