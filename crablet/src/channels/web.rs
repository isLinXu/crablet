use crate::agent::hitl::HumanDecision;
use crate::agent::swarm::{GraphStatus, TaskGraph, TaskNode, TaskStatus};
use crate::auth::handlers::{callback_handler, login_handler, me_handler, AuthState};
use crate::auth::middleware::auth_middleware;
use crate::auth::oidc::OidcProvider;
use crate::cognitive::router::CognitiveRouter;
use crate::events::AgentEvent;
use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        Form, Multipart, Path, State,
    },
    response::{IntoResponse, Json},
    routing::{get, post, put},
    Router,
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tower_http::services::{ServeDir, ServeFile};
use tracing::{error, info};

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
        .with_state(auth_state);

    let enable_legacy_api = std::env::var("CRABLET_ENABLE_LEGACY_WEB_API")
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);

    let app = if enable_legacy_api {
        info!("Legacy Web API is enabled on port {}", port);
        Router::new()
            .route("/ws", get(ws_handler))
            .nest("/api", api_router)
            .nest("/auth", auth_router)
            .route("/legacy_chat", post(chat))
            .fallback_service(
                ServeDir::new(static_dir)
                    .append_index_html_on_directories(true)
                    .not_found_service(ServeFile::new(index_file)),
            )
            .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024 * 50))
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
            .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024 * 50))
            .with_state(app_state)
    };

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    info!("Web UI listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// Remove index handler as it conflicts with static file serving (React App)
// or we can keep it mapped to a specific route like /old-ui

async fn upload_handler(
    State(router): State<Arc<CognitiveRouter>>,
    mut multipart: Multipart,
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

async fn swarm_pause_handler(
    State(router): State<Arc<CognitiveRouter>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        match orch.coordinator.pause_graph(&id).await {
            Ok(result) => {
                return Json(serde_json::json!({
                        "status": "success",
                        "message": if result.quiesced {
                            "Task graph paused and quiesced".to_string()
                        } else {
                            format!(
                                "Task graph pause requested; {} running task(s) are still draining",
                                result.running_tasks
                            )
                    },
                    "quiesced": result.quiesced,
                    "running_tasks": result.running_tasks
                }));
            }
            Err(e) => {
                return Json(serde_json::json!({ "status": "error", "message": e.to_string() }));
            }
        }
    }
    Json(
        serde_json::json!({ "status": "error", "message": "Graph not found or Orchestrator not initialized" }),
    )
}

async fn swarm_resume_handler(
    State(router): State<Arc<CognitiveRouter>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        match orch.coordinator.resume_graph(&id).await {
            Ok(_) => {
                return Json(
                    serde_json::json!({ "status": "success", "message": "Task graph resumed" }),
                );
            }
            Err(e) => {
                return Json(serde_json::json!({ "status": "error", "message": e.to_string() }));
            }
        }
    }
    Json(
        serde_json::json!({ "status": "error", "message": "Graph not found or Orchestrator not initialized" }),
    )
}

async fn swarm_cancel_handler(
    State(router): State<Arc<CognitiveRouter>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        match orch.cancel_graph(&id).await {
            Ok(cancelled_tasks) => {
                return Json(serde_json::json!({
                    "status": "success",
                    "message": format!(
                        "Cancelled {} running task(s) and paused the graph",
                        cancelled_tasks
                    ),
                    "cancelled_tasks": cancelled_tasks
                }));
            }
            Err(e) => {
                return Json(serde_json::json!({ "status": "error", "message": e.to_string() }));
            }
        }
    }
    Json(
        serde_json::json!({ "status": "error", "message": "Graph not found or Orchestrator not initialized" }),
    )
}

#[derive(Deserialize)]
struct HitlDecisionPayload {
    decision: String,
    value: Option<String>,
    selected_index: Option<usize>,
}

async fn swarm_list_reviews_handler(
    State(router): State<Arc<CognitiveRouter>>,
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        let reviews = orch.coordinator.executor.hitl.list_pending();
        return Json(serde_json::json!({ "status": "success", "reviews": reviews }));
    }
    Json(serde_json::json!({ "status": "error", "message": "Orchestrator not initialized" }))
}

async fn swarm_decide_review_handler(
    State(router): State<Arc<CognitiveRouter>>,
    Path(task_id): Path<String>,
    Json(payload): Json<HitlDecisionPayload>,
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        let decision = match payload.decision.to_lowercase().as_str() {
            "approved" | "approve" => HumanDecision::Approved,
            "rejected" | "reject" => HumanDecision::Rejected(
                payload
                    .value
                    .unwrap_or_else(|| "Rejected by user".to_string()),
            ),
            "edited" | "edit" => HumanDecision::Edited(payload.value.unwrap_or_default()),
            "selected" | "select" => HumanDecision::Selected(payload.selected_index.unwrap_or(0)),
            "feedback" => HumanDecision::Feedback(payload.value.unwrap_or_default()),
            _ => {
                return Json(serde_json::json!({
                    "status": "error",
                    "message": "Unsupported decision, use approved/rejected/edited/selected/feedback"
                }));
            }
        };
        let ok = orch
            .coordinator
            .executor
            .hitl
            .submit_decision(&task_id, decision);
        if ok {
            return Json(serde_json::json!({ "status": "success" }));
        }
        return Json(
            serde_json::json!({ "status": "error", "message": "Pending review not found" }),
        );
    }
    Json(serde_json::json!({ "status": "error", "message": "Orchestrator not initialized" }))
}

