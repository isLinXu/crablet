//! Routing configuration web handlers
//!
//! Handles router configuration, routing reports, and system settings.

use std::sync::Arc;
use std::fs;
use axum::{
    extract::{State, Json, Query, Path},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use crate::gateway::server::CrabletGateway;
use crate::cognitive::router::RouterConfig;
use crate::gateway::auth::ApiKeyInfo;

use super::handlers_shared::resolve_env_file_path;

#[derive(Deserialize)]
pub struct CreateKeyRequest {
    name: String,
}

#[derive(Deserialize)]
pub struct UpdateRoutingSettingsRequest {
    pub enable_adaptive_routing: bool,
    pub system2_threshold: f32,
    pub system3_threshold: f32,
    pub bandit_exploration: f32,
    pub enable_hierarchical_reasoning: bool,
    pub deliberate_threshold: f32,
    pub meta_reasoning_threshold: f32,
    pub mcts_simulations: u32,
    pub mcts_exploration_weight: f32,
    pub graph_rag_entity_mode: String,
}

#[derive(Serialize)]
pub struct RoutingSettingsResponse {
    pub enable_adaptive_routing: bool,
    pub system2_threshold: f32,
    pub system3_threshold: f32,
    pub bandit_exploration: f32,
    pub enable_hierarchical_reasoning: bool,
    pub deliberate_threshold: f32,
    pub meta_reasoning_threshold: f32,
    pub mcts_simulations: u32,
    pub mcts_exploration_weight: f32,
    pub graph_rag_entity_mode: String,
}

#[derive(Deserialize)]
pub struct RoutingReportQuery {
    pub window: Option<usize>,
}

pub async fn get_routing_settings(
    State(gateway): State<Arc<CrabletGateway>>,
) -> Result<Json<RoutingSettingsResponse>, StatusCode> {
    let cfg = gateway.router.config.read().await.clone();
    Ok(Json(RoutingSettingsResponse {
        enable_adaptive_routing: cfg.enable_adaptive_routing,
        system2_threshold: cfg.system2_threshold,
        system3_threshold: cfg.system3_threshold,
        bandit_exploration: cfg.bandit_exploration,
        enable_hierarchical_reasoning: cfg.enable_hierarchical_reasoning,
        deliberate_threshold: cfg.deliberate_threshold,
        meta_reasoning_threshold: cfg.meta_reasoning_threshold,
        mcts_simulations: cfg.mcts_simulations,
        mcts_exploration_weight: cfg.mcts_exploration_weight,
        graph_rag_entity_mode: cfg.graph_rag_entity_mode.clone(),
    }))
}

pub async fn update_routing_settings(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(req): Json<UpdateRoutingSettingsRequest>,
) -> Result<Json<RoutingSettingsResponse>, StatusCode> {
    if !(0.0..=1.0).contains(&req.system2_threshold) || !(0.0..=1.0).contains(&req.system3_threshold) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !(0.05..=2.0).contains(&req.bandit_exploration) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !(0.0..=1.0).contains(&req.deliberate_threshold) || !(0.0..=1.0).contains(&req.meta_reasoning_threshold) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if req.mcts_simulations == 0 || req.mcts_simulations > 512 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !(0.1..=3.0).contains(&req.mcts_exploration_weight) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mode = req.graph_rag_entity_mode.to_lowercase();
    if mode != "rule" && mode != "phrase" && mode != "hybrid" {
        return Err(StatusCode::BAD_REQUEST);
    }
    let new_cfg = RouterConfig {
        system2_threshold: req.system2_threshold,
        system3_threshold: req.system3_threshold,
        enable_adaptive_routing: req.enable_adaptive_routing,
        bandit_exploration: req.bandit_exploration,
        enable_hierarchical_reasoning: req.enable_hierarchical_reasoning,
        deliberate_threshold: req.deliberate_threshold,
        meta_reasoning_threshold: req.meta_reasoning_threshold,
        mcts_simulations: req.mcts_simulations,
        mcts_exploration_weight: req.mcts_exploration_weight,
        graph_rag_entity_mode: mode,
    };
    gateway.router.update_config(new_cfg).await;
    let cfg = gateway.router.config.read().await.clone();
    Ok(Json(RoutingSettingsResponse {
        enable_adaptive_routing: cfg.enable_adaptive_routing,
        system2_threshold: cfg.system2_threshold,
        system3_threshold: cfg.system3_threshold,
        bandit_exploration: cfg.bandit_exploration,
        enable_hierarchical_reasoning: cfg.enable_hierarchical_reasoning,
        deliberate_threshold: cfg.deliberate_threshold,
        meta_reasoning_threshold: cfg.meta_reasoning_threshold,
        mcts_simulations: cfg.mcts_simulations,
        mcts_exploration_weight: cfg.mcts_exploration_weight,
        graph_rag_entity_mode: cfg.graph_rag_entity_mode.clone(),
    }))
}

pub async fn get_routing_report(
    State(gateway): State<Arc<CrabletGateway>>,
    Query(query): Query<RoutingReportQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let window = query.window.unwrap_or(200).clamp(10, 2000);
    let meta = gateway.router.meta_router.read().await;
    let report = meta.evaluation_report(window);
    drop(meta);
    let cloud = gateway.router.sys2.hierarchical_stats().await;
    let local = gateway.router.sys2_local.hierarchical_stats().await;
    let cfg = gateway.router.config.read().await.clone();
    let hierarchical_stats = serde_json::json!({
        "total_requests": cloud.total_requests + local.total_requests,
        "deliberate_activations": cloud.deliberate_activations + local.deliberate_activations,
        "meta_activations": cloud.meta_activations + local.meta_activations,
        "strategy_switches": cloud.strategy_switches + local.strategy_switches,
        "bfs_runs": cloud.bfs_runs + local.bfs_runs,
        "dfs_runs": cloud.dfs_runs + local.dfs_runs,
        "mcts_runs": cloud.mcts_runs + local.mcts_runs
    });
    Ok(Json(serde_json::json!({
        "total_feedback": report.total_feedback,
        "avg_reward": report.avg_reward,
        "avg_latency_ms": report.avg_latency_ms,
        "avg_quality_score": report.avg_quality_score,
        "recent_window": report.recent_window,
        "by_choice": report.by_choice,
        "hierarchical": {
            "enabled": cfg.enable_hierarchical_reasoning,
            "deliberate_threshold": cfg.deliberate_threshold,
            "meta_reasoning_threshold": cfg.meta_reasoning_threshold,
            "mcts_simulations": cfg.mcts_simulations,
            "mcts_exploration_weight": cfg.mcts_exploration_weight
        },
        "hierarchical_stats": hierarchical_stats
    })))
}

// ============================================================================
// System Configuration
// ============================================================================

#[derive(Serialize, Deserialize)]
pub struct SystemConfigPayload {
    pub openai_api_key: Option<String>,
    pub openai_api_base: Option<String>,
    pub openai_model_name: Option<String>,
    pub ollama_model: Option<String>,
    pub llm_vendor: Option<String>,
}

pub async fn get_system_config(
    State(_gateway): State<Arc<CrabletGateway>>,
) -> Result<Json<SystemConfigPayload>, StatusCode> {
    let content = fs::read_to_string(resolve_env_file_path()).unwrap_or_default();
    let mut config = SystemConfigPayload {
        openai_api_key: None,
        openai_api_base: None,
        openai_model_name: None,
        ollama_model: None,
        llm_vendor: None,
    };

    for line in content.lines() {
        if let Some((key, value)) = line.split_once('=') {
            let val = value.trim().to_string();
            match key.trim() {
                "DASHSCOPE_API_KEY" => config.openai_api_key = Some(val),
                "OPENAI_API_KEY" => {
                    if config.openai_api_key.is_none() {
                        config.openai_api_key = Some(val);
                    }
                },
                "OPENAI_API_BASE" => config.openai_api_base = Some(val),
                "OPENAI_MODEL_NAME" => config.openai_model_name = Some(val),
                "OLLAMA_MODEL" => config.ollama_model = Some(val),
                "LLM_VENDOR" => config.llm_vendor = Some(val),
                _ => {}
            }
        }
    }

    Ok(Json(config))
}

pub async fn update_system_config(
    State(_gateway): State<Arc<CrabletGateway>>,
    Json(payload): Json<SystemConfigPayload>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let path = resolve_env_file_path();
    let content = fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

    let mut upsert = |key: &str, value: &str| {
        let mut found = false;
        for line in lines.iter_mut() {
            if line.starts_with(&format!("{}=", key)) {
                *line = format!("{}={}", key, value);
                found = true;
                break;
            }
        }
        if !found {
            lines.push(format!("{}={}", key, value));
        }
    };

    if let Some(v) = payload.openai_api_key {
        upsert("DASHSCOPE_API_KEY", &v);
        upsert("OPENAI_API_KEY", &v);
    }
    if let Some(v) = payload.openai_api_base {
        upsert("OPENAI_API_BASE", &v);
    }
    if let Some(v) = payload.openai_model_name {
        upsert("OPENAI_MODEL_NAME", &v);
    }
    if let Some(v) = payload.ollama_model {
        upsert("OLLAMA_MODEL", &v);
    }
    if let Some(v) = payload.llm_vendor {
        upsert("LLM_VENDOR", &v);
    }

    let new_content = lines.join("\n");
    let final_content = if new_content.ends_with('\n') { new_content } else { new_content + "\n" };

    fs::write(&path, final_content).map_err(|e| {
        tracing::error!("Failed to write .env: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(serde_json::json!({
        "status": "success",
        "message": "Configuration saved. Please restart the service to apply changes."
    })))
}

// ============================================================================
// API Keys Management
// ============================================================================

pub async fn list_api_keys(
    State(gateway): State<Arc<CrabletGateway>>,
) -> Result<Json<Vec<ApiKeyInfo>>, StatusCode> {
    match gateway.auth.list_api_keys().await {
        Ok(keys) => Ok(Json(keys)),
        Err(e) => {
            tracing::error!("Failed to list keys: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn create_api_key(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(req): Json<CreateKeyRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match gateway.auth.create_api_key(&req.name, "admin").await {
        Ok(key) => Ok(Json(serde_json::json!({
            "status": "created",
            "key": key
        }))),
        Err(e) => {
            tracing::error!("Failed to create key: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn revoke_api_key(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    match gateway.auth.revoke_api_key(&id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => {
            tracing::error!("Failed to revoke key: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}