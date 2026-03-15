//! Observability API Handlers
//!
//! Provides HTTP endpoints for the observability system:
//! - Trace sessions management
//! - Breakpoint control
//! - Execution replay
//! - Real-time event streaming

use axum::{
    extract::{Path, State, Query},
    response::{Json, sse::{Event, KeepAlive, Sse}},
    http::StatusCode,
};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use futures::stream::Stream;
use axum::BoxError;

use crate::gateway::server::CrabletGateway;
use crate::observability::{
    BreakpointCondition, BreakpointAction,
    TraceSession, AgentSpan, SessionStatus
};

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ListSessionsQuery {
    pub workflow_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct SessionsResponse {
    pub sessions: Vec<SessionSummary>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct SessionSummary {
    pub execution_id: String,
    pub workflow_id: String,
    pub status: String,
    pub started_at: u64,
    pub completed_at: Option<u64>,
    pub step_count: usize,
}

#[derive(Debug, Serialize)]
pub struct SessionDetail {
    pub session: TraceSession,
    pub spans: Vec<AgentSpan>,
}

#[derive(Debug, Deserialize)]
pub struct CreateBreakpointRequest {
    pub workflow_id: Option<String>,
    pub condition: BreakpointConditionRequest,
    pub action: BreakpointActionRequest,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum BreakpointConditionRequest {
    OnStep { count: usize },
    LowConfidence { threshold: f32 },
    LoopDetected,
    ThoughtContains { text: String },
    ExecutionTimeExceeded { max_duration_ms: u64 },
    TokenBudgetExceeded { max_tokens: usize },
    OnError { recoverable_only: bool },
    All { conditions: Vec<BreakpointConditionRequest> },
    Any { conditions: Vec<BreakpointConditionRequest> },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum BreakpointActionRequest {
    Pause,
    Continue,
    ModifyContext { variable_updates: std::collections::HashMap<String, serde_json::Value> },
    InjectHint { hint: String },
    Skip,
}

#[derive(Debug, Serialize)]
pub struct BreakpointResponse {
    pub id: String,
    pub workflow_id: Option<String>,
    pub condition: serde_json::Value,
    pub action: serde_json::Value,
    pub created_at: u64,
}

#[derive(Debug, Deserialize)]
pub struct HumanInterventionRequest {
    pub decision: String, // "continue", "modify", "abort"
    pub modifications: Option<serde_json::Value>,
    pub feedback: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MetricsResponse {
    pub execution_id: String,
    pub total_steps: usize,
    pub total_tokens: usize,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub estimated_cost: f64,
    pub duration_ms: u64,
    pub step_metrics: Vec<StepMetricDetail>,
}

#[derive(Debug, Serialize)]
pub struct StepMetricDetail {
    pub step_number: usize,
    pub duration_ms: u64,
    pub tokens_used: usize,
    pub llm_calls: usize,
}

// ============================================================================
// Handlers
// ============================================================================

/// List all trace sessions
pub async fn list_sessions(
    State(gateway): State<Arc<CrabletGateway>>,
    Query(query): Query<ListSessionsQuery>,
) -> Result<Json<SessionsResponse>, StatusCode> {
    // Get sessions from System2's observability manager
    let sys2 = &gateway.router.sys2;
    let tracer_arc = sys2.observability.tracer();
    let tracer = tracer_arc.read().await;
    
    let sessions: Vec<SessionSummary> = tracer
        .list_sessions()
        .into_iter()
        .filter(|s| {
            if let Some(ref workflow_id) = query.workflow_id {
                s.workflow_id == *workflow_id
            } else {
                true
            }
        })
        .filter(|s| {
            if let Some(ref status) = query.status {
                format!("{:?}", s.status).to_lowercase() == status.to_lowercase()
            } else {
                true
            }
        })
        .map(|s| SessionSummary {
            execution_id: s.execution_id,
            workflow_id: s.workflow_id,
            status: format!("{:?}", s.status),
            started_at: s.started_at,
            completed_at: s.ended_at,
            step_count: s.spans.len(),
        })
        .take(query.limit.unwrap_or(100))
        .collect();
    
    let total = sessions.len();
    
    Ok(Json(SessionsResponse { sessions, total }))
}

/// Get a specific session with all spans
pub async fn get_session(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(execution_id): Path<String>,
) -> Result<Json<SessionDetail>, StatusCode> {
    let sys2 = &gateway.router.sys2;
    let tracer_arc = sys2.observability.tracer();
    let tracer = tracer_arc.read().await;
    
    let session = tracer
        .get_session(&execution_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    
    let spans = tracer.get_spans(&execution_id).await.unwrap_or_default();
    
    Ok(Json(SessionDetail { session, spans }))
}

/// Delete a session
pub async fn delete_session(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(execution_id): Path<String>,
) -> StatusCode {
    let sys2 = &gateway.router.sys2;
    let tracer_arc = sys2.observability.tracer();
    let mut tracer = tracer_arc.write().await;
    
    tracer.delete_session(&execution_id);
    StatusCode::NO_CONTENT
}

/// Stream events for a session
pub async fn stream_session_events(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(execution_id): Path<String>,
) -> Sse<impl Stream<Item = Result<Event, BoxError>>> {
    let rx = gateway.router.sys2.observability.subscribe();
    
    let stream = BroadcastStream::new(rx)
        .filter_map(move |msg| {
            match msg {
                Ok(event) => {
                    // Filter events for this execution
                    let event_json = serde_json::to_value(&event).ok()?;
                    let event_execution_id = event_json.get("execution_id")?.as_str()?;
                    
                    if event_execution_id == execution_id {
                        let data = serde_json::to_string(&event).ok()?;
                        Some(Ok(Event::default().data(data)))
                    } else {
                        None
                    }
                }
                Err(_) => Some(Ok(Event::default().comment("missed message"))),
            }
        });
    
    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// List all breakpoints
pub async fn list_breakpoints(
    State(gateway): State<Arc<CrabletGateway>>,
) -> Json<Vec<BreakpointResponse>> {
    let sys2 = &gateway.router.sys2;
    let bp_manager_arc = sys2.observability.breakpoint_manager();
    let bp_manager = bp_manager_arc.read().await;
    
    let breakpoints_list = bp_manager.list_breakpoints().await;
    
    let breakpoints: Vec<BreakpointResponse> = breakpoints_list
        .into_iter()
        .map(|(id, bp)| BreakpointResponse {
            id,
            workflow_id: bp.workflow_id,
            condition: serde_json::to_value(&bp.condition).unwrap_or_default(),
            action: serde_json::to_value(&bp.action).unwrap_or_default(),
            created_at: bp.created_at,
        })
        .collect();
    
    Json(breakpoints)
}

/// Create a new breakpoint
pub async fn create_breakpoint(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(req): Json<CreateBreakpointRequest>,
) -> Result<Json<BreakpointResponse>, StatusCode> {
    let condition = convert_condition_request(req.condition)?;
    let action = convert_action_request(req.action)?;

    let sys2 = &gateway.router.sys2;
    let bp_manager_arc = sys2.observability.breakpoint_manager();
    let bp_manager = bp_manager_arc.write().await;

    let bp = crate::observability::Breakpoint::new(condition).with_action(action);
    let id = bp_manager.set_breakpoint(bp.clone()).await;

    Ok(Json(BreakpointResponse {
        id,
        workflow_id: bp.workflow_id,
        condition: serde_json::to_value(&bp.condition).unwrap_or_default(),
        action: serde_json::to_value(&bp.action).unwrap_or_default(),
        created_at: bp.created_at,
    }))
}

/// Delete a breakpoint
pub async fn delete_breakpoint(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(id): Path<String>,
) -> StatusCode {
    let sys2 = &gateway.router.sys2;
    let bp_manager_arc = sys2.observability.breakpoint_manager();
    let bp_manager = bp_manager_arc.read().await;
    
    bp_manager.remove_breakpoint(&id).await;
    StatusCode::NO_CONTENT
}

/// Get paused sessions waiting for human intervention
pub async fn get_paused_sessions(
    State(gateway): State<Arc<CrabletGateway>>,
) -> Json<Vec<SessionSummary>> {
    let sys2 = &gateway.router.sys2;
    let tracer_arc = sys2.observability.tracer();
    let tracer = tracer_arc.read().await;
    
    let sessions: Vec<SessionSummary> = tracer
        .list_sessions()
        .into_iter()
        .filter(|s| matches!(s.status, SessionStatus::Paused))
        .map(|s| SessionSummary {
            execution_id: s.execution_id.clone(),
            workflow_id: s.workflow_id.clone(),
            status: format!("{:?}", s.status),
            started_at: s.started_at,
            completed_at: s.ended_at,
            step_count: s.spans.len(),
        })
        .collect();
    
    Json(sessions)
}

/// Provide human intervention for a paused session
pub async fn intervene(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(execution_id): Path<String>,
    Json(req): Json<HumanInterventionRequest>,
) -> Result<StatusCode, StatusCode> {
    let sys2 = &gateway.router.sys2;
    let tracer_arc = sys2.observability.tracer();
    let mut tracer = tracer_arc.write().await;
    
    // Check if session exists and is paused
    let session = tracer
        .get_session(&execution_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    
    if !matches!(session.status, SessionStatus::Paused) {
        return Err(StatusCode::CONFLICT);
    }
    
    // Handle the intervention based on decision type
    match req.decision.as_str() {
        "continue" => {
            tracer.resume_session(&execution_id);
        }
        "modify" => {
            // Apply modifications if provided
            if let Some(mods) = req.modifications {
                tracer.modify_session_context(&execution_id, mods);
            }
            tracer.resume_session(&execution_id);
        }
        "abort" => {
            tracer.abort_session(&execution_id);
        }
        _ => return Err(StatusCode::BAD_REQUEST),
    }
    
    Ok(StatusCode::OK)
}

/// Get metrics for a session
pub async fn get_metrics(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(execution_id): Path<String>,
) -> Result<Json<MetricsResponse>, StatusCode> {
    let sys2 = &gateway.router.sys2;
    let tracer_arc = sys2.observability.tracer();
    let tracer = tracer_arc.read().await;
    
    let session = tracer
        .get_session(&execution_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    
    let metrics = session.metrics;
    
    let step_metrics: Vec<StepMetricDetail> = metrics
        .step_metrics
        .into_iter()
        .map(|m| StepMetricDetail {
            step_number: m.step_number,
            duration_ms: m.duration_ms,
            tokens_used: m.token_usage.total,
            llm_calls: m.llm_calls,
        })
        .collect();
    
    Ok(Json(MetricsResponse {
        execution_id: execution_id.clone(),
        total_steps: metrics.total_steps,
        total_tokens: metrics.total_tokens,
        prompt_tokens: metrics.prompt_tokens,
        completion_tokens: metrics.completion_tokens,
        estimated_cost: metrics.estimated_cost,
        duration_ms: metrics.duration_ms,
        step_metrics,
    }))
}

/// Stream all observability events
pub async fn stream_events(
    State(gateway): State<Arc<CrabletGateway>>,
) -> Sse<impl Stream<Item = Result<Event, BoxError>>> {
    let rx = gateway.router.sys2.observability.subscribe();
    
    let stream = BroadcastStream::new(rx).map(|msg| {
        match msg {
            Ok(event) => {
                let data = serde_json::to_string(&event).unwrap_or_default();
                Ok(Event::default().data(data))
            }
            Err(_) => Ok(Event::default().comment("missed message")),
        }
    });
    
    Sse::new(stream).keep_alive(KeepAlive::default())
}

// ============================================================================
// Helper Functions
// ============================================================================

fn convert_condition_request(req: BreakpointConditionRequest) -> Result<BreakpointCondition, StatusCode> {
    match req {
        BreakpointConditionRequest::OnStep { count } => Ok(BreakpointCondition::OnStep { count }),
        BreakpointConditionRequest::LowConfidence { threshold } => Ok(BreakpointCondition::LowConfidence { threshold }),
        BreakpointConditionRequest::LoopDetected => Ok(BreakpointCondition::LoopDetected),
        BreakpointConditionRequest::ThoughtContains { text } => Ok(BreakpointCondition::ThoughtContains { text }),
        BreakpointConditionRequest::ExecutionTimeExceeded { max_duration_ms } => Ok(BreakpointCondition::ExecutionTimeExceeded { max_duration_ms }),
        BreakpointConditionRequest::TokenBudgetExceeded { max_tokens } => Ok(BreakpointCondition::TokenBudgetExceeded { max_tokens }),
        BreakpointConditionRequest::OnError { recoverable_only } => Ok(BreakpointCondition::OnError { recoverable_only }),
        BreakpointConditionRequest::All { conditions } => {
            let converted: Result<Vec<_>, _> = conditions.into_iter()
                .map(convert_condition_request)
                .collect();
            Ok(BreakpointCondition::All(converted?))
        }
        BreakpointConditionRequest::Any { conditions } => {
            let converted: Result<Vec<_>, _> = conditions.into_iter()
                .map(convert_condition_request)
                .collect();
            Ok(BreakpointCondition::Any(converted?))
        }
    }
}

fn convert_action_request(req: BreakpointActionRequest) -> Result<BreakpointAction, StatusCode> {
    match req {
        BreakpointActionRequest::Pause => Ok(BreakpointAction::Pause),
        BreakpointActionRequest::Continue => Ok(BreakpointAction::Continue),
        BreakpointActionRequest::ModifyContext { variable_updates } => Ok(BreakpointAction::ModifyContext { variable_updates }),
        BreakpointActionRequest::InjectHint { hint } => Ok(BreakpointAction::InjectHint { hint }),
        BreakpointActionRequest::Skip => Ok(BreakpointAction::Skip),
    }
}
