use anyhow::{Result, Context};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::env;
use crate::types::{Message, ContentPart, ChatChunk};
use futures::Stream;
use std::pin::Pin;

pub mod cache;
pub mod kimi;
pub mod zhipu;

pub use kimi::KimiClient;
pub use zhipu::ZhipuClient;

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn chat_complete(&self, messages: &[Message]) -> Result<String>;
    async fn chat_complete_with_tools(&self, messages: &[Message], tools: &[serde_json::Value]) -> Result<Message>;
    async fn chat_complete_with_reasoning(&self, messages: &[Message]) -> Result<(String, String)> {
        // Default implementation: just return content as response, empty reasoning
        let response = self.chat_complete(messages).await?;
        Ok((String::new(), response))
    }
    
    async fn chat_stream(&self, _messages: &[Message]) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk>> + Send>>> {
        Err(anyhow::anyhow!("Streaming not implemented for this provider"))
    }

    fn model_name(&self) -> &str;
}

pub struct OpenAiClient {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub client: reqwest::Client,
}

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiWireMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
}

#[derive(Serialize)]
struct OpenAiWireMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<crate::types::ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Deserialize, Debug)]
struct OpenAiResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize, Debug)]
struct Choice {
    message: Message,
}

use tracing::{error, info};
// use futures::StreamExt;

#[derive(Serialize)]
struct OpenAiStreamRequest {
    model: String,
    messages: Vec<OpenAiWireMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
}

#[derive(Deserialize, Debug)]
struct OpenAiStreamResponse {
    choices: Vec<StreamChoice>,
}

#[derive(Deserialize, Debug)]
struct StreamChoice {
    delta: StreamDelta,
    finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
struct StreamDelta {
    content: Option<String>,
}

impl OpenAiClient {
    pub fn new(model: &str) -> Result<Self> {
        // Prioritize DASHSCOPE_API_KEY if present (since .env configures it)
        // This avoids conflict with global OPENAI_API_KEY in shell
        let api_key = env::var("DASHSCOPE_API_KEY")
            .or_else(|_| env::var("OPENAI_API_KEY"))
            .context("OPENAI_API_KEY or DASHSCOPE_API_KEY environment variable not set")?;
            
        // Support custom base URL (e.g., for DashScope or other compatible APIs)
        let base_url = env::var("OPENAI_API_BASE")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
            
        // Check for DashScope specific case
        if base_url.contains("dashscope") && env::var("DASHSCOPE_API_KEY").is_err() && !api_key.starts_with("sk-") {
             tracing::warn!("Using OpenAI API Base URL for DashScope without DASHSCOPE_API_KEY. This might fail if the key format is incorrect.");
        }
        
        info!("Initializing OpenAiClient with Base URL: {}", base_url);
        
        // Log which key source is being used (masked)
        let key_source = if env::var("DASHSCOPE_API_KEY").is_ok() { "DASHSCOPE_API_KEY" } else { "OPENAI_API_KEY" };
        let masked_key = if api_key.len() > 8 {
             format!("{}...{}", &api_key[0..4], &api_key[api_key.len()-4..])
        } else {
             "***".to_string()
        };
        info!("Using API Key from: {} ({})", key_source, masked_key);

        Ok(Self {
            api_key,
            base_url,
            model: model.to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120)) // 2 minute timeout for summary
                .build()?,
        })
    }
}

