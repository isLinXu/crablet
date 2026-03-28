//! Dynamic Agent Topology Manager
//!
//! Implements runtime dynamic agent collaboration graph formation.
//! Agents can be dynamically spawned, connected, and dissolved based on:
//! - Task decomposition results
//! - Available agent capabilities
//! - Resource constraints
//! - Performance metrics
//!
//! # Architecture
//! ```text
//! ┌──────────────────────────────────────────────────────────────────┐
//! │                    Dynamic Topology Manager                      │
//! ├──────────────────────────────────────────────────────────────────┤
//! │                                                                  │
//! │   Task ──▶ Task Analyzer ──▶ Capability Matcher ──▶ Topology     │
//! │   Request      │                      │            Builder       │
//! │                ▼                      ▼                │         │
//! │         Task Sub-tasks         Agent Pool       Agent Graph     │
//! │         Decomposition        (Capabilities)   (Spawn/Connect)  │
//! │                                                      │         │
//! │                                                      ▼         │
//! │                                              Execution Orch.   │
//! │                                                      │         │
//! │                                                      ▼         │
//! │                                              Performance       │
//! │                                              Feedback Loop      │
//! │                                                                  │
//! └──────────────────────────────────────────────────────────────────┘
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use petgraph::{Direction, Graph};
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, mpsc};
use anyhow::Result;

use crate::agent::swarm::{SwarmCoordinator, AgentId};

// ============================================================================
// Task Decomposition
// ============================================================================

/// Sub-task from task decomposition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    pub id: String,
    pub name: String,
    pub description: String,
    pub required_capabilities: Vec<Capability>,
    pub estimated_duration: Duration,
    pub dependencies: Vec<String>,  // IDs of dependent sub-tasks
    pub priority: u32,
}

/// Task decomposition result
#[derive(Debug, Clone)]
pub struct TaskDecomposition {
    pub sub_tasks: Vec<SubTask>,
    pub estimated_total_duration: Duration,
    pub requires_parallel: bool,
    pub critical_path: Vec<String>,
}

/// Decompose a complex task into sub-tasks
pub fn decompose_task(task: &str, task_type: TaskType) -> TaskDecomposition {
    match task_type {
        TaskType::Simple => TaskDecomposition {
            sub_tasks: vec![SubTask {
                id: "main".to_string(),
                name: "Main Task".to_string(),
                description: task.to_string(),
                required_capabilities: vec![Capability::BasicReasoning],
                estimated_duration: Duration::from_secs(5),
                dependencies: vec![],
                priority: 1,
            }],
            estimated_total_duration: Duration::from_secs(5),
            requires_parallel: false,
            critical_path: vec!["main".to_string()],
        },
        TaskType::Complex => TaskDecomposition {
            sub_tasks: vec![
                SubTask {
                    id: "analyze".to_string(),
                    name: "Analysis".to_string(),
                    description: "Analyze the task requirements".to_string(),
                    required_capabilities: vec![Capability::Analysis],
                    estimated_duration: Duration::from_secs(10),
                    dependencies: vec![],
                    priority: 2,
                },
                SubTask {
                    id: "plan".to_string(),
                    name: "Planning".to_string(),
                    description: "Create execution plan".to_string(),
                    required_capabilities: vec![Capability::Planning],
                    estimated_duration: Duration::from_secs(5),
                    dependencies: vec!["analyze".to_string()],
                    priority: 2,
                },
                SubTask {
                    id: "execute".to_string(),
                    name: "Execution".to_string(),
                    description: "Execute the plan".to_string(),
                    required_capabilities: vec![Capability::ToolUse],
                    estimated_duration: Duration::from_secs(30),
                    dependencies: vec!["plan".to_string()],
                    priority: 1,
                },
                SubTask {
                    id: "verify".to_string(),
                    name: "Verification".to_string(),
                    description: "Verify results".to_string(),
                    required_capabilities: vec![Capability::Analysis],
                    estimated_duration: Duration::from_secs(5),
                    dependencies: vec!["execute".to_string()],
                    priority: 2,
                },
            ],
            estimated_total_duration: Duration::from_secs(50),
            requires_parallel: true,
            critical_path: vec!["analyze".to_string(), "plan".to_string(), "execute".to_string(), "verify".to_string()],
        },
        TaskType::Code => TaskDecomposition {
            sub_tasks: vec![
                SubTask {
                    id: "understand".to_string(),
                    name: "Understand".to_string(),
                    description: "Understand the code requirements".to_string(),
                    required_capabilities: vec![Capability::CodeUnderstanding],
                    estimated_duration: Duration::from_secs(15),
                    dependencies: vec![],
                    priority: 2,
                },
                SubTask {
                    id: "generate".to_string(),
                    name: "Generate".to_string(),
                    description: "Generate the code".to_string(),
                    required_capabilities: vec![Capability::CodeGeneration],
                    estimated_duration: Duration::from_secs(20),
                    dependencies: vec!["understand".to_string()],
                    priority: 1,
                },
                SubTask {
                    id: "review".to_string(),
                    name: "Review".to_string(),
                    description: "Review and validate the code".to_string(),
                    required_capabilities: vec![Capability::CodeReview],
                    estimated_duration: Duration::from_secs(10),
                    dependencies: vec!["generate".to_string()],
                    priority: 1,
                },
            ],
            estimated_total_duration: Duration::from_secs(45),
            requires_parallel: false,
            critical_path: vec!["understand".to_string(), "generate".to_string(), "review".to_string()],
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    Simple, Complex, Code,
}

// ============================================================================
// Agent Capabilities
// ============================================================================

/// Agent capability descriptor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    BasicReasoning,
    Analysis,
    Planning,
    ToolUse,
    CodeUnderstanding,
    CodeGeneration,
    CodeReview,
    Research,
    Creative,
    Memory,
    Communication,
}

/// Agent capability profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    pub id: AgentId,
    pub name: String,
    pub capabilities: Vec<Capability>,
    pub capacity: f32,  // 0.0 - 1.0, how much capacity available
    pub cost_per_ms: f32,
    pub avg_latency_ms: u64,
}

