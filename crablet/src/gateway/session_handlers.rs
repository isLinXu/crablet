//! Session-related web handlers
//!
//! Handles session management, history, and deletion.

use std::sync::Arc;
use axum::{
    extract::{State, Json, Path},
    http::StatusCode,
};
use chrono::Utc;
use serde::Deserialize;
use sqlx::Row;

use crate::gateway::server::CrabletGateway;

#[derive(Deserialize)]
pub struct CompressRequest {
    pub keep_recent: Option<usize>,
}

pub async fn delete_session(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(session_id): Path<String>,
) -> StatusCode {
    gateway.session.remove_session(&session_id);

    if let Some(memory) = &gateway.router.memory_mgr.episodic {
        let pool = &memory.pool;

        if let Err(err) = sqlx::query("DELETE FROM messages WHERE session_id = ?")
            .bind(&session_id)
            .execute(pool)
            .await
        {
            tracing::error!("Failed to delete session messages for {}: {}", session_id, err);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }

        if let Err(err) = sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(&session_id)
            .execute(pool)
            .await
        {
            tracing::error!("Failed to delete session {}: {}", session_id, err);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }

        if let Err(err) = sqlx::query("DELETE FROM message_stars WHERE session_id = ?")
            .bind(&session_id)
            .execute(pool)
            .await
        {
            tracing::warn!("Failed to delete message stars for {}: {}", session_id, err);
        }
    }

    if let Err(err) = gateway.storage.session_context.delete_context(&session_id).await {
        tracing::warn!("Failed to delete session context for {}: {}", session_id, err);
    }

    StatusCode::NO_CONTENT
}

pub async fn get_session_history(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(session_id): Path<String>,
) -> Json<serde_json::Value> {
    if let Some(memory) = &gateway.router.memory_mgr.episodic {
        match memory.get_history(&session_id, 200).await {
            Ok(history) => return Json(serde_json::json!(history)),
            Err(err) => {
                tracing::warn!("Failed to read session history for {}: {}", session_id, err);
            }
        }
    }

    if let Some(history) = gateway.session.get_history(&session_id) {
        Json(serde_json::json!(history))
    } else {
        Json(serde_json::json!([]))
    }
}

pub async fn list_sessions(
    State(gateway): State<Arc<CrabletGateway>>,
) -> Json<serde_json::Value> {
    if let Some(memory) = &gateway.router.memory_mgr.episodic {
        match sqlx::query(
            "SELECT id, created_at, last_active, message_count
             FROM sessions
             ORDER BY last_active DESC
             LIMIT 100"
        )
        .fetch_all(&memory.pool)
        .await
        {
            Ok(rows) => {
                let sessions: Vec<serde_json::Value> = rows
                    .into_iter()
                    .map(|row| {
                        let id: String = row.get("id");
                        let created_at_ts: i64 = row.get("created_at");
                        let updated_at_ts: i64 = row.get("last_active");
                        let message_count: i64 = row.get("message_count");
                        let title = format!("Session {}", id.chars().take(8).collect::<String>());

                        serde_json::json!({
                            "id": id,
                            "title": title,
                            "created_at": chrono::DateTime::from_timestamp(created_at_ts, 0)
                                .map(|ts| ts.to_rfc3339())
                                .unwrap_or_else(|| Utc::now().to_rfc3339()),
                            "updated_at": chrono::DateTime::from_timestamp(updated_at_ts, 0)
                                .map(|ts| ts.to_rfc3339())
                                .unwrap_or_else(|| Utc::now().to_rfc3339()),
                            "message_count": message_count,
                        })
                    })
                    .collect();

                return Json(serde_json::json!(sessions));
            }
            Err(err) => {
                tracing::warn!("Failed to list sessions from SQLite: {}", err);
            }
        }
    }

    let now = Utc::now().to_rfc3339();
    Json(serde_json::json!([
        {
            "id": "mock-session-1",
            "title": "演示会话",
            "created_at": now,
            "updated_at": now
        }
    ]))
}

/// Compress session context to reduce token usage
pub async fn compress_session(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(session_id): Path<String>,
    Json(req): Json<CompressRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let keep_recent = req.keep_recent.unwrap_or(10);

    match gateway.storage.session_context.compress_context(&session_id, keep_recent).await {
        Ok(true) => Ok(Json(serde_json::json!({
            "status": "success",
            "session_id": session_id,
            "compressed": true,
            "kept_messages": keep_recent
        }))),
        Ok(false) => Ok(Json(serde_json::json!({
            "status": "no_change",
            "session_id": session_id,
            "message": "Session was not modified (already compressed or fewer messages than keep_recent)"
        }))),
        Err(e) => {
            tracing::error!("Failed to compress session: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
