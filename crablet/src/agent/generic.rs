use async_trait::async_trait;
use crate::agent::{Agent, AgentRole};
use crate::cognitive::llm::LlmClient;
use crate::types::Message;
use std::sync::Arc;
use anyhow::Result;

pub struct GenericAgent {
    role: AgentRole,
    llm: Arc<Box<dyn LlmClient>>,
    system_prompt: String,
}

impl GenericAgent {
    pub fn new(role: AgentRole, llm: Arc<Box<dyn LlmClient>>, system_prompt: &str) -> Self {
        Self {
            role,
            llm,
            system_prompt: system_prompt.to_string(),
        }
    }
}

#[async_trait]
impl Agent for GenericAgent {
    fn name(&self) -> &str {
        match &self.role {
            AgentRole::Researcher => "Researcher",
            AgentRole::Coder => "Coder",
            AgentRole::Analyst => "Analyst",
            AgentRole::Executor => "Executor",
            AgentRole::Reviewer => "Reviewer",
            AgentRole::Planner => "Planner",
            AgentRole::Moderator => "moderator",
            AgentRole::Drafter => "drafter",
            AgentRole::Critic => "critic",
            AgentRole::Custom(name) => name,
        }
    }

    fn role(&self) -> AgentRole {
        self.role.clone()
    }

    fn description(&self) -> &str {
        "A generic LLM-based agent."
    }

    async fn execute(&self, task: &str, context: &[Message]) -> Result<String> {
        let mut messages = vec![Message::new("system", &self.system_prompt)];
        messages.extend_from_slice(context);
        messages.push(Message::new("user", task));
        
        self.llm.chat_complete(&messages).await
    }
}
