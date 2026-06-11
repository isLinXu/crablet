//! Legacy compatibility web host.
//!
//! The supported product control plane is the Axum gateway in `crate::gateway`.
//! This module remains available for compatibility and migration testing, and
//! its legacy API surface is disabled by default.

mod swarm_handlers;
mod swarm_timeline;

use crate::auth::handlers::{callback_handler, login_handler, me_handler, AuthState};
use crate::auth::middleware::auth_middleware;
use crate::auth::oidc::OidcProvider;
use crate::cognitive::router::CognitiveRouter;
use crate::events::AgentEvent;
use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        Form, Multipart, State,
    },
    response::{IntoResponse, Json},
    routing::{get, post, put},
    Router,
};
use base64::Engine;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tower_http::services::{ServeDir, ServeFile};
use tracing::{error, info};

// Re-export handlers used by the router
use swarm_handlers::{
    swarm_add_task_handler, swarm_agents_handler, swarm_batch_action_handler,
    swarm_cancel_handler, swarm_create_template_handler, swarm_decide_review_handler,
    swarm_instantiate_template_handler, swarm_list_reviews_handler,
    swarm_list_templates_handler, swarm_pause_handler, swarm_recover_node_handler,
    swarm_resume_handler, swarm_retry_node_handler, swarm_update_node_handler,
};
use swarm_timeline::{
    swarm_replay_handler, swarm_stats_handler, swarm_tasks_handler,
    swarm_timeline_handler,
};

const MAX_LEGACY_UPLOAD_BYTES: usize = 10 * 1024 * 1024;
const MAX_LEGACY_UPLOADS_PER_REQUEST: usize = 20;

#[derive(Deserialize, Debug, Clone)]
struct ChatInput {
    prompt: String,
    session_id: Option<String>,
    image_paths: Option<String>, // JSON array of paths
}

