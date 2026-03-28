//! Chat Enhancement API handlers (Phase 3)
//!
//! Handles token usage, message starring, dual search, and TopK recommendations.

use std::sync::Arc;
use axum::{
    extract::{State, Json, Path, Query},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use crate::gateway::server::CrabletGateway;

// ============================================================================
// Token Usage & Session Context APIs
// ============================================================================

#[derive(Deserialize)]
pub struct StarRequest {
    pub message_id: String,
}

/// Get token usage statistics for a session
pub async fn get_token_usage(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(session_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let usage = gateway.storage.session_context.get_token_usage(&session_id).await;

    match usage {
        Ok(Some(token_usage)) => Ok(Json(serde_json::json!({
            "status": "success",
            "session_id": token_usage.session_id,
            "total_tokens": token_usage.total_tokens,
            "prompt_tokens": token_usage.prompt_tokens,
            "completion_tokens": token_usage.completion_tokens,
            "token_limit": token_usage.token_limit,
            "usage_percentage": token_usage.usage_percentage,
            "last_updated": token_usage.last_updated
        }))),
        Ok(None) => Ok(Json(serde_json::json!({
            "status": "not_found",
            "session_id": session_id,
            "message": "Session not found or no token data available"
        }))),
        Err(e) => {
            tracing::error!("Failed to get token usage: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Star (favorite) a message
pub async fn star_message(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(session_id): Path<String>,
    Json(req): Json<StarRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match gateway.storage.message_stars.star_message(&session_id, &req.message_id).await {
        Ok(Some(star)) => Ok(Json(serde_json::json!({
            "status": "starred",
            "id": star.id,
            "session_id": star.session_id,
            "message_id": star.message_id,
            "created_at": star.created_at
        }))),
        Ok(None) => Ok(Json(serde_json::json!({
            "status": "already_starred",
            "message": "Message is already starred"
        }))),
        Err(e) => {
            tracing::error!("Failed to star message: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Unstar (unfavorite) a message
pub async fn unstar_message(
    State(gateway): State<Arc<CrabletGateway>>,
    Path((session_id, message_id)): Path<(String, String)>,
) -> Result<StatusCode, StatusCode> {
    match gateway.storage.message_stars.unstar_message(&session_id, &message_id).await {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Ok(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("Failed to unstar message: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// List all starred messages for a session
pub async fn list_stars(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(session_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match gateway.storage.message_stars.list_stars(&session_id).await {
        Ok(stars) => Ok(Json(serde_json::json!({
            "status": "success",
            "session_id": session_id,
            "count": stars.len(),
            "stars": stars
        }))),
        Err(e) => {
            tracing::error!("Failed to list stars: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Check if a message is starred
pub async fn is_starred(
    State(gateway): State<Arc<CrabletGateway>>,
    Path((session_id, message_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match gateway.storage.message_stars.is_starred(&session_id, &message_id).await {
        Ok(starred) => Ok(Json(serde_json::json!({
            "status": "success",
            "session_id": session_id,
            "message_id": message_id,
            "starred": starred
        }))),
        Err(e) => {
            tracing::error!("Failed to check star status: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get star count for a session
pub async fn get_star_count(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(session_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match gateway.storage.message_stars.get_star_count(&session_id).await {
        Ok(count) => Ok(Json(serde_json::json!({
            "status": "success",
            "session_id": session_id,
            "star_count": count
        }))),
        Err(e) => {
            tracing::error!("Failed to get star count: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// ============================================================================
// RAG + History Dual Search API
// ============================================================================

#[derive(Deserialize)]
pub struct DualSearchQuery {
    pub q: String,
    pub mode: Option<String>,
    pub alpha: Option<f32>,
    pub limit: Option<usize>,
}

#[derive(Clone, Serialize)]
pub struct DualSearchResult {
    pub source: String,
    pub source_type: String,
    pub content: String,
    pub score: f32,
    pub session_id: Option<String>,
    pub message_id: Option<String>,
}

/// Dual search: RAG knowledge base + history session search with fused scoring
pub async fn dual_search(
    State(gateway): State<Arc<CrabletGateway>>,
    Query(query): Query<DualSearchQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mode = query.mode.as_deref().unwrap_or("dual");
    let alpha = query.alpha.unwrap_or(0.6).clamp(0.0, 1.0);
    let limit = query.limit.unwrap_or(10).min(50);

    #[allow(unused_mut)]
    let mut kb_results: Vec<DualSearchResult> = vec![];
    let mut history_results: Vec<DualSearchResult> = vec![];

    // Knowledge Base Search
    if mode != "history_only" {
        #[cfg(feature = "knowledge")]
        if let Some(ingestion) = &gateway.ingestion {
            if let Ok(results) = ingestion.search(&query.q, limit).await {
                for result in results {
                    let source = result.get("metadata")
                        .and_then(|m| m.get("source"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("kb")
                        .to_string();
                    let content = result.get("content")
                        .and_then(|c| c.as_str())
                        .unwrap_or("")
                        .to_string();
                    let score = result.get("score")
                        .and_then(|s| s.as_f64())
                        .unwrap_or(0.0) as f32;

                    kb_results.push(DualSearchResult {
                        source,
                        source_type: "knowledge_base".to_string(),
                        content: content.chars().take(500).collect(),
                        score,
                        session_id: None,
                        message_id: None,
                    });
                }
            }
        }
        let _ = gateway;
    }

    // History Search
    if mode != "kb_only" {
        if let Ok(results) = gateway.storage.session_context.search_history(&query.q, limit).await {
            for result in results {
                history_results.push(DualSearchResult {
                    source: format!("session:{}", result.session_id),
                    source_type: "history".to_string(),
                    content: result.content_preview,
                    score: result.relevance_score,
                    session_id: Some(result.session_id),
                    message_id: Some(result.message_id),
                });
            }
        }
    }

    // Fuse results if dual mode
    if mode == "dual" && !kb_results.is_empty() && !history_results.is_empty() {
        let max_kb_score = kb_results.iter().map(|r| r.score).fold(0.0f32, f32::max);
        let normalized_kb: Vec<f32> = kb_results.iter().map(|r| {
            if max_kb_score > 0.0 { r.score / max_kb_score } else { r.score }
        }).collect();

        let mut fused: Vec<DualSearchResult> = vec![];

        for (i, result) in kb_results.iter().enumerate() {
            let fused_score = alpha * normalized_kb[i];
            fused.push(DualSearchResult {
                source: result.source.clone(),
                source_type: result.source_type.clone(),
                content: result.content.clone(),
                score: fused_score,
                session_id: result.session_id.clone(),
                message_id: result.message_id.clone(),
            });
        }

        for result in &history_results {
            let fused_score = (1.0 - alpha) * result.score;
            fused.push(DualSearchResult {
                source: result.source.clone(),
                source_type: result.source_type.clone(),
                content: result.content.clone(),
                score: fused_score,
                session_id: result.session_id.clone(),
                message_id: result.message_id.clone(),
            });
        }

        fused.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        fused.truncate(limit);

        let kb_count = kb_results.len();
        let history_count = history_results.len();

        return Ok(Json(serde_json::json!({
            "status": "success",
            "query": query.q,
            "mode": mode,
            "alpha": alpha,
            "kb_count": kb_count,
            "history_count": history_count,
            "results": fused
        })));
    }

    let use_kb = mode == "kb_only" || (mode == "dual" && (kb_results.is_empty() || history_results.is_empty()));
    let history_count = history_results.len();

    let (kb_count, results) = if use_kb {
        let kb_count = kb_results.len();
        let mut results = kb_results;
        results.extend(history_results);
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        (kb_count, results)
    } else {
        let kb_count = 0;
        let mut results: Vec<DualSearchResult> = vec![];
        results.extend(history_results);
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        (kb_count, results)
    };

    Ok(Json(serde_json::json!({
        "status": "success",
        "query": query.q,
        "mode": mode,
        "alpha": alpha,
        "kb_count": kb_count,
        "history_count": history_count,
        "results": results
    })))
}

// ============================================================================
// TopK Dynamic Adjustment API
// ============================================================================

#[derive(Deserialize)]
pub struct TopKRecommendQuery {
    pub session_id: Option<String>,
    pub current_topk: Option<usize>,
}

#[derive(Serialize)]
pub struct TopKRecommendation {
    pub recommended_topk: usize,
    pub reason: String,
    pub token_usage_percentage: f32,
    pub session_token_count: u32,
    pub session_token_limit: u32,
}

/// Recommend optimal TopK value based on current token usage
pub async fn topk_recommend(
    State(gateway): State<Arc<CrabletGateway>>,
    Query(query): Query<TopKRecommendQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    const DEFAULT_TOKEN_LIMIT: u32 = 128000;
    const DEFAULT_TOPK: usize = 10;
    const MIN_TOPK: usize = 3;
    const MAX_TOPK: usize = 20;

    let (token_usage_pct, session_token_count, session_token_limit) = if let Some(ref session_id) = query.session_id {
        if let Ok(Some(usage)) = gateway.storage.session_context.get_token_usage(session_id).await {
            (usage.usage_percentage, usage.total_tokens, usage.token_limit)
        } else {
            (0.0, 0, DEFAULT_TOKEN_LIMIT)
        }
    } else {
        (0.0, 0, DEFAULT_TOKEN_LIMIT)
    };

    let current_topk = query.current_topk.unwrap_or(DEFAULT_TOPK);
    let recommended_topk = if token_usage_pct >= 80.0 {
        MIN_TOPK
    } else if token_usage_pct >= 60.0 {
        (current_topk as f32 * 0.5) as usize
    } else if token_usage_pct >= 40.0 {
        current_topk
    } else if token_usage_pct >= 20.0 {
        (current_topk as f32 * 1.25) as usize
    } else {
        (current_topk as f32 * 1.5) as usize
    };

    let recommended_topk = recommended_topk.clamp(MIN_TOPK, MAX_TOPK);

    let reason = if token_usage_pct >= 80.0 {
        format!("Token usage is critical ({}%). Recommend minimal retrieval to prevent context overflow.", token_usage_pct as u32)
    } else if token_usage_pct >= 60.0 {
        format!("Token usage is high ({}%). Reducing TopK from {} to {} to save context.", token_usage_pct as u32, current_topk, recommended_topk)
    } else if token_usage_pct >= 40.0 {
        format!("Token usage is moderate ({}%). Keeping TopK at {} for balanced retrieval.", token_usage_pct as u32, recommended_topk)
    } else if token_usage_pct >= 20.0 {
        format!("Token usage is low ({}%). Increasing TopK from {} to {} for better results.", token_usage_pct as u32, current_topk, recommended_topk)
    } else {
        format!("Token usage is very low ({}%). Increasing TopK from {} to {} for comprehensive retrieval.", token_usage_pct as u32, current_topk, recommended_topk)
    };

    Ok(Json(serde_json::json!({
        "status": "success",
        "recommended_topk": recommended_topk,
        "current_topk": current_topk,
        "reason": reason,
        "token_usage_percentage": token_usage_pct,
        "session_token_count": session_token_count,
        "session_token_limit": session_token_limit
    })))
}