#[derive(Deserialize)]
struct CreateTaskPayload {
    agent_role: String,
    prompt: String,
    dependencies: Option<Vec<String>>,
}

async fn swarm_add_task_handler(
    State(router): State<Arc<CognitiveRouter>>,
    Path(id): Path<String>,
    Json(payload): Json<CreateTaskPayload>,
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        let task_id = uuid::Uuid::new_v4().to_string();
        let new_task = crate::agent::swarm::TaskNode {
            id: task_id.clone(),
            agent_role: payload.agent_role,
            prompt: payload.prompt,
            dependencies: payload.dependencies.unwrap_or_default(),
            status: crate::agent::swarm::TaskStatus::Pending,
            result: None,
            logs: Vec::new(),
            priority: 128,
            timeout_ms: 30000,
            max_retries: 3,
            retry_count: 0,
            execution_state: None,
        };

        match orch.add_task_to_graph(&id, new_task).await {
            Ok(_) => Json(serde_json::json!({ "status": "success", "task_id": task_id })),
            Err(e) => Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
        }
    } else {
        Json(serde_json::json!({ "status": "error", "message": "Orchestrator not initialized" }))
    }
}

#[derive(Deserialize)]
struct UpdateNodeParams {
    prompt: String,
    dependencies: Option<Vec<String>>,
}

#[derive(Deserialize, Default)]
struct RecoverNodeParams {
    agent_role: Option<String>,
    prompt: Option<String>,
    dependencies: Option<Vec<String>>,
    resume_graph: Option<bool>,
}

#[derive(Deserialize)]
struct CreateTemplateParams {
    name: String,
    description: String,
    graph_id: String,
}

async fn swarm_list_templates_handler(
    State(router): State<Arc<CognitiveRouter>>,
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        match orch.list_templates().await {
            Ok(templates) => {
                Json(serde_json::json!({ "status": "success", "templates": templates }))
            }
            Err(e) => Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
        }
    } else {
        Json(serde_json::json!({ "status": "error", "message": "Orchestrator not initialized" }))
    }
}

async fn swarm_create_template_handler(
    State(router): State<Arc<CognitiveRouter>>,
    Json(payload): Json<CreateTemplateParams>,
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        let active_graphs = orch.coordinator.active_graphs.read().await;
        if let Some(graph) = active_graphs.get(&payload.graph_id) {
            match orch
                .save_template(&payload.name, &payload.description, graph)
                .await
            {
                Ok(id) => Json(serde_json::json!({ "status": "success", "template_id": id })),
                Err(e) => Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
            }
        } else {
            Json(
                serde_json::json!({ "status": "error", "message": "Graph not found in active memory" }),
            )
        }
    } else {
        Json(serde_json::json!({ "status": "error", "message": "Orchestrator not initialized" }))
    }
}

#[derive(Deserialize)]
struct InstantiateTemplateParams {
    goal: String,
}

async fn swarm_instantiate_template_handler(
    State(router): State<Arc<CognitiveRouter>>,
    Path(id): Path<String>,
    Json(payload): Json<InstantiateTemplateParams>,
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        match orch.instantiate_template(&id, &payload.goal).await {
            Ok(graph_id) => Json(serde_json::json!({ "status": "success", "graph_id": graph_id })),
            Err(e) => Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
        }
    } else {
        Json(serde_json::json!({ "status": "error", "message": "Orchestrator not initialized" }))
    }
}

#[derive(Serialize)]
struct AgentInfo {
    name: String,
    description: String,
    capabilities: Vec<String>,
}

async fn swarm_agents_handler() -> impl IntoResponse {
    // In a real system, this would be dynamic based on registered agents.
    // For now we list the supported roles in SwarmOrchestrator
    let agents = vec![
        AgentInfo {
            name: "researcher".to_string(),
            description: "Expert at gathering information from various sources.".to_string(),
            capabilities: vec!["web_search".to_string(), "summarization".to_string()],
        },
        AgentInfo {
            name: "coder".to_string(),
            description: "Writes, reviews, and fixes code in multiple languages.".to_string(),
            capabilities: vec![
                "rust".to_string(),
                "python".to_string(),
                "typescript".to_string(),
            ],
        },
        AgentInfo {
            name: "analyst".to_string(),
            description: "Analyzes data and provides insights.".to_string(),
            capabilities: vec![
                "data_analysis".to_string(),
                "pattern_recognition".to_string(),
            ],
        },
        AgentInfo {
            name: "reviewer".to_string(),
            description: "Reviews content for accuracy, style, and safety.".to_string(),
            capabilities: vec!["code_review".to_string(), "content_moderation".to_string()],
        },
        AgentInfo {
            name: "security".to_string(),
            description: "Ensures security best practices are followed.".to_string(),
            capabilities: vec![
                "vulnerability_scan".to_string(),
                "security_audit".to_string(),
            ],
        },
        AgentInfo {
            name: "planner".to_string(),
            description: "Breaks down complex goals into actionable plans.".to_string(),
            capabilities: vec!["task_decomposition".to_string(), "scheduling".to_string()],
        },
    ];
    Json(serde_json::json!({ "status": "success", "agents": agents }))
}

