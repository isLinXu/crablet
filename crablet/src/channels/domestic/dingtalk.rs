use crate::channels::Channel;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;
use tracing::{info, error, warn};
use reqwest::Client;
use base64::{Engine as _, engine::general_purpose};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use urlencoding::encode;

pub struct DingTalkChannel {
    webhook_url: String,
    secret: Option<String>,
    client: Client,
}

impl DingTalkChannel {
    pub fn new() -> Self {
        Self {
            webhook_url: std::env::var("DINGTALK_WEBHOOK").unwrap_or_default(),
            secret: std::env::var("DINGTALK_SECRET").ok(),
            client: Client::new(),
        }
    }

    fn sign(&self, timestamp: i64) -> Option<String> {
        if let Some(secret) = &self.secret {
            let string_to_sign = format!("{}\n{}", timestamp, secret);
            let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).ok()?;
            mac.update(string_to_sign.as_bytes());
            let result = mac.finalize();
            let sign = general_purpose::STANDARD.encode(result.into_bytes());
            Some(encode(&sign).to_string())
        } else {
            None
        }
    }
}

#[async_trait]
impl Channel for DingTalkChannel {
    async fn send(&self, _to: &str, content: &str) -> Result<()> {
        if self.webhook_url.is_empty() {
             error!("DINGTALK_WEBHOOK not set");
             return Ok(());
        }

        let mut url = self.webhook_url.clone();
        if self.secret.is_some() {
            let timestamp = chrono::Utc::now().timestamp_millis();
            if let Some(sign) = self.sign(timestamp) {
                if url.contains('?') {
                    url.push_str(&format!("&timestamp={}&sign={}", timestamp, sign));
                } else {
                    url.push_str(&format!("?timestamp={}&sign={}", timestamp, sign));
                }
            }
        }
        
        let payload = json!({
            "msgtype": "text",
            "text": {
                "content": content
            }
        });
        
        let res = self.client.post(&url)
            .json(&payload)
            .send()
            .await?;
            
        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            error!("DingTalk webhook send failed: {} - {}", status, text);
            return Err(anyhow::anyhow!("DingTalk send failed: {}", status).into());
        }
        
        Ok(())
    }

    async fn start(&self) -> Result<()> {
        info!("DingTalk channel initialized.");
        if self.webhook_url.is_empty() {
            warn!("DINGTALK_WEBHOOK not set.");
        }
        Ok(())
    }
    
    fn name(&self) -> &str {
        "dingtalk"
    }
}