pub async fn run(
    router: Arc<CognitiveRouter>,
    port: u16,
    auth_config: Option<(String, String, String, String)>,
) -> anyhow::Result<()> {
    // Create uploads directory if it doesn't exist
    fs::create_dir_all("uploads").await?;
    let static_dir = if PathBuf::from("frontend/dist/index.html").exists() {
        PathBuf::from("frontend/dist")
    } else {
        PathBuf::from("../frontend/dist")
    };
    let index_file = static_dir.join("index.html");

    let app_state = router;

    // Auth State
    let auth_state = if let Some((issuer, id, secret, jwt_secret)) = auth_config {
        match OidcProvider::discover(
            &issuer,
            &id,
            &secret,
            &format!("http://localhost:{}/auth/callback", port),
        )
        .await
        {
            Ok(oidc) => Arc::new(AuthState {
                oidc: Some(oidc),
                jwt_secret,
            }),
            Err(e) => {
                error!("Failed to initialize OIDC: {}. Auth will be disabled.", e);
                Arc::new(AuthState {
                    oidc: None,
                    jwt_secret: "secret".to_string(),
                })
            }
        }
    } else {
        Arc::new(AuthState {
            oidc: None,
            jwt_secret: "secret".to_string(),
        })
    };

    // API Router
    let api_router = Router::new()
        .route(
            "/health",
            get(|| async { Json(serde_json::json!({ "status": "ok" })) }),
        )
        .route("/chat", post(chat))
        .route("/upload", post(upload_handler))
        .route(
            "/knowledge",
            get(list_knowledge_handler).delete(delete_knowledge_handler),
        )
        .route("/me", get(me_handler))
        .route("/dashboard", get(dashboard_handler))
        .route("/swarm/stats", get(swarm_stats_handler))
        .route(
            "/swarm/templates",
            get(swarm_list_templates_handler).post(swarm_create_template_handler),
        )
        .route(
            "/swarm/templates/:id/instantiate",
            post(swarm_instantiate_template_handler),
        )
        .route("/swarm/agents", get(swarm_agents_handler))
        .route("/swarm/tasks", get(swarm_tasks_handler))
        .route("/swarm/tasks/batch", post(swarm_batch_action_handler))
        .route("/swarm/tasks/:id/pause", post(swarm_pause_handler))
        .route("/swarm/tasks/:id/cancel", post(swarm_cancel_handler))
        .route("/swarm/tasks/:id/resume", post(swarm_resume_handler))
        .route("/swarm/tasks/:id/timeline", get(swarm_timeline_handler))
        .route("/swarm/tasks/:id/replay", get(swarm_replay_handler))
        .route("/swarm/reviews", get(swarm_list_reviews_handler))
        .route(
            "/swarm/reviews/:task_id/decision",
            post(swarm_decide_review_handler),
        )
        .route("/swarm/tasks/:id/nodes", post(swarm_add_task_handler))
        .route(
            "/swarm/tasks/:id/nodes/:node_id",
            put(swarm_update_node_handler),
        )
        .route(
            "/swarm/tasks/:id/nodes/:node_id/retry",
            post(swarm_retry_node_handler),
        )
        .route(
            "/swarm/tasks/:id/nodes/:node_id/recover",
            post(swarm_recover_node_handler),
        )
        .layer(axum::middleware::from_fn_with_state(
            auth_state.clone(),
            auth_middleware,
        ));

    let auth_router = Router::new()
        .route("/login", get(login_handler))
        .route("/callback", get(callback_handler))
        .with_state(auth_state.clone());

    let enable_legacy_api = std::env::var("CRABLET_ENABLE_LEGACY_WEB_API")
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);

    let legacy_root_routes = Router::new()
        .route("/ws", get(ws_handler))
        .route("/legacy_chat", post(chat))
        .layer(axum::middleware::from_fn_with_state(
            auth_state.clone(),
            auth_middleware,
        ));

    let app = if enable_legacy_api {
        info!("Legacy Web API is enabled on port {}", port);
        Router::new()
            .merge(legacy_root_routes)
            .nest("/api", api_router)
            .nest("/auth", auth_router)
            .fallback_service(
                ServeDir::new(static_dir)
                    .append_index_html_on_directories(true)
                    .not_found_service(ServeFile::new(index_file)),
            )
            .layer(axum::extract::DefaultBodyLimit::max(
                MAX_LEGACY_UPLOAD_BYTES * MAX_LEGACY_UPLOADS_PER_REQUEST,
            ))
            .with_state(app_state)
    } else {
        info!(
            "Legacy Web API is disabled; serving static UI and auth endpoints only on port {}",
            port
        );
        Router::new()
            .nest("/auth", auth_router)
            .fallback_service(
                ServeDir::new(static_dir)
                    .append_index_html_on_directories(true)
                    .not_found_service(ServeFile::new(index_file)),
            )
            .layer(axum::extract::DefaultBodyLimit::max(
                MAX_LEGACY_UPLOAD_BYTES * MAX_LEGACY_UPLOADS_PER_REQUEST,
            ))
            .with_state(app_state)
    };

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    info!("Web UI listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// ---- Upload / Chat / WebSocket / Knowledge / Dashboard Handlers ----

fn sanitize_upload_filename(file_name: &str) -> Option<String> {
    let path = PathBuf::from(file_name);
    let base_name = path
        .file_name()
        .and_then(|name| name.to_str())?
        .trim()
        .trim_matches('.');

    if base_name.is_empty() {
        return None;
    }

    let safe_name: String = base_name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
        .take(120)
        .collect();

    if safe_name.is_empty() {
        None
    } else {
        Some(safe_name)
    }
}

fn is_allowed_upload_filename(file_name: &str) -> bool {
    let Some(ext) = PathBuf::from(file_name)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
    else {
        return false;
    };

    matches!(
        ext.as_str(),
        "txt"
            | "md"
            | "pdf"
            | "png"
            | "jpg"
            | "jpeg"
            | "webp"
            | "gif"
            | "json"
            | "toml"
            | "yaml"
            | "yml"
            | "csv"
            | "rs"
            | "py"
            | "js"
            | "ts"
            | "tsx"
            | "html"
            | "css"
    )
}

fn legacy_upload_path(file_name: &str) -> Option<PathBuf> {
    let safe_name = sanitize_upload_filename(file_name)?;
    if !is_allowed_upload_filename(&safe_name) {
        return None;
    }

    Some(PathBuf::from("uploads").join(format!("{}-{}", uuid::Uuid::new_v4(), safe_name)))
}

async fn upload_handler(
    State(router): State<Arc<CognitiveRouter>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut uploaded_files = Vec::new();
    let mut rejected_files = Vec::new();
    let mut seen_files = 0usize;

    while let Ok(Some(field)) = multipart.next_field().await {
        seen_files += 1;
        if seen_files > MAX_LEGACY_UPLOADS_PER_REQUEST {
            rejected_files.push(serde_json::json!({
                "file": "request",
                "reason": "too many files"
            }));
            break;
        }

        let file_name = if let Some(name) = field.file_name() {
            name.to_string()
        } else {
            rejected_files.push(serde_json::json!({
                "file": "unknown",
                "reason": "missing filename"
            }));
            continue;
        };

        let data = if let Ok(bytes) = field.bytes().await {
            bytes
        } else {
            rejected_files.push(serde_json::json!({
                "file": file_name,
                "reason": "failed to read multipart field"
            }));
            continue;
        };

        if data.len() > MAX_LEGACY_UPLOAD_BYTES {
            rejected_files.push(serde_json::json!({
                "file": file_name,
                "reason": "file too large"
            }));
            continue;
        }

        let Some(file_path) = legacy_upload_path(&file_name) else {
            rejected_files.push(serde_json::json!({
                "file": file_name,
                "reason": "unsupported or unsafe filename"
            }));
            continue;
        };

        if let Err(e) = fs::write(&file_path, &data).await {
            error!("Failed to save uploaded file: {}", e);
            rejected_files.push(serde_json::json!({
                "file": file_name,
                "reason": "failed to save file"
            }));
            continue;
        }

        info!("File uploaded successfully: {}", file_path.display());
        // Return absolute path for the agent to use
        let abs_path_str = if let Ok(abs_path) = std::fs::canonicalize(&file_path) {
            abs_path.to_string_lossy().to_string()
        } else {
            file_path.to_string_lossy().to_string()
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
        "files": uploaded_files,
        "rejected": rejected_files
    }))
}

