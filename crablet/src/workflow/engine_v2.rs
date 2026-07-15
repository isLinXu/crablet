use super::executor::NodeExecutorRegistry;
use super::types::{
    next_id, now_rfc3339, ExecuteWorkflowRequest, ExecutionEvent, NodeExecution, Workflow,
    WorkflowEdge, WorkflowExecution, WorkflowNode,
};
use futures::future::join_all;
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct WorkflowEngine {
    executions: Arc<RwLock<HashMap<String, WorkflowExecution>>>,
    executor_registry: Arc<NodeExecutorRegistry>,
    workflows: Arc<RwLock<HashMap<String, Workflow>>>,
}

impl WorkflowEngine {
    pub fn new(
        executor_registry: Arc<NodeExecutorRegistry>,
        workflows: Arc<RwLock<HashMap<String, Workflow>>>,
    ) -> Self {
        Self {
            executions: Arc::new(RwLock::new(HashMap::new())),
            executor_registry,
            workflows,
        }
    }

    /// Convenience constructor: creates an internal, empty workflows store.
    /// Use this when the caller wants to manage workflows via `WorkflowRegistry`
    /// separately and inject the shared store later via `with_workflows`.
    pub fn with_registry(executor_registry: Arc<NodeExecutorRegistry>) -> Self {
        Self::new(executor_registry, Arc::new(RwLock::new(HashMap::new())))
    }

    /// Replace the shared workflows store (e.g. to link with `WorkflowRegistry`).
    pub fn with_workflows(mut self, workflows: Arc<RwLock<HashMap<String, Workflow>>>) -> Self {
        self.workflows = workflows;
        self
    }

    /// Execute a workflow by ID with proper DAG scheduling
    pub async fn execute(
        &self,
        workflow_id: &str,
        req: ExecuteWorkflowRequest,
    ) -> Result<WorkflowExecution, WorkflowEngineError> {
        let workflow = {
            let map = self.workflows.read().await;
            map.get(workflow_id)
                .cloned()
                .ok_or_else(|| WorkflowEngineError::WorkflowNotFound(workflow_id.to_string()))?
        };

        let execution_id = next_id("exec");
        let started = now_rfc3339();

        // Initialize execution record
        let mut execution = WorkflowExecution {
            id: execution_id.clone(),
            workflow_id: workflow_id.to_string(),
            status: "running".to_string(),
            inputs: req.inputs.clone(),
            outputs: None,
            node_executions: Vec::new(),
            started_at: started.clone(),
            completed_at: None,
            error: None,
        };

        {
            let mut map = self.executions.write().await;
            map.insert(execution_id.clone(), execution.clone());
        }

        // Build DAG topology
        let dag = match DagGraph::from_workflow(&workflow) {
            Ok(dag) => dag,
            Err(e) => {
                execution.status = "failed".to_string();
                execution.error = Some(format!("DAG validation failed: {}", e));
                execution.completed_at = Some(now_rfc3339());
                let mut map = self.executions.write().await;
                map.insert(execution_id.clone(), execution.clone());
                return Err(e);
            }
        };

        // Execute DAG
        match self
            .execute_dag(&dag, &workflow, &req.inputs, &execution_id)
            .await
        {
            Ok(outputs) => {
                execution.status = "completed".to_string();
                execution.outputs = Some(outputs);
            }
            Err(e) => {
                execution.status = "failed".to_string();
                execution.error = Some(format!("Execution failed: {}", e));
            }
        }

        execution.completed_at = Some(now_rfc3339());

        // Update final execution state
        {
            let mut map = self.executions.write().await;
            map.insert(execution_id.clone(), execution.clone());
        }

        Ok(execution)
    }

