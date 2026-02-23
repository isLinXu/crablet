use crate::channels::Channel;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;
use tracing::{info, error, warn};
use reqwest::Client;

pub struct FeishuChannel {
    webhook_url: String,
    app_id: Option<String>,
    #[allow(dead_code)]
    app_secret: Option<String>,
    client: Client,
}

impl FeishuChannel {
    pub fn new() -> Self {
        let webhook = std::env::var("FEISHU_WEBHOOK").unwrap_or_default();
        Self {
            webhook_url: webhook,
            app_id: std::env::var("FEISHU_APP_ID").ok(),
            app_secret: std::env::var("FEISHU_APP_SECRET").ok(),
            client: Client::new(),
        }
    }
}

#[async_trait]
impl Channel for FeishuChannel {
    async fn send(&self, _to: &str, content: &str) -> Result<()> {
        // If webhook is set, use webhook (to is ignored or must match webhook target)
        if !self.webhook_url.is_empty() {
            let payload = json!({
                "msg_type": "text",
                "content": {
                    "text": content
                }
            });
            
            let res = self.client.post(&self.webhook_url)
                .json(&payload)
                .send()
                .await?;
                
            if !res.status().is_success() {
                let status = res.status();
                let text = res.text().await.unwrap_or_default();
                error!("Feishu webhook send failed: {} - {}", status, text);
                return Err(anyhow::anyhow!("Feishu send failed: {}", status).into());
            }
            return Ok(());
        }

        // TODO: Implement App Access Token flow for "to" (user_id/chat_id)
        warn!("Feishu App ID/Secret not implemented yet, and FEISHU_WEBHOOK not set.");
        Ok(())
    }

    async fn start(&self) -> Result<()> {
        info!("Feishu channel initialized.");
        if self.webhook_url.is_empty() && self.app_id.is_none() {
            warn!("Feishu configuration missing (FEISHU_WEBHOOK or FEISHU_APP_ID).");
        }
        // TODO: Implement event subscription server
        Ok(())
    }
    
    fn name(&self) -> &str {
        "feishu"
    }
}