async fn chat(
    State(router): State<Arc<CognitiveRouter>>,
    Form(input): Form<ChatInput>,
) -> impl IntoResponse {
    // Use provided session_id or generate new one
    let session_id = input
        .session_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

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
            }))
            .into_response();
        }
    };

    // Return JSON response for API usage
    Json(serde_json::json!({
        "status": "success",
        "response": response,
        "traces": traces,
        "session_id": session_id
    }))
    .into_response()
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
        let msg = match &event.payload {
            AgentEvent::UserInput(s) => format!("USER: {}", s),
            AgentEvent::SystemLog(s) => format!("SYSTEM: {}", s),
            AgentEvent::ThoughtGenerated(s) => format!("THOUGHT: {}", s),
            AgentEvent::ToolExecutionStarted { tool, args } => {
                format!("TOOL_START: {} {}", tool, args)
            }
            AgentEvent::ToolExecutionFinished { tool, output } => {
                format!("TOOL_END: {} => {}", tool, output)
            }
            AgentEvent::CanvasUpdate {
                title,
                content,
                kind,
            } => {
                // Encode content to Base64 to avoid newline issues in simple text protocol
                let b64_content = base64::engine::general_purpose::STANDARD.encode(content);
                format!("CANVAS: {}|{}|{}", kind, title, b64_content)
            }
            AgentEvent::ResponseGenerated(s) => format!("RESPONSE: {}", s),
            AgentEvent::CognitiveLayerChanged { layer } => format!("COGNITIVE_LAYER: {}", layer),
            AgentEvent::Error(s) => format!("ERROR: {}", s),
            AgentEvent::SwarmGraphUpdate {
                graph_id, status, ..
            } => format!("SWARM_GRAPH: {}|{:?}", graph_id, status),
            AgentEvent::SwarmTaskUpdate {
                graph_id,
                task_id,
                status,
                ..
            } => format!("SWARM_TASK: {}|{}|{:?}", graph_id, task_id, status),
            AgentEvent::SwarmLog {
                graph_id,
                task_id,
                content,
                ..
            } => {
                let b64_content = base64::engine::general_purpose::STANDARD.encode(content);
                format!("SWARM_LOG: {}|{}|{}", graph_id, task_id, b64_content)
            }
            AgentEvent::SwarmActivity {
                task_id,
                graph_id,
                from,
                to,
                message_type,
                content,
                ..
            } => {
                let b64_content = base64::engine::general_purpose::STANDARD.encode(content);
                format!(
                    "SWARM_MSG: {}|{}|{}|{}|{}|{}",
                    graph_id, task_id, from, to, message_type, b64_content
                )
            }
            // Handle unreachable patterns or future events gracefully
            #[allow(unreachable_patterns)]
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
            Err(e) => {
                return Json(serde_json::json!({ "status": "error", "message": e.to_string() }))
            }
        }
    }

    // Silence unused variable warning if feature is disabled
    #[cfg(not(feature = "knowledge"))]
    let _ = router;

    Json(
        serde_json::json!({ "status": "error", "message": "Knowledge feature disabled or Vector Store not initialized" }),
    )
}

