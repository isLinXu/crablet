use anyhow::Result;
use crate::cognitive::CognitiveSystem;
use crate::types::{Message, TraceStep};
use async_trait::async_trait;
use std::sync::Arc;
use crate::cognitive::llm::LlmClient;
use crate::agent::researcher::ResearchAgent;
use crate::agent::swarm::{Swarm, SwarmAgent, AgentId, SwarmMessage};
use tokio::sync::{RwLock, oneshot};
use std::collections::HashMap;

use crate::events::EventBus;

#[derive(Clone)]
pub struct System3 {
    swarm: Arc<Swarm>,
    // Map task_id -> sender
    pending_tasks: Arc<RwLock<HashMap<String, oneshot::Sender<String>>>>,
    self_id: AgentId,
    #[allow(dead_code)]
    event_bus: Arc<EventBus>,
}

struct UserProxyAgent {
    id: AgentId,
    pending_tasks: Arc<RwLock<HashMap<String, oneshot::Sender<String>>>>,
}

#[async_trait]
impl SwarmAgent for UserProxyAgent {
    fn id(&self) -> &AgentId {
        &self.id
    }
    
    fn name(&self) -> &str {
        "user_proxy"
    }

    async fn receive(&mut self, message: SwarmMessage, _sender: AgentId) -> Option<SwarmMessage> {
        match message {
            SwarmMessage::Result { task_id, content } => {
                let mut tasks = self.pending_tasks.write().await;
                if let Some(tx) = tasks.remove(&task_id) {
                    let _ = tx.send(content);
                }
            }
            SwarmMessage::StatusUpdate { task_id, status } => {
                 let mut tasks = self.pending_tasks.write().await;
                 if let Some(tx) = tasks.remove(&task_id) {
                     let _ = tx.send(format!("Task failed with status: {}", status));
                 }
            }
            _ => {}
        }
        None
    }
}

impl System3 {
    pub async fn new(llm: Arc<Box<dyn LlmClient>>, event_bus: Arc<EventBus>) -> Self {
        let swarm = Arc::new(Swarm::new());
        let pending_tasks = Arc::new(RwLock::new(HashMap::new()));
        let self_id = AgentId::from_name("user_proxy");
        
        // Register Researcher
        let researcher = Box::new(ResearchAgent::new(llm.clone(), event_bus.clone()));
        swarm.register_agent(researcher).await;
        
        // Register UserProxy (System3 itself)
        let proxy = Box::new(UserProxyAgent {
            id: self_id.clone(),
            pending_tasks: pending_tasks.clone(),
        });
        swarm.register_agent(proxy).await;

        Self {
            swarm,
            pending_tasks,
            self_id,
            event_bus,
        }
    }
}

#[async_trait]
impl CognitiveSystem for System3 {
    fn name(&self) -> &str {
        "System 3 (Swarm)"
    }

    async fn process(&self, input: &str, context: &[Message]) -> Result<(String, Vec<TraceStep>)> {
        // Extract topic (flexible matching)
        let topic = if input.to_lowercase().starts_with("research ") {
            input[9..].trim()
        } else if input.contains("deep research") {
            input.trim()
        } else {
            input.trim()
        };
        
        let task_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        
        {
            let mut tasks = self.pending_tasks.write().await;
            tasks.insert(task_id.clone(), tx);
        }
        
        let msg = SwarmMessage::Task {
            task_id: task_id.clone(),
            description: topic.to_string(),
            context: context.to_vec(),
        };
        
        // Send to researcher
        // Note: researcher ID is fixed "researcher" in ResearchAgent
        self.swarm.send(&AgentId::from_name("researcher"), msg, &self.self_id).await?;
        
        // Wait for result
        // TODO: Add timeout
        let response = rx.await?;
        
        let traces = vec![
            TraceStep {
                step: 1,
                thought: format!("Dispatched task {} to Swarm (Researcher)", task_id),
                action: Some("swarm_dispatch".to_string()),
                action_input: Some(topic.to_string()),
                observation: Some("Received result from Swarm".to_string()),
            }
        ];
        
        Ok((response, traces))
    }
}
