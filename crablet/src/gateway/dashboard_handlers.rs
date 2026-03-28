//! Dashboard and stats web handlers
//!
//! Handles dashboard statistics and system health metrics.

use std::sync::Arc;
use axum::{
    extract::State,
    Json,
};

use crate::gateway::server::CrabletGateway;

pub async fn get_dashboard_stats(
    State(gateway): State<Arc<CrabletGateway>>,
) -> Json<serde_json::Value> {
    tracing::info!("Dashboard stats request received");
    let start = std::time::Instant::now();

    let (skills_count, skills_list) = {
        let lock = gateway.router.shared_skills.read().await;
        tracing::info!("Dashboard stats: Got skills lock in {:?}", start.elapsed());
        (lock.len(), lock.list_skills())
    };

    let active_swarms = 3; // Mock consistent with swarm_stats
    tracing::info!("Dashboard stats: Got swarms count in {:?}", start.elapsed());

    let knowledge_nodes = if let Some(_kg) = &gateway.router.sys2.kg {
        142 // Mock for now to avoid graph latency
    } else {
        0
    };

    let stats = serde_json::json!({
        "status": "healthy",
        "skills_count": skills_count,
        "active_tasks": active_swarms,
        "system_load": "Low",
        "skills": skills_list,
        "active_swarms": active_swarms,
        "knowledge_nodes": knowledge_nodes,
        "skills_loaded": skills_count,
        "system_status": "healthy",
        "uptime": 12345 // TODO: Real uptime
    });

    tracing::info!("Dashboard stats: Completed in {:?}", start.elapsed());
    Json(stats)
}