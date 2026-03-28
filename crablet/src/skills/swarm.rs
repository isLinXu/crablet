//! Distributed Swarm Orchestration
//!
//! Implements a multi-agent coordination system where multiple AI agents
//! can collaborate on complex tasks through a swarm architecture.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, broadcast};
use serde::{Deserialize, Serialize};
use tracing::{info, debug, warn};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Agent identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub String);

impl AgentId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}

/// Agent role in the swarm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentRole {
    /// Coordinator - manages task distribution
    Coordinator,
    /// Worker - executes tasks
    Worker,
    /// Observer - monitors and reports
    Observer,
}

impl AgentRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentRole::Coordinator => "coordinator",
            AgentRole::Worker => "worker",
            AgentRole::Observer => "observer",
        }
    }
}

/// Task priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TaskPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl Default for TaskPriority {
    fn default() -> Self {
        TaskPriority::Normal
    }
}

/// Task status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

/// Task definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmTask {
    /// Unique task ID
    pub id: String,
    /// Task description
    pub description: String,
    /// Task payload (JSON)
    pub payload: serde_json::Value,
    /// Assigned agent (if any)
    pub assigned_to: Option<AgentId>,
    /// Task priority
    pub priority: TaskPriority,
    /// Current status
    pub status: TaskStatus,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
    /// Started timestamp (when picked up)
    pub started_at: Option<DateTime<Utc>>,
    /// Completed timestamp
    pub completed_at: Option<DateTime<Utc>>,
    /// Result (if completed)
    pub result: Option<serde_json::Value>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Parent task ID (for sub-tasks)
    pub parent_id: Option<String>,
}

impl SwarmTask {
    pub fn new(description: String, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            description,
            payload,
            assigned_to: None,
            priority: TaskPriority::default(),
            status: TaskStatus::Pending,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            result: None,
            error: None,
            parent_id: None,
        }
    }

    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_parent(mut self, parent_id: String) -> Self {
        self.parent_id = Some(parent_id);
        self
    }
}

/// Agent heartbeat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHeartbeat {
    pub agent_id: AgentId,
    pub timestamp: DateTime<Utc>,
    pub current_tasks: Vec<String>,
    pub load_factor: f32, // 0.0 = idle, 1.0 = fully loaded
    pub status: AgentStatus,
}

/// Agent status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Online,
    Busy,
    Idle,
    Offline,
}

/// Swarm message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SwarmMessage {
    /// Register a new agent
    Register { agent_id: AgentId, role: AgentRole },
    /// Unregister an agent
    Unregister { agent_id: AgentId },
    /// Submit a new task
    SubmitTask { task: SwarmTask },
    /// Assign task to agent
    AssignTask { task_id: String, agent_id: AgentId },
    /// Task completed
    TaskCompleted { task_id: String, result: serde_json::Value },
    /// Task failed
    TaskFailed { task_id: String, error: String },
    /// Heartbeat
    Heartbeat { heartbeat: AgentHeartbeat },
    /// Request for task
    RequestTask { agent_id: AgentId },
    /// Result aggregation request
    AggregateResults { task_ids: Vec<String> },
    /// Cancel task
    CancelTask { task_id: String },
}

/// Swarm agent node
pub struct SwarmAgent {
    id: AgentId,
    role: AgentRole,
    status: Arc<RwLock<AgentStatus>>,
    current_tasks: Arc<RwLock<Vec<String>>>,
    inbox: mpsc::Receiver<SwarmMessage>,
    outbox: mpsc::Sender<SwarmMessage>,
    orchestrator: Arc<SwarmOrchestrator>,
}

impl SwarmAgent {
    /// Create a new agent
    pub async fn new(
        role: AgentRole,
        orchestrator: Arc<SwarmOrchestrator>,
    ) -> Result<(Self, mpsc::Sender<SwarmMessage>), SwarmError> {
        let (tx, rx) = mpsc::channel(100);
        let (out_tx, _out_rx) = mpsc::channel(100);

        let agent = Self {
            id: AgentId::new(),
            role,
            status: Arc::new(RwLock::new(AgentStatus::Online)),
            current_tasks: Arc::new(RwLock::new(Vec::new())),
            inbox: rx,
            outbox: out_tx,
            orchestrator,
        };

        Ok((agent, tx))
    }