    /// Core DAG execution engine with topological scheduling
    async fn execute_dag(
        &self,
        dag: &DagGraph,
        workflow: &Workflow,
        initial_inputs: &HashMap<String, Value>,
        execution_id: &str,
    ) -> Result<HashMap<String, Value>, WorkflowEngineError> {
        let mut node_outputs: HashMap<String, HashMap<String, Value>> = HashMap::new();
        let mut completed_nodes: HashSet<String> = HashSet::new();
        let mut failed_nodes: HashMap<String, String> = HashMap::new();

        // Seed initial inputs as virtual "workflow_input" node outputs
        let mut workflow_inputs = HashMap::new();
        for (k, v) in initial_inputs {
            workflow_inputs.insert(k.clone(), v.clone());
        }
        node_outputs.insert("workflow_input".to_string(), workflow_inputs);
        completed_nodes.insert("workflow_input".to_string());

        // Kahn's algorithm for topological execution with parallel ready nodes
        let mut in_degree = dag.in_degree.clone();
        let mut ready_queue: VecDeque<String> = VecDeque::new();

        // Find initially ready nodes (in-degree 0, excluding workflow_input virtual node)
        for (node_id, degree) in &in_degree {
            if *degree == 0 && node_id != "workflow_input" {
                ready_queue.push_back(node_id.clone());
            }
        }

        while !ready_queue.is_empty() || completed_nodes.len() < dag.nodes.len() + 1 {
            // Collect all currently ready nodes for potential parallel execution
            let mut batch = Vec::new();
            while let Some(node_id) = ready_queue.pop_front() {
                if !completed_nodes.contains(&node_id) && !failed_nodes.contains_key(&node_id) {
                    batch.push(node_id);
                }
            }

            if batch.is_empty() {
                // Check for deadlock (nodes remaining but none ready)
                let remaining: Vec<String> = dag
                    .nodes
                    .keys()
                    .filter(|id| !completed_nodes.contains(*id) && !failed_nodes.contains_key(*id))
                    .cloned()
                    .collect();

                if remaining.is_empty() {
                    break;
                }

                // Deadlock or cycle detected
                return Err(WorkflowEngineError::DeadlockDetected(remaining.join(", ")));
            }

            // Execute ready batch (parallel execution of independent nodes)
            let mut futures = Vec::new();
            for node_id in &batch {
                let node = workflow
                    .nodes
                    .iter()
                    .find(|n| n.id == *node_id)
                    .cloned()
                    .ok_or_else(|| WorkflowEngineError::NodeNotFound(node_id.clone()))?;

                let inputs = self.resolve_node_inputs(node_id, &node, &node_outputs, dag)?;
                let registry = self.executor_registry.clone();
                let _exec_id = execution_id.to_string();
                let node_id_clone = node_id.clone();

                futures.push(tokio::spawn(async move {
                    let start = now_rfc3339();
                    let result = registry.execute(&node, &inputs).await;
                    let end = now_rfc3339();
                    (node_id_clone, result, start, end)
                }));
            }

            // Wait for all batch executions
            let results = join_all(futures).await;

            for res in results {
                let (node_id, result, started_at, completed_at) = match res {
                    Ok(r) => r,
                    Err(e) => {
                        let msg = format!("Join error: {}", e);
                        warn!("{}", msg);
                        // Cannot determine which node failed from join error in this simple impl
                        continue;
                    }
                };

                let node = workflow
                    .nodes
                    .iter()
                    .find(|n| n.id == node_id)
                    .ok_or_else(|| WorkflowEngineError::NodeNotFound(node_id.clone()))?;

                match result {
                    Ok(outputs) => {
                        info!("Node {} executed successfully", node_id);
                        node_outputs.insert(node_id.clone(), outputs.clone());
                        completed_nodes.insert(node_id.clone());

                        // Record execution
                        let node_exec = NodeExecution {
                            node_id: node_id.clone(),
                            status: "completed".to_string(),
                            inputs: Some(self.resolve_node_inputs(
                                &node_id,
                                node,
                                &node_outputs,
                                dag,
                            )?),
                            outputs: Some(outputs),
                            started_at: Some(started_at),
                            completed_at: Some(completed_at),
                            error: None,
                        };
                        self.append_node_execution(execution_id, node_exec).await;

                        // Update in-degrees and enqueue newly ready nodes
                        if let Some(children) = dag.adjacency.get(&node_id) {
                            for child in children {
                                let deg = in_degree.entry(child.clone()).or_insert(0);
                                *deg = deg.saturating_sub(1);
                                if *deg == 0 && !completed_nodes.contains(child) {
                                    ready_queue.push_back(child.clone());
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Node {} failed: {}", node_id, e);
                        failed_nodes.insert(node_id.clone(), e.to_string());

                        let node_exec = NodeExecution {
                            node_id: node_id.clone(),
                            status: "failed".to_string(),
                            inputs: Some(self.resolve_node_inputs(
                                &node_id,
                                node,
                                &node_outputs,
                                dag,
                            )?),
                            outputs: None,
                            started_at: Some(started_at),
                            completed_at: Some(completed_at),
                            error: Some(e.to_string()),
                        };
                        self.append_node_execution(execution_id, node_exec).await;

                        // Decide failure strategy: fail-fast vs. continue
                        // For now: fail-fast
                        return Err(WorkflowEngineError::NodeExecutionFailed {
                            node_id: node_id.clone(),
                            error: e.to_string(),
                        });
                    }
                }
            }
        }

        // Aggregate final outputs from end nodes (nodes with no outgoing edges)
        let mut final_outputs = HashMap::new();
        for node in &workflow.nodes {
            if dag.adjacency.get(&node.id).map_or(true, |v| v.is_empty()) {
                if let Some(outputs) = node_outputs.get(&node.id) {
                    for (k, v) in outputs {
                        final_outputs.insert(format!("{}.{}", node.id, k), v.clone());
                    }
                }
            }
        }

        // Also include any workflow-level outputs defined in variables
        if let Some(vars) = workflow.variables.get("outputs") {
            if let Some(arr) = vars.as_array() {
                for v in arr {
                    if let Some(name) = v.as_str() {
                        if let Some(val) = initial_inputs.get(name) {
                            final_outputs.insert(name.to_string(), val.clone());
                        }
                    }
                }
            }
        }

        Ok(final_outputs)
    }

    /// Resolve inputs for a node by mapping edge source outputs to target inputs
    fn resolve_node_inputs(
        &self,
        node_id: &str,
        node: &WorkflowNode,
        node_outputs: &HashMap<String, HashMap<String, Value>>,
        dag: &DagGraph,
    ) -> Result<HashMap<String, Value>, WorkflowEngineError> {
        let mut inputs = HashMap::new();

        // Default: inject workflow-level inputs if start node
        if node.node_type == "start" {
            if let Some(start_outputs) = node_outputs.get("workflow_input") {
                for (k, v) in start_outputs {
                    inputs.insert(k.clone(), v.clone());
                }
            }
        }

        // Map connected edges: source.output -> target.input
        for edge in &dag.edges {
            if edge.target == node_id {
                let source_outputs = node_outputs.get(&edge.source).ok_or_else(|| {
                    WorkflowEngineError::DependencyNotReady {
                        node_id: node_id.to_string(),
                        dependency: edge.source.clone(),
                    }
                })?;

                let output_key = edge.source_handle.as_deref().unwrap_or("output");
                let input_key = edge.target_handle.as_deref().unwrap_or("input");

                if let Some(val) = source_outputs.get(output_key) {
                    inputs.insert(input_key.to_string(), val.clone());
                } else if let Some(val) = source_outputs.get("output") {
                    // Fallback to generic output
                    inputs.insert(input_key.to_string(), val.clone());
                }
            }
        }

        // Apply node-specific config defaults
        if let Some(config) = &node.data.config {
            for (k, v) in config {
                if !inputs.contains_key(k) {
                    inputs.insert(k.clone(), v.clone());
                }
            }
        }

        Ok(inputs)
    }

    async fn append_node_execution(&self, execution_id: &str, node_exec: NodeExecution) {
        let mut map = self.executions.write().await;
        if let Some(exec) = map.get_mut(execution_id) {
            exec.node_executions.push(node_exec);
        }
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
            if execution.status == "completed" || execution.status == "failed" {
                return false;
            }
            execution.status = "cancelled".to_string();
            execution.completed_at = Some(now_rfc3339());
            return true;
        }
        false
    }

    pub async fn stream_events(
        &self,
        workflow_id: &str,
        req: ExecuteWorkflowRequest,
    ) -> Result<Vec<ExecutionEvent>, WorkflowEngineError> {
        let execution = self.execute(workflow_id, req).await?;
        let mut events = vec![ExecutionEvent {
            event_type: "started".to_string(),
            execution_id: execution.id.clone(),
            workflow_id: Some(execution.workflow_id.clone()),
            node_id: None,
            node_type: None,
            outputs: None,
            error: None,
            variable: None,
            value: None,
            timestamp: execution.started_at.clone(),
        }];

        for node_exec in &execution.node_executions {
            events.push(ExecutionEvent {
                event_type: node_exec.status.clone(),
                execution_id: execution.id.clone(),
                workflow_id: Some(execution.workflow_id.clone()),
                node_id: Some(node_exec.node_id.clone()),
                node_type: None,
                outputs: node_exec.outputs.clone(),
                error: node_exec.error.clone(),
                variable: None,
                value: None,
                timestamp: node_exec.started_at.clone().unwrap_or_else(now_rfc3339),
            });
        }

        events.push(ExecutionEvent {
            event_type: execution.status.clone(),
            execution_id: execution.id,
            workflow_id: Some(execution.workflow_id),
            node_id: None,
            node_type: None,
            outputs: execution.outputs,
            error: execution.error,
            variable: None,
            value: None,
            timestamp: execution.completed_at.unwrap_or_else(now_rfc3339),
        });

        Ok(events)
    }
}

#[derive(Debug, Clone)]
struct DagGraph {
    nodes: HashMap<String, WorkflowNode>,
    edges: Vec<WorkflowEdge>,
    adjacency: HashMap<String, Vec<String>>,
    in_degree: HashMap<String, usize>,
}

impl DagGraph {
    fn from_workflow(workflow: &Workflow) -> Result<Self, WorkflowEngineError> {
        let mut nodes = HashMap::new();
        let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();

        for node in &workflow.nodes {
            nodes.insert(node.id.clone(), node.clone());
            adjacency.entry(node.id.clone()).or_default();
            in_degree.entry(node.id.clone()).or_insert(0);
        }

        for edge in &workflow.edges {
            if !nodes.contains_key(&edge.source) {
                return Err(WorkflowEngineError::InvalidEdge(format!(
                    "Source node '{}' not found",
                    edge.source
                )));
            }
            if !nodes.contains_key(&edge.target) {
                return Err(WorkflowEngineError::InvalidEdge(format!(
                    "Target node '{}' not found",
                    edge.target
                )));
            }

            adjacency
                .entry(edge.source.clone())
                .or_default()
                .push(edge.target.clone());
            *in_degree.entry(edge.target.clone()).or_insert(0) += 1;
        }

        // Cycle detection (Kahn's algorithm dry-run)
        let mut temp_degree = in_degree.clone();
        let mut queue: VecDeque<String> = temp_degree
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(id, _)| id.clone())
            .collect();
        let mut visited = 0usize;

        while let Some(node_id) = queue.pop_front() {
            visited += 1;
            if let Some(children) = adjacency.get(&node_id) {
                for child in children {
                    if let Some(deg) = temp_degree.get_mut(child) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(child.clone());
                        }
                    }
                }
            }
        }