async fn swarm_retry_node_handler(
    State(router): State<Arc<CognitiveRouter>>,
    Path((id, node_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        match orch.retry_node(&id, &node_id).await {
            Ok(started) => {
                return Json(serde_json::json!({
                    "status": "success",
                    "message": if started {
                        "Node reset for retry and graph execution resumed"
                    } else {
                        "Node reset for retry"
                    }
                }));
            }
            Err(e) => {
                return Json(serde_json::json!({
                    "status": "error",
                    "message": e.to_string()
                }));
            }
        }
    }
    Json(serde_json::json!({ "status": "error", "message": "Graph not found" }))
}

async fn swarm_recover_node_handler(
    State(router): State<Arc<CognitiveRouter>>,
    Path((id, node_id)): Path<(String, String)>,
    Json(payload): Json<RecoverNodeParams>,
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        match orch
            .recover_node(
                &id,
                &node_id,
                crate::agent::swarm::NodeRecoveryOptions {
                    agent_role: payload.agent_role,
                    prompt: payload.prompt,
                    dependencies: payload.dependencies,
                    resume_graph: payload.resume_graph.unwrap_or(false),
                },
            )
            .await
        {
            Ok(started) => {
                return Json(serde_json::json!({
                    "status": "success",
                    "message": if started {
                        "Node recovered, updated, and graph execution resumed"
                    } else {
                        "Node recovered and queued with the requested overrides"
                    }
                }));
            }
            Err(e) => {
                return Json(serde_json::json!({
                    "status": "error",
                    "message": e.to_string()
                }));
            }
        }
    }
    Json(serde_json::json!({ "status": "error", "message": "Graph not found" }))
}

async fn swarm_update_node_handler(
    State(router): State<Arc<CognitiveRouter>>,
    Path((id, node_id)): Path<(String, String)>,
    Json(payload): Json<UpdateNodeParams>,
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        match orch
            .update_node(&id, &node_id, payload.prompt, payload.dependencies)
            .await
        {
            Ok(_) => {
                return Json(serde_json::json!({
                    "status": "success",
                    "message": "Node updated"
                }));
            }
            Err(e) => {
                return Json(serde_json::json!({
                    "status": "error",
                    "message": e.to_string()
                }));
            }
        }
    }
    Json(serde_json::json!({ "status": "error", "message": "Graph or node not found" }))
}

#[derive(Deserialize)]
struct BatchActionParams {
    action: String, // "pause", "resume", "cancel", "delete"
    ids: Vec<String>,
}

async fn swarm_batch_action_handler(
    State(router): State<Arc<CognitiveRouter>>,
    Json(payload): Json<BatchActionParams>,
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        if !matches!(
            payload.action.as_str(),
            "pause" | "resume" | "cancel" | "delete"
        ) {
            return Json(serde_json::json!({
                "status": "error",
                "message": format!("Unsupported batch action '{}'", payload.action)
            }));
        }

        let mut success_count = 0;
        let mut failed_count = 0;

        for id in &payload.ids {
            match payload.action.as_str() {
                "pause" => {
                    if orch.coordinator.pause_graph(id).await.is_ok() {
                        success_count += 1;
                    } else {
                        failed_count += 1;
                    }
                }
                "resume" => {
                    if orch.coordinator.resume_graph(id).await.is_ok() {
                        success_count += 1;
                    } else {
                        failed_count += 1;
                    }
                }
                "cancel" => {
                    if orch.cancel_graph(id).await.is_ok() {
                        success_count += 1;
                    } else {
                        failed_count += 1;
                    }
                }
                "delete" => {
                    if orch.delete_graph(id).await.is_ok() {
                        success_count += 1;
                    } else {
                        failed_count += 1;
                    }
                }
                _ => {}
            }
        }

        return Json(serde_json::json!({
            "status": "success",
            "message": format!("Processed batch action '{}'. Success: {}, Failed: {}", payload.action, success_count, failed_count)
        }));
    }
    Json(serde_json::json!({ "status": "error", "message": "Orchestrator not initialized" }))
}

async fn swarm_stats_handler(State(router): State<Arc<CognitiveRouter>>) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        let active_graphs = orch.coordinator.active_graphs.read().await;

        // Calculate stats from DB if available, else from memory
        let (total, active, completed, failed, avg_duration) = if let Some(pool) =
            &orch.coordinator.persister.pool
        {
            // Total
            let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM swarm_graphs")
                .fetch_one(pool)
                .await
                .unwrap_or(0);

            // Active (in DB 'Active' or 'Paused')
            let active: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM swarm_graphs WHERE status IN ('Active', 'Paused')",
            )
            .fetch_one(pool)
            .await
            .unwrap_or(0);

            // Completed
            let completed: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM swarm_graphs WHERE status = 'Completed'")
                    .fetch_one(pool)
                    .await
                    .unwrap_or(0);

            // Failed
            let failed: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM swarm_graphs WHERE status = 'Failed'")
                    .fetch_one(pool)
                    .await
                    .unwrap_or(0);

            // Avg Duration (only for completed graphs)
            // updated_at - created_at roughly
            let avg_dur: Option<f64> = sqlx::query_scalar(
                "SELECT AVG(updated_at - created_at) FROM swarm_graphs WHERE status = 'Completed'",
            )
            .fetch_one(pool)
            .await
            .unwrap_or(None);

            (total, active, completed, failed, avg_dur.unwrap_or(0.0))
        } else {
            // Memory fallback (only active usually)
            let total = active_graphs.len() as i64;
            let active = active_graphs
                .values()
                .filter(|g| matches!(g.status, GraphStatus::Active | GraphStatus::Paused))
                .count() as i64;
            let completed = active_graphs
                .values()
                .filter(|g| matches!(g.status, GraphStatus::Completed))
                .count() as i64;
            let failed = active_graphs
                .values()
                .filter(|g| matches!(g.status, GraphStatus::Failed))
                .count() as i64;
            (total, active, completed, failed, 0.0)
        };

        Json(serde_json::json!({
            "status": "success",
            "stats": {
                "total_tasks": total,
                "active": active,
                "completed": completed,
                "failed": failed,
                "success_rate": if total > 0 { (completed as f64 / total as f64) * 100.0 } else { 0.0 },
                "avg_duration_sec": avg_duration
            }
        }))
    } else {
        Json(serde_json::json!({ "status": "error", "message": "Orchestrator not initialized" }))
    }
}