async fn delete_knowledge_handler(
    State(router): State<Arc<CognitiveRouter>>,
    axum::extract::Query(params): axum::extract::Query<DeleteKnowledgeParams>,
) -> impl IntoResponse {
    #[cfg(feature = "knowledge")]
    if let Some(vs) = &router.sys2.vector_store {
        match vs.delete_document(&params.source).await {
            Ok(_) => return Json(serde_json::json!({ "status": "success" })),
            Err(e) => {
                return Json(serde_json::json!({ "status": "error", "message": e.to_string() }))
            }
        }
    }

    // Silence unused variable warning if feature is disabled
    #[cfg(not(feature = "knowledge"))]
    {
        let _ = router;
        let _ = params;
    }

    Json(serde_json::json!({ "status": "error", "message": "Knowledge feature disabled" }))
}

async fn dashboard_handler(State(router): State<Arc<CognitiveRouter>>) -> impl IntoResponse {
    let skills = router.shared_skills.read().await;
    let skill_list = skills.list_skills();

    // In a real app, we would get this from a SwarmManager or similar
    let active_tasks = 0;
    let system_load = "Normal";

    Json(serde_json::json!({
        "status": "success",
        "skills_count": skill_list.len(),
        "active_tasks": active_tasks,
        "system_load": system_load,
        "skills": skill_list
    }))
}

#[cfg(test)]
mod tests {
    use super::{
        sanitize_upload_filename, is_allowed_upload_filename, legacy_upload_path,
    };
    use super::swarm_timeline::{
        build_swarm_replay_snapshot, swarm_timeline_entry_from_event,
        swarm_timeline_entry_matches, SwarmGraphResponse, SwarmTimelineEntry,
        SwarmTimelineQuery,
    };
    use crate::agent::swarm::{GraphStatus, TaskGraph, TaskStatus};
    use crate::events::AgentEvent;

    #[test]
    fn upload_filename_sanitizer_strips_paths_and_unsafe_chars() {
        assert_eq!(
            sanitize_upload_filename("../../my report!.md"),
            Some("myreport.md".to_string())
        );
        assert_eq!(
            sanitize_upload_filename("...hidden"),
            Some("hidden".to_string())
        );
        assert_eq!(sanitize_upload_filename("../..."), None);
    }

    #[test]
    fn upload_filename_allowlist_blocks_executables() {
        assert!(is_allowed_upload_filename("notes.md"));
        assert!(is_allowed_upload_filename("image.PNG"));
        assert!(!is_allowed_upload_filename("script.sh"));
        assert!(!is_allowed_upload_filename("archive.zip"));
        assert!(!is_allowed_upload_filename("no-extension"));
    }

    #[test]
    fn legacy_upload_path_uses_unique_uploads_prefix() {
        let first = legacy_upload_path("../photo.png").expect("png should be allowed");
        let second = legacy_upload_path("../photo.png").expect("png should be allowed");

        assert!(first.starts_with("uploads"));
        assert!(second.starts_with("uploads"));
        assert_ne!(first, second);
        assert!(first
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap()
            .ends_with("-photo.png"));
    }

    #[test]
    fn legacy_upload_path_rejects_disallowed_extensions() {
        assert!(legacy_upload_path("payload.sh").is_none());
        assert!(legacy_upload_path("../").is_none());
    }

    #[test]
    fn swarm_graph_response_exposes_observability_fields() {
        let mut graph = TaskGraph::new().with_goal("observe swarm".to_string());
        graph.status = GraphStatus::Paused;
        graph.add_task(
            "run".to_string(),
            "coder".to_string(),
            "running task".to_string(),
            vec![],
        );
        graph.add_task(
            "cancel".to_string(),
            "coder".to_string(),
            "cancelled task".to_string(),
            vec![],
        );

        graph.nodes.get_mut("run").unwrap().status = TaskStatus::Running { started_at: 1 };
        graph.nodes.get_mut("cancel").unwrap().status = TaskStatus::Cancelled {
            cancelled_at: 2,
            reason: "operator cancel".to_string(),
        };

        let response =
            SwarmGraphResponse::from_graph("graph-1".to_string(), graph, Some(10), Some(11));

        assert_eq!(response.running_tasks, 1);
        assert_eq!(response.cancelled_tasks, 1);
        assert_eq!(response.recoverable_tasks, 1);
        assert!(response.is_draining);
        assert_eq!(response.created_at, Some(10));
        assert_eq!(response.updated_at, Some(11));
    }

