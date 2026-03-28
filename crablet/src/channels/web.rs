use axum::{
    extract::{State, Form, Multipart, Path, ws::{WebSocketUpgrade, WebSocket, Message as WsMessage}},
    response::{IntoResponse, Json},
    routing::{get, post, put},
    Router,
};
use tower_http::services::{ServeDir, ServeFile};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::cognitive::router::CognitiveRouter;
use crate::agent::hitl::HumanDecision;
use crate::agent::swarm::{GraphStatus, TaskStatus};
use crate::events::AgentEvent;
use tracing::{info, error};
use std::path::PathBuf;
use tokio::fs;
use base64::Engine;
use crate::auth::oidc::OidcProvider;
use crate::auth::middleware::auth_middleware;
use crate::auth::handlers::{login_handler, callback_handler, me_handler, AuthState};
use sqlx::Row;

#[derive(Deserialize, Debug, Clone)]
struct ChatInput {
    prompt: String,
    session_id: Option<String>,
    image_paths: Option<String>, // JSON array of paths
}

pub async fn run(router: Arc<CognitiveRouter>, port: u16, auth_config: Option<(String, String, String, String)>) -> anyhow::Result<()> {
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
        match OidcProvider::discover(&issuer, &id, &secret, &format!("http://localhost:{}/auth/callback", port)).await {
            Ok(oidc) => Arc::new(AuthState {
                oidc: Some(oidc),
                jwt_secret,
            }),
            Err(e) => {
                error!("Failed to initialize OIDC: {}. Auth will be disabled.", e);
                Arc::new(AuthState { oidc: None, jwt_secret: "secret".to_string() })
            }
        }
    } else {
        Arc::new(AuthState { oidc: None, jwt_secret: "secret".to_string() })
    };

    // API Router
    let api_router = Router::new()
        .route("/health", get(|| async { Json(serde_json::json!({ "status": "ok" })) }))
        .route("/chat", post(chat))
        .route("/upload", post(upload_handler))
        .route("/knowledge", get(list_knowledge_handler).delete(delete_knowledge_handler))
        .route("/me", get(me_handler))
        .route("/dashboard", get(dashboard_handler))
        .route("/swarm/stats", get(swarm_stats_handler))
        .route("/swarm/templates", get(swarm_list_templates_handler).post(swarm_create_template_handler))
        .route("/swarm/templates/:id/instantiate", post(swarm_instantiate_template_handler))
        .route("/swarm/agents", get(swarm_agents_handler))
        .route("/swarm/tasks", get(swarm_tasks_handler))
        .route("/swarm/tasks/batch", post(swarm_batch_action_handler))
        .route("/swarm/tasks/:id/pause", post(swarm_pause_handler))
        .route("/swarm/tasks/:id/resume", post(swarm_resume_handler))
        .route("/swarm/reviews", get(swarm_list_reviews_handler))
        .route("/swarm/reviews/:task_id/decision", post(swarm_decide_review_handler))
        .route("/swarm/tasks/:id/nodes", post(swarm_add_task_handler))
        .route("/swarm/tasks/:id/nodes/:node_id", put(swarm_update_node_handler))
        .route("/swarm/tasks/:id/nodes/:node_id/retry", post(swarm_retry_node_handler))
        .layer(axum::middleware::from_fn_with_state(auth_state.clone(), auth_middleware));

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
                    .not_found_service(ServeFile::new(index_file))
            )
            .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024 * 50))
            .with_state(app_state)
    } else {
        info!("Legacy Web API is disabled; serving static UI and auth endpoints only on port {}", port);
        Router::new()
            .nest("/auth", auth_router)
            .fallback_service(
                ServeDir::new(static_dir)
                    .append_index_html_on_directories(true)
                    .not_found_service(ServeFile::new(index_file))
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
        let msg = match &event.payload {
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
            AgentEvent::CognitiveLayerChanged { layer } => format!("COGNITIVE_LAYER: {}", layer),
            AgentEvent::Error(s) => format!("ERROR: {}", s),
            AgentEvent::SwarmGraphUpdate { graph_id, status, .. } => format!("SWARM_GRAPH: {}|{:?}", graph_id, status),
            AgentEvent::SwarmTaskUpdate { graph_id, task_id, status, .. } => format!("SWARM_TASK: {}|{}|{:?}", graph_id, task_id, status),
            AgentEvent::SwarmLog { graph_id, task_id, content, .. } => {
                 let b64_content = base64::engine::general_purpose::STANDARD.encode(content);
                 format!("SWARM_LOG: {}|{}|{}", graph_id, task_id, b64_content)
            },
            AgentEvent::SwarmActivity { task_id, graph_id, from, to, message_type, content, .. } => {
                 let b64_content = base64::engine::general_purpose::STANDARD.encode(content);
                 format!("SWARM_MSG: {}|{}|{}|{}|{}|{}", graph_id, task_id, from, to, message_type, b64_content)
            },
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
            Err(e) => return Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
        }
    }
    
    // Silence unused variable warning if feature is disabled
    #[cfg(not(feature = "knowledge"))]
    let _ = router;
    
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
    Path(id): Path<String>
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        let mut graphs = orch.coordinator.active_graphs.write().await;
        if let Some(graph) = graphs.get_mut(&id) {
            graph.status = GraphStatus::Paused;
            return Json(serde_json::json!({ "status": "success", "message": "Task graph paused" }));
        }
    }
    Json(serde_json::json!({ "status": "error", "message": "Graph not found or Orchestrator not initialized" }))
}

