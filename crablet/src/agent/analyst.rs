use async_trait::async_trait;
use crate::types::Message;
use std::sync::Arc;
use crate::cognitive::llm::LlmClient;
use crate::agent::swarm::{SwarmAgent, AgentId, SwarmMessage};

pub struct AnalystAgent {
    id: AgentId,
    llm: Arc<Box<dyn LlmClient>>,
}

impl AnalystAgent {
    pub fn new(llm: Arc<Box<dyn LlmClient>>) -> Self {
        Self {
            id: AgentId::from_name("analyst"),
            llm,
        }
    }
}

#[async_trait]
impl SwarmAgent for AnalystAgent {
    fn id(&self) -> &AgentId { &self.id }
    fn name(&self) -> &str { "analyst" }
    
    async fn receive(&mut self, message: SwarmMessage, _sender: AgentId) -> Option<SwarmMessage> {
        if let SwarmMessage::Task { task_id, description, .. } = message {
            let prompt = format!(
                "You are a professional data analyst. Please analyze the following task:\n{}\n\nOutput Format:\n1. Key Insights\n2. Detailed Analysis\n3. Recommendations",
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
                    error: format!("Error analyzing: {}", e) 
                }),
            }
        } else {
            None
        }
    }
}
