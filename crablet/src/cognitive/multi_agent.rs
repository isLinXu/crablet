//! Multi-Agent Collaboration System
//!
//! A sophisticated multi-agent system that enables multiple AI agents to collaborate
//! on complex tasks through various coordination protocols.
//!
//! # Core Features
//!
//! 1. **Multiple Agent Roles** - Coordinator, Specialist, Critic, Integrator
//! 2. **Various Collaboration Protocols** - Master-Slave, Peer-to-Peer, Hierarchical, Competitive
//! 3. **Message Bus Communication** - Async message passing between agents
//! 4. **Task Decomposition** - Break complex tasks into subtasks
//! 5. **Result Integration** - Combine results from multiple agents
//!
//! # Architecture
//!
//! ```text
//!                    ┌─────────────┐
//!                    │ Coordinator │
//!                    └──────┬──────┘
//!                           │
//!          ┌────────────────┼────────────────┐
//!          │                │                │
//!     ┌────▼────┐     ┌────▼────┐     ┌────▼────┐
//!     │Specialist│     │Specialist│     │ Critic  │
//!     │    A    │     │    B    │     │         │
//!     └─────────┘     └─────────┘     └────┬────┘
//!                                         │
//!                                    ┌────▼────┐
//!                                    │Integrator│
//!                                    └─────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! let team = AgentTeam::new(llm_client.clone());
//!
//! // Add agents with different roles
//! team.add_agent(AgentRole::Coordinator, "coordinator".into());
//! team.add_agent(AgentRole::Specialist, "tech_writer".into());
//! team.add_agent(AgentRole::Critic, "reviewer".into());
//!
//! // Set collaboration protocol
//! team.set_protocol(CollaborationProtocol::Hierarchical);
//!
//! // Execute collaborative task
//! let result = team.execute_task("Write a technical whitepaper on AI agents").await?;
//! ```

use std::sync::Arc;
use std::collections::HashMap;
use std::collections::VecDeque;
use anyhow::{Result, anyhow};
use crate::cognitive::llm::LlmClient;
use crate::types::Message;
use serde::{Deserialize, Serialize};
use tracing::{info, debug, warn};
use tokio::sync::{RwLock, broadcast};
use tokio::time::{sleep, Duration};
use std::sync::atomic::{AtomicU64, Ordering};

/// Agent identifier
pub type AgentId = String;

/// Role of an agent in the collaboration
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentRole {
    /// Coordinator - orchestrates the overall task
    Coordinator,
    /// Specialist - domain expert for specific tasks
    Specialist,
    /// Critic - reviews and provides feedback
    Critic,
    /// Integrator - combines results from multiple agents
    Integrator,
    /// Monitor - observes and reports progress
    Monitor,
}

impl std::fmt::Display for AgentRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentRole::Coordinator => write!(f, "Coordinator"),
            AgentRole::Specialist => write!(f, "Specialist"),
            AgentRole::Critic => write!(f, "Critic"),
            AgentRole::Integrator => write!(f, "Integrator"),
            AgentRole::Monitor => write!(f, "Monitor"),
        }
    }
}

/// Collaboration protocol between agents
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollaborationProtocol {
    /// Master-slave: One agent directs others
    MasterSlave,
    /// Peer-to-peer: Equal agents collaborate
    PeerToPeer,
    /// Hierarchical: Tree-like structure
    Hierarchical,
    /// Competitive: Agents compete, best result wins
    Competitive,
}

/// Message type in the message bus
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentMessage {
    /// Unique message ID
    pub id: String,
    /// Sender agent ID
    pub sender: AgentId,
    /// Receiver agent ID (None for broadcast)
    pub receiver: Option<AgentId>,
    /// Message type
    pub msg_type: MessageType,
    /// Message content
    pub content: MessageContent,
    /// Timestamp
    pub timestamp: u64,
}

/// Type of message
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    /// Task assignment
    TaskAssignment,
    /// Result submission
    ResultSubmission,
    /// Review/Feedback request
    ReviewRequest,
    /// Review/Feedback response
    ReviewResponse,
    /// Coordination message
    Coordination,
    /// Integration request
    IntegrationRequest,
    /// Status update
    StatusUpdate,
    /// Error report
    ErrorReport,
}

