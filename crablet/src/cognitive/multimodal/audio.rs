use anyhow::{Context, Result};
use reqwest::multipart;
use serde::Deserialize;
use std::env;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{error, info, warn};

#[derive(Debug, Deserialize)]
struct TranscriptionResponse {
    text: String,
}

pub struct AudioTool {
    client: reqwest::Client,
    api_key: String,
    api_base: String,
}

impl AudioTool {
    pub fn new() -> Result<Self> {
        let api_key = env::var("OPENAI_API_KEY")
            .or_else(|_| env::var("DASHSCOPE_API_KEY"))
            .unwrap_or_default(); 
            
        let api_base = env::var("OPENAI_API_BASE")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());

        if api_key.is_empty() {
            warn!("AudioTool: No API Key found. Audio features will fail.");
        }

        Ok(Self {
            client: reqwest::Client::new(),
            api_key,
            api_base,
        })
    }

    /// Transcribe audio file to text using Whisper
    pub async fn transcribe(&self, file_path: &str) -> Result<String> {
        if self.api_key.is_empty() {
            return Err(anyhow::anyhow!("Missing API Key for Audio"));
        }
        
        info!("Transcribing audio file: {}", file_path);
        
        let path = Path::new(file_path);
        let file_name = path
            .file_name()
            .context("Invalid file path")?
            .to_string_lossy()
            .to_string();

        // Read file content
        let file_content = tokio::fs::read(path).await
            .context(format!("Failed to read file: {}", file_path))?;
            
        // Create multipart form
        // Note: reqwest multipart requires a 'static or owned bytes for Part::bytes if we want async ease.
        // Or we can use Part::stream.
        // For simplicity with small files, `Part::bytes` takes `Cow<'static, [u8]>` or `Vec<u8>`.
        // Wait, `Part::bytes` takes `impl Into<Cow<'static, [u8]>>`. `Vec<u8>` implements this.
        
        let part = multipart::Part::bytes(file_content)
            .file_name(file_name)
            .mime_str("audio/mpeg")?; // Default to mp3/mpeg, whisper handles most
            
        let form = multipart::Form::new()
            .part("file", part)
            .text("model", "whisper-1");

        let base_url = self.api_base.trim_end_matches('/');
        let url = format!("{}/audio/transcriptions", base_url);
        
        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            error!("Transcription failed: Status={}, Body={}", status, error_text);
            return Err(anyhow::anyhow!("Transcription failed: {}", error_text));
        }

        let result: TranscriptionResponse = response.json().await?;
        Ok(result.text)
    }

    /// Convert text to speech using TTS
    pub async fn speak(&self, text: &str, output_path: &str) -> Result<()> {
        if self.api_key.is_empty() {
            return Err(anyhow::anyhow!("Missing API Key for Audio"));
        }

        info!("Generating speech for text (length={}), output to: {}", text.len(), output_path);
        
        let base_url = self.api_base.trim_end_matches('/');
        let url = format!("{}/audio/speech", base_url);
        
        let body = serde_json::json!({
            "model": "tts-1",
            "input": text,
            "voice": "alloy"
        });

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            error!("TTS failed: Status={}, Body={}", status, error_text);
            return Err(anyhow::anyhow!("TTS failed: {}", error_text));
        }

        let bytes = response.bytes().await?;
        let mut file = File::create(output_path).await?;
        file.write_all(&bytes).await?;
        
        info!("Speech saved to {}", output_path);
        Ok(())
    }
}