#[derive(Deserialize, Debug)]
struct SwarmTasksQuery {
    page: Option<u32>,
    limit: Option<u32>,
    status: Option<String>,
    q: Option<String>, // Added search query
}

#[derive(Deserialize, Debug)]
struct SwarmTimelineQuery {
    limit: Option<u32>,
    node_id: Option<String>,
    event_type: Option<String>,
    message_type: Option<String>,
    status: Option<String>,
    q: Option<String>,
}

#[derive(Deserialize, Debug)]
struct SwarmReplayQuery {
    at: Option<i64>,
    node_id: Option<String>,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
struct SwarmTimelineEntry {
    timestamp: i64,
    graph_id: String,
    task_id: Option<String>,
    event_type: String,
    message_type: String,
    status: Option<String>,
    from: Option<String>,
    to: Option<String>,
    content: String,
}

#[derive(Serialize, Debug, Clone)]
struct SwarmReplaySnapshot {
    graph_id: String,
    goal: String,
    status: String,
    at: i64,
    source: String,
    focus_node_id: Option<String>,
    nodes: std::collections::HashMap<String, TaskNode>,
    timeline_len: usize,
}

fn swarm_timeline_entry_from_event(event: &AgentEvent) -> Option<SwarmTimelineEntry> {
    match event {
        AgentEvent::SwarmGraphUpdate {
            graph_id,
            status,
            timestamp,
        } => Some(SwarmTimelineEntry {
            timestamp: *timestamp,
            graph_id: graph_id.clone(),
            task_id: None,
            event_type: "graph_status".to_string(),
            message_type: "GraphStatus".to_string(),
            status: Some(status.clone()),
            from: Some("Runtime".to_string()),
            to: Some(graph_id.clone()),
            content: format!("Graph status changed to {}", status),
        }),
        AgentEvent::SwarmTaskUpdate {
            graph_id,
            task_id,
            status,
            result,
            timestamp,
        } => Some(SwarmTimelineEntry {
            timestamp: *timestamp,
            graph_id: graph_id.clone(),
            task_id: Some(task_id.clone()),
            event_type: "task_status".to_string(),
            message_type: "TaskStatus".to_string(),
            status: Some(status.clone()),
            from: Some("Runtime".to_string()),
            to: Some(task_id.clone()),
            content: result
                .clone()
                .unwrap_or_else(|| format!("Task status changed to {}", status)),
        }),
        AgentEvent::SwarmLog {
            graph_id,
            task_id,
            content,
            timestamp,
        } => Some(SwarmTimelineEntry {
            timestamp: *timestamp,
            graph_id: graph_id.clone(),
            task_id: Some(task_id.clone()),
            event_type: "log".to_string(),
            message_type: "TaskLog".to_string(),
            status: None,
            from: Some("Runtime".to_string()),
            to: Some(task_id.clone()),
            content: content.clone(),
        }),
        AgentEvent::SwarmActivity {
            task_id,
            graph_id,
            from,
            to,
            message_type,
            content,
            timestamp,
        } => Some(SwarmTimelineEntry {
            timestamp: *timestamp,
            graph_id: graph_id.clone(),
            task_id: Some(task_id.clone()),
            event_type: "activity".to_string(),
            message_type: message_type.clone(),
            status: None,
            from: Some(from.clone()),
            to: Some(to.clone()),
            content: content.clone(),
        }),
        _ => None,
    }
}

fn normalized_query_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("all"))
        .map(|value| value.to_ascii_lowercase())
}

fn swarm_timeline_entry_matches(
    entry: &SwarmTimelineEntry,
    graph_id: &str,
    query: &SwarmTimelineQuery,
) -> bool {
    if entry.graph_id != graph_id {
        return false;
    }

    if let Some(expected) = query.node_id.as_deref() {
        if entry.task_id.as_deref() != Some(expected) {
            return false;
        }
    }

    if let Some(expected) = normalized_query_filter(query.event_type.as_deref()) {
        if entry.event_type.to_ascii_lowercase() != expected {
            return false;
        }
    }

    if let Some(expected) = normalized_query_filter(query.message_type.as_deref()) {
        if entry.message_type.to_ascii_lowercase() != expected {
            return false;
        }
    }

    if let Some(expected) = normalized_query_filter(query.status.as_deref()) {
        let actual = entry
            .status
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if actual != expected {
            return false;
        }
    }

    if let Some(expected) = normalized_query_filter(query.q.as_deref()) {
        let searchable = [
            entry.event_type.as_str(),
            entry.message_type.as_str(),
            entry.status.as_deref().unwrap_or_default(),
            entry.from.as_deref().unwrap_or_default(),
            entry.to.as_deref().unwrap_or_default(),
            entry.task_id.as_deref().unwrap_or_default(),
            entry.content.as_str(),
        ]
        .join(" ")
        .to_ascii_lowercase();

        if !searchable.contains(&expected) {
            return false;
        }
    }

    true
}