impl AgentProfile {
    /// Check if agent has all required capabilities
    pub fn has_capabilities(&self, required: &[Capability]) -> bool {
        required.iter().all(|cap| self.capabilities.contains(cap))
    }

    /// Score agent for a task (higher is better)
    pub fn score_for_task(&self, task: &SubTask) -> f32 {
        if !self.has_capabilities(&task.required_capabilities) {
            return 0.0;
        }

        let capability_score = task.required_capabilities.len() as f32 / self.capabilities.len().max(1) as f32;
        let capacity_score = self.capacity;
        let latency_penalty = 1.0 / (self.avg_latency_ms.max(1) as f32);
        let cost_penalty = 1.0 / (self.cost_per_ms.max(0.001) * 1000.0);

        capability_score * 0.4 + capacity_score * 0.3 + latency_penalty * 0.2 + cost_penalty * 0.1
    }
}

/// Agent pool for dynamic selection
pub struct AgentPool {
    available: HashMap<AgentId, AgentProfile>,
}

impl AgentPool {
    pub fn new() -> Self {
        let mut available = HashMap::new();

        // Add some default agents
        available.insert(
            AgentId("analyst".to_string()),
            AgentProfile {
                id: AgentId("analyst".to_string()),
                name: "Analyst Agent".to_string(),
                capabilities: vec![Capability::BasicReasoning, Capability::Analysis, Capability::Planning],
                capacity: 0.8,
                cost_per_ms: 0.001,
                avg_latency_ms: 100,
            },
        );

        available.insert(
            AgentId("coder".to_string()),
            AgentProfile {
                id: AgentId("coder".to_string()),
                name: "Code Agent".to_string(),
                capabilities: vec![Capability::CodeUnderstanding, Capability::CodeGeneration, Capability::CodeReview],
                capacity: 0.7,
                cost_per_ms: 0.002,
                avg_latency_ms: 150,
            },
        );

        available.insert(
            AgentId("tool_user".to_string()),
            AgentProfile {
                id: AgentId("tool_user".to_string()),
                name: "Tool User Agent".to_string(),
                capabilities: vec![Capability::ToolUse, Capability::BasicReasoning],
                capacity: 0.9,
                cost_per_ms: 0.0015,
                avg_latency_ms: 80,
            },
        );

        Self { available }
    }

    /// Get agents that can handle a sub-task
    pub fn find_capable(&self, task: &SubTask) -> Vec<&AgentProfile> {
        self.available
            .values()
            .filter(|agent| agent.has_capabilities(&task.required_capabilities))
            .collect()
    }

    /// Select best agent for a task
    pub fn select_best(&self, task: &SubTask) -> Option<&AgentProfile> {
        let capable = self.find_capable(task);
        capable.into_iter()
            .max_by(|a, b| a.score_for_task(task).partial_cmp(&b.score_for_task(task)).unwrap())
    }

