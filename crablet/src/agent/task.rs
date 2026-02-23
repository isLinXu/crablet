use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed(String),
    Blocked(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    pub id: String,
    pub description: String,
    pub assigned_to: Option<String>, // Agent name
    pub status: TaskStatus,
    pub result: Option<String>,
    pub dependencies: Vec<String>, // SubTask IDs
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub description: String,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub subtasks: HashMap<String, SubTask>,
    pub context: HashMap<String, String>,
}

impl Task {
    pub fn new(description: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            description,
            status: TaskStatus::Pending,
            created_at: Utc::now(),
            subtasks: HashMap::new(),
            context: HashMap::new(),
        }
    }

    pub fn add_subtask(&mut self, description: String, dependencies: Vec<String>) -> String {
        let id = Uuid::new_v4().to_string();
        let subtask = SubTask {
            id: id.clone(),
            description,
            assigned_to: None,
            status: TaskStatus::Pending,
            result: None,
            dependencies,
        };
        self.subtasks.insert(id.clone(), subtask);
        id
    }
}