fn graph_status_from_str(status: &str) -> GraphStatus {
    match status {
        "Paused" => GraphStatus::Paused,
        "Completed" => GraphStatus::Completed,
        "Failed" => GraphStatus::Failed,
        _ => GraphStatus::Active,
    }
}

fn runtime_replay_task_status(status: &str, timestamp: i64) -> TaskStatus {
    match status {
        "Running" => TaskStatus::Running {
            started_at: timestamp,
        },
        "Paused" => TaskStatus::Paused {
            paused_at: timestamp,
        },
        "Cancelled" => TaskStatus::Cancelled {
            cancelled_at: timestamp,
            reason: "Recovered from event replay".to_string(),
        },
        "Completed" => TaskStatus::Completed { duration: 0 },
        "Failed" => TaskStatus::Failed {
            error: "Recovered from event replay".to_string(),
            retries: 0,
        },
        _ => TaskStatus::Pending,
    }
}

fn reset_graph_for_runtime_replay(template: &TaskGraph) -> TaskGraph {
    let mut replay = template.clone();
    replay.status = GraphStatus::Active;

    for node in replay.nodes.values_mut() {
        node.status = TaskStatus::Pending;
        node.result = None;
        node.logs.clear();
        node.retry_count = 0;
        node.execution_state = None;
    }

    replay
}

fn apply_swarm_replay_event(graph_id: &str, graph: &mut TaskGraph, event: &AgentEvent) -> bool {
    match event {
        AgentEvent::SwarmGraphUpdate {
            graph_id: event_graph_id,
            status,
            ..
        } if event_graph_id == graph_id => {
            graph.status = graph_status_from_str(status);
            true
        }
        AgentEvent::SwarmTaskUpdate {
            graph_id: event_graph_id,
            task_id,
            status,
            result,
            timestamp,
        } if event_graph_id == graph_id => {
            let Some(node) = graph.nodes.get_mut(task_id) else {
                return false;
            };

            node.status = runtime_replay_task_status(status, *timestamp);
            match status.as_str() {
                "Completed" => node.result = result.clone(),
                "Failed" => node.result = result.clone(),
                _ => node.result = None,
            }
            true
        }
        AgentEvent::SwarmLog {
            graph_id: event_graph_id,
            task_id,
            content,
            ..
        } if event_graph_id == graph_id => {
            let Some(node) = graph.nodes.get_mut(task_id) else {
                return false;
            };

            node.logs.push(content.clone());
            true
        }
        _ => false,
    }
}

fn swarm_event_timestamp(event: &AgentEvent) -> Option<i64> {
    match event {
        AgentEvent::SwarmActivity { timestamp, .. }
        | AgentEvent::SwarmGraphUpdate { timestamp, .. }
        | AgentEvent::SwarmTaskUpdate { timestamp, .. }
        | AgentEvent::SwarmLog { timestamp, .. } => Some(*timestamp),
        _ => None,
    }
}

fn build_swarm_replay_snapshot(
    graph_id: &str,
    template: &TaskGraph,
    events: &[AgentEvent],
    at: i64,
    source: &str,
    focus_node_id: Option<String>,
) -> SwarmReplaySnapshot {
    let mut replay_graph = reset_graph_for_runtime_replay(template);
    let mut applied_events = 0;

    for event in events {
        if swarm_event_timestamp(event)
            .map(|timestamp| timestamp <= at)
            .unwrap_or(false)
            && apply_swarm_replay_event(graph_id, &mut replay_graph, event)
        {
            applied_events += 1;
        }
    }

    SwarmReplaySnapshot {
        graph_id: graph_id.to_string(),
        goal: replay_graph.goal.clone(),
        status: replay_graph.status.as_str().to_string(),
        at,
        source: source.to_string(),
        focus_node_id,
        nodes: replay_graph.nodes,
        timeline_len: applied_events,
    }
}