    /// Select agents for parallel execution
    pub fn select_parallel(&self, tasks: &[SubTask]) -> HashMap<String, AgentId> {
        let mut assignment = HashMap::new();

        for task in tasks {
            if let Some(agent) = self.select_best(task) {
                assignment.insert(task.id.clone(), agent.id.clone());
            }
        }

        assignment
    }
}

impl Default for AgentPool {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Dynamic Topology
// ============================================================================

/// Edge type in agent topology graph
#[derive(Debug, Clone)]
pub struct AgentEdge {
    pub edge_type: EdgeType,
    pub created_at: Instant,
    pub message_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeType {
    /// Agent A depends on Agent B
    Dependency,
    /// Agent A publishes to Agent B
    Publish,
    /// Bidirectional communication
    Bidirectional,
}

/// Node in agent topology graph
#[derive(Debug, Clone)]
pub struct AgentNode {
    pub agent_id: AgentId,
    pub task_id: String,
    pub status: NodeStatus,
    pub created_at: Instant,
    pub started_at: Option<Instant>,
    pub completed_at: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

/// Dynamic agent topology graph
pub struct AgentTopology {
    graph: Graph<AgentNode, AgentEdge>,
    pool: AgentPool,
    swarm: Option<Arc<SwarmCoordinator>>,
}

impl AgentTopology {
    pub fn new(pool: AgentPool) -> Self {
        Self {
            graph: Graph::new(),
            pool,
            swarm: None,
        }
    }

    /// Build topology from task decomposition
    pub async fn build_from_decomposition(&mut self, decomposition: &TaskDecomposition) -> Result<Vec<AgentId>> {
        let mut subtask_nodes: HashMap<String, NodeIndex> = HashMap::new();
        let mut spawned_agents: Vec<AgentId> = Vec::new();

        // Create nodes for each sub-task
        for task in &decomposition.sub_tasks {
            if let Some(agent_profile) = self.pool.select_best(task) {
                let node = AgentNode {
                    agent_id: agent_profile.id.clone(),
                    task_id: task.id.clone(),
                    status: NodeStatus::Pending,
                    created_at: Instant::now(),
                    started_at: None,
                    completed_at: None,
                };

                let idx = self.graph.add_node(node);
                subtask_nodes.insert(task.id.clone(), idx);
                spawned_agents.push(agent_profile.id.clone());
            }
        }

        // Create edges based on dependencies
        for task in &decomposition.sub_tasks {
            if let Some(&from_idx) = subtask_nodes.get(&task.id) {
                for dep_id in &task.dependencies {
                    if let Some(&to_idx) = subtask_nodes.get(dep_id) {
                        let edge = AgentEdge {
                            edge_type: EdgeType::Dependency,
                            created_at: Instant::now(),
                            message_count: 0,
                        };
                        self.graph.add_edge(from_idx, to_idx, edge);
                    }
                }
            }
        }

        Ok(spawned_agents)
    }

    /// Get nodes that are ready to execute (dependencies satisfied)
    pub fn ready_nodes(&self) -> Vec<NodeIndex> {
        self.graph
            .node_indices()
            .filter(|&idx| {
                let node = &self.graph[idx];
                if node.status != NodeStatus::Pending {
                    return false;
                }

                // Check if all dependencies are completed
                let deps: Vec<_> = self
                    .graph
                    .edges_directed(idx, Direction::Outgoing)
                    .map(|e| e.target())
                    .collect();

                deps.iter().all(|&dep_idx| {
                    self.graph[dep_idx].status == NodeStatus::Completed
                })
            })
            .collect()
    }

    /// Mark node as started
    pub fn mark_started(&mut self, node_idx: NodeIndex) {
        self.graph[node_idx].status = NodeStatus::Running;
        self.graph[node_idx].started_at = Some(Instant::now());
    }

    /// Mark node as completed
    pub fn mark_completed(&mut self, node_idx: NodeIndex) {
        self.graph[node_idx].status = NodeStatus::Completed;
        self.graph[node_idx].completed_at = Some(Instant::now());
    }

    /// Mark node as failed
    pub fn mark_failed(&mut self, node_idx: NodeIndex) {
        self.graph[node_idx].status = NodeStatus::Failed;
    }