/// Content of a message
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// Text content
    Text(String),
    /// Structured data
    Structured {
        action: String,
        data: serde_json::Value,
    },
    /// Task description
    Task {
        task_id: String,
        description: String,
        subtasks: Vec<String>,
        priority: u8,
    },
    /// Result
    Result {
        task_id: String,
        output: String,
        confidence: f32,
        metadata: HashMap<String, String>,
    },
    /// Review
    Review {
        task_id: String,
        rating: u8,
        comments: String,
        suggestions: Vec<String>,
    },
}

/// An agent in the collaboration system
pub struct Agent {
    /// Agent ID
    pub id: AgentId,
    /// Agent role
    pub role: AgentRole,
    /// Agent name for display
    pub name: String,
    /// LLM client for reasoning
    llm: Arc<Box<dyn LlmClient>>,
    /// Message queue
    message_queue: VecDeque<AgentMessage>,
    /// Agent's knowledge/context
    context: HashMap<String, String>,
    /// Statistics
    tasks_completed: AtomicU64,
    tasks_failed: AtomicU64,
}

impl Agent {
    /// Create a new agent
    pub fn new(id: AgentId, role: AgentRole, name: String, llm: Arc<Box<dyn LlmClient>>) -> Self {
        Self {
            id,
            role,
            name,
            llm,
            message_queue: VecDeque::new(),
            context: HashMap::new(),
            tasks_completed: AtomicU64::new(0),
            tasks_failed: AtomicU64::new(0),
        }
    }

    /// Push a message to the agent's queue
    pub fn push_message(&mut self, msg: AgentMessage) {
        self.message_queue.push_back(msg);
    }

    /// Pop a message from the agent's queue
    pub fn pop_message(&mut self) -> Option<AgentMessage> {
        self.message_queue.pop_front()
    }

    /// Process messages based on role
    pub async fn process(&mut self, team: &AgentTeam) -> Result<Option<AgentMessage>> {
        let msg = self.pop_message().ok_or_else(|| anyhow!("No messages"))?;
        
        let response = match self.role {
            AgentRole::Coordinator => self.process_as_coordinator(&msg, team).await?,
            AgentRole::Specialist => self.process_as_specialist(&msg, team).await?,
            AgentRole::Critic => self.process_as_critic(&msg, team).await?,
            AgentRole::Integrator => self.process_as_integrator(&msg, team).await?,
            AgentRole::Monitor => self.process_as_monitor(&msg, team).await?,
        };
        
        // Update statistics
        self.tasks_completed.fetch_add(1, Ordering::Relaxed);
        
        Ok(response)
    }