async fn swarm_resume_handler(
    State(router): State<Arc<CognitiveRouter>>,
    Path(id): Path<String>
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        let mut graphs = orch.coordinator.active_graphs.write().await;
        if let Some(graph) = graphs.get_mut(&id) {
            graph.status = GraphStatus::Active;
            return Json(serde_json::json!({ "status": "success", "message": "Task graph resumed" }));
        }
    }
    Json(serde_json::json!({ "status": "error", "message": "Graph not found or Orchestrator not initialized" }))
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
            "rejected" | "reject" => HumanDecision::Rejected(payload.value.unwrap_or_else(|| "Rejected by user".to_string())),
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
        let ok = orch.coordinator.executor.hitl.submit_decision(&task_id, decision);
        if ok {
            return Json(serde_json::json!({ "status": "success" }));
        }
        return Json(serde_json::json!({ "status": "error", "message": "Pending review not found" }));
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
    Json(payload): Json<CreateTaskPayload>
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

#[derive(Deserialize)]
struct CreateTemplateParams {
    name: String,
    description: String,
    graph_id: String,
}

async fn swarm_list_templates_handler(State(router): State<Arc<CognitiveRouter>>) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        match orch.list_templates().await {
            Ok(templates) => Json(serde_json::json!({ "status": "success", "templates": templates })),
            Err(e) => Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
        }
    } else {
        Json(serde_json::json!({ "status": "error", "message": "Orchestrator not initialized" }))
    }
}