    /// Get agent ID
    pub fn id(&self) -> &AgentId {
        &self.id
    }

    /// Get agent role
    pub fn role(&self) -> AgentRole {
        self.role
    }

    /// Process incoming messages
    pub async fn process_messages(&mut self) {
        while let Some(msg) = self.inbox.recv().await {
            match msg {
                SwarmMessage::AssignTask { task_id, agent_id } => {
                    if agent_id == self.id {
                        let mut tasks = self.current_tasks.write().await;
                        tasks.push(task_id.clone());
                        *self.status.write().await = AgentStatus::Busy;
                        info!("Agent {} assigned task {}", self.id.0, task_id);
                    }
                }
                SwarmMessage::CancelTask { task_id } => {
                    let mut tasks = self.current_tasks.write().await;
                    tasks.retain(|t| t != &task_id);
                    if tasks.is_empty() {
                        *self.status.write().await = AgentStatus::Idle;
                    }
                }
                SwarmMessage::Heartbeat { .. } => {
                    debug!("Agent {} received heartbeat", self.id.0);
                }
                _ => {}
            }
        }
    }

    /// Send message to orchestrator
    pub async fn send(&self, msg: SwarmMessage) -> Result<(), SwarmError> {
        self.outbox.send(msg).await.map_err(|_| SwarmError::SendError)?;
        Ok(())
    }

    /// Get current load
    pub async fn load(&self) -> f32 {
        let tasks = self.current_tasks.read().await;
        (tasks.len() as f32 / 10.0).min(1.0)
    }

    /// Mark task as completed
    pub async fn complete_task(&mut self, task_id: &str, result: serde_json::Value) {
        {
            let mut tasks = self.current_tasks.write().await;
            tasks.retain(|t| t != task_id);
            if tasks.is_empty() {
                *self.status.write().await = AgentStatus::Idle;
            }
        }
        self.send(SwarmMessage::TaskCompleted {
            task_id: task_id.to_string(),
            result,
        }).await.ok();
    }

    /// Mark task as failed
    pub async fn fail_task(&mut self, task_id: &str, error: String) {
        {
            let mut tasks = self.current_tasks.write().await;
            tasks.retain(|t| t != task_id);
            if tasks.is_empty() {
                *self.status.write().await = AgentStatus::Idle;
            }
        }
        self.send(SwarmMessage::TaskFailed {
            task_id: task_id.to_string(),
            error,
        }).await.ok();
    }
}

/// Swarm orchestrator - manages the swarm
pub struct SwarmOrchestrator {
    agents: Arc<RwLock<HashMap<AgentId, AgentInfo>>>,
    tasks: Arc<RwLock<HashMap<String, SwarmTask>>>,
    config: SwarmConfig,
    broadcast_tx: broadcast::Sender<SwarmMessage>,
}

struct AgentInfo {
    role: AgentRole,
    status: AgentStatus,
    current_tasks: Vec<String>,
    last_heartbeat: DateTime<Utc>,
    sender: mpsc::Sender<SwarmMessage>,
}

impl Clone for SwarmOrchestrator {
    fn clone(&self) -> Self {
        Self {
            agents: self.agents.clone(),
            tasks: self.tasks.clone(),
            config: self.config.clone(),
            broadcast_tx: self.broadcast_tx.clone(),
        }
    }
}

/// Swarm configuration
#[derive(Debug, Clone)]
pub struct SwarmConfig {
    /// Maximum tasks per agent
    pub max_tasks_per_agent: usize,
    /// Heartbeat interval
    pub heartbeat_interval_secs: u64,
    /// Task timeout
    pub task_timeout_secs: u64,
    /// Enable auto-scaling
    pub enable_auto_scaling: bool,
    /// Target idle agent ratio
    pub target_idle_ratio: f32,
}