    #[test]
    fn swarm_timeline_maps_task_update_event() {
        let event = AgentEvent::SwarmTaskUpdate {
            graph_id: "graph-1".to_string(),
            task_id: "task-1".to_string(),
            status: "Cancelled".to_string(),
            result: Some("preview".to_string()),
            timestamp: 42,
        };

        let entry = swarm_timeline_entry_from_event(&event).unwrap();
        assert_eq!(entry.graph_id, "graph-1");
        assert_eq!(entry.task_id.as_deref(), Some("task-1"));
        assert_eq!(entry.event_type, "task_status");
        assert_eq!(entry.message_type, "TaskStatus");
        assert_eq!(entry.status.as_deref(), Some("Cancelled"));
        assert_eq!(entry.content, "preview");
        assert_eq!(entry.timestamp, 42);
    }

    #[test]
    fn swarm_timeline_applies_status_and_search_filters() {
        let entry = SwarmTimelineEntry {
            timestamp: 7,
            graph_id: "graph-1".to_string(),
            task_id: Some("task-1".to_string()),
            event_type: "activity".to_string(),
            message_type: "NodeRecoveryScheduled".to_string(),
            status: Some("Cancelled".to_string()),
            from: Some("swarm-control".to_string()),
            to: Some("task-1".to_string()),
            content: "Recovered node task-1 across 2 task(s)".to_string(),
        };

        let query = SwarmTimelineQuery {
            limit: Some(20),
            node_id: Some("task-1".to_string()),
            event_type: Some("activity".to_string()),
            message_type: Some("NodeRecoveryScheduled".to_string()),
            status: Some("Cancelled".to_string()),
            q: Some("recovered node".to_string()),
        };

        assert!(swarm_timeline_entry_matches(&entry, "graph-1", &query));
        assert!(!swarm_timeline_entry_matches(&entry, "graph-2", &query));

        let mismatch = SwarmTimelineQuery {
            q: Some("resume requested".to_string()),
            ..query
        };
        assert!(!swarm_timeline_entry_matches(&entry, "graph-1", &mismatch));
    }

    #[test]
    fn swarm_replay_reconstructs_runtime_state() {
        let mut graph = TaskGraph::new().with_goal("replay graph".to_string());
        graph.add_task(
            "task-1".to_string(),
            "coder".to_string(),
            "implement replay".to_string(),
            vec![],
        );

        let events = vec![
            AgentEvent::SwarmTaskUpdate {
                graph_id: "graph-1".to_string(),
                task_id: "task-1".to_string(),
                status: "Running".to_string(),
                result: None,
                timestamp: 10,
            },
            AgentEvent::SwarmLog {
                graph_id: "graph-1".to_string(),
                task_id: "task-1".to_string(),
                content: "starting work".to_string(),
                timestamp: 11,
            },
            AgentEvent::SwarmTaskUpdate {
                graph_id: "graph-1".to_string(),
                task_id: "task-1".to_string(),
                status: "Completed".to_string(),
                result: Some("done".to_string()),
                timestamp: 12,
            },
            AgentEvent::SwarmGraphUpdate {
                graph_id: "graph-1".to_string(),
                status: "Completed".to_string(),
                timestamp: 13,
            },
        ];

        let snapshot = build_swarm_replay_snapshot(
            "graph-1",
            &graph,
            &events,
            12,
            "event_log",
            Some("task-1".to_string()),
        );
        let node = snapshot.nodes.get("task-1").unwrap();

        assert_eq!(snapshot.goal, "replay graph");
        assert_eq!(snapshot.status, "Active");
        assert_eq!(snapshot.source, "event_log");
        assert_eq!(snapshot.focus_node_id.as_deref(), Some("task-1"));
        assert!(matches!(node.status, TaskStatus::Completed { .. }));
        assert_eq!(node.result.as_deref(), Some("done"));
        assert_eq!(node.logs, vec!["starting work".to_string()]);
        assert_eq!(snapshot.timeline_len, 3);
    }
}