async fn swarm_timeline_handler(
    State(router): State<Arc<CognitiveRouter>>,
    Path(id): Path<String>,
    axum::extract::Query(query): axum::extract::Query<SwarmTimelineQuery>,
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    let limit = query.limit.unwrap_or(100).clamp(1, 500) as usize;

    if let Some(orch) = orchestrator {
        if let Some(pool) = &orch.coordinator.persister.pool {
            let filter_multiplier = if query.q.is_some()
                || query.event_type.is_some()
                || query.message_type.is_some()
                || query.status.is_some()
            {
                40
            } else {
                20
            };
            let fetch_limit = ((limit as i64) * filter_multiplier).clamp(limit as i64, 10_000);
            let rows = if let Some(node_id) = query.node_id.as_deref() {
                sqlx::query(
                    "SELECT payload FROM event_log
                     WHERE event_type IN (?, ?, ?, ?)
                       AND graph_id = ?
                       AND task_id = ?
                     ORDER BY COALESCE(event_timestamp_ms, 0) DESC, id DESC
                     LIMIT ?",
                )
                .bind("SwarmActivity")
                .bind("SwarmGraphUpdate")
                .bind("SwarmTaskUpdate")
                .bind("SwarmLog")
                .bind(&id)
                .bind(node_id)
                .bind(fetch_limit)
                .fetch_all(pool)
                .await
            } else {
                sqlx::query(
                    "SELECT payload FROM event_log
                     WHERE event_type IN (?, ?, ?, ?)
                       AND graph_id = ?
                     ORDER BY COALESCE(event_timestamp_ms, 0) DESC, id DESC
                     LIMIT ?",
                )
                .bind("SwarmActivity")
                .bind("SwarmGraphUpdate")
                .bind("SwarmTaskUpdate")
                .bind("SwarmLog")
                .bind(&id)
                .bind(fetch_limit)
                .fetch_all(pool)
                .await
            };

            if let Ok(rows) = rows {
                let mut timeline = rows
                    .into_iter()
                    .filter_map(|row| {
                        let payload: String = row.get("payload");
                        let event = serde_json::from_str::<AgentEvent>(&payload).ok()?;
                        let entry = swarm_timeline_entry_from_event(&event)?;
                        swarm_timeline_entry_matches(&entry, &id, &query).then_some(entry)
                    })
                    .collect::<Vec<_>>();

                timeline.reverse();
                if timeline.len() > limit {
                    let drain_len = timeline.len() - limit;
                    timeline.drain(0..drain_len);
                }

                return Json(serde_json::json!({
                    "status": "success",
                    "timeline": timeline
                }));
            }
        }

        let graph = {
            let graphs = orch.coordinator.active_graphs.read().await;
            graphs.get(&id).cloned()
        };

        if let Some(graph) = graph {
            let mut timeline = vec![SwarmTimelineEntry {
                timestamp: 0,
                graph_id: id.clone(),
                task_id: None,
                event_type: "graph_status".to_string(),
                message_type: "GraphSnapshot".to_string(),
                status: Some(graph.status.as_str().to_string()),
                from: Some("MemoryFallback".to_string()),
                to: Some(id.clone()),
                content: format!("Current graph status: {}", graph.status.as_str()),
            }];
            timeline.retain(|entry| swarm_timeline_entry_matches(entry, &id, &query));

            for node in graph.nodes.values() {
                let entry = SwarmTimelineEntry {
                    timestamp: 0,
                    graph_id: id.clone(),
                    task_id: Some(node.id.clone()),
                    event_type: "task_status".to_string(),
                    message_type: "TaskSnapshot".to_string(),
                    status: Some(node.status.as_str().to_string()),
                    from: Some("MemoryFallback".to_string()),
                    to: Some(node.id.clone()),
                    content: format!("Current task status: {}", node.status.as_str()),
                };
                if swarm_timeline_entry_matches(&entry, &id, &query) {
                    timeline.push(entry);
                }

                timeline.extend(node.logs.iter().rev().take(10).filter_map(|log| {
                    let entry = SwarmTimelineEntry {
                        timestamp: 0,
                        graph_id: id.clone(),
                        task_id: Some(node.id.clone()),
                        event_type: "log".to_string(),
                        message_type: "TaskLog".to_string(),
                        status: None,
                        from: Some("MemoryFallback".to_string()),
                        to: Some(node.id.clone()),
                        content: log.clone(),
                    };
                    swarm_timeline_entry_matches(&entry, &id, &query).then_some(entry)
                }));
            }

            return Json(serde_json::json!({
                "status": "success",
                "timeline": timeline
            }));
        }
    }

    Json(serde_json::json!({
        "status": "error",
        "message": "Graph not found or timeline unavailable"
    }))
}

async fn swarm_replay_handler(
    State(router): State<Arc<CognitiveRouter>>,
    Path(id): Path<String>,
    axum::extract::Query(query): axum::extract::Query<SwarmReplayQuery>,
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;

    if let Some(orch) = orchestrator {
        let template_graph = {
            let graphs = orch.coordinator.active_graphs.read().await;
            graphs.get(&id).cloned()
        };

        let template_graph = if let Some(graph) = template_graph {
            graph
        } else if let Ok(Some(graph)) = orch.coordinator.persister.load_graph(&id).await {
            graph
        } else {
            return Json(serde_json::json!({
                "status": "error",
                "message": "Graph not found for replay"
            }));
        };

        if let Some(pool) = &orch.coordinator.persister.pool {
            let rows = if let Some(at) = query.at {
                sqlx::query(
                    "SELECT payload FROM event_log
                     WHERE event_type IN (?, ?, ?, ?)
                       AND graph_id = ?
                       AND COALESCE(event_timestamp_ms, 0) <= ?
                     ORDER BY COALESCE(event_timestamp_ms, 0) ASC, id ASC",
                )
                .bind("SwarmActivity")
                .bind("SwarmGraphUpdate")
                .bind("SwarmTaskUpdate")
                .bind("SwarmLog")
                .bind(&id)
                .bind(at)
                .fetch_all(pool)
                .await
            } else {
                sqlx::query(
                    "SELECT payload FROM event_log
                     WHERE event_type IN (?, ?, ?, ?)
                       AND graph_id = ?
                     ORDER BY COALESCE(event_timestamp_ms, 0) ASC, id ASC",
                )
                .bind("SwarmActivity")
                .bind("SwarmGraphUpdate")
                .bind("SwarmTaskUpdate")
                .bind("SwarmLog")
                .bind(&id)
                .fetch_all(pool)
                .await
            };

            if let Ok(rows) = rows {
                let events = rows
                    .into_iter()
                    .filter_map(|row| {
                        let payload: String = row.get("payload");
                        let event = serde_json::from_str::<AgentEvent>(&payload).ok()?;
                        Some(event)
                    })
                    .collect::<Vec<_>>();

                let at = query.at.unwrap_or_else(|| {
                    events
                        .iter()
                        .filter_map(swarm_event_timestamp)
                        .max()
                        .unwrap_or(0)
                });

                let snapshot = build_swarm_replay_snapshot(
                    &id,
                    &template_graph,
                    &events,
                    at,
                    "event_log",
                    query.node_id.clone(),
                );
                return Json(serde_json::json!({
                    "status": "success",
                    "snapshot": snapshot
                }));
            }
        }

        let at = query.at.unwrap_or(0);
        let snapshot = SwarmReplaySnapshot {
            graph_id: id.clone(),
            goal: template_graph.goal.clone(),
            status: template_graph.status.as_str().to_string(),
            at,
            source: "persisted_graph".to_string(),
            focus_node_id: query.node_id.clone(),
            nodes: template_graph.nodes,
            timeline_len: 0,
        };

        return Json(serde_json::json!({
            "status": "success",
            "snapshot": snapshot
        }));
    }

    Json(serde_json::json!({
        "status": "error",
        "message": "Orchestrator not initialized"
    }))
}

