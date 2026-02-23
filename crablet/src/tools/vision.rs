use anyhow::{Result, Context};
use crate::cognitive::llm::LlmClient;
use crate::types::{Message, ContentPart, ImageUrl};
use std::sync::Arc;
use base64::{Engine as _, engine::general_purpose};
use crate::plugins::Plugin;
use async_trait::async_trait;
use serde_json::Value;

pub struct VisionPlugin {
    tool: VisionTool,
}

impl VisionPlugin {
    pub fn new(llm: Arc<Box<dyn LlmClient>>) -> Self {
        Self {
            tool: VisionTool::new(llm),
        }
    }
}

#[async_trait]
impl Plugin for VisionPlugin {
    fn name(&self) -> &str {
        "see"
    }

    fn description(&self) -> &str {
        "Analyze an image. Args: {\"image_path\": \"...\", \"prompt\": \"...\"}"
    }

    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    async fn execute(&self, _command: &str, args: Value) -> Result<String> {
        let image_path = args.get("image_path")
            .and_then(|v| v.as_str())
            .context("Missing 'image_path' argument")?;
            
        let prompt = args.get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("Describe this image.");
            
        self.tool.analyze_image(image_path, prompt).await
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

pub struct VisionTool {
    llm: Arc<Box<dyn LlmClient>>,
}

impl VisionTool {
    pub fn new(llm: Arc<Box<dyn LlmClient>>) -> Self {
        Self { llm }
    }

    pub async fn analyze_image(&self, image_path: &str, prompt: &str) -> Result<String> {
        // Read and encode image
        let image_data = std::fs::read(image_path)?;
        let base64_image = general_purpose::STANDARD.encode(&image_data);
        let mime_type = if image_path.ends_with(".png") { "image/png" } else { "image/jpeg" };
        let data_url = format!("data:{};base64,{}", mime_type, base64_image);

        // Construct message
        let message = Message {
            role: "user".to_string(),
            content: Some(vec![
                ContentPart::Text { text: prompt.to_string() },
                ContentPart::ImageUrl { 
                    image_url: ImageUrl { url: data_url } 
                }
            ]),
            tool_calls: None,
            tool_call_id: None,
        };

        self.llm.chat_complete(&[message]).await
    }
}