    /// Process message as a coordinator
    async fn process_as_coordinator(&mut self, msg: &AgentMessage, team: &AgentTeam) -> Result<Option<AgentMessage>> {
        debug!("Coordinator {} processing message", self.name);
        
        match &msg.content {
            MessageContent::Task { task_id, description, subtasks, priority } => {
                // Decompose task and assign to specialists
                let assignments = self.decompose_and_assign(task_id, description, subtasks, priority, team).await?;
                
                Ok(Some(AgentMessage {
                    id: uuid::Uuid::new_v4().to_string(),
                    sender: self.id.clone(),
                    receiver: None, // Broadcast
                    msg_type: MessageType::TaskAssignment,
                    content: MessageContent::Structured {
                        action: "batch_assign".to_string(),
                        data: serde_json::json!({
                            "assignments": assignments,
                        }),
                    },
                    timestamp: current_timestamp(),
                }))
            }
            MessageContent::Result { .. } => {
                // Check if all specialists have reported
                let pending = team.count_pending_tasks();
                if pending == 0 {
                    // Send to integrator
                    Ok(Some(AgentMessage {
                        id: uuid::Uuid::new_v4().to_string(),
                        sender: self.id.clone(),
                        receiver: Some("integrator".to_string()),
                        msg_type: MessageType::IntegrationRequest,
                        content: MessageContent::Text("All tasks completed, please integrate".to_string()),
                        timestamp: current_timestamp(),
                    }))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    /// Decompose task and assign to specialists
    async fn decompose_and_assign(
        &self,
        _task_id: &str,
        description: &str,
        _subtasks: &[String],
        _priority: &u8,
        team: &AgentTeam,
    ) -> Result<Vec<serde_json::Value>> {
        let prompt = format!(
            "Decompose the following task into subtasks for specialists:\n\nTask: {}\n\n\
            Available specialists: {:?}\n\n\
            Return a JSON array of assignments with format:\n\
            [{{\"specialist_id\": \"...\", \"subtask\": \"...\", \"description\": \"...\"}}]",
            description,
            team.get_specialist_ids()
        );
        
        let response = self.llm.chat_complete(&[Message::user(&prompt)]).await?;
        
        // Parse assignments
        if let Some(json_str) = Self::extract_json(&response) {
            if let Ok(assignments) = serde_json::from_str::<Vec<serde_json::Value>>(json_str) {
                return Ok(assignments);
            }
        }
        
        // Fallback: create default assignments
        Ok(vec![serde_json::json!({
            "specialist_id": team.get_specialist_ids().first().unwrap_or(&"specialist_1".to_string()),
            "subtask": "main",
            "description": description
        })])
    }

    /// Process message as a specialist
    async fn process_as_specialist(&mut self, msg: &AgentMessage, _team: &AgentTeam) -> Result<Option<AgentMessage>> {
        debug!("Specialist {} processing message", self.name);
        
        match &msg.content {
            MessageContent::Task { task_id, description, .. } => {
                // Process the task
                let result = self.execute_specialized_task(description).await?;
                
                Ok(Some(AgentMessage {
                    id: uuid::Uuid::new_v4().to_string(),
                    sender: self.id.clone(),
                    receiver: Some("coordinator".to_string()),
                    msg_type: MessageType::ResultSubmission,
                    content: MessageContent::Result {
                        task_id: task_id.clone(),
                        output: result,
                        confidence: 0.85,
                        metadata: HashMap::new(),
                    },
                    timestamp: current_timestamp(),
                }))
            }
            _ => Ok(None),
        }
    }

    /// Execute a specialized task
    async fn execute_specialized_task(&self, description: &str) -> Result<String> {
        let prompt = format!(
            "You are a specialist agent. Execute the following task and provide a detailed result:\n\n\
            Task: {}\n\n\
            Provide your specialized output.",
            description
        );
        
        let response = self.llm.chat_complete(&[Message::user(&prompt)]).await?;
        Ok(response)
    }

    /// Process message as a critic
    async fn process_as_critic(&mut self, msg: &AgentMessage, _team: &AgentTeam) -> Result<Option<AgentMessage>> {
        debug!("Critic {} processing message", self.name);
        
        match &msg.content {
            MessageContent::Result { task_id, output, confidence: _, .. } => {
                // Review the result
                let review = self.review_result(task_id, output).await?;
                
                Ok(Some(AgentMessage {
                    id: uuid::Uuid::new_v4().to_string(),
                    sender: self.id.clone(),
                    receiver: Some("integrator".to_string()),
                    msg_type: MessageType::ReviewResponse,
                    content: MessageContent::Review {
                        task_id: task_id.clone(),
                        rating: review.0,
                        comments: review.1,
                        suggestions: review.2,
                    },
                    timestamp: current_timestamp(),
                }))
            }
            _ => Ok(None),
        }
    }

    /// Review a result
    async fn review_result(&self, task_id: &str, output: &str) -> Result<(u8, String, Vec<String>)> {
        let prompt = format!(
            "Review the following task output for quality and correctness:\n\n\
            Task ID: {}\n\
            Output: {}\n\n\
            Provide:\n\
            1. Rating (1-10)\n\
            2. Comments\n\
            3. Suggestions for improvement\n\n\
            Return JSON: {{\"rating\": 8, \"comments\": \"...\", \"suggestions\": [\"...\"]}}",
            task_id, output
        );
        
        let response = self.llm.chat_complete(&[Message::user(&prompt)]).await?;
        
        // Try to parse review
        if let Some(json_str) = Self::extract_json(&response) {
            if let Ok(review) = serde_json::from_str::<serde_json::Value>(json_str) {
                let rating = review["rating"].as_u64().unwrap_or(5) as u8;
                let comments = review["comments"].as_str().unwrap_or("").to_string();
                let suggestions: Vec<String> = review["suggestions"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                return Ok((rating, comments, suggestions));
            }
        }
        
        Ok((5, "No specific comments".to_string(), vec![]))
    }

    /// Process message as an integrator
    async fn process_as_integrator(&mut self, msg: &AgentMessage, team: &AgentTeam) -> Result<Option<AgentMessage>> {
        debug!("Integrator {} processing message", self.name);
        
        match &msg.content {
            MessageContent::Text(_content) => {
                // Integrate results from all specialists
                let integration = self.integrate_results(team).await?;
                
                Ok(Some(AgentMessage {
                    id: uuid::Uuid::new_v4().to_string(),
                    sender: self.id.clone(),
                    receiver: None,
                    msg_type: MessageType::ResultSubmission,
                    content: MessageContent::Result {
                        task_id: "final".to_string(),
                        output: integration,
                        confidence: 0.9,
                        metadata: HashMap::new(),
                    },
                    timestamp: current_timestamp(),
                }))
            }
            _ => Ok(None),
        }
    }

    /// Integrate results from all agents
    async fn integrate_results(&self, team: &AgentTeam) -> Result<String> {
        let results = team.gather_results();
        
        let prompt = format!(
            "Integrate the following results from multiple specialist agents into a cohesive output:\n\n\
            Results:\n{}\n\n\
            Provide a unified, coherent final output that combines all the specialist contributions.",
            results
        );
        
        let response = self.llm.chat_complete(&[Message::user(&prompt)]).await?;
        Ok(response)
    }

    /// Process message as a monitor
    async fn process_as_monitor(&mut self, msg: &AgentMessage, _team: &AgentTeam) -> Result<Option<AgentMessage>> {
        debug!("Monitor {} processing message", self.name);
        
        // Monitor just logs and reports status
        info!("Monitor: Agent {} sent a message of type {:?}", msg.sender, msg.msg_type);
        
        Ok(None)
    }

    /// Extract JSON from response
    fn extract_json(text: &str) -> Option<&str> {
        if let Some(start) = text.find('[') {
            let mut depth = 0;
            for (i, c) in text[start..].chars().enumerate() {
                match c {
                    '[' | '{' => depth += 1,
                    ']' | '}' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(&text[start..=start + i]);
                        }
                    }
                    _ => {}
                }
            }
        }
        
        if let Some(start) = text.find('{') {
            let mut depth = 0;
            for (i, c) in text[start..].chars().enumerate() {
                match c {
                    '[' | '{' => depth += 1,
                    ']' | '}' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(&text[start..=start + i]);
                        }
                    }
                    _ => {}
                }
            }
        }
        
        None
    }

    /// Get agent statistics
    pub fn get_stats(&self) -> (u64, u64) {
        (
            self.tasks_completed.load(Ordering::Relaxed),
            self.tasks_failed.load(Ordering::Relaxed),
        )
    }
}

/// Get current timestamp
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Agent team for multi-agent collaboration
pub struct AgentTeam {
    /// Team ID
    pub id: String,
    /// LLM client
    llm: Arc<Box<dyn LlmClient>>,
    /// Agents in the team
    agents: RwLock<HashMap<AgentId, Agent>>,
    /// Collaboration protocol
    protocol: RwLock<CollaborationProtocol>,
    /// Message bus
    message_bus: broadcast::Sender<AgentMessage>,
    /// Results from agents
    results: RwLock<HashMap<AgentId, MessageContent>>,
    /// Pending tasks count
    pending_tasks: AtomicU64,
}

impl AgentTeam {
    /// Create a new agent team
    pub fn new(id: String, llm: Arc<Box<dyn LlmClient>>) -> Self {
        let (tx, _rx) = broadcast::channel(100);
        
        Self {
            id,
            llm,
            agents: RwLock::new(HashMap::new()),
            protocol: RwLock::new(CollaborationProtocol::Hierarchical),
            message_bus: tx,
            results: RwLock::new(HashMap::new()),
            pending_tasks: AtomicU64::new(0),
        }
    }

