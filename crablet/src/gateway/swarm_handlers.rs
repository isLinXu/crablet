//! Swarm-related web handlers
//!
//! Handles Swarm orchestration, agent management, and HITL (Human-in-the-Loop).

use std::sync::Arc;
use axum::{
    extract::{State, Json, Path},
};
use serde::Deserialize;

use crate::gateway::server::CrabletGateway;
use crate::agent::hitl::HumanDecision;

#[derive(Deserialize)]
pub struct HitlDecisionPayload {
    pub decision: String,
    pub value: Option<String>,
    pub selected_index: Option<usize>,
}

pub async fn get_swarm_stats(
    State(_gateway): State<Arc<CrabletGateway>>,
) -> Json<serde_json::Value> {
     Json(serde_json::json!({
        "stats": {
            "total_tasks": 12,
            "active": 3,
            "completed": 8,
            "failed": 1,
            "success_rate": 88.5,
            "avg_duration_sec": 4.2
        }
    }))
}

pub async fn get_swarm_tasks(
    State(_gateway): State<Arc<CrabletGateway>>,
) -> Json<serde_json::Value> {
    let graphs = vec![
        serde_json::json!({
            "id": "graph-001",
            "status": "Active",
            "nodes": {
                "task-1": {
                    "id": "task-1",
                    "agent_role": "manager",
                    "prompt": "Coordinate project plan",
                    "dependencies": [],
                    "status": { "Completed": { "duration": 1.2 } },
                    "result": "Plan created."
                },
                "task-2": {
                    "id": "task-2",
                    "agent_role": "researcher",
                    "prompt": "Find relevant libraries",
                    "dependencies": ["task-1"],
                    "status": { "Running": { "started_at": 1234567890 } }
                },
                "task-3": {
                    "id": "task-3",
                    "agent_role": "coder",
                    "prompt": "Implement core logic",
                    "dependencies": ["task-2"],
                    "status": "Pending"
                }
            }
        }),
        serde_json::json!({
            "id": "graph-002",
            "status": "Completed",
            "nodes": {
                "task-A": {
                    "id": "task-A",
                    "agent_role": "writer",
                    "prompt": "Draft blog post",
                    "dependencies": [],
                    "status": { "Completed": { "duration": 3.5 } },
                    "result": "Draft ready."
                }
            }
        })
    ];

    Json(serde_json::json!({
        "graphs": graphs,
        "pagination": {
            "page": 1,
            "limit": 10,
            "total": 2,
            "total_pages": 1
        }
    }))
}

pub async fn get_swarm_state(
    State(_gateway): State<Arc<CrabletGateway>>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "active",
        "agents": [],
        "tasks": []
    }))
}

pub async fn list_agents(
    State(_gateway): State<Arc<CrabletGateway>>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!([
        {"role": "manager", "description": "Task coordinator"},
        {"role": "coder", "description": "Software engineer"},
        {"role": "researcher", "description": "Information gatherer"},
        {"role": "reviewer", "description": "Code/Content reviewer"}
    ]))
}

pub async fn cancel_task(
    State(_gateway): State<Arc<CrabletGateway>>,
    Path(task_id): Path<String>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "cancelled",
        "task_id": task_id
    }))
}

pub async fn list_swarm_reviews(
    State(gateway): State<Arc<CrabletGateway>>,
) -> Json<serde_json::Value> {
    let orchestrator = &gateway.router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        let reviews = orch.coordinator.executor.hitl.list_pending();
        return Json(serde_json::json!({
            "status": "success",
            "reviews": reviews
        }));
    }
    Json(serde_json::json!({
        "status": "error",
        "message": "Orchestrator not initialized",
        "reviews": []
    }))
}

pub async fn decide_swarm_review(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(task_id): Path<String>,
    Json(payload): Json<HitlDecisionPayload>,
) -> Json<serde_json::Value> {
    let orchestrator = &gateway.router.sys3.orchestrator;
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
        if orch.coordinator.executor.hitl.submit_decision(&task_id, decision) {
            return Json(serde_json::json!({ "status": "success" }));
        }
        return Json(serde_json::json!({
            "status": "error",
            "message": "Pending review not found"
        }));
    }
    Json(serde_json::json!({
        "status": "error",
        "message": "Orchestrator not initialized"
    }))
}