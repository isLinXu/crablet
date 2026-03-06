use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{RwLock, mpsc};
use tokio::time::Duration;
use sqlx::sqlite::SqlitePool;
use anyhow::Result;

use crate::memory::shared::SharedBlackboard;
use crate::events::{EventBus, AgentEvent};
use crate::cognitive::llm::LlmClient;
use crate::agent::factory::AgentFactory;
use crate::agent::capability::CapabilityRouter;

// Declare submodules
pub mod types;
pub mod persister;
pub mod executor;
pub mod coordinator;

// Re-export common types
pub use types::{AgentId, SwarmMessage, SwarmAgent, TaskGraph, TaskNode, TaskStatus, GraphStatus, TaskGraphTemplate};

use persister::SwarmPersister;
use executor::SwarmExecutor;
use coordinator::SwarmCoordinator;

#[derive(Clone)]
pub struct SwarmOrchestrator {
    pub coordinator: Arc<SwarmCoordinator>,
    pub swarm: Arc<Swarm>,
}

impl SwarmOrchestrator {
    pub fn new(llm: Arc<Box<dyn LlmClient>>, swarm: Arc<Swarm>, pool: Option<SqlitePool>, agent_factory: Arc<AgentFactory>) -> Self {
        let capability_router = Arc::new(CapabilityRouter::new());
        let persister = Arc::new(SwarmPersister::new(pool));
        let executor = Arc::new(SwarmExecutor::new(
            llm.clone(),
            agent_factory,
            capability_router,
            swarm.event_bus.clone(),
            persister.clone(),
        ));
        let coordinator = Arc::new(SwarmCoordinator::new(
            llm,
            executor,
            persister,
        ));
        
        Self {
            coordinator,
            swarm,
        }
    }

    pub async fn init(&self) {
        if let Err(e) = self.coordinator.init().await {
            tracing::error!("Failed to initialize SwarmCoordinator: {}", e);
        }
    }

    pub async fn decompose_and_execute(&self, goal: &str) -> Result<String> {
        self.coordinator.decompose_and_execute(goal).await
    }
    
    pub async fn save_template(&self, name: &str, description: &str, graph: &TaskGraph) -> Result<String> {
        self.coordinator.save_template(name, description, graph).await
    }

    pub async fn list_templates(&self) -> Result<Vec<TaskGraphTemplate>> {
        self.coordinator.list_templates().await
    }

    pub async fn instantiate_template(&self, template_id: &str, goal: &str) -> Result<String> {
        self.coordinator.instantiate_template(template_id, goal).await
    }
    
    pub async fn add_task_to_graph(&self, graph_id: &str, task: TaskNode) -> Result<()> {
        self.coordinator.add_task_to_graph(graph_id, task).await
    }
    
    pub async fn execute_graph(&self, graph: TaskGraph, graph_id: &str, goal: &str) -> Result<String> {
        self.coordinator.executor.execute_graph(graph, graph_id, goal, self.coordinator.active_graphs.clone()).await
    }
    
    pub async fn get_active_graph(&self, graph_id: &str) -> Option<TaskGraph> {
         self.coordinator.active_graphs.read().await.get(graph_id).cloned()
    }
}

#[derive(Clone)]
pub struct Swarm {
    // Only hold channels. Agents run in their own tasks.
    channels: Arc<RwLock<HashMap<AgentId, mpsc::Sender<(SwarmMessage, AgentId)>>>>,
    topics: Arc<RwLock<HashMap<String, Vec<AgentId>>>>,
    pub blackboard: SharedBlackboard,
    pub event_bus: Option<Arc<EventBus>>,
}

impl Default for Swarm {
    fn default() -> Self {
        Self::new()
    }
}

