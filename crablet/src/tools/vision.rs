use crate::cognitive::llm::LlmClient;
use crate::plugins::Plugin;
use crate::types::{ContentPart, ImageUrl, Message};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;

pub struct VisionPlugin {
    tool: VisionTool,
}

impl VisionPlugin {
    pub fn new(llm: Arc<dyn LlmClient>) -> Self {
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
        let image_path = args
            .get("image_path")
            .and_then(|v| v.as_str())
            .context("Missing 'image_path' argument")?;

        let prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("Describe this image.");

        self.tool.analyze_image(image_path, prompt).await
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

pub struct VisionTool {
    llm: Arc<dyn LlmClient>,
}

impl VisionTool {
    pub fn new(llm: Arc<dyn LlmClient>) -> Self {
        Self { llm }
    }

    pub async fn analyze_image(&self, image_path: &str, prompt: &str) -> Result<String> {
        let (image_data, mime_type) = read_validated_image(image_path).await?;
        let base64_image = general_purpose::STANDARD.encode(&image_data);
        let data_url = format!("data:{};base64,{}", mime_type, base64_image);

        // Construct message
        let message = Message {
            role: "user".to_string(),
            content: Some(vec![
                ContentPart::Text {
                    text: prompt.to_string(),
                },
                ContentPart::ImageUrl {
                    image_url: ImageUrl { url: data_url },
                },
            ]),
            tool_calls: None,
            tool_call_id: None,
        };

        self.llm.chat_complete(&[message]).await
    }
}

pub(crate) async fn read_validated_image(image_path: &str) -> Result<(Vec<u8>, &'static str)> {
    let canonical_path = validate_image_path(image_path).await?;
    let image_data = tokio::fs::read(&canonical_path).await?;
    let mime_type = detect_mime_type(&image_data)?;
    Ok((image_data, mime_type))
}

fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

fn configured_allowed_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Ok(value) = std::env::var("CRABLET_VISION_ALLOWED_DIRS") {
        roots.extend(std::env::split_paths(&value));
    }

    if let Ok(current_dir) = std::env::current_dir() {
        roots.push(current_dir);
    }
    roots.push(std::env::temp_dir());
    roots
}

async fn canonical_allowed_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    for root in configured_allowed_roots() {
        if let Ok(canonical) = tokio::fs::canonicalize(root).await {
            roots.push(canonical);
        }
    }

    roots
}

async fn validate_image_path(image_path: &str) -> Result<PathBuf> {
    let canonical_path = tokio::fs::canonicalize(Path::new(image_path))
        .await
        .with_context(|| format!("Failed to resolve image path '{}'", image_path))?;

    if !env_flag_enabled("CRABLET_ALLOW_VISION_ANY_PATH") {
        let allowed_roots = canonical_allowed_roots().await;
        if !allowed_roots
            .iter()
            .any(|root| canonical_path.starts_with(root))
        {
            return Err(anyhow!(
                "Image path '{}' is outside allowed vision directories. Set CRABLET_VISION_ALLOWED_DIRS to add trusted upload roots.",
                canonical_path.display()
            ));
        }
    }

    let metadata = tokio::fs::metadata(&canonical_path)
        .await
        .with_context(|| format!("Failed to read metadata for '{}'", canonical_path.display()))?;

    if !metadata.is_file() {
        return Err(anyhow!(
            "Image path '{}' is not a regular file",
            canonical_path.display()
        ));
    }

    if metadata.len() > MAX_IMAGE_BYTES {
        return Err(anyhow!(
            "Image '{}' is too large: {} bytes exceeds {} bytes",
            canonical_path.display(),
            metadata.len(),
            MAX_IMAGE_BYTES
        ));
    }

    Ok(canonical_path)
}

fn detect_mime_type(bytes: &[u8]) -> Result<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Ok("image/png");
    }
    if bytes.len() >= 3 && bytes[0] == 0xff && bytes[1] == 0xd8 && bytes[2] == 0xff {
        return Ok("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Ok("image/gif");
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Ok("image/webp");
    }
    if bytes.starts_with(b"BM") {
        return Ok("image/bmp");
    }

    Err(anyhow!(
        "Unsupported or invalid image format. Supported formats: PNG, JPEG, GIF, WEBP, BMP."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn detects_supported_image_mime_types_from_magic_bytes() {
        assert_eq!(
            detect_mime_type(b"\x89PNG\r\n\x1a\nrest").unwrap(),
            "image/png"
        );
        assert_eq!(detect_mime_type(b"\xff\xd8\xffrest").unwrap(), "image/jpeg");
        assert_eq!(detect_mime_type(b"GIF89arest").unwrap(), "image/gif");
        assert_eq!(
            detect_mime_type(b"RIFF\x00\x00\x00\x00WEBPrest").unwrap(),
            "image/webp"
        );
        assert_eq!(detect_mime_type(b"BMrest").unwrap(), "image/bmp");
    }

    #[test]
    fn rejects_non_image_magic_bytes() {
        assert!(detect_mime_type(b"not actually an image").is_err());
    }

    #[tokio::test]
    async fn validates_small_image_inside_temp_dir() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tiny.png");
        tokio::fs::write(&path, b"\x89PNG\r\n\x1a\n").await.unwrap();

        let validated = validate_image_path(path.to_str().unwrap()).await.unwrap();
        assert!(validated.ends_with("tiny.png"));
    }

    #[tokio::test]
    async fn rejects_oversized_image_before_reading_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("huge.png");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(b"\x89PNG\r\n\x1a\n").unwrap();
        file.set_len(MAX_IMAGE_BYTES + 1).unwrap();

        let error = validate_image_path(path.to_str().unwrap())
            .await
            .expect_err("oversized image should be rejected");
        assert!(error.to_string().contains("too large"));
    }
}
