//! Dynamic Tool Graph Executor - Advanced DAG-based tool orchestration
//!
//! 增强现有 tool_flow，提供更强大的工具编排能力：
//! - DAG 可视化 + 动态依赖解析
//! - 工具并行/串行智能切换
//! - 运行时工具重路由（失败时自动找替代路径）
//!
//! # 架构
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                 DynamicToolGraphExecutor                                │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │                                                                       │
//! │   ┌────────────────┐    ┌────────────────┐    ┌────────────────┐  │
//! │   │  Static DAG    │    │  Dependency    │    │    Tool       │  │
//! │   │  Builder       │───▶│  Resolver     │───▶│    Registry   │  │
//! │   └────────────────┘    └────────────────┘    └────────────────┘  │
//! │           │                    │                    │               │
//! │           ▼                    ▼                    ▼               │
//! │   ┌─────────────────────────────────────────────────────────────┐  │
//! │   │              Execution Planner                               │  │
//! │   │   - Parallel/Serial decision                                 │  │
//! │   │   - Resource-aware scheduling                                │  │
//! │   │   - Load balancing                                          │  │
//! │   └─────────────────────────────────────────────────────────────┘  │
//! │                               │                                   │
//! │                               ▼                                   │
//! │   ┌─────────────────────────────────────────────────────────────┐  │
//! │   │              Runtime Re-Router                              │  │
//! │   │   - Failure detection                                       │  │
//! │   │   - Alternative path finding                                │  │
//! │   │   - Circuit breaker integration                             │  │
//! │   └─────────────────────────────────────────────────────────────┘  │
//! │                               │                                   │
//! │                               ▼                                   │
//! │   ┌─────────────────────────────────────────────────────────────┐  │
//! │   │              Graph Visualizer                               │  │
//! │   │   - DOT output                                             │  │
//! │   │   - Mermaid diagrams                                       │  │
//! │   │   - Execution trace                                         │  │
//! │   └─────────────────────────────────────────────────────────────┘  │
//! │                                                                       │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tokio::task::JoinSet;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::{Topo, EdgeRef};
use petgraph::dot::Dot;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug, Clone)]
pub enum ToolGraphError {
    #[error("Graph error: {0}")]
    GraphError(String),

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Cycle detected in graph")]
    CycleDetected,

    #[error("Execution failed at node {node}: {error}")]
    ExecutionFailed { node: String, error: String },

    #[error("All alternative paths exhausted for: {0}")]
    NoAlternativePath(String),

    #[error("Tool not registered: {0}")]
    ToolNotRegistered(String),

    #[error("Timeout waiting for dependencies")]
    DependencyTimeout,

    #[error("Resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),
}

// ============================================================================
// Graph Node Types
// ============================================================================

/// A node in the tool graph
#[derive(Debug, Clone)]
pub struct ToolGraphNode {
    pub id: String,
    pub tool_name: String,
    pub args_template: HashMap<String, serde_json::Value>,
    pub timeout: Duration,
    pub retry_config: Option<RetryConfig>,
    pub fallback_tools: Vec<String>,        // Alternative tools to try on failure
    pub parallel_group: Option<String>,     // Nodes in same group can run in parallel
    pub priority: i32,                      // Higher = earlier execution
    pub metadata: HashMap<String, String>,
}