    /// Get execution statistics
    pub fn stats(&self) -> TopologyStats {
        let total = self.graph.node_count();
        let completed = self.graph
            .node_indices()
            .filter(|&idx| self.graph[idx].status == NodeStatus::Completed)
            .count();
        let running = self.graph
            .node_indices()
            .filter(|&idx| self.graph[idx].status == NodeStatus::Running)
            .count();
        let failed = self.graph
            .node_indices()
            .filter(|&idx| self.graph[idx].status == NodeStatus::Failed)
            .count();
        let pending = total - completed - running - failed;

        let edge_count = self.graph.edge_count();

        TopologyStats {
            total_nodes: total,
            completed,
            running,
            failed,
            pending,
            total_edges: edge_count,
        }
    }
}

/// Topology execution statistics
#[derive(Debug, Clone)]
pub struct TopologyStats {
    pub total_nodes: usize,
    pub completed: usize,
    pub running: usize,
    pub failed: usize,
    pub pending: usize,
    pub total_edges: usize,
}

impl TopologyStats {
    pub fn is_complete(&self) -> bool {
        self.completed + self.failed == self.total_nodes
    }

    pub fn progress(&self) -> f64 {
        if self.total_nodes == 0 {
            return 1.0;
        }
        (self.completed + self.failed) as f64 / self.total_nodes as f64
    }
}

// ============================================================================
// Topology Orchestrator
// ============================================================================

/// Orchestrates dynamic topology execution
pub struct TopologyOrchestrator {
    topology: Arc<RwLock<AgentTopology>>,
    event_sender: mpsc::UnboundedSender<TopologyEvent>,
    event_receiver: Option<mpsc::UnboundedReceiver<TopologyEvent>>,
}

impl TopologyOrchestrator {
    pub fn new(topology: AgentTopology) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        Self {
            topology: Arc::new(RwLock::new(topology)),
            event_sender: tx,
            event_receiver: Some(rx),
        }
    }

    /// Execute the topology
    pub async fn execute(&mut self) -> Result<()> {
        loop {
            let ready = {
                let topo = self.topology.read().await;
                topo.ready_nodes()
            };

            if ready.is_empty() {
                // Check if complete
                let stats = {
                    let topo = self.topology.read().await;
                    topo.stats()
                };

                if stats.is_complete() {
                    break;
                }

                // Wait for events
                if let Some(rx) = &mut self.event_receiver {
                    if rx.recv().await.is_some() {
                        continue;
                    }
                }
            }

            // Spawn tasks for ready nodes
            for node_idx in ready {
                let _topo = self.topology.clone();
                let tx = self.event_sender.clone();

                tokio::spawn(async move {
                    let _ = tx.send(TopologyEvent::NodeStarted(node_idx));
                    
                    // Execute node (placeholder)
                    tokio::time::sleep(Duration::from_millis(100)).await;

                    let _ = tx.send(TopologyEvent::NodeCompleted(node_idx));
                });
            }
        }

        Ok(())
    }
}

/// Topology execution events
#[derive(Debug, Clone)]
pub enum TopologyEvent {
    NodeStarted(NodeIndex),
    NodeCompleted(NodeIndex),
    NodeFailed(NodeIndex, String),
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_decomposition() {
        let decomp = decompose_task("Write a function", TaskType::Code);
        assert_eq!(decomp.sub_tasks.len(), 3);
        assert_eq!(decomp.sub_tasks[0].id, "understand");
    }

    #[test]
    fn test_agent_pool() {
        let pool = AgentPool::new();
        let task = SubTask {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: "".to_string(),
            required_capabilities: vec![Capability::CodeGeneration],
            estimated_duration: Duration::from_secs(10),
            dependencies: vec![],
            priority: 1,
        };

        let best = pool.select_best(&task);
        assert!(best.is_some());
        assert_eq!(best.unwrap().id.0, "coder");
    }

    #[test]
    fn test_topology_build() {
        let pool = AgentPool::new();
        let mut topology = AgentTopology::new(pool);

        let decomp = decompose_task("Analyze and plan", TaskType::Complex);
        let result = tokio::runtime::Runtime::new().unwrap().block_on(
            topology.build_from_decomposition(&decomp)
        );

        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }

    #[test]
    fn test_topology_stats() {
        let pool = AgentPool::new();
        let mut topology = AgentTopology::new(pool);

        let stats = topology.stats();
        assert_eq!(stats.total_nodes, 0);
        assert_eq!(stats.progress(), 1.0);
    }
}