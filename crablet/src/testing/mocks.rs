use async_trait::async_trait;
use anyhow::Result;
use crate::cognitive::llm::LlmClient;
use crate::types::{Message, ContentPart};
use std::sync::Arc;
use parking_lot::Mutex;

#[derive(Clone)]
pub struct MockLlmClient {
    pub responses: Arc<Mutex<Vec<String>>>,
    pub last_prompt: Arc<Mutex<String>>,
}

impl MockLlmClient {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(Vec::new())),
            last_prompt: Arc::new(Mutex::new(String::new())),
        }
    }

    pub fn with_response(self, response: &str) -> Self {
        self.responses.lock().push(response.to_string());
        self
    }
    
    pub fn get_last_prompt(&self) -> String {
        self.last_prompt.lock().clone()
    }
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn chat_complete(&self, messages: &[Message]) -> Result<String> {
        // Record prompt
        let prompt = messages.iter()
            .map(|m| {
                if let Some(parts) = &m.content {
                    parts.iter().map(|p| match p {
                        ContentPart::Text { text } => text.clone(),
                        _ => String::new(),
                    }).collect::<Vec<_>>().join("")
                } else {
                    String::new()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        *self.last_prompt.lock() = prompt;

        // Return next response or default
        let mut responses = self.responses.lock();
        if !responses.is_empty() {
            Ok(responses.remove(0))
        } else {
            Ok("Mock response".to_string())
        }
    }

    async fn chat_complete_with_tools(&self, messages: &[Message], _tools: &[serde_json::Value]) -> Result<Message> {
        let content = self.chat_complete(messages).await?;
        Ok(Message::new("assistant", &content))
    }

    fn model_name(&self) -> &str {
        "mock-model"
    }
}
