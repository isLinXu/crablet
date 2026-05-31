//! Gateway compatibility layer for legacy "web handlers" imports.
//!
//! The repository contains newer, more focused handler modules
//! (`chat_handlers`, `swarm_handlers`, `dashboard_handlers`, etc.).
//! This module re-exports those stable entry points and provides small
//! compatibility handlers for skill- and MCP-related endpoints.

use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;

use crate::gateway::server::CrabletGateway;

pub use super::chat_handlers::{chat_handler, chat_stream, image_handler};
pub use super::dashboard_handlers::get_dashboard_stats;
pub use super::harness_handlers::{
    delete_harness as delete_distributed_harness, get_cluster_status,
    get_harness as get_distributed_harness, list_harnesses as list_distributed_harnesses,
    list_nodes as list_distributed_harness_nodes,
    send_harness_signal as signal_distributed_harness,
};
pub use super::swarm_handlers::{
    cancel_task, decide_swarm_review, get_swarm_state, get_swarm_stats, get_swarm_tasks,
    list_agents, list_swarm_reviews,
};

#[derive(Debug, Deserialize)]
pub struct RegistrySearchQuery {
    pub q: String,
}

#[derive(Debug, Deserialize)]
pub struct InstallSkillRequest {
    pub name: String,
    pub target_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SemanticSearchRequest {
    pub query: String,
    pub limit: Option<usize>,
}

/// List locally loaded skills from the shared skill registry.
pub async fn list_skills(State(gateway): State<Arc<CrabletGateway>>) -> Json<serde_json::Value> {
    let registry = gateway.router.shared_skills.read().await;
    let skills = registry.list_skills();

    Json(serde_json::json!({
        "status": "success",
        "count": skills.len(),
        "skills": skills
    }))
}

/// Compatibility endpoint for enable/disable skill state.
///
/// The current shared registry does not persist enabled/disabled state yet,
/// so we return the current presence of the skill and keep the route stable.
pub async fn toggle_skill(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let registry = gateway.router.shared_skills.read().await;
    let enabled = registry.get_skill(&name).is_some();

    if !enabled {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(Json(serde_json::json!({
        "status": "noop",
        "skill": name,
        "enabled": true,
        "message": "Persistent skill enable/disable state is not implemented yet."
    })))
}

/// Search the remote registry index using the existing registry helper.
pub async fn search_registry_skills(
    State(gateway): State<Arc<CrabletGateway>>,
    Query(query): Query<RegistrySearchQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let registry = gateway.router.shared_skills.read().await;
    let results = registry
        .search(&query.q)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    Ok(Json(serde_json::json!({
        "status": "success",
        "query": query.q,
        "results": results
    })))
}

/// Install a skill into the local `skills/` directory via the registry helper.
pub async fn install_skill(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(payload): Json<InstallSkillRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let target_dir = payload
        .target_dir
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("skills"));

    let mut registry = gateway.router.shared_skills.write().await;
    registry
        .install(&payload.name, target_dir.clone())
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    Ok(Json(serde_json::json!({
        "status": "success",
        "skill": payload.name,
        "target_dir": target_dir
    })))
}

/// Execute a loaded skill directly.
pub async fn run_skill(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(name): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let args = payload.get("args").cloned().unwrap_or(payload);

    let registry = gateway.router.shared_skills.read().await;
    let output = registry
        .execute(&name, args)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    Ok(Json(serde_json::json!({
        "status": "success",
        "skill": name,
        "output": output
    })))
}

/// Lightweight semantic search over currently loaded skills.
pub async fn semantic_search_skills(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(payload): Json<SemanticSearchRequest>,
) -> Json<serde_json::Value> {
    let limit = payload.limit.unwrap_or(10).min(50);
    let query = payload.query.to_lowercase();

    let registry = gateway.router.shared_skills.read().await;
    let mut results: Vec<serde_json::Value> = registry
        .list_skills()
        .into_iter()
        .filter_map(|skill| {
            let name_hit = skill.name.to_lowercase().contains(&query);
            let desc_hit = skill.description.to_lowercase().contains(&query);

            if !name_hit && !desc_hit {
                return None;
            }

            let score = if name_hit && desc_hit {
                1.0
            } else if name_hit {
                0.9
            } else {
                0.7
            };

            Some(serde_json::json!({
                "name": skill.name,
                "description": skill.description,
                "version": skill.version,
                "score": score,
                "match_type": if name_hit { "name" } else { "description" }
            }))
        })
        .collect();

    results.sort_by(|a, b| {
        let a_score = a.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let b_score = b.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
        b_score
            .partial_cmp(&a_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(limit);

    Json(serde_json::json!({
        "status": "success",
        "query": payload.query,
        "results": results
    }))
}

/// Return a simple "top skills" view derived from the loaded registry.
pub async fn get_skills_sh_top(
    State(gateway): State<Arc<CrabletGateway>>,
) -> Json<serde_json::Value> {
    let registry = gateway.router.shared_skills.read().await;
    let mut skills = registry.list_skills();
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills.truncate(10);

    Json(serde_json::json!({
        "status": "success",
        "skills": skills
    }))
}

/// Batch-test skills by checking whether they are currently loaded.
pub async fn batch_test_skills(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(payload): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let requested = payload
        .get("skills")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let registry = gateway.router.shared_skills.read().await;
    let results: Vec<serde_json::Value> = requested
        .into_iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .map(|name| {
            let loaded = registry.get_skill(&name).is_some();
            serde_json::json!({
                "name": name,
                "loaded": loaded,
                "passed": loaded
            })
        })
        .collect();

    Json(serde_json::json!({
        "status": "success",
        "results": results
    }))
}

/// Summarize MCP resources and prompts exposed through the shared registry.
pub async fn get_mcp_overview(
    State(gateway): State<Arc<CrabletGateway>>,
) -> Json<serde_json::Value> {
    let registry = gateway.router.shared_skills.read().await;
    let resources = registry.list_resources();
    let prompts = registry.list_prompts();

    Json(serde_json::json!({
        "status": "success",
        "resources_count": resources.len(),
        "prompts_count": prompts.len(),
        "resources": resources,
        "prompts": prompts
    }))
}

/// Skill execution logs are not wired into persistent storage yet.
pub async fn get_skill_logs(Path(name): Path<String>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "success",
        "skill": name,
        "logs": [],
        "message": "Skill execution logs are not persisted yet."
    }))
}

/// Aggregated skill logs placeholder.
pub async fn get_all_skill_logs() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "success",
        "logs": [],
        "message": "Skill execution logs are not persisted yet."
    }))
}
