use anyhow::{Result, Context};
use crate::cognitive::llm::{LlmClient, OpenAiClient};
use crate::types::{Message, ContentPart};
use std::sync::Arc;
use std::env;
use std::fs;
use base64::{Engine as _, engine::general_purpose};

pub struct ImageProcessor {
    llm: Arc<Box<dyn LlmClient>>,
}

impl ImageProcessor {
    pub fn new() -> Result<Self> {
        // Use a vision-capable model
        let model = env::var("OPENAI_VISION_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
        let client = OpenAiClient::new(&model)?;
        Ok(Self {
            llm: Arc::new(Box::new(client)),
        })
    }

    pub async fn describe(&self, image_path: &str) -> Result<String> {
        // Read image and encode to base64
        let image_data = fs::read(image_path).context("Failed to read image file")?;
        let base64_image = general_purpose::STANDARD.encode(&image_data);
        let mime_type = Self::guess_mime_type(image_path);
        let data_url = format!("data:{};base64,{}", mime_type, base64_image);

        // Construct message with image
        let message = Message {
            role: "user".to_string(),
            content: Some(vec![
                ContentPart::Text { text: "Please describe this image in detail.".to_string() },
                ContentPart::ImageUrl { 
                    image_url: crate::types::ImageUrl { url: data_url } 
                },
            ]),
            tool_calls: None,
            tool_call_id: None,
        };

        self.llm.chat_complete(&[message]).await
    }
    
    fn guess_mime_type(path: &str) -> &'static str {
        if path.ends_with(".png") { "image/png" }
        else if path.ends_with(".jpg") || path.ends_with(".jpeg") { "image/jpeg" }
        else if path.ends_with(".webp") { "image/webp" }
        else { "image/jpeg" } // Default fallback
    }
}
