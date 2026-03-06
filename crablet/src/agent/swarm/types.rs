use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use uuid::Uuid;
use crate::types::Message as ChatMessage;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub String);

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}

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
        payload: Option<serde_json::Value>,
    },
    Result {
        task_id: String,
        content: String,
        payload: Option<serde_json::Value>,
    },
    StatusUpdate {
        task_id: String,
        status: String,
    },
    Broadcast {
        topic: String,
        content: String,
        payload: Option<serde_json::Value>,
    },
    Error {
        task_id: String,
        error: String,
    }
}

#[async_trait::async_trait]
pub trait SwarmAgent: Send + Sync {
    fn id(&self) -> &AgentId;
    fn name(&self) -> &str;
    fn description(&self) -> &str { "" }
    // Capabilities: e.g. ["coding", "python", "analysis"]
    fn capabilities(&self) -> Vec<String> { vec![] }
    fn subscriptions(&self) -> Vec<String> { vec![] }
    async fn receive(&mut self, message: SwarmMessage, sender: AgentId) -> Option<SwarmMessage>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Running { started_at: i64 },
    Paused { paused_at: i64 },
    Completed { duration: u64 },
    Failed { error: String, retries: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskNode {
    pub id: String,
    pub agent_role: String,
    pub prompt: String,
    pub dependencies: Vec<String>,
    pub status: TaskStatus,
    pub result: Option<String>,
    pub logs: Vec<String>,
    #[serde(default = "default_priority")]
    pub priority: u8,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default)]
    pub retry_count: u32,
}

fn default_priority() -> u8 { 128 }
fn default_timeout_ms() -> u64 { 30000 }
fn default_max_retries() -> u32 { 3 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraph {
    pub nodes: HashMap<String, TaskNode>,
    pub status: GraphStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GraphStatus {
    Active,
    Paused,
    Completed,
    Failed,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TaskGraphTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub graph: TaskGraph,
    pub created_at: i64,
}

impl Default for TaskGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskGraph {
    pub fn new() -> Self {
        Self { 
            nodes: HashMap::new(),
            status: GraphStatus::Active,
        }
    }
    
    pub fn add_task(&mut self, id: String, role: String, prompt: String, deps: Vec<String>) {
        self.nodes.insert(id.clone(), TaskNode {
            id,
            agent_role: role,
            prompt,
            dependencies: deps,
            status: TaskStatus::Pending,
            result: None,
            logs: Vec::new(),
            priority: default_priority(),
            timeout_ms: default_timeout_ms(),
            max_retries: default_max_retries(),
            retry_count: 0,
        });
    }
    
    pub fn get_ready_tasks(&self) -> Vec<String> {
        self.nodes.iter()
            .filter(|(_, node)| {
                matches!(node.status, TaskStatus::Pending) &&
                node.dependencies.iter().all(|dep_id| {
                    if let Some(dep) = self.nodes.get(dep_id) {
                        matches!(dep.status, TaskStatus::Completed { .. })
                    } else {
                        false // Dependency missing
                    }
                })
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub fn detects_cycle(&self, node_id: &str, new_dependencies: &[String]) -> bool {
        for dep in new_dependencies {
            if self.depends_on(dep, node_id) {
                return true;
            }
        }
        false
    }

    pub fn depends_on(&self, subject: &str, target: &str) -> bool {
        // Does 'subject' depend on 'target'? (i.e. is there a path target -> ... -> subject)
        if subject == target { return true; }
        
        let mut stack = vec![subject.to_string()];
        let mut visited = std::collections::HashSet::new();
        
        while let Some(current) = stack.pop() {
            if !visited.insert(current.clone()) { continue; }
            
            if let Some(node) = self.nodes.get(&current) {
                for dep in &node.dependencies {
                    if dep == target {
                        return true;
                    }
                    stack.push(dep.clone());
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_graph_cycle_detection() {
        let mut graph = TaskGraph::new();
        
        // A -> B -> C
        graph.add_task("A".to_string(), "coder".to_string(), "task A".to_string(), vec![]);
        graph.add_task("B".to_string(), "coder".to_string(), "task B".to_string(), vec!["A".to_string()]);
        graph.add_task("C".to_string(), "coder".to_string(), "task C".to_string(), vec!["B".to_string()]);
        
        // No cycle initially
        assert!(!graph.detects_cycle("A", &[]));
        assert!(!graph.detects_cycle("C", &["B".to_string()]));
        
        // Try adding edge C -> A (Cycle!)
        assert!(graph.detects_cycle("A", &["C".to_string()]));
        
        // Try adding edge A -> C (Redundant but safe)
        assert!(!graph.detects_cycle("C", &["A".to_string()]));
    }

    #[test]
    fn test_task_graph_dependency_resolution() {
        let mut graph = TaskGraph::new();
        
        graph.add_task("A".to_string(), "coder".to_string(), "task A".to_string(), vec![]);
        graph.add_task("B".to_string(), "coder".to_string(), "task B".to_string(), vec!["A".to_string()]);
        graph.add_task("C".to_string(), "coder".to_string(), "task C".to_string(), vec!["A".to_string()]);
        graph.add_task("D".to_string(), "coder".to_string(), "task D".to_string(), vec!["B".to_string(), "C".to_string()]);
        
        // Initially only A is ready
        let ready = graph.get_ready_tasks();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0], "A");
        
        // Complete A
        if let Some(node) = graph.nodes.get_mut("A") {
            node.status = TaskStatus::Completed { duration: 100 };
        }
        
        // Now B and C should be ready
        let mut ready = graph.get_ready_tasks();
        ready.sort(); // Sort for deterministic assertion
        assert_eq!(ready.len(), 2);
        assert_eq!(ready, vec!["B", "C"]);
        
        // Complete B
        if let Some(node) = graph.nodes.get_mut("B") {
            node.status = TaskStatus::Completed { duration: 100 };
        }
        
        // D still not ready (needs C)
        let ready = graph.get_ready_tasks();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0], "C");
        
        // Complete C
        if let Some(node) = graph.nodes.get_mut("C") {
            node.status = TaskStatus::Completed { duration: 100 };
        }
        
        // Now D is ready
        let ready = graph.get_ready_tasks();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0], "D");
    }
}