impl Swarm {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            topics: Arc::new(RwLock::new(HashMap::new())),
            blackboard: SharedBlackboard::new(),
            event_bus: None,
        }
    }
    
    pub fn with_event_bus(mut self, event_bus: Arc<EventBus>) -> Self {
        self.event_bus = Some(event_bus);
        self
    }

    pub async fn register_agent(&self, mut agent: Box<dyn SwarmAgent>) {
        let id = agent.id().clone();
        let (tx, mut rx) = mpsc::channel(100);
        
        let subs = agent.subscriptions();
        {
            let mut channels = self.channels.write().await;
            channels.insert(id.clone(), tx);
            
            let mut topics = self.topics.write().await;
            for topic in subs {
                topics.entry(topic).or_default().push(id.clone());
            }
        }
        
        // Clone channels map to use inside the task for replying
        let channels_map = self.channels.clone();
        let topics_map = self.topics.clone();
        let my_id = id.clone();
        let event_bus = self.event_bus.clone();
        
        tokio::spawn(async move {
            while let Some((msg, sender)) = rx.recv().await {
                // Agent processes message with timeout protection
                // Default timeout 30s for agent processing
                let process_future = agent.receive(msg, sender.clone());
                
                let result = tokio::time::timeout(Duration::from_secs(30), process_future).await;
                
                match result {
                    Ok(Some(response)) => {
                         // Publish Event
                         if let Some(bus) = &event_bus {
                             let (task_id, graph_id, msg_type, content) = match &response {
                                 SwarmMessage::Task { task_id, description, .. } => (task_id.clone(), "unknown".to_string(), "Task".to_string(), description.clone()),
                                 SwarmMessage::Result { task_id, content, .. } => (task_id.clone(), "unknown".to_string(), "Result".to_string(), content.clone()),
                                 SwarmMessage::StatusUpdate { task_id, status, .. } => (task_id.clone(), "unknown".to_string(), "Status".to_string(), status.clone()),
                                 SwarmMessage::Broadcast { topic, content, .. } => ("global".to_string(), "unknown".to_string(), format!("Broadcast:{}", topic), content.clone()),
                                 SwarmMessage::Error { task_id, error } => (task_id.clone(), "unknown".to_string(), "Error".to_string(), error.clone()),
                             };
                             
                             bus.publish(AgentEvent::SwarmActivity {
                                 task_id,
                                 graph_id,
                                 from: my_id.0.clone(),
                                 to: sender.0.clone(),
                                 message_type: msg_type,
                                 content,
                                 timestamp: chrono::Utc::now().timestamp_millis(),
                             });
                         }

                         // Handle Broadcast differently
                         if let SwarmMessage::Broadcast { topic, .. } = &response {
                             let topics = topics_map.read().await;
                             let channels = channels_map.read().await;
                             
                             if let Some(subscribers) = topics.get(topic) {
                                 for sub_id in subscribers {
                                     if sub_id != &my_id {
                                         if let Some(tx) = channels.get(sub_id) {
                                             let _ = tx.send((response.clone(), my_id.clone())).await;
                                         }
                                     }
                                 }
                             }
                         } else {
                             // Send response back to sender
                             let map = channels_map.read().await;
                             if let Some(tx) = map.get(&sender) {
                                 if let Err(e) = tx.send((response, my_id.clone())).await {
                                     tracing::error!("Agent {} failed to reply to {}: {}", my_id.0, sender.0, e);
                                 } else {
                                     tracing::info!("Agent {} sent reply to {}", my_id.0, sender.0);
                                 }
                             } else {
                                 tracing::warn!("Agent {} could not reply to {}: sender not found", my_id.0, sender.0);
                             }
                         }
                    },
                    Ok(None) => {
                        // No response needed
                    },
                    Err(_) => {
                        tracing::error!("Agent {} timed out processing message from {}", my_id.0, sender.0);
                        // Send error back if possible
                        let map = channels_map.read().await;
                        if let Some(tx) = map.get(&sender) {
                             let error_msg = SwarmMessage::Error { 
                                 task_id: "unknown".to_string(), // Ideally we propagate task_id from input msg
                                 error: format!("Agent {} timed out", my_id.0) 
                             };
                             let _ = tx.send((error_msg, my_id.clone())).await;
                        }
                    }
                }
            }
        });
    }

    pub async fn send(&self, to: &AgentId, message: SwarmMessage, from: &AgentId) -> Result<()> {
        let channels = self.channels.read().await;
        if let Some(tx) = channels.get(to) {
            // Publish Event
             if let Some(bus) = &self.event_bus {
                             let (task_id, graph_id, msg_type, content) = match &message {
                                 SwarmMessage::Task { task_id, description, .. } => (task_id.clone(), "unknown".to_string(), "Task".to_string(), description.clone()),
                                 SwarmMessage::Result { task_id, content, .. } => (task_id.clone(), "unknown".to_string(), "Result".to_string(), content.clone()),
                                 SwarmMessage::StatusUpdate { task_id, status, .. } => (task_id.clone(), "unknown".to_string(), "Status".to_string(), status.clone()),
                                 SwarmMessage::Broadcast { topic, content, .. } => ("global".to_string(), "unknown".to_string(), format!("Broadcast:{}", topic), content.clone()),
                                 SwarmMessage::Error { task_id, error } => (task_id.clone(), "unknown".to_string(), "Error".to_string(), error.clone()),
                             };
                             
                             bus.publish(AgentEvent::SwarmActivity {
                                 task_id,
                                 graph_id,
                                 from: from.0.clone(),
                                 to: to.0.clone(),
                                 message_type: msg_type,
                                 content,
                                 timestamp: chrono::Utc::now().timestamp_millis(),
                             });
             }

            tx.send((message, from.clone())).await.map_err(|e| anyhow::anyhow!("Failed to send message: {}", e))?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Agent not found: {:?}", to))
        }
    }

    pub async fn publish(&self, topic: &str, message: SwarmMessage, from: &AgentId) -> Result<()> {
        let topics = self.topics.read().await;
        let channels = self.channels.read().await;
        
        let mut failures = Vec::new();
        
        if let Some(subscribers) = topics.get(topic) {
            for sub_id in subscribers {
                if sub_id != from {
                    if let Some(tx) = channels.get(sub_id) {
                        if let Err(e) = tx.send((message.clone(), from.clone())).await {
                            failures.push(format!("Failed to publish to {}: {}", sub_id.0, e));
                        }
                    }
                }
            }
        }
        
        if !failures.is_empty() {
            for fail in failures {
                tracing::warn!("{}", fail);
            }
        }
        
        Ok(())
    }
    
    // Deprecated: use publish with topic "global" instead
    pub async fn broadcast(&self, message: SwarmMessage, from: &AgentId) -> Result<()> {
        self.publish("global", message, from).await
    }
}