        if visited != nodes.len() {
            return Err(WorkflowEngineError::CycleDetected);
        }

        Ok(DagGraph {
            nodes,
            edges: workflow.edges.clone(),
            adjacency,
            in_degree,
        })
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum WorkflowEngineError {
    #[error("Workflow not found: {0}")]
    WorkflowNotFound(String),
    #[error("Node not found: {0}")]
    NodeNotFound(String),
    #[error("Dependency not ready for node '{node_id}': {dependency}")]
    DependencyNotReady { node_id: String, dependency: String },
    #[error("Invalid edge: {0}")]
    InvalidEdge(String),
    #[error("Cycle detected in workflow graph")]
    CycleDetected,
    #[error("Deadlock detected: remaining nodes [{0}] have unresolved dependencies")]
    DeadlockDetected(String),
    #[error("Node '{node_id}' execution failed: {error}")]
    NodeExecutionFailed { node_id: String, error: String },
    #[error("Execution registry error: {0}")]
    RegistryError(String),
}

// NOTE: NodeExecutorRegistry must be extended with an execute method:
// impl NodeExecutorRegistry {
//     pub async fn execute(
//         &self,
//         node: &WorkflowNode,
//         inputs: &HashMap<String, Value>,
//     ) -> Result<HashMap<String, Value>, anyhow::Error> { ... }
// }

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_node(id: &str, node_type: &str) -> WorkflowNode {
        WorkflowNode {
            id: id.to_string(),
            node_type: node_type.to_string(),
            position: super::super::types::Position { x: 0.0, y: 0.0 },
            data: super::super::types::WorkflowNodeData {
                label: id.to_string(),
                description: None,
                config: None,
                inputs: None,
                outputs: None,
            },
        }
    }

