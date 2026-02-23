use uuid::Uuid;
use tokio::sync::mpsc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use crate::types::Message as ChatMessage;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub String);

impl AgentId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
    
    pub fn from_name(name: &str) -> Self {
        Self(name.to_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SwarmMessage {
    Task {
        task_id: String,
        description: String,
        context: Vec<ChatMessage>,
    },
    Result {
        task_id: String,
        content: String,
    },
    StatusUpdate {
        task_id: String,
        status: String,
    },
    Broadcast {
        content: String,
    },
}

#[async_trait::async_trait]
pub trait SwarmAgent: Send + Sync {
    fn id(&self) -> &AgentId;
    fn name(&self) -> &str;
    async fn receive(&mut self, message: SwarmMessage, sender: AgentId) -> Option<SwarmMessage>;
}

#[derive(Clone)]
pub struct Swarm {
    // Only hold channels. Agents run in their own tasks.
    channels: Arc<RwLock<HashMap<AgentId, mpsc::Sender<(SwarmMessage, AgentId)>>>>,
}

impl Swarm {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register_agent(&self, mut agent: Box<dyn SwarmAgent>) {
        let id = agent.id().clone();
        let (tx, mut rx) = mpsc::channel(100);
        
        {
            let mut channels = self.channels.write().await;
            channels.insert(id.clone(), tx);
        }
        
        // Clone channels map to use inside the task for replying
        let channels_map = self.channels.clone();
        let my_id = id.clone();
        
        tokio::spawn(async move {
            while let Some((msg, sender)) = rx.recv().await {
                // Agent processes message
                if let Some(response) = agent.receive(msg, sender.clone()).await {
                     // Send response back to sender
                     let map = channels_map.read().await;
                     if let Some(tx) = map.get(&sender) {
                         let _ = tx.send((response, my_id.clone())).await;
                         tracing::info!("Agent {} sent reply to {}", my_id.0, sender.0);
                     } else {
                         tracing::warn!("Agent {} could not reply to {}: sender not found", my_id.0, sender.0);
                     }
                }
            }
        });
    }

    pub async fn send(&self, to: &AgentId, message: SwarmMessage, from: &AgentId) -> anyhow::Result<()> {
        let channels = self.channels.read().await;
        if let Some(tx) = channels.get(to) {
            tx.send((message, from.clone())).await.map_err(|e| anyhow::anyhow!("Failed to send message: {}", e))?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Agent not found: {:?}", to))
        }
    }

    pub async fn broadcast(&self, message: SwarmMessage, from: &AgentId) -> anyhow::Result<()> {
        let channels = self.channels.read().await;
        for (id, tx) in channels.iter() {
            if id != from {
                let _ = tx.send((message.clone(), from.clone())).await;
            }
        }
        Ok(())
    }
}
