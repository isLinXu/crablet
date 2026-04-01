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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_publish_subscribe() {
        let bus = EventBus::new(10);
        let mut rx = bus.subscribe();

        let event = GatewayEvent::ClientConnected("session-1".to_string());
        bus.publish(event).expect("publish");

        let received = rx.recv().await.expect("receive event");
        assert!(matches!(received, GatewayEvent::ClientConnected(id) if id == "session-1"));
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let bus = EventBus::new(10);
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        bus.publish(GatewayEvent::SystemAlert("test".to_string())).expect("publish");

        let r1 = rx1.recv().await.expect("rx1 receive");
        let r2 = rx2.recv().await.expect("rx2 receive");
        assert!(matches!(r1, GatewayEvent::SystemAlert(_)));
        assert!(matches!(r2, GatewayEvent::SystemAlert(_)));
    }

    #[tokio::test]
    async fn test_event_serialization() {
        let event = GatewayEvent::AgentThinking {
            agent_id: "agent-1".to_string(),
            thought: "thinking...".to_string(),
        };
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(json.contains("agent-1"));
        assert!(json.contains("thinking"));
    }

    #[tokio::test]
    async fn test_publish_without_subscribers_errors() {
        // broadcast channel returns Err when there are no active receivers
        let bus = EventBus::new(10);
        let result = bus.publish(GatewayEvent::SystemAlert("test".to_string()));
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_publish_with_subscriber_succeeds() {
        let bus = EventBus::new(10);
        let _rx = bus.subscribe(); // Keep receiver alive
        let result = bus.publish(GatewayEvent::SystemAlert("test".to_string()));
        assert!(result.is_ok());
    }
}
