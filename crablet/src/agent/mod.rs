pub mod researcher;
pub mod coordinator;
pub mod task;
pub mod factory;
pub mod swarm;
pub mod coder;
pub mod analyst;
pub mod planning;
pub mod aggregator;
pub mod debate;
pub mod voting;
pub mod reviewer;
pub mod security;
pub mod analyst_v2;
pub mod generic;
pub mod capability;
pub mod smart_allocator;
pub mod hitl;
pub mod handoff;

use anyhow::Result;
use async_trait::async_trait;
use crate::types::Message;
use std::sync::Arc;
use serde::{Serialize, Deserialize};

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
