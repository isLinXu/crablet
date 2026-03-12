use crate::channels::Channel;
use anyhow::{Result, Context};
use async_trait::async_trait;
use serde_json::json;
use tracing::{info, error, warn};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{Duration, Instant};
use crate::config::Config;

pub struct WeComChannel {
    webhook_url: String,
    corp_id: Option<String>,
    corp_secret: Option<String>,
    agent_id: Option<String>,
    client: Client,
    token_cache: Arc<RwLock<TokenCache>>,
}

struct TokenCache {
    token: Option<String>,
    expires_at: Option<Instant>,
}

impl WeComChannel {
    pub fn new(config: &Config) -> Self {
        let webhook = std::env::var("WECOM_WEBHOOK").unwrap_or_default();
        Self {
            webhook_url: webhook,
            corp_id: config.wecom_corp_id.clone(),
            corp_secret: config.wecom_corp_secret.clone(),
            agent_id: config.wecom_agent_id.clone(),
            client: Client::new(),
            token_cache: Arc::new(RwLock::new(TokenCache { token: None, expires_at: None })),
        }
    }

    async fn get_access_token(&self) -> Result<String> {
        if self.corp_id.is_none() || self.corp_secret.is_none() {
            return Err(anyhow::anyhow!("WECOM_CORP_ID or WECOM_CORP_SECRET not set"));
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

        let corp_id = self.corp_id.as_ref().ok_or_else(|| anyhow::anyhow!("Corp ID missing"))?;
        let corp_secret = self.corp_secret.as_ref().ok_or_else(|| anyhow::anyhow!("Corp Secret missing"))?;

        let url = format!("https://qyapi.weixin.qq.com/cgi-bin/gettoken?corpid={}&corpsecret={}", corp_id, corp_secret);
        let res = self.client.get(&url)
            .send()
            .await
            .context("Failed to request WeCom token")?;

        if !res.status().is_success() {
             return Err(anyhow::anyhow!("WeCom token request failed: {}", res.status()));
        }

        let body: serde_json::Value = res.json().await?;
        if body["errcode"].as_i64().unwrap_or(-1) != 0 {
            return Err(anyhow::anyhow!("WeCom API error: {}", body["errmsg"].as_str().unwrap_or("Unknown")));
        }

        let token = body["access_token"].as_str().ok_or(anyhow::anyhow!("No access_token"))?.to_string();
        let expire_seconds = body["expires_in"].as_u64().unwrap_or(7200);
        
        cache.token = Some(token.clone());
        cache.expires_at = Some(Instant::now() + Duration::from_secs(expire_seconds - 60)); // Buffer 60s

        Ok(token)
    }

    async fn send_app_message(&self, to_user: &str, content: &str) -> Result<()> {
        let token = self.get_access_token().await?;
        let agent_id = self.agent_id.as_ref().ok_or_else(|| anyhow::anyhow!("WECOM_AGENT_ID not set"))?;
        let url = format!("https://qyapi.weixin.qq.com/cgi-bin/message/send?access_token={}", token);
        
        // Support touser, toparty, totag
        // Format: "user:id" or "party:id" or just "id" (default to user)
        let (type_key, id) = if let Some((t, i)) = to_user.split_once(':') {
            match t {
                "party" => ("toparty", i),
                "tag" => ("totag", i),
                _ => ("touser", i),
            }
        } else {
            ("touser", to_user)
        };

        let mut payload = json!({
            "msgtype": "text",
            "agentid": agent_id.parse::<i64>().unwrap_or(0),
            "text": {
                "content": content
            },
            "safe": 0,
            "enable_id_trans": 0,
            "enable_duplicate_check": 0,
            "duplicate_check_interval": 1800
        });
        
        payload[type_key] = json!(id);

        let res = self.client.post(&url)
            .json(&payload)
            .send()
            .await?;

        if !res.status().is_success() {
             let text = res.text().await.unwrap_or_default();
             return Err(anyhow::anyhow!("WeCom send message failed: {}", text));
        }
        
        let body: serde_json::Value = res.json().await?;
        if body["errcode"].as_i64().unwrap_or(-1) != 0 {
             return Err(anyhow::anyhow!("WeCom API send error: {}", body["errmsg"].as_str().unwrap_or("Unknown")));
        }
        
        Ok(())
    }
}

#[async_trait]
impl Channel for WeComChannel {
    async fn send(&self, to: &str, content: &str) -> Result<()> {
        // If webhook is set and 'to' is empty or "webhook", use webhook
        if !self.webhook_url.is_empty() && (to.is_empty() || to == "webhook") {
            let payload = json!({
                "msgtype": "text",
                "text": {
                    "content": content
                }
            });
            
            let res = self.client.post(&self.webhook_url)
                .json(&payload)
                .send()
                .await?;
                
            if !res.status().is_success() {
                let status = res.status();
                let text = res.text().await.unwrap_or_default();
                error!("WeCom webhook send failed: {} - {}", status, text);
                return Err(anyhow::anyhow!("WeCom webhook failed: {}", status));
            }
            
            let body: serde_json::Value = res.json().await?;
            if body["errcode"].as_i64().unwrap_or(-1) != 0 {
                return Err(anyhow::anyhow!("WeCom Webhook API error: {}", body["errmsg"].as_str().unwrap_or("Unknown")));
            }
            return Ok(());
        }

        // Use App API
        if self.corp_id.is_some() {
            self.send_app_message(to, content).await
        } else {
             warn!("WeCom Corp ID/Secret not configured, and WECOM_WEBHOOK not set.");
             Ok(())
        }
    }

    async fn start(&self) -> Result<()> {
        info!("WeCom channel initialized.");
        if self.webhook_url.is_empty() && self.corp_id.is_none() {
            warn!("WeCom configuration missing (WECOM_WEBHOOK or WECOM_CORP_ID).");
        }
        Ok(())
    }
    
    fn name(&self) -> &str {
        "wecom"
    }
}
