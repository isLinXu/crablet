use anyhow::Result;
use async_trait::async_trait;
use crate::types::Message;
use std::sync::Arc;
use serde::{Serialize, Deserialize};

pub mod researcher;
pub mod coordinator;
pub mod task;
pub mod factory;
pub mod swarm;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentRole {
    Researcher,
    Coder,
    Analyst,
    Executor,
    Reviewer,
    Planner,
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