impl Default for SwarmConfig {
    fn default() -> Self {
        Self {
            max_tasks_per_agent: 10,
            heartbeat_interval_secs: 30,
            task_timeout_secs: 300,
            enable_auto_scaling: false,
            target_idle_ratio: 0.2,
        }
    }
}

/// Swarm error
#[derive(Debug, thiserror::Error)]
pub enum SwarmError {
    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Task already assigned")]
    TaskAlreadyAssigned,

    #[error("No available agents")]
    NoAvailableAgents,

    #[error("Send error")]
    SendError,

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}

impl SwarmOrchestrator {
    /// Create a new swarm orchestrator
    pub fn new(config: SwarmConfig) -> Self {
        let (broadcast_tx, _) = broadcast::channel(1000);
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            tasks: Arc::new(RwLock::new(HashMap::new())),
            config,
            broadcast_tx,
        }
    }

    /// Create with default config
    pub fn default_config() -> Self {
        Self::new(SwarmConfig::default())
    }

    /// Register a new agent
    pub async fn register_agent(
        &self,
        agent_id: AgentId,
        role: AgentRole,
        sender: mpsc::Sender<SwarmMessage>,
    ) {
        let info = AgentInfo {
            role,
            status: AgentStatus::Online,
            current_tasks: Vec::new(),
            last_heartbeat: Utc::now(),
            sender,
        };
        self.agents.write().await.insert(agent_id.clone(), info);
        info!("Registered agent: {:?} with role {}", agent_id, role.as_str());
    }

    /// Unregister an agent
    pub async fn unregister_agent(&self, agent_id: &AgentId) -> Result<(), SwarmError> {
        let mut agents = self.agents.write().await;
        if agents.remove(agent_id).is_some() {
            info!("Unregistered agent: {}", agent_id.0);
            Ok(())
        } else {
            Err(SwarmError::AgentNotFound(agent_id.0.clone()))
        }
    }

    /// Submit a new task
    pub async fn submit_task(&self, task: SwarmTask) -> String {
        let task_id = task.id.clone();
        let description = task.description.clone();
        self.tasks.write().await.insert(task_id.clone(), task);
        info!("Submitted task: {} - {}", task_id, description);
        task_id
    }

    /// Submit task and try to assign immediately
    pub async fn submit_and_assign(&self, task: SwarmTask) -> Result<String, SwarmError> {
        let task_id = self.submit_task(task).await;
        self.assign_next_available_task().await?;
        Ok(task_id)
    }

    /// Find the best available agent for a task
    pub async fn find_best_agent(&self, _task: &SwarmTask) -> Option<AgentId> {
        let agents = self.agents.read().await;

        // Filter available workers
        let available: Vec<(AgentId, &AgentInfo)> = agents.iter()
            .filter(|(_, info)| {
                info.role == AgentRole::Worker
                    && info.status == AgentStatus::Online
                    && info.current_tasks.len() < self.config.max_tasks_per_agent
            })
            .map(|(id, info)| (id.clone(), info))
            .collect();

        if available.is_empty() {
            return None;
        }

        // Pick agent with lowest load
        available.into_iter()
            .min_by(|(_, a), (_, b)| {
                a.current_tasks.len().cmp(&b.current_tasks.len())
            })
            .map(|(id, _)| id)
    }

    /// Assign task to agent
    pub async fn assign_task(&self, task_id: &str, agent_id: &AgentId) -> Result<(), SwarmError> {
        let task_id_str = task_id.to_string();
        let agent_id_clone = agent_id.clone();

        let _task = {
            let mut tasks = self.tasks.write().await;
            let task = tasks.get_mut(&task_id_str)
                .ok_or(SwarmError::TaskNotFound(task_id_str.clone()))?;

            if task.assigned_to.is_some() {
                return Err(SwarmError::TaskAlreadyAssigned);
            }

            task.assigned_to = Some(agent_id_clone.clone());
            task.status = TaskStatus::InProgress;
            task.started_at = Some(Utc::now());
            task.clone()
        };

        let sender = {
            let agents = self.agents.read().await;
            agents.get(&agent_id_clone)
                .map(|a| a.sender.clone())
                .ok_or(SwarmError::AgentNotFound(agent_id_clone.0.clone()))?
        };

        // Send task to agent
        sender.send(SwarmMessage::AssignTask {
            task_id: task_id_str.clone(),
            agent_id: agent_id_clone.clone(),
        }).await.map_err(|_| SwarmError::SendError)?;

        info!("Assigned task {} to agent {}", task_id_str, agent_id_clone.0);
        Ok(())
    }

    /// Assign next available task to an available agent
    pub async fn assign_next_available_task(&self) -> Result<(), SwarmError> {
        let (task_id, task) = {
            let tasks = self.tasks.read().await;
            tasks.iter()
                .filter(|(_, t)| t.status == TaskStatus::Pending && t.assigned_to.is_none())
                .min_by(|(_, a), (_, b)| a.priority.cmp(&b.priority))
                .map(|(id, t)| (id.clone(), t.clone()))
                .ok_or(SwarmError::NoAvailableAgents)?
        };

        let agent_id = self.find_best_agent(&task).await
            .ok_or(SwarmError::NoAvailableAgents)?;

        self.assign_task(&task_id, &agent_id).await
    }

    /// Handle task completion
    pub async fn complete_task(&self, task_id: &str, result: serde_json::Value) -> Result<(), SwarmError> {
        let _task = {
            let mut tasks = self.tasks.write().await;
            let task = tasks.get_mut(task_id)
                .ok_or(SwarmError::TaskNotFound(task_id.to_string()))?;

            task.status = TaskStatus::Completed;
            task.completed_at = Some(Utc::now());
            task.result = Some(result);
            task.clone()
        };

        info!("Task {} completed", task_id);

        // Try to assign next available task
        let _ = self.assign_next_available_task().await;

        Ok(())
    }

    /// Handle task failure
    pub async fn fail_task(&self, task_id: &str, error: String) -> Result<(), SwarmError> {
        let _task = {
            let mut tasks = self.tasks.write().await;
            let task = tasks.get_mut(task_id)
                .ok_or(SwarmError::TaskNotFound(task_id.to_string()))?;

            task.status = TaskStatus::Failed;
            task.completed_at = Some(Utc::now());
            task.error = Some(error.clone());
            task.clone()
        };

        warn!("Task {} failed: {}", task_id, error);
        Ok(())
    }

    /// Cancel a task
    pub async fn cancel_task(&self, task_id: &str) -> Result<(), SwarmError> {
        let was_assigned = {
            let mut tasks = self.tasks.write().await;
            let task = tasks.get_mut(task_id)
                .ok_or(SwarmError::TaskNotFound(task_id.to_string()))?;

            task.status = TaskStatus::Cancelled;
            task.assigned_to.is_some()
        };

        // Notify agent if task was assigned
        if was_assigned {
            let task = self.tasks.read().await.get(task_id).unwrap().clone();
            if let Some(agent_id) = &task.assigned_to {
                if let Some(agent) = self.agents.read().await.get(agent_id) {
                    let _ = agent.sender.send(SwarmMessage::CancelTask {
                        task_id: task_id.to_string(),
                    }).await;
                }
            }
        }

        info!("Task {} cancelled", task_id);
        Ok(())
    }

    /// Get task status
    pub async fn get_task(&self, task_id: &str) -> Option<SwarmTask> {
        self.tasks.read().await.get(task_id).cloned()
    }

    /// Get swarm statistics
    pub async fn stats(&self) -> SwarmStats {
        let agents = self.agents.read().await;
        let tasks = self.tasks.read().await;

        let mut online_count = 0;
        let mut busy_count = 0;
        let mut idle_count = 0;
        let mut coordinator_count = 0;
        let mut worker_count = 0;
        let mut observer_count = 0;

        for info in agents.values() {
            match info.status {
                AgentStatus::Online => online_count += 1,
                AgentStatus::Busy => busy_count += 1,
                AgentStatus::Idle => idle_count += 1,
                AgentStatus::Offline => {}
            }
            match info.role {
                AgentRole::Coordinator => coordinator_count += 1,
                AgentRole::Worker => worker_count += 1,
                AgentRole::Observer => observer_count += 1,
            }
        }

        let pending = tasks.values().filter(|t| t.status == TaskStatus::Pending).count();
        let in_progress = tasks.values().filter(|t| t.status == TaskStatus::InProgress).count();
        let completed = tasks.values().filter(|t| t.status == TaskStatus::Completed).count();
        let failed = tasks.values().filter(|t| t.status == TaskStatus::Failed).count();

        SwarmStats {
            total_agents: agents.len(),
            online_agents: online_count,
            busy_agents: busy_count,
            idle_agents: idle_count,
            coordinator_count,
            worker_count,
            observer_count,
            total_tasks: tasks.len(),
            pending_tasks: pending,
            in_progress_tasks: in_progress,
            completed_tasks: completed,
            failed_tasks: failed,
        }
    }

    /// Broadcast message to all agents
    pub async fn broadcast(&self, msg: SwarmMessage) {
        let _ = self.broadcast_tx.send(msg);
    }
}

