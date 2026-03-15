use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
    pub variables: HashMap<String, Value>,
    pub created_at: String,
    pub updated_at: String,
    pub created_by: Option<String>,
    pub version: i32,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowNode {
    pub id: String,
    pub node_type: String,
    pub position: Position,
    pub data: WorkflowNodeData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowNodeData {
    pub label: String,
    pub description: Option<String>,
    pub config: Option<HashMap<String, Value>>,
    pub inputs: Option<Vec<Variable>>,
    pub outputs: Option<Vec<Variable>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub source_handle: Option<String>,
    pub target_handle: Option<String>,
    pub label: Option<String>,
    pub condition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variable {
    pub name: String,
    pub r#type: String,
    pub description: Option<String>,
    pub required: Option<bool>,
    pub default: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWorkflowRequest {
    pub name: String,
    pub description: Option<String>,
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateWorkflowRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub nodes: Option<Vec<WorkflowNode>>,
    pub edges: Option<Vec<WorkflowEdge>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteWorkflowRequest {
    pub inputs: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeTypeDefinition {
    pub r#type: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub icon: String,
    pub color: String,
    pub inputs: Option<Vec<NodePort>>,
    pub outputs: Option<Vec<NodePort>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePort {
    pub name: String,
    pub r#type: String,
    pub optional: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecution {
    pub id: String,
    pub workflow_id: String,
    pub status: String,
    pub inputs: HashMap<String, Value>,
    pub outputs: Option<HashMap<String, Value>>,
    pub node_executions: Vec<NodeExecution>,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeExecution {
    pub node_id: String,
    pub status: String,
    pub inputs: Option<HashMap<String, Value>>,
    pub outputs: Option<HashMap<String, Value>>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEvent {
    pub event_type: String,
    pub execution_id: String,
    pub workflow_id: Option<String>,
    pub node_id: Option<String>,
    pub node_type: Option<String>,
    pub outputs: Option<HashMap<String, Value>>,
    pub error: Option<String>,
    pub variable: Option<String>,
    pub value: Option<Value>,
    pub timestamp: String,
}

pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

pub fn next_id(prefix: &str) -> String {
    format!("{}-{}", prefix, Uuid::new_v4())
}
