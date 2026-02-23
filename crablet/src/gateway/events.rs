use tokio::sync::broadcast;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::agent::swarm::SwarmMessage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GatewayEvent {
    ClientConnected(String),
    ClientDisconnected(String),
    MessageReceived { session_id: String, content: String },
    SystemAlert(String),
    AgentThinking { agent_id: String, thought: String },
    AgentAction { agent_id: String, action: String },
    CanvasUpdate {
        session_id: String,
        action: String,
        data: serde_json::Value,
    },
    SwarmMessage(SwarmMessage),
}

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<GatewayEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<GatewayEvent> {
        self.sender.subscribe()
    }

    pub fn publish(&self, event: GatewayEvent) -> Result<usize, broadcast::error::SendError<GatewayEvent>> {
        self.sender.send(event)
    }
}

// Global EventBus instance wrapped in Arc for sharing
pub type SharedEventBus = Arc<EventBus>;