/// Swarm statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmStats {
    pub total_agents: usize,
    pub online_agents: usize,
    pub busy_agents: usize,
    pub idle_agents: usize,
    pub coordinator_count: usize,
    pub worker_count: usize,
    pub observer_count: usize,
    pub total_tasks: usize,
    pub pending_tasks: usize,
    pub in_progress_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
}

/// Simple task result aggregator
pub struct ResultAggregator {
    results: Arc<RwLock<HashMap<String, serde_json::Value>>>,
}

impl ResultAggregator {
    pub fn new() -> Self {
        Self {
            results: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_result(&self, task_id: String, result: serde_json::Value) {
        self.results.write().await.insert(task_id, result);
    }

    pub async fn get_result(&self, task_id: &str) -> Option<serde_json::Value> {
        self.results.read().await.get(task_id).cloned()
    }

    pub async fn aggregate(&self, task_ids: &[String]) -> serde_json::Value {
        let results = self.results.read().await;
        let aggregated: Vec<_> = task_ids.iter()
            .filter_map(|id| results.get(id).cloned())
            .collect();
        serde_json::json!({ "results": aggregated })
    }
}

impl Default for ResultAggregator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_swarm_submit_and_complete() {
        let orchestrator = SwarmOrchestrator::default_config();

        let task = SwarmTask::new("Test task".to_string(), serde_json::json!({}));
        let task_id = orchestrator.submit_task(task).await;

        let retrieved = orchestrator.get_task(&task_id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().description, "Test task");

        orchestrator.complete_task(&task_id, serde_json::json!({"status": "ok"})).await.unwrap();

        let completed = orchestrator.get_task(&task_id).await.unwrap();
        assert_eq!(completed.status, TaskStatus::Completed);
    }

    #[tokio::test]
    async fn test_swarm_stats() {
        let orchestrator = SwarmOrchestrator::default_config();

        let task1 = SwarmTask::new("Task 1".to_string(), serde_json::json!({}));
        let task2 = SwarmTask::new("Task 2".to_string(), serde_json::json!({}));

        orchestrator.submit_task(task1).await;
        orchestrator.submit_task(task2).await;

        let stats = orchestrator.stats().await;
        assert_eq!(stats.total_tasks, 2);
        assert_eq!(stats.pending_tasks, 2);
    }

    #[tokio::test]
    async fn test_cancel_task() {
        let orchestrator = SwarmOrchestrator::default_config();

        let task = SwarmTask::new("Cancel me".to_string(), serde_json::json!({}));
        let task_id = orchestrator.submit_task(task).await;

        orchestrator.cancel_task(&task_id).await.unwrap();

        let cancelled = orchestrator.get_task(&task_id).await.unwrap();
        assert_eq!(cancelled.status, TaskStatus::Cancelled);
    }
}