    fn make_edge(source: &str, target: &str) -> WorkflowEdge {
        WorkflowEdge {
            id: format!("e_{}_{}", source, target),
            source: source.to_string(),
            target: target.to_string(),
            source_handle: None,
            target_handle: None,
            label: None,
            condition: None,
        }
    }

    #[test]
    fn test_dag_from_workflow_valid() {
        let workflow = Workflow {
            id: "wf1".to_string(),
            name: "Test".to_string(),
            description: None,
            nodes: vec![
                make_test_node("a", "start"),
                make_test_node("b", "llm"),
                make_test_node("c", "end"),
            ],
            edges: vec![make_edge("a", "b"), make_edge("b", "c")],
            variables: HashMap::new(),
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
            created_by: None,
            version: 1,
            is_active: true,
        };

        let dag = DagGraph::from_workflow(&workflow).unwrap();
        assert_eq!(dag.nodes.len(), 3);
        assert_eq!(dag.in_degree.get("a"), Some(&0));
        assert_eq!(dag.in_degree.get("b"), Some(&1));
        assert_eq!(dag.in_degree.get("c"), Some(&1));
    }

    #[test]
    fn test_dag_cycle_detection() {
        let workflow = Workflow {
            id: "wf1".to_string(),
            name: "Test".to_string(),
            description: None,
            nodes: vec![
                make_test_node("a", "start"),
                make_test_node("b", "llm"),
                make_test_node("c", "end"),
            ],
            edges: vec![
                make_edge("a", "b"),
                make_edge("b", "c"),
                make_edge("c", "a"),
            ],
            variables: HashMap::new(),
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
            created_by: None,
            version: 1,
            is_active: true,
        };

        let result = DagGraph::from_workflow(&workflow);
        assert!(matches!(result, Err(WorkflowEngineError::CycleDetected)));
    }
}
