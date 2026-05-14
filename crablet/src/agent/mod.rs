pub mod aggregator;
pub mod analyst;
pub mod analyst_v2;
pub mod capability;
pub mod coder;
pub mod coordinator;
pub mod debate;
pub mod factory;
pub mod generic;
pub mod handoff;
pub mod hitl;
pub mod hooks;
pub mod planning;
pub mod researcher;
pub mod reviewer;
pub mod security;
pub mod smart_allocator;
#[path = "swarm.rs"]
pub mod swarm;
pub mod task;
pub mod voting;

// Harness subsystem - core execution context and fault tolerance
pub mod adaptive_harness;
pub mod distributed_harness;
pub mod harness;
pub mod harness_agent;
pub mod harness_fusion;
pub mod harness_manager;
pub mod memory_pipeline;
pub mod metrics;
pub mod self_healing_agent;
pub mod step_executor;
pub mod tool_executor;

use crate::types::Message;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentRole {
    Researcher,
    Coder,
    Analyst,
    Executor,
    Reviewer,
    Planner,
    Moderator,
    Drafter,
    Critic,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub role: AgentRole,
    pub description: String,
    pub model: String,
    pub system_prompt: String,
    pub tools: Vec<String>,
}

#[async_trait]
pub trait Agent: Send + Sync {
    fn name(&self) -> &str;
    fn role(&self) -> AgentRole;
    fn description(&self) -> &str;
    async fn execute(&self, task: &str, context: &[Message]) -> Result<String>;
}

pub type SharedAgent = Arc<dyn Agent>;
