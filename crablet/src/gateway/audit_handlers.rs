//! Audit-related web handlers
//!
//! Handles audit logs and security event tracking.

use std::sync::Arc;
use axum::{
    extract::{State, Json, Query},
    http::StatusCode,
};
use serde::Deserialize;

use crate::gateway::server::CrabletGateway;
use crate::audit::AuditLog;

#[derive(Deserialize)]
pub struct LogsQuery {
    page: Option<i64>,
    per_page: Option<i64>,
}

pub async fn list_audit_logs(
    State(gateway): State<Arc<CrabletGateway>>,
    Query(query): Query<LogsQuery>,
) -> Result<Json<Vec<AuditLog>>, StatusCode> {
    if let Some(pool) = &gateway.auth.pool {
        let logger = crate::audit::AuditLogger::new(pool.clone());
        let limit = query.per_page.unwrap_or(50);
        let offset = (query.page.unwrap_or(1) - 1) * limit;

        match logger.list_logs(limit, offset).await {
            Ok(logs) => Ok(Json(logs)),
            Err(e) => {
                tracing::error!("Failed to list logs: {}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}