fn to_wire_messages(messages: &[Message]) -> Vec<OpenAiWireMessage> {
    messages
        .iter()
        .map(|m| {
            let content = m.content.as_ref().and_then(|parts| {
                if parts.is_empty() {
                    return None;
                }
                let all_text = parts.iter().all(|p| matches!(p, ContentPart::Text { .. }));
                if all_text {
                    let joined = parts
                        .iter()
                        .filter_map(|p| match p {
                            ContentPart::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("");
                    if joined.is_empty() {
                        None
                    } else {
                        Some(serde_json::Value::String(joined))
                    }
                } else {
                    let arr = parts
                        .iter()
                        .map(|p| match p {
                            ContentPart::Text { text } => serde_json::json!({
                                "type": "text",
                                "text": text
                            }),
                            ContentPart::ImageUrl { image_url } => serde_json::json!({
                                "type": "image_url",
                                "image_url": { "url": image_url.url }
                            }),
                        })
                        .collect::<Vec<_>>();
                    Some(serde_json::Value::Array(arr))
                }
            });

            OpenAiWireMessage {
                role: m.role.clone(),
                content,
                tool_calls: m.tool_calls.clone(),
                tool_call_id: m.tool_call_id.clone(),
            }
        })
        .collect()
}

#[async_trait]
impl LlmClient for OpenAiClient {
    async fn chat_complete(&self, messages: &[Message]) -> Result<String> {
        let msg = self.chat_complete_with_tools(messages, &[]).await?;
        
        if let Some(content) = msg.content {
            if let Some(text_content) = content.iter().find_map(|p| match p {
                ContentPart::Text { text } => Some(text.clone()),
                _ => None,
            }) {
                return Ok(text_content);
            }
            // Fallback: try to join all text parts
            let joined = content.iter().filter_map(|p| match p {
                ContentPart::Text { text } => Some(text.as_str()),
                _ => None,
            }).collect::<Vec<_>>().join("");
            if !joined.is_empty() {
                return Ok(joined);
            }
        }
        
        Ok(String::new())
    }

    async fn chat_complete_with_tools(&self, messages: &[Message], tools: &[serde_json::Value]) -> Result<Message> {
        let request = OpenAiRequest {
            model: self.model.clone(),
            messages: to_wire_messages(messages),
            tools: if tools.is_empty() { None } else { Some(tools.to_vec()) },
        };

        if let Ok(body) = serde_json::to_string(&request) {
            info!("OpenAI Request Body: {}", body);
        }

        // Construct full URL, handling potential double slashes
        let base_url = self.base_url.trim_end_matches('/');
        let url = format!("{}/chat/completions", base_url);

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await?;
            
        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            error!("OpenAI API Error: Status={}, Body={}", status, text);
            return Err(anyhow::anyhow!("OpenAI API returned error: {}", status));
        }

        let response: OpenAiResponse = serde_json::from_str(&text)
            .context(format!("Failed to parse OpenAI response: {}", text))?;

        response.choices.into_iter().next()
            .map(|c| c.message)
            .context("No response from OpenAI")
    }

    fn model_name(&self) -> &str {
        &self.model
    }
    async fn chat_stream(&self, messages: &[Message]) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk>> + Send>>> {
        let request = OpenAiStreamRequest {
            model: self.model.clone(),
            messages: to_wire_messages(messages),
            stream: true,
            tools: None, // Streaming tools not supported yet in this simple implementation
        };

        let base_url = self.base_url.trim_end_matches('/');
        let url = format!("{}/chat/completions", base_url);

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await?;
            error!("OpenAI API Stream Error: Status={}, Body={}", status, text);
            return Err(anyhow::anyhow!("OpenAI API returned error: {}", status));
        }

        let stream = response.bytes_stream();
        
        let chunk_stream = async_stream::try_stream! {
            let mut buffer = Vec::new();
            
            for await chunk in stream {
                let chunk = chunk?;
                buffer.extend_from_slice(&chunk);
                
                // Find double newline which separates SSE events
                while let Some(pos) = buffer.windows(2).position(|w| w == b"\n\n") {
                    // Extract the event including the double newline
                    let event_bytes: Vec<u8> = buffer.drain(..pos+2).collect();
                    
                    if let Ok(event_str) = String::from_utf8(event_bytes) {
                        for line in event_str.lines() {
                            let line = line.trim();
                            if let Some(data) = line.strip_prefix("data: ") {
                                if data == "[DONE]" {
                                    break;
                                }
                                
                                if let Ok(response) = serde_json::from_str::<OpenAiStreamResponse>(data) {
                                    if let Some(choice) = response.choices.first() {
                                        if let Some(content) = &choice.delta.content {
                                            yield ChatChunk {
                                                delta: content.clone(),
                                                finish_reason: choice.finish_reason.clone(),
                                            };
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        };

        Ok(Box::pin(chunk_stream))
    }
}

pub struct MockClient;

#[async_trait]
impl LlmClient for MockClient {
    async fn chat_complete(&self, messages: &[Message]) -> Result<String> {
        let last_msg = messages.last()
            .and_then(|m| {
                m.content.as_ref().and_then(|c| c.iter().find_map(|p| match p {
                    ContentPart::Text { text } => Some(text.as_str()),
                    _ => None,
                }))
            })
            .unwrap_or("");
            
        // Mock ReAct behavior for testing
        if last_msg.contains("Calculate 15 * 7 + 3") && !last_msg.contains("Observation:") {
            return Ok("Action: use calculator {\"expression\": \"15 * 7 + 3\"}".to_string());
        }
        
        if last_msg.contains("Observation:") && last_msg.contains("108") {
             return Ok("The result is 108.".to_string());
        }

        Ok(format!("(Mock LLM) Processed: {}", last_msg))
    }

    async fn chat_complete_with_tools(&self, messages: &[Message], _tools: &[serde_json::Value]) -> Result<Message> {
        let text = self.chat_complete(messages).await?;
        Ok(Message::new("assistant", &text))
    }

    fn model_name(&self) -> &str {
        "mock-model"
    }
}

pub struct OllamaClient {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

impl OllamaClient {
    pub fn new(model: &str) -> Self {
        let base_url = env::var("OLLAMA_API_BASE")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());
        
        // Auto-detect model if "auto" is passed or if env var is not set
        let model = if model == "auto" || model.is_empty() {
            env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen2.5:14b".to_string())
        } else {
            model.to_string()
        };
        
        Self {
            base_url,
            model,
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
}

#[derive(Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OllamaToolCall>>,
}

#[derive(Serialize)]
struct OllamaToolCall {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    r#type: String,
    function: OllamaFunction,
}

#[derive(Serialize)]
struct OllamaFunction {
    name: String,
    arguments: serde_json::Value,
}

#[async_trait]
impl LlmClient for OllamaClient {
    async fn chat_complete(&self, messages: &[Message]) -> Result<String> {
        let msg = self.chat_complete_with_tools(messages, &[]).await?;
        
        if let Some(content) = msg.content {
            if let Some(text_content) = content.iter().find_map(|p| match p {
                ContentPart::Text { text } => Some(text.clone()),
                _ => None,
            }) {
                return Ok(text_content);
            }
        }
        
        Ok(String::new())
    }

    async fn chat_complete_with_tools(&self, messages: &[Message], tools: &[serde_json::Value]) -> Result<Message> {
        // Convert internal Message to OllamaMessage
        let ollama_messages: Vec<OllamaMessage> = messages.iter().map(|m| {
            let mut content_str = String::new();
            let mut images = Vec::new();
            
            if let Some(parts) = &m.content {
                for part in parts {
                    match part {
                        ContentPart::Text { text } => content_str.push_str(text),
                        ContentPart::ImageUrl { image_url } => {
                            if let Some(base64) = image_url.url.split(',').nth(1) {
                                images.push(base64.to_string());
                            }
                        }
                    }
                }
            }
            
            // Map tool_calls and parse arguments string to JSON Value for Ollama API
            let tool_calls = m.tool_calls.as_ref().map(|tcs| {
                tcs.iter().map(|tc| {
                    let args_value: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                        .unwrap_or_else(|_| serde_json::Value::String(tc.function.arguments.clone()));
                    
                    OllamaToolCall {
                        id: Some(tc.id.clone()),
                        r#type: tc.r#type.clone(),
                        function: OllamaFunction {
                            name: tc.function.name.clone(),
                            arguments: args_value,
                        },
                    }
                }).collect()
            }).filter(|v: &Vec<_>| !v.is_empty());
            
            OllamaMessage {
                role: m.role.clone(),
                content: content_str,
                images: if images.is_empty() { None } else { Some(images) },
                tool_calls,
            }
        }).collect();

        // Ollama requires "stream": false
        // For tools, Ollama expects tools as a list of tool definitions
        // If tools list is empty, don't send it or send None
        // Crucially, if we send 'tools', we must ensure the model supports it and format is correct.
        // Some users report that sending empty tools list causes 400.
        
        let tools_option = if !tools.is_empty() { Some(tools.to_vec()) } else { None };

        let request = OllamaRequest {
            model: self.model.clone(),
            messages: ollama_messages,
            stream: false,
            tools: tools_option,
        };

        let url = format!("{}/api/chat", self.base_url.trim_end_matches('/'));
        
        // Debug Log (Promoted to Info for debugging)
        if let Ok(body) = serde_json::to_string(&request) {
            info!("Ollama Request Body: {}", body);
        }

        info!("Sending request to Ollama: {} (model: {})", url, self.model);

        // Serialize to bytes to ensure Content-Length is set and avoid chunked encoding issues
        let body_bytes = serde_json::to_vec(&request)?;
        info!("Ollama Request Body Size: {} bytes", body_bytes.len());

        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body_bytes)
            .timeout(std::time::Duration::from_secs(60)) // Add 60s timeout
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            // Check if it's the specific "json: cannot unmarshal array" error which usually means we sent an array content when string was expected, or vice versa.
            // But we already flattened content to string above.
            // Wait, maybe some older Ollama versions or specific models don't like `tools` being null/missing vs empty array?
            // Or maybe `tool_calls` in message history should be handled differently?
            // The error `json: cannot unmarshal array into Go struct field ChatRequest.messages.content of type string`
            // implies that somewhere `content` is an array.
            // We construct `OllamaMessage` with `content: String`. 
            // So `content` should be a string in the JSON.
            // Let's verify `OllamaMessage` struct.
            
            error!("Ollama API Error: Status={}, Body={}", status, text);
            return Err(anyhow::anyhow!("Ollama API returned error: {}", status));
        }

        // Handle case where Ollama returns direct content for a tool call (non-standard but possible)
        // Or if it returns `message` field as expected.
        // Some users report Ollama might return just the message object or different structure?
        // Standard Ollama chat API returns: { "model": "...", "created_at": "...", "message": { "role": "assistant", "content": "..." }, "done": true, ... }
        
        #[derive(Deserialize)]
        struct OllamaResponse {
            message: Message,
        }

        // Try to parse standard response
        let response: OllamaResponse = match serde_json::from_str(&text) {
            Ok(r) => r,
            Err(e) => {
                // Fallback: If parsing failed, maybe it's because content is valid JSON but we failed to deserialize Message struct?
                // Or maybe error message?
                // The user reported: `Value looks like object, but can't find closing '}' symbol`
                // This usually means truncated JSON.
                // But we are using `stream: false`, so it should be full.
                // Wait, if the model generates a HUGE response (e.g. valid JSON tool call but very long), maybe we hit a timeout or buffer limit?
                // But `text()` reads whole body.
                
                // Another possibility: The model generated invalid JSON *inside* the content string?
                // But we are parsing the outer JSON structure here.
                
                // Let's log the raw text to see what happened.
                error!("Failed to parse Ollama response: {}. Raw Text: {}", e, text);
                return Err(anyhow::anyhow!("Failed to parse Ollama response: {}", e));
            }
        };

        Ok(response.message)
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}
