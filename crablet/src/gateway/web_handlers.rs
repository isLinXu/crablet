use askama::Template;
use axum::{
    extract::{State, Json},
    response::{Html, IntoResponse},
    Form,
};
use serde::Deserialize;
use std::sync::Arc;
use crate::gateway::CrabletGateway;
// use crate::gateway::events::GatewayEvent;
// use crate::agent::swarm::SwarmMessage;

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    #[allow(dead_code)]
    response: Option<String>,
    #[allow(dead_code)]
    traces: Vec<crate::types::TraceStep>,
    skills: Vec<crate::skills::SkillManifest>,
}

// ...

pub async fn swarm_handler(
    State(_gateway): State<Arc<CrabletGateway>>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    tracing::info!("Received swarm message: {:?}", payload);
    
    // Broadcast to internal event bus so agents can pick it up
    // For now, just log, as converting arbitrary JSON to SwarmMessage enum might fail without strict schema
    // let _ = gateway.event_bus.publish(GatewayEvent::SwarmMessage(message));
    
    axum::http::StatusCode::OK
}


#[derive(Template)]
#[template(path = "message_user.html")]
struct UserMessageTemplate {
    content: String,
}

#[derive(Template)]
#[template(path = "message_assistant.html")]
struct AssistantMessageTemplate {
    content: String,
}

#[derive(Deserialize)]
pub struct ChatForm {
    message: String,
}

pub async fn index() -> impl IntoResponse {
    let template = IndexTemplate { 
        response: None, 
        traces: vec![],
        skills: vec![] 
    };
    Html(template.render().unwrap())
}

pub async fn chat_handler(
    State(_gateway): State<Arc<CrabletGateway>>,
    Form(form): Form<ChatForm>,
) -> impl IntoResponse {
    // 1. Render User Message immediately
    let user_msg_html = UserMessageTemplate { content: form.message.clone() }.render().unwrap();
    
    // 2. Process via Gateway (in background or await?)
    // For HTMX simplicity, we await here. For streaming, we'd use SSE or WS.
    // Let's assume sync response for MVP.
    
    // We need to access the router through gateway but gateway encapsulates it.
    // The gateway architecture we built is async event bus based.
    // Let's use the RPC dispatcher for now if possible, or direct access if we refactor.
    
    // Hack: For now, return a static response or mock processing
    // Real implementation needs to hook into the System2 pipeline.
    
    let assistant_msg_html = AssistantMessageTemplate { 
        content: format!("I received: {}. (Backend integration pending)", form.message) 
    }.render().unwrap();
    
    Html(format!("{}{}", user_msg_html, assistant_msg_html))
}