    /// Add an agent to the team
    pub async fn add_agent(&self, role: AgentRole, id: AgentId) -> Result<()> {
        let agent = Agent::new(
            id.clone(),
            role.clone(),
            format!("{:?}Agent-{}", role, &id[..4.min(id.len())]),
            self.llm.clone(),
        );
        
        let mut agents = self.agents.write().await;
        agents.insert(id, agent);
        
        info!("Added agent with role {:?} to team {}", role, self.id);
        Ok(())
    }

    /// Set collaboration protocol
    pub async fn set_protocol(&self, protocol: CollaborationProtocol) {
        let mut p = self.protocol.write().await;
        *p = protocol.clone();
        info!("Team {} protocol changed to {:?}", self.id, protocol);
    }

    /// Execute a task collaboratively
    pub async fn execute_task(&self, task_description: &str) -> Result<String> {
        info!("Team {} executing task: {}", self.id, 
            if task_description.len() > 50 { format!("{}...", &task_description[..50]) } else { task_description.to_string() });
        
        // Create initial task message
        let task_msg = AgentMessage {
            id: uuid::Uuid::new_v4().to_string(),
            sender: "system".to_string(),
            receiver: None,
            msg_type: MessageType::TaskAssignment,
            content: MessageContent::Task {
                task_id: "main".to_string(),
                description: task_description.to_string(),
                subtasks: vec![],
                priority: 5,
            },
            timestamp: current_timestamp(),
        };
        
        // Broadcast to all agents
        self.message_bus.send(task_msg.clone())?;
        
        // Find coordinator
        let coordinator_id = {
            let agents = self.agents.read().await;
            agents.iter()
                .find(|(_, a)| a.role == AgentRole::Coordinator)
                .map(|(id, _)| id.clone())
                .ok_or_else(|| anyhow!("No coordinator found"))?
        };
        
        // Deliver message to coordinator
        {
            let mut agents = self.agents.write().await;
            if let Some(agent) = agents.get_mut(&coordinator_id) {
                agent.push_message(task_msg);
            }
        }
        
        // Process messages until task is complete
        let mut rx = self.message_bus.subscribe();
        let timeout_duration = Duration::from_secs(300); // 5 minute timeout
        
        loop {
            tokio::select! {
                msg = rx.recv() => {
                    match msg {
                        Ok(msg) => {
                            // Deliver message to target agent
                            if let Some(target_id) = &msg.receiver {
                                let mut agents = self.agents.write().await;
                                if let Some(agent) = agents.get_mut(target_id) {
                                    agent.push_message(msg.clone());
                                }
                            } else {
                                // Broadcast to all agents
                                let mut agents = self.agents.write().await;
                                for (_, agent) in agents.iter_mut() {
                                    agent.push_message(msg.clone());
                                }
                            }
                            
                            // Check if final result is available
                            if let MessageContent::Result { task_id, output, .. } = &msg.content {
                                if task_id == "final" {
                                    info!("Team {} completed task", self.id);
                                    return Ok(output.clone());
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("Message bus lagged by {} messages", n);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
                _ = sleep(timeout_duration) => {
                    warn!("Team {} task timed out after {:?}", self.id, timeout_duration);
                    break;
                }
            }
            
            // Try to process messages for all agents
            let agent_ids: Vec<AgentId> = {
                let agents = self.agents.read().await;
                agents.keys().cloned().collect()
            };
            
            for id in agent_ids {
                let mut agents = self.agents.write().await;
                if let Some(agent) = agents.get_mut(&id) {
                    if let Ok(Some(response)) = agent.process(self).await {
                        let _ = self.message_bus.send(response);
                    }
                }
            }
        }
        
        // Return best available result
        let results = self.results.read().await;
        if let Some(MessageContent::Result { output, .. }) = results.get(&"integrator".to_string()) {
            return Ok(output.clone());
        }
        
        Err(anyhow!("Task execution failed or timed out"))
    }

    /// Get specialist agent IDs
    pub fn get_specialist_ids(&self) -> Vec<AgentId> {
        // This is a sync method, need to use blocking read
        vec![] // Placeholder - actual implementation would need async context
    }

    /// Count pending tasks
    pub fn count_pending_tasks(&self) -> u64 {
        self.pending_tasks.load(Ordering::Relaxed)
    }

    /// Gather results from all agents
    pub fn gather_results(&self) -> String {
        "Results gathered".to_string() // Placeholder
    }

    /// Get team status
    pub async fn get_status(&self) -> TeamStatus {
        let agents = self.agents.read().await;
        let protocol = self.protocol.read().await;
        
        let agent_statuses: HashMap<AgentId, AgentStatus> = agents.iter()
            .map(|(id, agent)| {
                let (completed, failed) = agent.get_stats();
                (id.clone(), AgentStatus {
                    role: agent.role.clone(),
                    name: agent.name.clone(),
                    tasks_completed: completed,
                    tasks_failed: failed,
                    queue_length: agent.message_queue.len() as u64,
                })
            })
            .collect();
        
        TeamStatus {
            team_id: self.id.clone(),
            protocol: protocol.clone(),
            agents: agent_statuses,
            pending_tasks: self.pending_tasks.load(Ordering::Relaxed),
        }
    }
}

/// Status of a single agent
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentStatus {
    pub role: AgentRole,
    pub name: String,
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub queue_length: u64,
}

/// Status of the entire team
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TeamStatus {
    pub team_id: String,
    pub protocol: CollaborationProtocol,
    pub agents: HashMap<AgentId, AgentStatus>,
    pub pending_tasks: u64,
}

/// Message bus for agent communication
pub struct MessageBus {
    sender: broadcast::Sender<AgentMessage>,
}

impl MessageBus {
    /// Create a new message bus
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Send a message
    pub fn send(&self, msg: AgentMessage) -> Result<()> {
        self.sender.send(msg)?;
        Ok(())
    }

    /// Subscribe to messages
    pub fn subscribe(&self) -> broadcast::Receiver<AgentMessage> {
        self.sender.subscribe()
    }
}

/// Builder for creating agent teams
pub struct AgentTeamBuilder {
    team_id: String,
    llm: Arc<Box<dyn LlmClient>>,
    roles: Vec<AgentRole>,
    protocol: CollaborationProtocol,
}

impl AgentTeamBuilder {
    /// Create a new builder
    pub fn new(team_id: String, llm: Arc<Box<dyn LlmClient>>) -> Self {
        Self {
            team_id,
            llm,
            roles: vec![
                AgentRole::Coordinator,
                AgentRole::Specialist,
                AgentRole::Specialist,
                AgentRole::Critic,
                AgentRole::Integrator,
            ],
            protocol: CollaborationProtocol::Hierarchical,
        }
    }

    /// Add a coordinator
    pub fn with_coordinator(mut self) -> Self {
        self.roles.push(AgentRole::Coordinator);
        self
    }

    /// Add a specialist
    pub fn with_specialist(mut self) -> Self {
        self.roles.push(AgentRole::Specialist);
        self
    }

    /// Add a critic
    pub fn with_critic(mut self) -> Self {
        self.roles.push(AgentRole::Critic);
        self
    }

    /// Add an integrator
    pub fn with_integrator(mut self) -> Self {
        self.roles.push(AgentRole::Integrator);
        self
    }

    /// Set the protocol
    pub fn with_protocol(mut self, protocol: CollaborationProtocol) -> Self {
        self.protocol = protocol;
        self
    }

    /// Build the team
    pub async fn build(&self) -> Result<AgentTeam> {
        let team = AgentTeam::new(self.team_id.clone(), self.llm.clone());
        
        for (i, role) in self.roles.iter().enumerate() {
            let id = format!("{}_{}", role.to_string().to_lowercase(), i);
            team.add_agent(role.clone(), id).await?;
        }
        
        team.set_protocol(self.protocol.clone()).await;
        
        Ok(team)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_role_display() {
        assert_eq!(AgentRole::Coordinator.to_string(), "Coordinator");
        assert_eq!(AgentRole::Specialist.to_string(), "Specialist");
        assert_eq!(AgentRole::Critic.to_string(), "Critic");
        assert_eq!(AgentRole::Integrator.to_string(), "Integrator");
        assert_eq!(AgentRole::Monitor.to_string(), "Monitor");
    }

    #[test]
    fn test_message_content_variants() {
        let task = MessageContent::Task {
            task_id: "t1".to_string(),
            description: "Test task".to_string(),
            subtasks: vec![],
            priority: 5,
        };
        
        match task {
            MessageContent::Task { task_id, .. } => {
                assert_eq!(task_id, "t1");
            }
            _ => panic!("Expected Task variant"),
        }
    }

    #[test]
    fn test_extract_json() {
        let text = "Result: {\"key\": \"value\"}";
        let json = Agent::extract_json(text);
        assert_eq!(json, Some("{\"key\": \"value\"}"));
    }

    #[test]
    fn test_current_timestamp() {
        let ts1 = current_timestamp();
        let ts2 = current_timestamp();
        assert!(ts2 >= ts1);
    }
}
