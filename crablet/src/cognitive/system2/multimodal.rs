use crate::types::Message;
use crate::types::ContentPart;
use crate::types::ImageUrl;
use tracing::{info, warn};
use base64::Engine;

pub async fn inject_vision_content(input: &str, context: &mut Vec<Message>) {
    // Native Vision Enhancement: Scan for [System Note] with image paths and inject them as Image parts
    if input.contains("[System Note: The user has uploaded the following files") {
        // We need to inject images into the LAST user message, or append a new one if structure is weird.
        // But context is [User, Assistant, User, ...] usually.
        // Let's find the last message with role "user"
        
        if let Some(user_msg_idx) = context.iter().rposition(|m| m.role == "user") {
            let mut user_msg = context[user_msg_idx].clone();
            let mut new_parts = Vec::new();
            
            // Add existing content.
            // If it was just text, convert to Text part.
            // If it was already parts, keep them.
            if let Some(text) = user_msg.text() {
                new_parts.push(ContentPart::Text { text: text.clone() });
            } else if let Some(parts) = &user_msg.content {
                new_parts.extend(parts.clone());
            }
            
            // Parse paths from input string
            for line in input.lines() {
                if let Some(path) = line.trim().strip_prefix("- File: ") {
                    let path = path.trim();
                    // Try to read and encode
                    // Check if it's an image before injecting as ImageUrl
                    let ext = std::path::Path::new(path).extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                    let is_image = matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp");

                    if is_image {
                        if let Ok(bytes) = tokio::fs::read(path).await {
                            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                            // Detect mime type simple (extension based)
                            let mime = if path.ends_with(".png") { "image/png" } 
                                else if path.ends_with(".jpg") || path.ends_with(".jpeg") { "image/jpeg" }
                                else if path.ends_with(".webp") { "image/webp" }
                                else { "application/octet-stream" };
                                
                            let data_url = format!("data:{};base64,{}", mime, b64);
                            
                            info!("Injecting image content for: {}", path);
                            new_parts.push(ContentPart::ImageUrl { 
                                image_url: ImageUrl { url: data_url } 
                            });
                        } else {
                            warn!("Failed to read image for vision injection: {}", path);
                        }
                    } else {
                        // Non-image file (e.g. PDF), skip injection or handle differently
                        // For PDFs, we already ingested them into Knowledge Base in Router.
                        // We can optionally add a system note saying "Document X is available in knowledge base."
                        // But for now, just don't try to send it as an image to OpenAI.
                        info!("Skipping non-image file injection for: {}", path);
                        
                        // Inject a system prompt hint about RAG availability
                        if let Some(filename) = std::path::Path::new(path).file_name().and_then(|s| s.to_str()) {
                            new_parts.push(ContentPart::Text { 
                                text: format!("\n[System Hint] The file '{}' has been ingested into the knowledge base. Use retrieved context to answer questions about it.", filename)
                            });
                        }
                    }
                }
            }
            
            if !new_parts.is_empty() {
                user_msg.content = Some(new_parts);
                context[user_msg_idx] = user_msg;
                
                // Add system prompt to inform the model about the image
                context.insert(0, Message::new("system", 
                    "An image has been uploaded to the context. You can see it directly using your vision capabilities. \
                    Do NOT use tools to read or analyze the image file. Just describe what you see in the image provided in the user message."
                ));
            }
        }
    }
}
