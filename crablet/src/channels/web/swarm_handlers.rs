//! Swarm action handlers for the legacy web API.
//!
//! Contains HTTP handlers for swarm task operations:
//! - Pause / Resume / Cancel graphs
//! - HITL review decisions
//! - Add / Update / Recover / Retry nodes
//! - Template management (list / create / instantiate)
//! - Agent listing
//! - Batch actions

use crate::agent::hitl::HumanDecision;
use crate::agent::swarm::{TaskNode, TaskStatus};
use crate::cognitive::router::CognitiveRouter;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ---- Request / Response Types ----

#[derive(Deserialize)]
pub(crate) struct HitlDecisionPayload {
    pub decision: String,
    pub value: Option<String>,
    pub selected_index: Option<usize>,
}

#[derive(Deserialize)]
pub(crate) struct CreateTaskPayload {
    pub agent_role: String,
    pub prompt: String,
    pub dependencies: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub(crate) struct UpdateNodeParams {
    pub prompt: String,
    pub dependencies: Option<Vec<String>>,
}

#[derive(Deserialize, Default)]
pub(crate) struct RecoverNodeParams {
    pub agent_role: Option<String>,
    pub prompt: Option<String>,
    pub dependencies: Option<Vec<String>>,
    pub resume_graph: Option<bool>,
}

#[derive(Deserialize)]
pub(crate) struct CreateTemplateParams {
    pub name: String,
    pub description: String,
    pub graph_id: String,
}

#[derive(Deserialize)]
pub(crate) struct InstantiateTemplateParams {
    pub goal: String,
}

#[derive(Deserialize)]
pub(crate) struct BatchActionParams {
    pub action: String, // "pause", "resume", "cancel", "delete"
    pub ids: Vec<String>,
}

#[derive(Serialize)]
pub(crate) struct AgentInfo {
    pub name: String,
    pub description: String,
    pub capabilities: Vec<String>,
}

// ---- Handlers ----

pub(crate) async fn swarm_pause_handler(
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

pub(crate) async fn swarm_resume_handler(
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

pub(crate) async fn swarm_cancel_handler(
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

pub(crate) async fn swarm_list_reviews_handler(
    State(router): State<Arc<CognitiveRouter>>,
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        let reviews = orch.coordinator.executor.hitl.list_pending();
        return Json(serde_json::json!({ "status": "success", "reviews": reviews }));
    }
    Json(serde_json::json!({ "status": "error", "message": "Orchestrator not initialized" }))
}

pub(crate) async fn swarm_decide_review_handler(
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

pub(crate) async fn swarm_add_task_handler(
    State(router): State<Arc<CognitiveRouter>>,
    Path(id): Path<String>,
    Json(payload): Json<CreateTaskPayload>,
) -> impl IntoResponse {
    let orchestrator = &router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        let task_id = uuid::Uuid::new_v4().to_string();
        let new_task = TaskNode {
            id: task_id.clone(),
            agent_role: payload.agent_role,
            prompt: payload.prompt,
            dependencies: payload.dependencies.unwrap_or_default(),
            status: TaskStatus::Pending,
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

pub(crate) async fn swarm_list_templates_handler(
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

pub(crate) async fn swarm_create_template_handler(
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

pub(crate) async fn swarm_instantiate_template_handler(
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

pub(crate) async fn swarm_agents_handler() -> impl IntoResponse {
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

pub(crate) async fn swarm_retry_node_handler(
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

pub(crate) async fn swarm_recover_node_handler(
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

pub(crate) async fn swarm_update_node_handler(
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

pub(crate) async fn swarm_batch_action_handler(
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
