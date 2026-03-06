use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
#[allow(unused_imports)]
use anyhow::Result;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    pub name: String,
    pub description: String,
    pub version: String,
    pub capabilities: Vec<AgentCapabilityDescriptor>,
    pub endpoints: AgentEndpoints,
    pub supported_content_types: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCapabilityDescriptor {
    pub name: String,
    pub description: String,
    pub inputs: serde_json::Value, // JSON Schema
    pub outputs: serde_json::Value, // JSON Schema
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEndpoints {
    pub task_url: String,
    pub status_url: String,
    pub handoff_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2ATask {
    pub id: String,
    pub initiator_agent_id: String,
    pub target_agent_id: String,
    pub instruction: String,
    pub parameters: HashMap<String, serde_json::Value>,
    pub artifacts: Vec<Artifact>,
    pub status: A2ATaskStatus,
    pub created_at: DateTime<Utc>,
    pub deadline: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum A2ATaskStatus {
    Pending,
    Accepted,
    Running,
    Completed(A2ATaskResult),
    Failed(String),
    Rejected(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2ATaskResult {
    pub summary: String,
    pub output_data: serde_json::Value,
    pub artifacts: Vec<Artifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: String,
    pub name: String,
    pub uri: String,
    pub mime_type: String,
    pub metadata: HashMap<String, String>,
}

// Protocol Adapter
pub struct A2AAdapter {
    pub agent_id: String,
    pub agent_card: AgentCard,
}

impl A2AAdapter {
    pub fn new(agent_id: &str, name: &str, description: &str, base_url: &str) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            agent_card: AgentCard {
                name: name.to_string(),
                description: description.to_string(),
                version: "1.0.0".to_string(),
                capabilities: Vec::new(),
                endpoints: AgentEndpoints {
                    task_url: format!("{}/a2a/task", base_url),
                    status_url: format!("{}/a2a/status", base_url),
                    handoff_url: Some(format!("{}/a2a/handoff", base_url)),
                },
                supported_content_types: vec!["application/json".to_string()],
            },
        }
    }
    
    pub fn register_capability(&mut self, name: &str, desc: &str, input_schema: serde_json::Value, output_schema: serde_json::Value) {
        self.agent_card.capabilities.push(AgentCapabilityDescriptor {
            name: name.to_string(),
            description: desc.to_string(),
            inputs: input_schema,
            outputs: output_schema,
        });
    }
    
    pub async fn create_task(&self, instruction: &str, target_id: &str) -> A2ATask {
        A2ATask {
            id: Uuid::new_v4().to_string(),
            initiator_agent_id: self.agent_id.clone(),
            target_agent_id: target_id.to_string(),
            instruction: instruction.to_string(),
            parameters: HashMap::new(),
            artifacts: Vec::new(),
            status: A2ATaskStatus::Pending,
            created_at: Utc::now(),
            deadline: None,
        }
    }
}
