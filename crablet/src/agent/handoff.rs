use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Handoff {
    pub id: String,
    pub from_agent: String,
    pub to_agent: String,
    pub context: HandoffContext,
    pub reason: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffContext {
    pub conversation_summary: String,
    pub task_state: String, // TaskStatus as string to avoid circular dependency
    pub variables: HashMap<String, Value>,
    pub artifacts: Vec<HandoffArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffArtifact {
    pub name: String,
    pub content_type: String,
    pub content: String, // Or URI
}

impl Handoff {
    pub fn new(from: &str, to: &str, reason: &str, summary: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            from_agent: from.to_string(),
            to_agent: to.to_string(),
            context: HandoffContext {
                conversation_summary: summary.to_string(),
                task_state: "Pending".to_string(),
                variables: HashMap::new(),
                artifacts: Vec::new(),
            },
            reason: reason.to_string(),
            timestamp: Utc::now(),
        }
    }
    
    pub fn with_artifact(mut self, name: &str, content: &str) -> Self {
        self.context.artifacts.push(HandoffArtifact {
            name: name.to_string(),
            content_type: "text/plain".to_string(), // Default
            content: content.to_string(),
        });
        self
    }
    
    pub fn with_variable(mut self, key: &str, value: Value) -> Self {
        self.context.variables.insert(key.to_string(), value);
        self
    }
}
