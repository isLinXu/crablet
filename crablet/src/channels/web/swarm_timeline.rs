//! Swarm timeline, replay, stats, and tasks handlers for the legacy web API.
//!
//! Contains:
//! - Stats handler (aggregated swarm metrics from DB or memory)
//! - Timeline handler (event log with filtering)
//! - Replay handler (point-in-time graph state reconstruction)
//! - Tasks handler (paginated graph listing)
//! - Helper functions for event mapping and replay logic

use crate::agent::swarm::{GraphStatus, TaskGraph, TaskNode, TaskStatus};
use crate::cognitive::router::CognitiveRouter;
use crate::events::AgentEvent;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Json};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::sync::Arc;

// ---- Query / Response Types ----

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct SwarmTimelineQuery {
    pub limit: Option<i64>,
    pub node_id: Option<String>,
    pub event_type: Option<String>,
    pub message_type: Option<String>,
    pub status: Option<String>,
    pub q: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct SwarmReplayQuery {
    pub at: Option<i64>,
    pub node_id: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct SwarmTasksQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub status: Option<String>,
    pub q: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
pub(crate) struct SwarmTimelineEntry {
    pub timestamp: i64,
    pub graph_id: String,
    pub task_id: Option<String>,
    pub event_type: String,
    pub message_type: String,
    pub status: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub content: String,
}

#[derive(Serialize, Debug, Clone)]
pub(crate) struct SwarmReplaySnapshot {
    pub(crate) graph_id: String,
    pub(crate) goal: String,
    pub(crate) status: String,
    pub(crate) at: i64,
    pub(crate) source: String,
    pub(crate) focus_node_id: Option<String>,
    pub(crate) nodes: std::collections::HashMap<String, TaskNode>,
    pub(crate) timeline_len: usize,
}

#[derive(Serialize)]
pub(crate) struct SwarmGraphResponse {
    pub(crate) id: String,
    #[serde(flatten)]
    pub(crate) graph: TaskGraph,
    pub(crate) running_tasks: usize,
    pub(crate) cancelled_tasks: usize,
    pub(crate) recoverable_tasks: usize,
    pub(crate) is_draining: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) created_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) updated_at: Option<i64>,
}

impl SwarmGraphResponse {
    pub(crate) fn from_graph(
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

// ---- Helper Functions ----

pub(crate) fn swarm_timeline_entry_from_event(event: &AgentEvent) -> Option<SwarmTimelineEntry> {
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

pub(crate) fn normalized_query_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("all"))
        .map(|value| value.to_ascii_lowercase())
}

pub(crate) fn swarm_timeline_entry_matches(
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

pub(crate) fn build_swarm_replay_snapshot(
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

// ---- Handlers ----

pub(crate) async fn swarm_stats_handler(
    State(router): State<Arc<CognitiveRouter>>,
) -> impl IntoResponse {
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

            // Average duration (completed graphs only)
            let avg_duration: f64 = sqlx::query_scalar(
                "SELECT AVG(updated_at - created_at) FROM swarm_graphs WHERE status = 'Completed'",
            )
            .fetch_one(pool)
            .await
            .unwrap_or(0.0);

            (total, active, completed, failed, avg_duration)
        } else {
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

        let mut graph_details = Vec::new();
        for (id, graph) in active_graphs.iter() {
            graph_details.push(serde_json::json!({
                "id": id,
                "status": graph.status.as_str(),
                "task_count": graph.nodes.len(),
                "running_tasks": graph.running_task_count(),
                "pending_tasks": graph.nodes.len() - graph.running_task_count(),
            }));
        }

        return Json(serde_json::json!({
            "status": "success",
            "stats": {
                "total": total,
                "active": active,
                "completed": completed,
                "failed": failed,
                "avg_duration_ms": avg_duration,
            },
            "graphs": graph_details
        }));
    }
    Json(serde_json::json!({ "status": "error", "message": "Orchestrator not initialized" }))
}

pub(crate) async fn swarm_timeline_handler(
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

pub(crate) async fn swarm_replay_handler(
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

pub(crate) async fn swarm_tasks_handler(
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

            let rows = if status_filter == "All" {
                if has_search {
                    sqlx::query(list_sql)
                        .bind(&search_pattern)
                        .bind(limit)
                        .bind(offset)
                        .fetch_all(pool)
                        .await
                } else {
                    sqlx::query(list_sql)
                        .bind(limit)
                        .bind(offset)
                        .fetch_all(pool)
                        .await
                }
            } else if has_search {
                sqlx::query(list_sql)
                    .bind(status_filter)
                    .bind(&search_pattern)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(pool)
                    .await
            } else {
                sqlx::query(list_sql)
                    .bind(status_filter)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(pool)
                    .await
            };

            if let Ok(rows) = rows {
                let mut graphs = Vec::new();
                for row in rows {
                    let id: String = row.get("id");
                    let goal: String = row.get("goal");
                    let created_at: i64 = row.get("created_at");
                    let updated_at: i64 = row.get("updated_at");

                    let active_graphs = orch.coordinator.active_graphs.read().await;
                    if let Some(g) = active_graphs.get(&id) {
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
                                TaskNode {
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

                        let graph_obj = TaskGraph {
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
