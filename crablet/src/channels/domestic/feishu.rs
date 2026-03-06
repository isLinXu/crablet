use crate::channels::Channel;
use anyhow::{Result, Context};
use async_trait::async_trait;
use serde_json::json;
use tracing::{info, error, warn};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{Duration, Instant};

pub struct FeishuChannel {
    webhook_url: String,
    app_id: Option<String>,
    app_secret: Option<String>,
    client: Client,
    token_cache: Arc<RwLock<TokenCache>>,
}

struct TokenCache {
    token: Option<String>,
    expires_at: Option<Instant>,
}

use crate::config::Config;

impl FeishuChannel {
    pub fn new(config: &Config) -> Self {
        let webhook = std::env::var("FEISHU_WEBHOOK").unwrap_or_default();
        Self {
            webhook_url: webhook,
            app_id: config.feishu_app_id.clone(),
            app_secret: config.feishu_app_secret.clone(),
            client: Client::new(),
            token_cache: Arc::new(RwLock::new(TokenCache { token: None, expires_at: None })),
        }
    }

    async fn get_tenant_access_token(&self) -> Result<String> {
        if self.app_id.is_none() || self.app_secret.is_none() {
            return Err(anyhow::anyhow!("FEISHU_APP_ID or FEISHU_APP_SECRET not set"));
        }

        let cache = self.token_cache.read().await;
        if let Some(token) = &cache.token {
            if let Some(expires) = cache.expires_at {
                if Instant::now() < expires {
                    return Ok(token.clone());
                }
            }
        }
        drop(cache);

        // Fetch new token
        let mut cache = self.token_cache.write().await;
        // Double check
        if let Some(token) = &cache.token {
             if let Some(expires) = cache.expires_at {
                if Instant::now() < expires {
                    return Ok(token.clone());
                }
            }
        }

        let app_id = self.app_id.as_ref().ok_or_else(|| anyhow::anyhow!("App ID missing"))?;
        let app_secret = self.app_secret.as_ref().ok_or_else(|| anyhow::anyhow!("App Secret missing"))?;

        let url = "https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal";
        let res = self.client.post(url)
            .json(&json!({
                "app_id": app_id,
                "app_secret": app_secret
            }))
            .send()
            .await
            .context("Failed to request Feishu token")?;

        if !res.status().is_success() {
             return Err(anyhow::anyhow!("Feishu token request failed: {}", res.status()));
        }

        let body: serde_json::Value = res.json().await?;
        if body["code"].as_i64().unwrap_or(-1) != 0 {
            return Err(anyhow::anyhow!("Feishu API error: {}", body["msg"].as_str().unwrap_or("Unknown")));
        }

        let token = body["tenant_access_token"].as_str().ok_or(anyhow::anyhow!("No tenant_access_token"))?.to_string();
        let expire_seconds = body["expire"].as_u64().unwrap_or(7200);
        
        cache.token = Some(token.clone());
        cache.expires_at = Some(Instant::now() + Duration::from_secs(expire_seconds - 60)); // Buffer 60s

        Ok(token)
    }

    async fn send_message(&self, receive_id: &str, content: &str, receive_id_type: &str) -> Result<()> {
        let token = self.get_tenant_access_token().await?;
        let url = format!("https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type={}", receive_id_type);
        
        let payload = json!({
            "receive_id": receive_id,
            "msg_type": "text",
            "content": serde_json::to_string(&json!({
                "text": content
            }))?
        });

        let res = self.client.post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&payload)
            .send()
            .await?;

        if !res.status().is_success() {
             let text = res.text().await.unwrap_or_default();
             return Err(anyhow::anyhow!("Feishu send message failed: {}", text));
        }
        
        Ok(())
    }
}

#[async_trait]
impl Channel for FeishuChannel {
    async fn send(&self, to: &str, content: &str) -> Result<()> {
        // If webhook is set and 'to' is empty or matches webhook logic, use webhook
        if !self.webhook_url.is_empty() && (to.is_empty() || to == "webhook") {
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
                return Err(anyhow::anyhow!("Feishu send failed: {}", status));
            }
            return Ok(());
        }

        // Use API with receive_id (default to open_id if not specified format)
        // Format of 'to': "open_id:ou_xxx" or "chat_id:oc_xxx" or just "ou_xxx" (assume open_id)
        let (id_type, id) = if let Some((t, i)) = to.split_once(':') {
            (t, i)
        } else {
            ("open_id", to)
        };

        if self.app_id.is_some() {
            self.send_message(id, content, id_type).await
        } else {
             warn!("Feishu App ID/Secret not implemented yet, and FEISHU_WEBHOOK not set.");
             Ok(())
        }
    }

    async fn start(&self) -> Result<()> {
        info!("Feishu channel initialized.");
        if self.webhook_url.is_empty() && self.app_id.is_none() {
            warn!("Feishu configuration missing (FEISHU_WEBHOOK or FEISHU_APP_ID).");
        }
        // TODO: Implement event subscription server (WebHook Handler)
        // This requires Axum integration which is in gateway module.
        // We might need to expose a handler function here that gateway can call.
        Ok(())
    }
    
    fn name(&self) -> &str {
        "feishu"
    }
}
