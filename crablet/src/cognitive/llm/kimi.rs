use anyhow::{Result, Context};
use async_trait::async_trait;
use std::env;
use crate::types::Message;
use crate::cognitive::llm::{LlmClient, OpenAiClient};

pub struct KimiClient {
    client: OpenAiClient,
}

impl KimiClient {
    pub fn new(model: &str) -> Result<Self> {
        let api_key = env::var("MOONSHOT_API_KEY")
            .context("MOONSHOT_API_KEY environment variable not set")?;
            
        let base_url = "https://api.moonshot.cn/v1".to_string();
        
        Ok(Self {
            client: OpenAiClient {
                api_key,
                base_url,
                model: model.to_string(),
                client: reqwest::Client::new(),
            }
        })
    }
}

#[async_trait]
impl LlmClient for KimiClient {
    async fn chat_complete(&self, messages: &[Message]) -> Result<String> {
        self.client.chat_complete(messages).await
    }

    async fn chat_complete_with_tools(&self, messages: &[Message], tools: &[serde_json::Value]) -> Result<Message> {
        self.client.chat_complete_with_tools(messages, tools).await
    }
}
