use axum::{
    extract::{State, Form, Multipart, ws::{WebSocketUpgrade, WebSocket, Message as WsMessage}},
    response::{Html, IntoResponse, Json},
    routing::{get, post, delete},
    Router,
};
use askama::Template;
use serde::Deserialize;
use std::sync::Arc;
use crate::cognitive::router::CognitiveRouter;
use crate::types::TraceStep;
use crate::events::AgentEvent;
use tracing::{info, error};
use std::path::PathBuf;
use tokio::fs;
use base64::Engine;

use crate::skills::SkillManifest;

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    #[allow(dead_code)]
    response: Option<String>,
    #[allow(dead_code)]
    traces: Vec<TraceStep>,
    skills: Vec<SkillManifest>,
}

#[derive(Deserialize)]
struct ChatInput {
    prompt: String,
    session_id: Option<String>,
    image_paths: Option<String>, // JSON array of paths
}

pub async fn run(router: Arc<CognitiveRouter>, port: u16) -> anyhow::Result<()> {
    // Create uploads directory if it doesn't exist
    fs::create_dir_all("uploads").await?;

    let app_state = router;

    let app = Router::new()
        .route("/", get(index))
        .route("/chat", post(chat))
        .route("/api/upload", post(upload_handler))
        .route("/api/knowledge", get(list_knowledge_handler).delete(delete_knowledge_handler))
        .route("/ws", get(ws_handler))
        .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024 * 50)) // 50MB limit
        .with_state(app_state);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    info!("Web UI listening on http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn index(State(router): State<Arc<CognitiveRouter>>) -> impl IntoResponse {
    let skills = router.get_all_skills().await;
    let template = IndexTemplate { response: None, traces: vec![], skills };
    Html(template.render().unwrap())
}

async fn upload_handler(
    State(router): State<Arc<CognitiveRouter>>,
    mut multipart: Multipart
) -> impl IntoResponse {
    let mut uploaded_files = Vec::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let file_name = if let Some(name) = field.file_name() {
            name.to_string()
        } else {
            continue;
        };

        let data = if let Ok(bytes) = field.bytes().await {
            bytes
        } else {
            continue;
        };

        // Sanitize filename to prevent directory traversal
        let safe_name = PathBuf::from(&file_name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown_file")
            .to_string();

        let file_path = format!("uploads/{}", safe_name);
        
        if let Err(e) = fs::write(&file_path, &data).await {
            error!("Failed to save uploaded file: {}", e);
            continue;
        }

        info!("File uploaded successfully: {}", file_path);
        // Return absolute path for the agent to use
        let abs_path_str = if let Ok(abs_path) = std::fs::canonicalize(&file_path) {
             abs_path.to_string_lossy().to_string()
        } else {
             file_path.clone()
        };
        
        uploaded_files.push(abs_path_str.clone());
        
        // Trigger Ingestion (Async)
        let router_clone = router.clone();
        let path_clone = abs_path_str.clone();
        tokio::spawn(async move {
            if let Err(e) = router_clone.ingest_file(&path_clone).await {
                error!("Failed to ingest file {}: {}", path_clone, e);
            }
        });
    }

    Json(serde_json::json!({
        "status": "success",
        "files": uploaded_files
    }))
}

async fn chat(
    State(router): State<Arc<CognitiveRouter>>,
    Form(input): Form<ChatInput>,
) -> impl IntoResponse {
    // Use provided session_id or generate new one
    let session_id = input.session_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    
    let mut final_prompt = input.prompt.clone();
    
    if let Some(paths_json) = input.image_paths {
        if let Ok(paths) = serde_json::from_str::<Vec<String>>(&paths_json) {
            if !paths.is_empty() {
                final_prompt.push_str("\n\n[System Note: The user has uploaded the following files. Use them if requested.]\n");
                for path in paths {
                    final_prompt.push_str(&format!("- File: {}\n", path));
                }
            }
        }
    }
    
    let (response, traces) = match router.process(&final_prompt, &session_id).await {
        Ok((res, steps)) => (Some(res), steps),
        Err(e) => {
            error!("Chat processing failed: {}", e);
            return Json(serde_json::json!({
                "status": "error",
                "message": e.to_string(),
                "session_id": session_id
            })).into_response();
        }
    };

    // Return JSON response for API usage
    Json(serde_json::json!({
        "status": "success",
        "response": response,
        "traces": traces,
        "session_id": session_id
    })).into_response()
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(router): State<Arc<CognitiveRouter>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, router))
}

async fn handle_socket(mut socket: WebSocket, router: Arc<CognitiveRouter>) {
    let mut rx = router.event_bus.subscribe();
    
    while let Ok(event) = rx.recv().await {
        let msg = match event {
            AgentEvent::UserInput(s) => format!("USER: {}", s),
            AgentEvent::SystemLog(s) => format!("SYSTEM: {}", s),
            AgentEvent::ThoughtGenerated(s) => format!("THOUGHT: {}", s),
            AgentEvent::ToolExecutionStarted { tool, args } => format!("TOOL_START: {} {}", tool, args),
            AgentEvent::ToolExecutionFinished { tool, output } => format!("TOOL_END: {} => {}", tool, output),
            AgentEvent::CanvasUpdate { title, content, kind } => {
                // Encode content to Base64 to avoid newline issues in simple text protocol
                let b64_content = base64::engine::general_purpose::STANDARD.encode(content);
                format!("CANVAS: {}|{}|{}", kind, title, b64_content)
            },
            AgentEvent::ResponseGenerated(s) => format!("RESPONSE: {}", s),
            AgentEvent::Error(s) => format!("ERROR: {}", s),
            _ => continue,
        };
        
        if let Err(_e) = socket.send(WsMessage::Text(msg)).await {
            // Client disconnected
            break;
        }
    }
}

#[derive(Deserialize)]
struct DeleteKnowledgeParams {
    #[allow(dead_code)]
    source: String,
}

async fn list_knowledge_handler(State(router): State<Arc<CognitiveRouter>>) -> impl IntoResponse {
    #[cfg(feature = "knowledge")]
    if let Some(vs) = &router.sys2.vector_store {
        match vs.list_documents().await {
            Ok(docs) => return Json(serde_json::json!({ "status": "success", "documents": docs })),
            Err(e) => return Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
        }
    }
    
    Json(serde_json::json!({ "status": "error", "message": "Knowledge feature disabled or Vector Store not initialized" }))
}

async fn delete_knowledge_handler(
    State(router): State<Arc<CognitiveRouter>>,
    axum::extract::Query(params): axum::extract::Query<DeleteKnowledgeParams>
) -> impl IntoResponse {
    #[cfg(feature = "knowledge")]
    if let Some(vs) = &router.sys2.vector_store {
        match vs.delete_document(&params.source).await {
            Ok(_) => return Json(serde_json::json!({ "status": "success" })),
            Err(e) => return Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
        }
    }
    
    Json(serde_json::json!({ "status": "error", "message": "Knowledge feature disabled" }))
}
