use anyhow::{Result, Context};
use reqwest::Client;
use std::time::Duration;
use crate::plugins::Plugin;
use async_trait::async_trait;
use serde_json::Value;

pub struct HttpPlugin;

#[async_trait]
impl Plugin for HttpPlugin {
    fn name(&self) -> &str {
        "read_url"
    }

    fn description(&self) -> &str {
        "Read the content of a URL. Args: {\"url\": \"...\"}"
    }

    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    async fn execute(&self, _command: &str, args: Value) -> Result<String> {
        let url = args.get("url")
            .and_then(|v| v.as_str())
            .context("Missing 'url' argument")?;
            
        HttpTool::read_url(url).await
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

pub struct HttpTool;

impl HttpTool {
    pub async fn read_url(url: &str) -> Result<String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("Crablet/0.1.0")
            .build()?;
            
        let response = client.get(url)
            .send()
            .await
            .context("Failed to send HTTP request")?;
            
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("HTTP request failed: {}", response.status()));
        }
        
        let html = response.text().await?;
        
        // Convert HTML to plain text (Markdown-like)
        // width: 80 characters
        let text = html2text::from_read(html.as_bytes(), 80);
        
        Ok(text)
    }
}
