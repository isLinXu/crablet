use async_trait::async_trait;
// use crate::agent::{Agent, AgentRole};
use crate::types::Message;
// use anyhow::Result;
// use tracing::info;
use std::sync::Arc;
use crate::cognitive::llm::LlmClient;
use crate::agent::swarm::{AgentId, SwarmAgent, SwarmMessage};

pub struct AggregatorAgent {
    id: AgentId,
    llm: Arc<Box<dyn LlmClient>>,
}

impl AggregatorAgent {
    pub fn new(llm: Arc<Box<dyn LlmClient>>) -> Self {
        Self {
            id: AgentId::from_name("aggregator"),
            llm,
        }
    }
}

#[async_trait]
impl SwarmAgent for AggregatorAgent {
    fn id(&self) -> &AgentId { &self.id }
    fn name(&self) -> &str { "aggregator" }
    fn capabilities(&self) -> Vec<String> { vec!["aggregation".to_string(), "summarization".to_string()] }
    
    async fn receive(&mut self, message: SwarmMessage, _sender: AgentId) -> Option<SwarmMessage> {
        if let SwarmMessage::Task { task_id, description, .. } = message {
            // Description contains the collected results to aggregate
            let prompt = format!(
                "You are a Result Aggregator. Your job is to synthesize the results from multiple subtasks into a coherent final response.\n\nContext & Results:\n{}\n\nPlease provide a comprehensive summary and final answer.",
                description
            );
            
            let messages = vec![Message::user(prompt)];
            match self.llm.chat_complete(&messages).await {
                Ok(response) => Some(SwarmMessage::Result { 
                    task_id, 
                    content: response,
                    payload: None 
                }),
                Err(e) => Some(SwarmMessage::Error { 
                    task_id, 
                    error: format!("Error aggregating: {}", e) 
                }),
            }
        } else {
            None
        }
    }
}