async fn swarm_create_template_handler(
    State(router): State<Arc<CognitiveRouter>>,
    Json(payload): Json<CreateTemplateParams>
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        let active_graphs = orch.coordinator.active_graphs.read().await;
        if let Some(graph) = active_graphs.get(&payload.graph_id) {
             match orch.save_template(&payload.name, &payload.description, graph).await {
                 Ok(id) => Json(serde_json::json!({ "status": "success", "template_id": id })),
                 Err(e) => Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
             }
        } else {
             Json(serde_json::json!({ "status": "error", "message": "Graph not found in active memory" }))
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
    Json(payload): Json<InstantiateTemplateParams>
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        match orch.instantiate_template(&id, &payload.goal).await {
            Ok(graph_id) => {
                 let orch_clone = orch.clone();
                 let graph_id_clone = graph_id.clone();
                 let goal_clone = payload.goal.clone();
                 
                 tokio::spawn(async move {
                     let _ = orch_clone.execute_graph(crate::agent::swarm::TaskGraph::new(), &graph_id_clone, &goal_clone).await;
                 });
                 
                 Json(serde_json::json!({ "status": "success", "graph_id": graph_id }))
            },
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
            capabilities: vec!["rust".to_string(), "python".to_string(), "typescript".to_string()],
        },
        AgentInfo {
            name: "analyst".to_string(),
            description: "Analyzes data and provides insights.".to_string(),
            capabilities: vec!["data_analysis".to_string(), "pattern_recognition".to_string()],
        },
        AgentInfo {
            name: "reviewer".to_string(),
            description: "Reviews content for accuracy, style, and safety.".to_string(),
            capabilities: vec!["code_review".to_string(), "content_moderation".to_string()],
        },
        AgentInfo {
            name: "security".to_string(),
            description: "Ensures security best practices are followed.".to_string(),
            capabilities: vec!["vulnerability_scan".to_string(), "security_audit".to_string()],
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
    Path((id, node_id)): Path<(String, String)>
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        let mut graphs = orch.coordinator.active_graphs.write().await;
        if let Some(graph) = graphs.get_mut(&id) {
            let mut should_restart = false;
            
            if let Some(node) = graph.nodes.get_mut(&node_id) {
                let can_retry = matches!(node.status, crate::agent::swarm::TaskStatus::Failed { .. } | crate::agent::swarm::TaskStatus::Completed { .. });
                
                if can_retry {
                    node.status = crate::agent::swarm::TaskStatus::Pending;
                    node.result = None;
                    node.logs.push("Status reset to Pending for retry.".to_string());
                    
                    if matches!(graph.status, GraphStatus::Completed | GraphStatus::Failed) {
                        should_restart = true;
                    }
                } else {
                    return Json(serde_json::json!({ "status": "error", "message": "Node is not in a retriable state" }));
                }
            } else {
                return Json(serde_json::json!({ "status": "error", "message": "Node not found" }));
            }
            
            if should_restart {
                 graph.status = GraphStatus::Active;
                 let orch_clone = orch.clone();
                 let graph_clone = graph.clone();
                 let graph_id_clone = id.clone();
                 
                 tokio::spawn(async move {
                     let goal = "Retry operation"; 
                     let _ = orch_clone.execute_graph(graph_clone, &graph_id_clone, goal).await;
                 });
            }
            
            return Json(serde_json::json!({ "status": "success", "message": "Node reset for retry" }));
        }
    }
    Json(serde_json::json!({ "status": "error", "message": "Graph not found" }))
}

async fn swarm_update_node_handler(
    State(router): State<Arc<CognitiveRouter>>,
    Path((id, node_id)): Path<(String, String)>,
    Json(payload): Json<UpdateNodeParams>
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        let mut graphs = orch.coordinator.active_graphs.write().await;
        if let Some(graph) = graphs.get_mut(&id) {
            // Only allow updates if graph is paused
            if graph.status != GraphStatus::Paused {
                 return Json(serde_json::json!({ "status": "error", "message": "Graph must be paused to update nodes" }));
            }
            
            // Perform validations before mutable borrow of node
            if let Some(deps) = &payload.dependencies {
                 // Validate dependencies: must be valid node IDs in the graph
                 let valid = deps.iter().all(|d| graph.nodes.contains_key(d));
                 if !valid {
                     return Json(serde_json::json!({ "status": "error", "message": "Invalid dependencies" }));
                 }
                 // Check for cycles
                 if graph.detects_cycle(&node_id, deps) {
                     return Json(serde_json::json!({ "status": "error", "message": "Cycle detected in dependencies" }));
                 }
            }

            if let Some(node) = graph.nodes.get_mut(&node_id) {
                // Only allow updates if node is pending
                match node.status {
                    crate::agent::swarm::TaskStatus::Pending => {
                        node.prompt = payload.prompt.clone();
                        if let Some(deps) = payload.dependencies {
                             node.dependencies = deps;
                        }
                        return Json(serde_json::json!({ "status": "success", "message": "Node updated" }));
                    },
                    _ => return Json(serde_json::json!({ "status": "error", "message": "Only pending tasks can be updated" }))
                }
            }
        }
    }
    Json(serde_json::json!({ "status": "error", "message": "Graph or node not found" }))
}

