use serde::{Serialize, Deserialize};
use tokio::sync::broadcast;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AgentEvent {
    UserInput(String),
    SystemLog(String),
    ThoughtGenerated(String),
    ToolExecutionStarted { tool: String, args: String },
    ToolExecutionFinished { tool: String, output: String },
    CanvasUpdate { title: String, content: String, kind: String }, // kind: markdown, mermaid, code, html
    ResponseGenerated(String),
    Error(String),
}

pub struct EventBus {
    sender: broadcast::Sender<AgentEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(100);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.sender.subscribe()
    }

    pub fn publish(&self, event: AgentEvent) {
        // We ignore errors if there are no active subscribers
        let _ = self.sender.send(event);
    }
}

