use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::sse::{Event, Sse},
};
use futures::stream;
use std::convert::Infallible;
use std::sync::Arc;

use super::server::CrabletGateway;
use crate::workflow::types::{CreateWorkflowRequest, ExecuteWorkflowRequest, UpdateWorkflowRequest, ValidationResult};

pub async fn create_workflow(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(payload): Json<CreateWorkflowRequest>,
) -> Result<Json<crate::workflow::types::Workflow>, StatusCode> {
    let workflow = gateway.workflow_registry.create(payload).await;
    Ok(Json(workflow))
}

pub async fn list_workflows(
    State(gateway): State<Arc<CrabletGateway>>,
) -> Result<Json<Vec<crate::workflow::types::Workflow>>, StatusCode> {
    Ok(Json(gateway.workflow_registry.list().await))
}

pub async fn get_workflow(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(id): Path<String>,
) -> Result<Json<crate::workflow::types::Workflow>, StatusCode> {
    let workflow = gateway.workflow_registry.get(&id).await.ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(workflow))
}

pub async fn update_workflow(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateWorkflowRequest>,
) -> Result<Json<crate::workflow::types::Workflow>, StatusCode> {
    let workflow = gateway.workflow_registry.update(&id, payload).await.ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(workflow))
}

pub async fn delete_workflow(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    if gateway.workflow_registry.delete(&id).await {
        return Ok(StatusCode::NO_CONTENT);
    }
    Err(StatusCode::NOT_FOUND)
}

pub async fn execute_workflow(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(id): Path<String>,
    Json(payload): Json<ExecuteWorkflowRequest>,
) -> Result<Json<crate::workflow::types::WorkflowExecution>, StatusCode> {
    if gateway.workflow_registry.get(&id).await.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }
    let execution = gateway.workflow_engine.execute(&id, payload).await;
    Ok(Json(execution))
}

pub async fn run_workflow_stream(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(id): Path<String>,
    Json(payload): Json<ExecuteWorkflowRequest>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    if gateway.workflow_registry.get(&id).await.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }
    let events = gateway.workflow_engine.stream_events(&id, payload).await;
    let s = stream::iter(events.into_iter().map(|evt| {
        let data = serde_json::to_string(&evt).unwrap_or_else(|_| "{}".to_string());
        Ok::<Event, Infallible>(Event::default().data(data))
    }));
    Ok(Sse::new(s))
}

pub async fn list_executions(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(workflow_id): Path<String>,
) -> Result<Json<Vec<crate::workflow::types::WorkflowExecution>>, StatusCode> {
    Ok(Json(gateway.workflow_engine.list_executions(&workflow_id).await))
}

pub async fn get_execution(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(id): Path<String>,
) -> Result<Json<crate::workflow::types::WorkflowExecution>, StatusCode> {
    let execution = gateway.workflow_engine.get_execution(&id).await.ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(execution))
}

pub async fn cancel_execution(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    if gateway.workflow_engine.cancel_execution(&id).await {
        return Ok(StatusCode::NO_CONTENT);
    }
    Err(StatusCode::NOT_FOUND)
}

pub async fn validate_workflow(
    Json(payload): Json<CreateWorkflowRequest>,
) -> Result<Json<ValidationResult>, StatusCode> {
    Ok(Json(crate::workflow::registry::WorkflowRegistry::validate(&payload)))
}

pub async fn get_node_types() -> Result<Json<Vec<crate::workflow::types::NodeTypeDefinition>>, StatusCode> {
    Ok(Json(crate::workflow::registry::WorkflowRegistry::node_types()))
}
