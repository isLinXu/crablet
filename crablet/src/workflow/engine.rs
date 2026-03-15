use super::executor::NodeExecutorRegistry;
use super::types::{ExecutionEvent, ExecuteWorkflowRequest, NodeExecution, WorkflowExecution, next_id, now_rfc3339};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct WorkflowEngine {
    executions: Arc<RwLock<HashMap<String, WorkflowExecution>>>,
    _executor_registry: Arc<NodeExecutorRegistry>,
}

impl WorkflowEngine {
    pub fn new(executor_registry: Arc<NodeExecutorRegistry>) -> Self {
        Self {
            executions: Arc::new(RwLock::new(HashMap::new())),
            _executor_registry: executor_registry,
        }
    }

    pub async fn execute(&self, workflow_id: &str, req: ExecuteWorkflowRequest) -> WorkflowExecution {
        let id = next_id("exec");
        let started = now_rfc3339();
        let mut outputs: HashMap<String, Value> = HashMap::new();
        outputs.insert("status".to_string(), Value::String("ok".to_string()));
        let execution = WorkflowExecution {
            id: id.clone(),
            workflow_id: workflow_id.to_string(),
            status: "completed".to_string(),
            inputs: req.inputs,
            outputs: Some(outputs),
            node_executions: vec![NodeExecution {
                node_id: "start".to_string(),
                status: "completed".to_string(),
                inputs: None,
                outputs: None,
                started_at: Some(started.clone()),
                completed_at: Some(now_rfc3339()),
                error: None,
            }],
            started_at: started,
            completed_at: Some(now_rfc3339()),
            error: None,
        };
        let mut map = self.executions.write().await;
        map.insert(id, execution.clone());
        execution
    }

    pub async fn get_execution(&self, execution_id: &str) -> Option<WorkflowExecution> {
        let map = self.executions.read().await;
        map.get(execution_id).cloned()
    }

    pub async fn list_executions(&self, workflow_id: &str) -> Vec<WorkflowExecution> {
        let map = self.executions.read().await;
        map.values()
            .filter(|x| x.workflow_id == workflow_id)
            .cloned()
            .collect()
    }

    pub async fn cancel_execution(&self, execution_id: &str) -> bool {
        let mut map = self.executions.write().await;
        if let Some(execution) = map.get_mut(execution_id) {
            if execution.status == "completed" {
                return false;
            }
            execution.status = "cancelled".to_string();
            execution.completed_at = Some(now_rfc3339());
            return true;
        }
        false
    }

    pub async fn stream_events(&self, workflow_id: &str, req: ExecuteWorkflowRequest) -> Vec<ExecutionEvent> {
        let execution = self.execute(workflow_id, req).await;
        vec![
            ExecutionEvent {
                event_type: "started".to_string(),
                execution_id: execution.id.clone(),
                workflow_id: Some(execution.workflow_id.clone()),
                node_id: None,
                node_type: None,
                outputs: None,
                error: None,
                variable: None,
                value: None,
                timestamp: now_rfc3339(),
            },
            ExecutionEvent {
                event_type: "completed".to_string(),
                execution_id: execution.id,
                workflow_id: Some(execution.workflow_id),
                node_id: None,
                node_type: None,
                outputs: execution.outputs,
                error: None,
                variable: None,
                value: None,
                timestamp: now_rfc3339(),
            },
        ]
    }
}