#[derive(Deserialize)]
struct BatchActionParams {
    action: String, // "pause", "resume", "delete"
    ids: Vec<String>,
}

async fn swarm_batch_action_handler(
    State(router): State<Arc<CognitiveRouter>>,
    Json(payload): Json<BatchActionParams>
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        let mut graphs = orch.coordinator.active_graphs.write().await;
        
        let mut success_count = 0;
        let mut failed_count = 0;
        
        for id in &payload.ids {
            match payload.action.as_str() {
                "pause" => {
                    if let Some(graph) = graphs.get_mut(id) {
                        graph.status = GraphStatus::Paused;
                        success_count += 1;
                    } else {
                        failed_count += 1;
                    }
                },
                "resume" => {
                    if let Some(graph) = graphs.get_mut(id) {
                        graph.status = GraphStatus::Active;
                        success_count += 1;
                    } else {
                        failed_count += 1;
                    }
                },
                "delete" => {
                    // Remove from active graphs
                    if graphs.remove(id).is_some() {
                        success_count += 1;
                    } else {
                        // Check DB? If we want to delete history.
                        // For now, only active.
                        failed_count += 1;
                    }
                    // Also need to delete from DB
                    if let Some(pool) = &orch.coordinator.persister.pool {
                        let _ = sqlx::query("DELETE FROM swarm_graphs WHERE id = ?")
                            .bind(id)
                            .execute(pool)
                            .await;
                         // Cascade delete handles tasks
                    }
                },
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
        let (total, active, completed, failed, avg_duration) = if let Some(pool) = &orch.coordinator.persister.pool {
            // Total
            let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM swarm_graphs")
                .fetch_one(pool).await.unwrap_or(0);
                
            // Active (in DB 'Active' or 'Paused')
            let active: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM swarm_graphs WHERE status IN ('Active', 'Paused')")
                .fetch_one(pool).await.unwrap_or(0);
                
            // Completed
            let completed: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM swarm_graphs WHERE status = 'Completed'")
                .fetch_one(pool).await.unwrap_or(0);
                
            // Failed
            let failed: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM swarm_graphs WHERE status = 'Failed'")
                .fetch_one(pool).await.unwrap_or(0);
                
            // Avg Duration (only for completed graphs)
            // updated_at - created_at roughly
            let avg_dur: Option<f64> = sqlx::query_scalar("SELECT AVG(updated_at - created_at) FROM swarm_graphs WHERE status = 'Completed'")
                .fetch_one(pool).await.unwrap_or(None);
                
            (total, active, completed, failed, avg_dur.unwrap_or(0.0))
        } else {
            // Memory fallback (only active usually)
            let total = active_graphs.len() as i64;
            let active = active_graphs.values().filter(|g| matches!(g.status, GraphStatus::Active | GraphStatus::Paused)).count() as i64;
            let completed = active_graphs.values().filter(|g| matches!(g.status, GraphStatus::Completed)).count() as i64;
            let failed = active_graphs.values().filter(|g| matches!(g.status, GraphStatus::Failed)).count() as i64;
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

async fn swarm_tasks_handler(
    State(router): State<Arc<CognitiveRouter>>,
    axum::extract::Query(query): axum::extract::Query<SwarmTasksQuery>
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
                     sqlx::query_scalar(count_sql).bind(&search_pattern).fetch_one(pool).await.unwrap_or(0)
                } else {
                     sqlx::query_scalar(count_sql).fetch_one(pool).await.unwrap_or(0)
                }
            } else if has_search {
                 sqlx::query_scalar(count_sql).bind(status_filter).bind(&search_pattern).fetch_one(pool).await.unwrap_or(0)
            } else {
                 sqlx::query_scalar(count_sql).bind(status_filter).fetch_one(pool).await.unwrap_or(0)
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
                    // We need to fetch tasks for each graph to reconstruct TaskGraph object
                    // Or we can return a lighter object. 
                    // Frontend expects TaskGraph which includes nodes.
                    // This is N+1 query, but for page size 10 it's fine.
                    
                    // Optimization: Check if it's in active_graphs first?
                    let active_graphs = orch.coordinator.active_graphs.read().await;
                    if let Some(g) = active_graphs.get(&id) {
                        // Use active in-memory version as it's definitely latest
                        let g_clone = g.clone();
                        // Inject ID (frontend needs it, though it might not be in struct?)
                        // TaskGraph struct in rust doesn't have ID field, it's key in map.
                        // We serialize it as list, frontend interface expects it.
                        // We should wrap it.
                        // Wait, previous implementation: `let graph_list: Vec<_> = graphs.values().collect();`
                        // It lost the ID! 
                        // The frontend `TaskGraph` interface has optional `id`.
                        // We should construct a response that includes ID.
                        
                        // We need to modify TaskGraph to include ID or wrap it.
                        // Let's wrap it in a anonymous struct or serde_json::Value
                        
                        // Actually, let's fix the previous implementation first. 
                        // The previous implementation `graphs.values().collect()` indeed lost the ID key from HashMap.
                        // So frontend probably didn't get IDs unless TaskGraph has it.
                        // Checking TaskGraph struct... it does NOT have id.
                        // Checking frontend... `id?: string`.
                        // So frontend might be broken for ID dependent actions?
                        // Ah, `getSwarmGraphs` in frontend API: `return response.data.graphs;`
                        // If backend sent `[ {nodes: ...}, {nodes: ...} ]`, frontend has no IDs.
                        // WE MUST FIX THIS.
                        
                        // Let's create a DTO.
                         let json_graph = serde_json::to_value(&g_clone).unwrap();
                         if let serde_json::Value::Object(mut map) = json_graph {
                             map.insert("id".to_string(), serde_json::Value::String(id.clone()));
                             graphs.push(serde_json::Value::Object(map));
                         }
                    } else {
                        // Fetch from DB
                         let tasks = sqlx::query("SELECT id, agent_role, prompt, dependencies, status, result, logs FROM swarm_tasks WHERE graph_id = ?")
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
                             
                             let dependencies: Vec<String> = serde_json::from_str(&deps_json).unwrap_or_default();
                             let status: TaskStatus = serde_json::from_str(&status_json).unwrap_or(TaskStatus::Pending);
                             let logs: Vec<String> = serde_json::from_str(&logs_json).unwrap_or_default();
                             
                             nodes.insert(task_id.clone(), crate::agent::swarm::TaskNode {
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
                             });
                        }
                        
                        let goal: String = row.get("goal");
                        let created_at: i64 = row.get("created_at");
                        let updated_at: i64 = row.get("updated_at");

                        let status_str: String = row.get("status");
                        let status = match status_str.as_str() {
                            "Active" => GraphStatus::Active,
                            "Paused" => GraphStatus::Paused,
                            "Completed" => GraphStatus::Completed,
                            "Failed" => GraphStatus::Failed,
                            _ => GraphStatus::Active,
                        };
                        
                        let graph_obj = crate::agent::swarm::TaskGraph { nodes, status, goal: String::new() };
                        let json_graph = serde_json::to_value(&graph_obj).unwrap();
                        if let serde_json::Value::Object(mut map) = json_graph {
                            map.insert("id".to_string(), serde_json::Value::String(id));
                            map.insert("goal".to_string(), serde_json::Value::String(goal));
                            map.insert("created_at".to_string(), serde_json::json!(created_at));
                            map.insert("updated_at".to_string(), serde_json::json!(updated_at));
                            graphs.push(serde_json::Value::Object(map));
                        }
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
             let json_graph = serde_json::to_value(g).unwrap();
             if let serde_json::Value::Object(mut map) = json_graph {
                 map.insert("id".to_string(), serde_json::Value::String(id.clone()));
                 graph_list.push(serde_json::Value::Object(map));
             }
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
