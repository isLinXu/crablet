use axum::{
    extract::{State, Json},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::Value;
use std::sync::Arc;
use crate::events::{EventBus, AgentEvent};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

#[derive(Deserialize, Serialize, Debug)]
pub struct FeishuConfig {
    pub verification_token: String,
    pub encrypt_key: Option<String>,
}

pub async fn feishu_event_handler(
    State(event_bus): State<Arc<EventBus>>,
    Json(payload): Json<Value>,
) -> Response {
    // 1. URL Verification Challenge
    if let Some(challenge) = payload.get("challenge").and_then(|v| v.as_str()) {
        if let Some(_type) = payload.get("type").and_then(|v| v.as_str()) {
             if _type == "url_verification" {
                 return Json(serde_json::json!({ "challenge": challenge })).into_response();
             }
        }
    }

    // 2. Schema 2.0 Check
    if let Some(schema) = payload.get("schema").and_then(|v| v.as_str()) {
        if schema != "2.0" {
            warn!("Unsupported Feishu event schema: {}", schema);
            return StatusCode::OK.into_response(); // Return OK to avoid retry
        }
    }

    // 3. Handle Event
    if let Some(header) = payload.get("header") {
        let event_type = header.get("event_type").and_then(|v| v.as_str()).unwrap_or("unknown");
        let event_id = header.get("event_id").and_then(|v| v.as_str()).unwrap_or("unknown");
        
        info!("Received Feishu event: {} (ID: {})", event_type, event_id);

        if event_type == "im.message.receive_v1" {
            if let Some(event) = payload.get("event") {
                 let message = &event["message"];
                 let content_json = message["content"].as_str().unwrap_or("{}");
                 let content: Value = serde_json::from_str(content_json).unwrap_or(serde_json::json!({}));
                 let text = content["text"].as_str().unwrap_or("");
                 
                 let sender_id = event["sender"]["sender_id"]["open_id"].as_str().unwrap_or("unknown");
                 let _chat_id = message["chat_id"].as_str().unwrap_or("unknown");
                 
                 // Publish to EventBus
                 // We format the input as "feishu:open_id:xxx|text" so router knows source?
                 // Or better: AgentEvent::UserInput should carry metadata. 
                 // For now, simple string prefix.
                 let input = format!("[feishu:{}:{}] {}", "open_id", sender_id, text);
                 event_bus.publish(AgentEvent::UserInput(input));
            }
        }
    }

    StatusCode::OK.into_response()
}
