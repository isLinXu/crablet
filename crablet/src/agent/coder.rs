use async_trait::async_trait;
// use crate::agent::{Agent, AgentRole};
use crate::types::Message;
// use anyhow::Result;
// use tracing::info;
use std::sync::Arc;
use crate::cognitive::llm::LlmClient;
use crate::agent::swarm::{AgentId, SwarmAgent, SwarmMessage};

pub struct CoderAgent {
    id: AgentId,
    llm: Arc<Box<dyn LlmClient>>,
}

impl CoderAgent {
    pub fn new(llm: Arc<Box<dyn LlmClient>>) -> Self {
        Self {
            id: AgentId::from_name("coder"),
            llm,
        }
    }
}

#[async_trait]
impl SwarmAgent for CoderAgent {
    fn id(&self) -> &AgentId { &self.id }
    fn name(&self) -> &str { "coder" }
    
    async fn receive(&mut self, message: SwarmMessage, _sender: AgentId) -> Option<SwarmMessage> {
        if let SwarmMessage::Task { task_id, description, .. } = message {
            let prompt = format!(
                "You are an expert software engineer. Please write code for the following task:\n{}\n\nRequirements:\n1. Code must be complete and runnable\n2. Include necessary comments\n3. Handle potential errors",
                description
            );
            
            let messages = vec![Message::new("user", prompt)];
            match self.llm.chat_complete(&messages).await {
                Ok(response) => Some(SwarmMessage::Result { 
                    task_id, 
                    content: response,
                    payload: None 
                }),
                Err(e) => Some(SwarmMessage::Error { 
                    task_id, 
                    error: format!("Error generating code: {}", e) 
                }),
            }
        } else {
            None
        }
    }
}