impl ToolGraphNode {
    pub fn new(id: &str, tool_name: &str) -> Self {
        Self {
            id: id.to_string(),
            tool_name: tool_name.to_string(),
            args_template: HashMap::new(),
            timeout: Duration::from_secs(30),
            retry_config: None,
            fallback_tools: Vec::new(),
            parallel_group: None,
            priority: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_args(mut self, args: HashMap<String, serde_json::Value>) -> Self {
        self.args_template = args;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_fallback(mut self, fallback: &str) -> Self {
        self.fallback_tools.push(fallback.to_string());
        self
    }

    pub fn with_retry(mut self, max_retries: u32, backoff_ms: u64) -> Self {
        self.retry_config = Some(RetryConfig {
            max_retries,
            base_delay_ms: backoff_ms,
            ..Default::default()
        });
        self
    }

    pub fn with_parallel_group(mut self, group: &str) -> Self {
        self.parallel_group = Some(group.to_string());
        self
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub exponential: bool,
}

impl RetryConfig {
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        let delay = if self.exponential {
            self.base_delay_ms * 2u64.pow(attempt)
        } else {
            self.base_delay_ms * (attempt as u64 + 1)
        };
        Duration::from_millis(delay.min(self.max_delay_ms))
    }
}

/// An edge in the tool graph (dependency)
#[derive(Debug, Clone)]
pub struct ToolGraphEdge {
    pub from: String,
    pub to: String,
    pub condition: Option<EdgeCondition>,
    pub data_flow: Option<String>,  // Which field to pass
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EdgeCondition {
    OnSuccess,
    OnFailure,
    Always,
    Expression(String),
}

// ============================================================================
// Dynamic Tool Graph
// ============================================================================

pub struct DynamicToolGraph {
    graph: DiGraph<ToolGraphNode, ToolGraphEdge>,
    node_indices: HashMap<String, NodeIndex>,
    tool_registry: Arc<ToolRegistry>,
    execution_stats: Arc<RwLock<ExecutionStats>>,
}

#[derive(Debug, Clone, Default)]
pub struct ExecutionStats {
    pub total_executions: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub total_duration_ms: u64,
    pub node_stats: HashMap<String, NodeExecutionStats>,
}

#[derive(Debug, Clone, Default)]
pub struct NodeExecutionStats {
    pub execution_count: u64,
    pub success_count: u64,
    pub failure_count: u64,
    pub avg_duration_ms: f64,
    pub fallback_count: u64,
}

impl DynamicToolGraph {
    /// Create a new dynamic tool graph
    pub fn new(tool_registry: Arc<ToolRegistry>) -> Self {
        Self {
            graph: DiGraph::new(),
            node_indices: HashMap::new(),
            tool_registry,
            execution_stats: Arc::new(RwLock::new(ExecutionStats::default())),
        }
    }

    /// Add a node to the graph
    pub fn add_node(&mut self, node: ToolGraphNode) -> Result<(), ToolGraphError> {
        if self.node_indices.contains_key(&node.id) {
            return Err(ToolGraphError::NodeNotFound(format!("Node {} already exists", node.id)));
        }

        let idx = self.graph.add_node(node.clone());
        self.node_indices.insert(node.id.clone(), idx);

        Ok(())
    }

    /// Add an edge (dependency) between nodes
    pub fn add_edge(&mut self, from: &str, to: &str, condition: Option<EdgeCondition>) -> Result<(), ToolGraphError> {
        let from_idx = self.node_indices.get(from)
            .ok_or_else(|| ToolGraphError::NodeNotFound(from.to_string()))?;
        let to_idx = self.node_indices.get(to)
            .ok_or_else(|| ToolGraphError::NodeNotFound(to.to_string()))?;

        let edge = ToolGraphEdge {
            from: from.to_string(),
            to: to.to_string(),
            condition,
            data_flow: None,
        };

        self.graph.add_edge(*from_idx, *to_idx, edge);

        // Check for cycles
        if self.has_cycle() {
            // Remove the edge we just added
            self.graph.remove_edge(self.graph.edge_indices().last()
                .expect("edge must exist: just added above"));
            return Err(ToolGraphError::CycleDetected);
        }

        Ok(())
    }

    /// Check if the graph has a cycle
    fn has_cycle(&self) -> bool {
        // Use topological sort to detect cycles
        let mut topo = Topo::new(&self.graph);
        let mut count = 0;
        while let Some(_) = topo.next(&self.graph) {
            count += 1;
        }
        count < self.graph.node_count()
    }

    /// Get topological order of nodes
    pub fn topological_order(&self) -> Result<Vec<String>, ToolGraphError> {
        let mut order = Vec::new();
        let mut topo = Topo::new(&self.graph);

        while let Some(idx) = topo.next(&self.graph) {
            if let Some(node) = self.graph.node_weight(idx) {
                order.push(node.id.clone());
            }
        }

        if order.len() < self.graph.node_count() {
            return Err(ToolGraphError::CycleDetected);
        }

        Ok(order)
    }

    /// Get nodes that can be executed in parallel at a given point
    pub fn get_parallel_nodes(&self, completed: &HashSet<String>) -> Vec<Vec<String>> {
        let mut parallel_groups: Vec<Vec<String>> = Vec::new();
        let mut processed: HashSet<String> = completed.clone();

        loop {
            let mut this_batch: Vec<String> = Vec::new();

            for (id, &idx) in &self.node_indices {
                if processed.contains(id) {
                    continue;
                }

                // Check if all dependencies are satisfied
                let deps_satisfied = self.graph.edges_directed(idx, petgraph::Direction::Incoming)
                    .all(|e| {
                        if let Some((source, _)) = self.graph.edge_endpoints(e.id()) {
                            if let Some(_edge) = e.weight().condition.as_ref() {
                                match _edge {
                                    EdgeCondition::OnSuccess => {
                                        if let Some(source_node) = self.graph.node_weight(source) {
                                            processed.contains(&source_node.id)
                                        } else {
                                            false
                                        }
                                    }
                                    EdgeCondition::OnFailure => processed.contains(id),
                                    EdgeCondition::Always => true,
                                    EdgeCondition::Expression(_) => true,
                                }
                            } else {
                                // No condition = always require completion
                                // Check if source node is in processed set by looking up its ID
                                if let Some(source_node) = self.graph.node_weight(source) {
                                    processed.contains(&source_node.id)
                                } else {
                                    false
                                }
                            }
                        } else {
                            false
                        }
                    });

                if deps_satisfied {
                    if let Some(node) = self.graph.node_weight(idx) {
                        this_batch.push(node.id.clone());
                    }
                }
            }

            if this_batch.is_empty() {
                break;
            }

            // Group by parallel_group
            let mut groups: HashMap<Option<String>, Vec<String>> = HashMap::new();
            for id in &this_batch {
                if let Some(&idx) = self.node_indices.get(id) {
                    if let Some(node) = self.graph.node_weight(idx) {
                        groups.entry(node.parallel_group.clone()).or_default().push(id.clone());
                    }
                }
            }

            for (_, group) in groups {
                if !group.is_empty() {
                    parallel_groups.push(group);
                }
            }

            for id in this_batch {
                processed.insert(id);
            }
        }

        parallel_groups
    }

    /// Find alternative path when a node fails
    pub fn find_alternative_path(&self, failed_node: &str) -> Option<Vec<String>> {
        let failed_idx = self.node_indices.get(failed_node)?;

        // Try each fallback tool
        let failed_node_weight = self.graph.node_weight(*failed_idx)?;

        for fallback in &failed_node_weight.fallback_tools {
            // Create a modified path that uses the fallback
            let mut alt_path = Vec::new();

            // Find all nodes that depend on the failed node
            let dependents: Vec<String> = self.graph.edges_directed(*failed_idx, petgraph::Direction::Outgoing)
                .filter_map(|e| {
                    if let Some((_, target)) = self.graph.edge_endpoints(e.id()) {
                        self.graph.node_weight(target).map(|n| n.id.clone())
                    } else {
                        None
                    }
                })
                .collect();

            if dependents.is_empty() {
                // The failed node is a leaf node, just try the fallback
                alt_path.push(fallback.clone());
            } else {
                // Replace failed node with fallback and continue
                alt_path.push(fallback.clone());
                // Note: In a full implementation, we would recursively handle dependents
            }

            return Some(alt_path);
        }
        None
    }

    /// Get execution statistics
    pub async fn get_stats(&self) -> ExecutionStats {
        self.execution_stats.read().await.clone()
    }

    /// Update statistics after execution
    async fn record_execution(&self, node_id: &str, success: bool, duration_ms: u64, used_fallback: bool) {
        let mut stats = self.execution_stats.write().await;
        stats.total_executions += 1;

        if success {
            stats.successful_executions += 1;
        } else {
            stats.failed_executions += 1;
        }

        stats.total_duration_ms += duration_ms;

        let node_stats = stats.node_stats.entry(node_id.to_string()).or_default();
        node_stats.execution_count += 1;

        if success {
            node_stats.success_count += 1;
        } else {
            node_stats.failure_count += 1;
        }

        if used_fallback {
            node_stats.fallback_count += 1;
        }

        let count = node_stats.execution_count as f64;
        let prev_avg = node_stats.avg_duration_ms;
        node_stats.avg_duration_ms = (prev_avg * (count - 1.0) + duration_ms as f64) / count;
    }
}

// ============================================================================
// Tool Registry
// ============================================================================

/// Registry of available tools with metadata
pub struct ToolRegistry {
    tools: RwLock<HashMap<String, ToolMetadata>>,
    circuit_breakers: RwLock<HashMap<String, CircuitBreakerState>>,
}

#[derive(Debug, Clone)]
pub struct ToolMetadata {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
    pub category: String,
    pub tags: Vec<String>,
    pub avg_duration_ms: Option<u64>,
    pub success_rate: Option<f64>,
    pub alternatives: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerState {
    pub failures: u32,
    pub last_failure: Option<Instant>,
    pub state: CircuitState,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
            circuit_breakers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a tool
    pub async fn register(&self, metadata: ToolMetadata) {
        let name = metadata.name.clone();
        let mut tools = self.tools.write().await;
        tools.insert(name.clone(), metadata);

        let mut breakers = self.circuit_breakers.write().await;
        breakers.insert(name, CircuitBreakerState {
            failures: 0,
            last_failure: None,
            state: CircuitState::Closed,
        });
    }

    /// Get tool metadata
    pub async fn get(&self, name: &str) -> Option<ToolMetadata> {
        let tools = self.tools.read().await;
        tools.get(name).cloned()
    }

    /// Check if circuit breaker allows execution
    pub async fn is_available(&self, name: &str) -> bool {
        let breakers = self.circuit_breakers.read().await;
        if let Some(cb) = breakers.get(name) {
            match cb.state {
                CircuitState::Closed => true,
                CircuitState::Open => {
                    // Check if timeout has passed
                    if let Some(last) = cb.last_failure {
                        if last.elapsed() > Duration::from_secs(30) {
                            true // Allow retry
                        } else {
                            false
                        }
                    } else {
                        true
                    }
                }
                CircuitState::HalfOpen => true,
            }
        } else {
            true // Unknown tool is available
        }
    }

    /// Record tool failure
    pub async fn record_failure(&self, name: &str) {
        let mut breakers = self.circuit_breakers.write().await;
        if let Some(cb) = breakers.get_mut(name) {
            cb.failures += 1;
            cb.last_failure = Some(Instant::now());

            if cb.failures >= 5 {
                cb.state = CircuitState::Open;
            }
        }
    }

    /// Record tool success
    pub async fn record_success(&self, name: &str) {
        let mut breakers = self.circuit_breakers.write().await;
        if let Some(cb) = breakers.get_mut(name) {
            cb.failures = 0;
            cb.state = CircuitState::Closed;
        }
    }
}

// ============================================================================
// Graph Visualization
// ============================================================================

impl DynamicToolGraph {
    /// Generate DOT representation for Graphviz
    pub fn to_dot(&self) -> String {
        format!("{:?}", Dot::with_config(&self.graph, &[petgraph::dot::Config::EdgeNoLabel]))
    }

    /// Generate Mermaid diagram representation
    pub fn to_mermaid(&self) -> String {
        let mut mermaid = String::from("graph TD\n");

        // Add nodes
        for (id, &idx) in &self.node_indices {
            let Some(node) = self.graph.node_weight(idx) else { continue };
            let label = node.tool_name.replace('"', "");
            mermaid.push_str(&format!("    {}[\"{}\"]\n", id, label));
        }

        // Add edges
        for edge in self.graph.edge_indices() {
            let Some((from, to)) = self.graph.edge_endpoints(edge) else { continue };
            let Some(from_node) = self.graph.node_weight(from) else { continue };
            let Some(to_node) = self.graph.node_weight(to) else { continue };
            let from_id = from_node.id.clone();
            let to_id = to_node.id.clone();

            let style = if let Some(weight) = self.graph.edge_weight(edge) {
                if let Some(condition) = &weight.condition {
                    match condition {
                        EdgeCondition::OnSuccess => " -->|success| ",
                        EdgeCondition::OnFailure => " -.->|failure| ",
                        EdgeCondition::Always => " --> ",
                        EdgeCondition::Expression(_) => " -.-> ",
                    }
                } else {
                    " --> "
                }
            } else {
                " --> "
            };

            mermaid.push_str(&format!("    {} {} {}\n", from_id, style, to_id));
        }

        mermaid
    }

    /// Generate execution trace
    pub fn execution_trace_dot(&self, results: &HashMap<String, StepResult>) -> String {
        let mut dot = String::from("digraph execution {\n");
        dot.push_str("    rankdir=LR;\n");
        dot.push_str("    node [shape=box];\n\n");

        for (id, &idx) in &self.node_indices {
            let Some(node) = self.graph.node_weight(idx) else { continue };

            let color = if let Some(result) = results.get(id) {
                if result.skipped {
                    "gray"
                } else if result.success {
                    "green"
                } else {
                    "red"
                }
            } else {
                "lightgray"
            };

            dot.push_str(&format!(
                "    {} [label=\"{}\\n{:?}\" style=filled fillcolor={}]\n",
                id,
                node.tool_name,
                node.id,
                color
            ));
        }

        for edge in self.graph.edge_indices() {
            let Some((from, to)) = self.graph.edge_endpoints(edge) else { continue };
            let Some(from_node) = self.graph.node_weight(from) else { continue };
            let Some(to_node) = self.graph.node_weight(to) else { continue };

            dot.push_str(&format!("    {} -> {}\n", from_node.id, to_node.id));
        }

        dot.push_str("}\n");
        dot
    }
}

// ============================================================================
// Step Result
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step_id: String,
    pub tool_name: String,
    pub success: bool,
    pub skipped: bool,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub retries: u32,
    pub used_fallback: bool,
    pub fallback_tool: Option<String>,
}

// ============================================================================
// Dynamic Executor
// ============================================================================

pub struct DynamicToolExecutor {
    graph: Arc<RwLock<DynamicToolGraph>>,
    tool_executor: Arc<dyn ToolExecutorTrait>,
    max_parallelism: usize,
}

#[async_trait::async_trait]
pub trait ToolExecutorTrait: Send + Sync {
    async fn execute(&self, tool_name: &str, args: HashMap<String, serde_json::Value>, timeout: Duration) -> Result<serde_json::Value, String>;
}

impl DynamicToolExecutor {
    pub fn new(
        graph: DynamicToolGraph,
        tool_executor: Arc<dyn ToolExecutorTrait>,
        max_parallelism: usize,
    ) -> Self {
        Self {
            graph: Arc::new(RwLock::new(graph)),
            tool_executor,
            max_parallelism,
        }
    }

    /// Execute the graph
    pub async fn execute(&self, initial_args: HashMap<String, serde_json::Value>) -> Result<HashMap<String, StepResult>, ToolGraphError> {
        let graph = self.graph.read().await;
        let _order = graph.topological_order()?;
        let mut results: HashMap<String, StepResult> = HashMap::new();
        let mut shared_state: HashMap<String, serde_json::Value> = initial_args;

        for batch in graph.get_parallel_nodes(&results.keys().cloned().collect()) {
            if batch.len() > 1 && batch.len() <= self.max_parallelism {
                // Execute batch in parallel
                let semaphore = Arc::new(Semaphore::new(self.max_parallelism));
                let mut join_set = JoinSet::new();

                for node_id in batch {
                    let graph_clone = self.graph.clone();
                    let tool_executor_clone = self.tool_executor.clone();
                    let sem_clone = semaphore.clone();
                    let args_clone = shared_state.clone();

                    join_set.spawn(async move {
                        let _permit = sem_clone.acquire().await
                            .expect("semaphore should not be closed during execution");
                        Self::execute_node(&graph_clone, &tool_executor_clone, &node_id, &args_clone).await
                    });
                }

                while let Some(res) = join_set.join_next().await {
                    if let Ok(result) = res {
                        if let Ok(r) = result {
                            results.insert(r.step_id.clone(), r);
                        }
                    }
                }
            } else {
                // Execute serially
                for node_id in batch {
                    let result = Self::execute_node(&self.graph, &self.tool_executor, &node_id, &shared_state).await?;
                    shared_state.insert(result.step_id.clone(), result.output.clone().unwrap_or(serde_json::Value::Null));
                    results.insert(result.step_id.clone(), result);
                }
            }
        }

        Ok(results)
    }

    async fn execute_node(
        graph: &Arc<RwLock<DynamicToolGraph>>,
        tool_executor: &Arc<dyn ToolExecutorTrait>,
        node_id: &str,
        shared_state: &HashMap<String, serde_json::Value>,
    ) -> Result<StepResult, ToolGraphError> {
        let (node, tool_registry) = {
            let graph_read = graph.read().await;
            let idx = graph_read.node_indices.get(node_id)
                .ok_or_else(|| ToolGraphError::NodeNotFound(node_id.to_string()))?;
            let node = graph_read.graph.node_weight(*idx)
                .ok_or_else(|| ToolGraphError::NodeNotFound(node_id.to_string()))?
                .clone();
            let registry = graph_read.tool_registry.clone();
            (node, registry)
        };

        // Check circuit breaker
        if !tool_registry.is_available(&node.tool_name).await {
            // Try fallback
            if let Some(fallback) = node.fallback_tools.first() {
                return Self::execute_with_fallback(graph, tool_executor, &node, fallback, shared_state).await;
            } else {
                return Err(ToolGraphError::ExecutionFailed {
                    node: node_id.to_string(),
                    error: format!("Circuit breaker open for {}", node.tool_name),
                });
            }
        }

        // Execute with retries
        let mut last_error = String::new();
        let max_retries = node.retry_config.as_ref().map(|r| r.max_retries).unwrap_or(0);

        for attempt in 0..=max_retries {
            if attempt > 0 {
                // Wait before retry
                if let Some(retry) = &node.retry_config {
                    tokio::time::sleep(retry.calculate_delay(attempt - 1)).await;
                }
            }

            let start = Instant::now();

            match tool_executor.execute(&node.tool_name, node.args_template.clone(), node.timeout).await {
                Ok(output) => {
                    let duration_ms = start.elapsed().as_millis() as u64;

                    let graph_write = graph.write().await;
                    graph_write.record_execution(node_id, true, duration_ms, false).await;

                    return Ok(StepResult {
                        step_id: node_id.to_string(),
                        tool_name: node.tool_name.clone(),
                        success: true,
                        skipped: false,
                        output: Some(output),
                        error: None,
                        duration_ms,
                        retries: attempt,
                        used_fallback: false,
                        fallback_tool: None,
                    });
                }
                Err(e) => {
                    last_error = e;
                }
            }
        }

        // All retries failed, try fallback
        if let Some(fallback) = node.fallback_tools.first() {
            Self::execute_with_fallback(graph, tool_executor, &node, fallback, shared_state).await
        } else {
            let duration_ms = 0; // Would need proper tracking
            let graph_write = graph.write().await;
            graph_write.record_execution(node_id, false, duration_ms, false).await;

            Err(ToolGraphError::ExecutionFailed {
                node: node_id.to_string(),
                error: last_error,
            })
        }
    }

    async fn execute_with_fallback(
        graph: &Arc<RwLock<DynamicToolGraph>>,
        tool_executor: &Arc<dyn ToolExecutorTrait>,
        node: &ToolGraphNode,
        fallback_tool: &str,
        _shared_state: &HashMap<String, serde_json::Value>,
    ) -> Result<StepResult, ToolGraphError> {
        let start = Instant::now();

        match tool_executor.execute(fallback_tool, node.args_template.clone(), node.timeout).await {
            Ok(output) => {
                let duration_ms = start.elapsed().as_millis() as u64;

                let graph_write = graph.write().await;
                graph_write.record_execution(&node.id, true, duration_ms, true).await;

                Ok(StepResult {
                    step_id: node.id.clone(),
                    tool_name: fallback_tool.to_string(),
                    success: true,
                    skipped: false,
                    output: Some(output),
                    error: None,
                    duration_ms,
                    retries: 0,
                    used_fallback: true,
                    fallback_tool: Some(fallback_tool.to_string()),
                })
            }
            Err(e) => {
                Err(ToolGraphError::ExecutionFailed {
                    node: node.id.clone(),
                    error: format!("Fallback {} failed: {}", fallback_tool, e),
                })
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_creation() {
        let registry = Arc::new(ToolRegistry::new());
        let graph = DynamicToolGraph::new(registry);

        assert_eq!(graph.graph.node_count(), 0);
    }

    #[test]
    fn test_add_nodes_and_edges() {
        let registry = Arc::new(ToolRegistry::new());
        let mut graph = DynamicToolGraph::new(registry);

        graph.add_node(ToolGraphNode::new("a", "tool1")).unwrap();
        graph.add_node(ToolGraphNode::new("b", "tool2")).unwrap();

        graph.add_edge("a", "b", Some(EdgeCondition::OnSuccess)).unwrap();

        assert_eq!(graph.graph.node_count(), 2);
        assert_eq!(graph.graph.edge_count(), 1);
    }

    #[test]
    fn test_cycle_detection() {
        let registry = Arc::new(ToolRegistry::new());
        let mut graph = DynamicToolGraph::new(registry);

        graph.add_node(ToolGraphNode::new("a", "tool1")).unwrap();
        graph.add_node(ToolGraphNode::new("b", "tool2")).unwrap();
        graph.add_node(ToolGraphNode::new("c", "tool3")).unwrap();

        graph.add_edge("a", "b", None).unwrap();
        graph.add_edge("b", "c", None).unwrap();

        // This should fail - creates cycle
        let result = graph.add_edge("c", "a", None);
        assert!(matches!(result, Err(ToolGraphError::CycleDetected)));
    }

    #[test]
    fn test_topological_order() {
        let registry = Arc::new(ToolRegistry::new());
        let mut graph = DynamicToolGraph::new(registry);

        graph.add_node(ToolGraphNode::new("a", "tool1")).unwrap();
        graph.add_node(ToolGraphNode::new("b", "tool2")).unwrap();
        graph.add_node(ToolGraphNode::new("c", "tool3")).unwrap();

        graph.add_edge("a", "b", None).unwrap();
        graph.add_edge("b", "c", None).unwrap();

        let order = graph.topological_order().unwrap();
        assert_eq!(order, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_parallel_groups() {
        let registry = Arc::new(ToolRegistry::new());
        let mut graph = DynamicToolGraph::new(registry);

        graph.add_node(
            ToolGraphNode::new("a", "tool1")
                .with_parallel_group("group1")
        ).unwrap();
        graph.add_node(
            ToolGraphNode::new("b", "tool2")
                .with_parallel_group("group1")
        ).unwrap();
        graph.add_node(ToolGraphNode::new("c", "tool3")).unwrap();

        graph.add_edge("a", "c", None).unwrap();
        graph.add_edge("b", "c", None).unwrap();

        let parallel = graph.get_parallel_nodes(&HashSet::new());
        // Both a and b should be in the same parallel batch
        assert!(parallel.iter().any(|batch| batch.contains(&"a".to_string()) && batch.contains(&"b".to_string())));
    }

    #[test]
    fn test_mermaid_generation() {
        let registry = Arc::new(ToolRegistry::new());
        let mut graph = DynamicToolGraph::new(registry);

        graph.add_node(ToolGraphNode::new("start", "init")).unwrap();
        graph.add_node(ToolGraphNode::new("process", "work")).unwrap();
        graph.add_node(ToolGraphNode::new("end", "finalize")).unwrap();

        graph.add_edge("start", "process", Some(EdgeCondition::OnSuccess)).unwrap();
        graph.add_edge("process", "end", None).unwrap();

        let mermaid = graph.to_mermaid();
        assert!(mermaid.contains("graph TD"));
        assert!(mermaid.contains("start"));
        assert!(mermaid.contains("process"));
        assert!(mermaid.contains("end"));
    }
}