#[derive(Serialize)]
struct SwarmGraphResponse {
    id: String,
    #[serde(flatten)]
    graph: TaskGraph,
    running_tasks: usize,
    cancelled_tasks: usize,
    recoverable_tasks: usize,
    is_draining: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    created_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<i64>,
}

impl SwarmGraphResponse {
    fn from_graph(
        id: String,
        graph: TaskGraph,
        created_at: Option<i64>,
        updated_at: Option<i64>,
    ) -> Self {
        Self {
            running_tasks: graph.running_task_count(),
            cancelled_tasks: graph.cancelled_task_count(),
            recoverable_tasks: graph.recoverable_task_count(),
            is_draining: graph.is_draining(),
            id,
            graph,
            created_at,
            updated_at,
        }
    }
}

async fn swarm_tasks_handler(
    State(router): State<Arc<CognitiveRouter>>,
    axum::extract::Query(query): axum::extract::Query<SwarmTasksQuery>,
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        if let Some(pool) = &orch.coordinator.persister.pool {
            let page = query.page.unwrap_or(1);
            let limit = query.limit.unwrap_or(10);
            let offset = (page - 1) * limit;

            let status_filter = query.status.as_deref().unwrap_or("Active");
            let search_term = query.q.as_deref().unwrap_or("");
            let has_search = !search_term.is_empty();
            let search_pattern = format!("%{}%", search_term);

            // Build query dynamically or use if/else hell
            // SQLx doesn't support dynamic query building easily without query_builder.
            // Let's use simple logic.

            let (count_sql, list_sql) = match (status_filter, has_search) {
                ("All", false) => (
                    "SELECT COUNT(*) FROM swarm_graphs",
                    "SELECT id, goal, status, created_at, updated_at FROM swarm_graphs ORDER BY updated_at DESC LIMIT ? OFFSET ?"
                ),
                ("All", true) => (
                    "SELECT COUNT(*) FROM swarm_graphs WHERE goal LIKE ?",
                    "SELECT id, goal, status, created_at, updated_at FROM swarm_graphs WHERE goal LIKE ? ORDER BY updated_at DESC LIMIT ? OFFSET ?"
                ),
                (_, false) => (
                    "SELECT COUNT(*) FROM swarm_graphs WHERE status = ?",
                    "SELECT id, goal, status, created_at, updated_at FROM swarm_graphs WHERE status = ? ORDER BY updated_at DESC LIMIT ? OFFSET ?"
                ),
                (_, true) => (
                    "SELECT COUNT(*) FROM swarm_graphs WHERE status = ? AND goal LIKE ?",
                    "SELECT id, goal, status, created_at, updated_at FROM swarm_graphs WHERE status = ? AND goal LIKE ? ORDER BY updated_at DESC LIMIT ? OFFSET ?"
                ),
            };

            // Execute Count
            let total_count: i64 = if status_filter == "All" {
                if has_search {
                    sqlx::query_scalar(count_sql)
                        .bind(&search_pattern)
                        .fetch_one(pool)
                        .await
                        .unwrap_or(0)
                } else {
                    sqlx::query_scalar(count_sql)
                        .fetch_one(pool)
                        .await
                        .unwrap_or(0)
                }
            } else if has_search {
                sqlx::query_scalar(count_sql)
                    .bind(status_filter)
                    .bind(&search_pattern)
                    .fetch_one(pool)
                    .await
                    .unwrap_or(0)
            } else {
                sqlx::query_scalar(count_sql)
                    .bind(status_filter)
                    .fetch_one(pool)
                    .await
                    .unwrap_or(0)
            };

            // Execute List
            let rows = if status_filter == "All" {
                if has_search {
                    sqlx::query(list_sql)
                        .bind(&search_pattern)
                        .bind(limit as i64)
                        .bind(offset as i64)
                        .fetch_all(pool)
                        .await
                } else {
                    sqlx::query(list_sql)
                        .bind(limit as i64)
                        .bind(offset as i64)
                        .fetch_all(pool)
                        .await
                }
            } else if has_search {
                sqlx::query(list_sql)
                    .bind(status_filter)
                    .bind(&search_pattern)
                    .bind(limit as i64)
                    .bind(offset as i64)
                    .fetch_all(pool)
                    .await
            } else {
                sqlx::query(list_sql)
                    .bind(status_filter)
                    .bind(limit as i64)
                    .bind(offset as i64)
                    .fetch_all(pool)
                    .await
            };

            // ... (rest is same)

            if let Ok(rows) = rows {
                let mut graphs = Vec::new();
                for row in rows {
                    let id: String = row.get("id");
                    let goal: String = row.get("goal");
                    let created_at: i64 = row.get("created_at");
                    let updated_at: i64 = row.get("updated_at");
                    // We need to fetch tasks for each graph to reconstruct TaskGraph object
                    // Or we can return a lighter object.
                    // Frontend expects TaskGraph which includes nodes.
                    // This is N+1 query, but for page size 10 it's fine.

                    // Optimization: Check if it's in active_graphs first?
                    let active_graphs = orch.coordinator.active_graphs.read().await;
                    if let Some(g) = active_graphs.get(&id) {
                        // Use active in-memory version as it's definitely latest
                        let mut g_clone = g.clone();
                        if g_clone.goal.is_empty() {
                            g_clone.goal = goal.clone();
                        }
                        graphs.push(SwarmGraphResponse::from_graph(
                            id.clone(),
                            g_clone,
                            Some(created_at),
                            Some(updated_at),
                        ));
                    } else {
                        // Fetch from DB
                        let tasks = sqlx::query("SELECT id, agent_role, prompt, dependencies, status, result, logs, execution_state FROM swarm_tasks WHERE graph_id = ?")
                            .bind(&id)
                            .fetch_all(pool)
                            .await
                            .unwrap_or_default();

                        let mut nodes = std::collections::HashMap::new();
                        for task_row in tasks {
                            let task_id: String = task_row.get("id");
                            let role: String = task_row.get("agent_role");
                            let prompt: String = task_row.get("prompt");
                            let deps_json: String = task_row.get("dependencies");
                            let status_json: String = task_row.get("status");
                            let result: Option<String> = task_row.get("result");
                            let logs_json: String = task_row.get("logs");
                            let execution_state_json: Option<String> =
                                task_row.get("execution_state");

                            let dependencies: Vec<String> =
                                serde_json::from_str(&deps_json).unwrap_or_default();
                            let status: TaskStatus =
                                serde_json::from_str(&status_json).unwrap_or(TaskStatus::Pending);
                            let logs: Vec<String> =
                                serde_json::from_str(&logs_json).unwrap_or_default();
                            let execution_state = execution_state_json.and_then(|json| {
                                serde_json::from_str::<
                                    crate::agent::harness_agent::HarnessExecutionState,
                                >(&json)
                                .ok()
                            });

                            nodes.insert(
                                task_id.clone(),
                                crate::agent::swarm::TaskNode {
                                    id: task_id,
                                    agent_role: role,
                                    prompt,
                                    dependencies,
                                    status,
                                    result,
                                    logs,
                                    priority: 128,
                                    timeout_ms: 30000,
                                    max_retries: 3,
                                    retry_count: 0,
                                    execution_state,
                                },
                            );
                        }

                        let status_str: String = row.get("status");
                        let status = match status_str.as_str() {
                            "Active" => GraphStatus::Active,
                            "Paused" => GraphStatus::Paused,
                            "Completed" => GraphStatus::Completed,
                            "Failed" => GraphStatus::Failed,
                            _ => GraphStatus::Active,
                        };

                        let graph_obj = crate::agent::swarm::TaskGraph {
                            nodes,
                            status,
                            goal,
                        };
                        graphs.push(SwarmGraphResponse::from_graph(
                            id,
                            graph_obj,
                            Some(created_at),
                            Some(updated_at),
                        ));
                    }
                }

                return Json(serde_json::json!({
                    "status": "success",
                    "graphs": graphs,
                    "pagination": {
                        "page": page,
                        "limit": limit,
                        "total": total_count,
                        "total_pages": (total_count as f64 / limit as f64).ceil() as i64
                    }
                }));
            }
        }

        // Fallback if no pool or DB error
        let graphs = orch.coordinator.active_graphs.read().await;
        // Fix the ID issue here too
        let mut graph_list = Vec::new();
        for (id, g) in graphs.iter() {
            graph_list.push(SwarmGraphResponse::from_graph(
                id.clone(),
                g.clone(),
                None,
                None,
            ));
        }

        Json(serde_json::json!({
            "status": "success",
            "graphs": graph_list
        }))
    } else {
        Json(serde_json::json!({
            "status": "error",
            "message": "Swarm Orchestrator not initialized"
        }))
    }
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
        build_swarm_replay_snapshot, swarm_timeline_entry_from_event, swarm_timeline_entry_matches,
        SwarmGraphResponse, SwarmTimelineEntry, SwarmTimelineQuery,
    };
    use crate::agent::swarm::{GraphStatus, TaskGraph, TaskStatus};
    use crate::events::AgentEvent;

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
