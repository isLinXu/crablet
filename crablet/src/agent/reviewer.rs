use crate::agent::swarm::{SwarmAgent, AgentId, SwarmMessage};
use crate::cognitive::llm::LlmClient;
use crate::types::Message;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::info;

pub struct ReviewerAgent {
    id: AgentId,
    llm: Arc<Box<dyn LlmClient>>,
}

impl ReviewerAgent {
    pub fn new(name: &str, llm: Arc<Box<dyn LlmClient>>) -> Self {
        Self {
            id: AgentId::from_name(name),
            llm,
        }
    }
}

#[async_trait]
impl SwarmAgent for ReviewerAgent {
    fn id(&self) -> &AgentId { &self.id }
    fn name(&self) -> &str { &self.id.0 }
    fn description(&self) -> &str { "Reviews code and provides feedback." }

    async fn receive(&mut self, message: SwarmMessage, sender: AgentId) -> Option<SwarmMessage> {
        match message {
            SwarmMessage::Task { task_id, description, .. } => {
                info!("Reviewing code from {}: {}", sender.0, description);
                
                let prompt = format!("Please review the following code/design and provide constructive feedback, security concerns, and optimization suggestions:\n\n{}", description);
                let messages = vec![Message::user(prompt)];
                
                let review = match self.llm.chat_complete(&messages).await {
                    Ok(r) => r,
                    Err(e) => format!("Failed to generate review: {}", e),
                };
                
                Some(SwarmMessage::Result {
                    task_id,
                    content: review,
                    payload: None,
                })
            },
            _ => None,
        }
    }
}
