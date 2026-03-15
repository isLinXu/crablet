use super::types::{CreateWorkflowRequest, NodeTypeDefinition, NodePort, UpdateWorkflowRequest, ValidationResult, Workflow, next_id, now_rfc3339};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Default)]
pub struct WorkflowRegistry {
    workflows: Arc<RwLock<HashMap<String, Workflow>>>,
}

impl WorkflowRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn list(&self) -> Vec<Workflow> {
        let map = self.workflows.read().await;
        map.values().cloned().collect()
    }

    pub async fn get(&self, id: &str) -> Option<Workflow> {
        let map = self.workflows.read().await;
        map.get(id).cloned()
    }

    pub async fn create(&self, req: CreateWorkflowRequest) -> Workflow {
        let now = now_rfc3339();
        let workflow = Workflow {
            id: next_id("wf"),
            name: req.name,
            description: req.description,
            nodes: req.nodes,
            edges: req.edges,
            variables: HashMap::new(),
            created_at: now.clone(),
            updated_at: now,
            created_by: None,
            version: 1,
            is_active: true,
        };
        let mut map = self.workflows.write().await;
        map.insert(workflow.id.clone(), workflow.clone());
        workflow
    }

    pub async fn update(&self, id: &str, req: UpdateWorkflowRequest) -> Option<Workflow> {
        let mut map = self.workflows.write().await;
        let current = map.get(id)?.clone();
        let updated = Workflow {
            id: current.id.clone(),
            name: req.name.unwrap_or(current.name),
            description: req.description.or(current.description),
            nodes: req.nodes.unwrap_or(current.nodes),
            edges: req.edges.unwrap_or(current.edges),
            variables: current.variables,
            created_at: current.created_at,
            updated_at: now_rfc3339(),
            created_by: current.created_by,
            version: current.version + 1,
            is_active: current.is_active,
        };
        map.insert(id.to_string(), updated.clone());
        Some(updated)
    }

    pub async fn delete(&self, id: &str) -> bool {
        let mut map = self.workflows.write().await;
        map.remove(id).is_some()
    }

    pub fn validate(req: &CreateWorkflowRequest) -> ValidationResult {
        let mut errors = Vec::new();
        if req.name.trim().is_empty() {
            errors.push("workflow name must not be empty".to_string());
        }
        if req.nodes.is_empty() {
            errors.push("workflow must contain at least one node".to_string());
        }
        ValidationResult {
            valid: errors.is_empty(),
            errors,
        }
    }

    pub fn node_types() -> Vec<NodeTypeDefinition> {
        vec![
            // Control Flow
            NodeTypeDefinition {
                r#type: "start".to_string(),
                name: "Start".to_string(),
                description: "Workflow start node".to_string(),
                category: "control".to_string(),
                icon: "Play".to_string(),
                color: "#10b981".to_string(),
                inputs: None,
                outputs: Some(vec![NodePort { name: "output".to_string(), r#type: "any".to_string(), optional: None }]),
            },
            NodeTypeDefinition {
                r#type: "end".to_string(),
                name: "End".to_string(),
                description: "Workflow end node".to_string(),
                category: "control".to_string(),
                icon: "Square".to_string(),
                color: "#ef4444".to_string(),
                inputs: Some(vec![NodePort { name: "input".to_string(), r#type: "any".to_string(), optional: None }]),
                outputs: None,
            },
            NodeTypeDefinition {
                r#type: "condition".to_string(),
                name: "Condition".to_string(),
                description: "Branch by condition".to_string(),
                category: "control".to_string(),
                icon: "GitBranch".to_string(),
                color: "#ec4899".to_string(),
                inputs: Some(vec![NodePort { name: "input".to_string(), r#type: "any".to_string(), optional: Some(false) }]),
                outputs: Some(vec![
                    NodePort { name: "true".to_string(), r#type: "any".to_string(), optional: None },
                    NodePort { name: "false".to_string(), r#type: "any".to_string(), optional: None },
                ]),
            },
            NodeTypeDefinition {
                r#type: "loop".to_string(),
                name: "Loop".to_string(),
                description: "Iterate over collection".to_string(),
                category: "control".to_string(),
                icon: "Repeat".to_string(),
                color: "#84cc16".to_string(),
                inputs: Some(vec![NodePort { name: "items".to_string(), r#type: "array".to_string(), optional: Some(false) }]),
                outputs: Some(vec![NodePort { name: "item".to_string(), r#type: "any".to_string(), optional: None }]),
            },
            // AI & Agents
            NodeTypeDefinition {
                r#type: "llm".to_string(),
                name: "LLM".to_string(),
                description: "Call an LLM model".to_string(),
                category: "ai".to_string(),
                icon: "Brain".to_string(),
                color: "#8b5cf6".to_string(),
                inputs: Some(vec![
                    NodePort { name: "prompt".to_string(), r#type: "string".to_string(), optional: Some(false) },
                    NodePort { name: "system_prompt".to_string(), r#type: "string".to_string(), optional: Some(true) },
                ]),
                outputs: Some(vec![NodePort { name: "text".to_string(), r#type: "string".to_string(), optional: None }]),
            },
            NodeTypeDefinition {
                r#type: "agent".to_string(),
                name: "Agent".to_string(),
                description: "AI agent execution".to_string(),
                category: "ai".to_string(),
                icon: "Bot".to_string(),
                color: "#3b82f6".to_string(),
                inputs: Some(vec![NodePort { name: "task".to_string(), r#type: "string".to_string(), optional: Some(false) }]),
                outputs: Some(vec![NodePort { name: "result".to_string(), r#type: "string".to_string(), optional: None }]),
            },
            NodeTypeDefinition {
                r#type: "knowledge".to_string(),
                name: "Knowledge".to_string(),
                description: "Query knowledge base".to_string(),
                category: "ai".to_string(),
                icon: "Book".to_string(),
                color: "#a855f7".to_string(),
                inputs: Some(vec![NodePort { name: "query".to_string(), r#type: "string".to_string(), optional: Some(false) }]),
                outputs: Some(vec![NodePort { name: "results".to_string(), r#type: "array".to_string(), optional: None }]),
            },
            // Processing
            NodeTypeDefinition {
                r#type: "code".to_string(),
                name: "Code".to_string(),
                description: "Execute code".to_string(),
                category: "processing".to_string(),
                icon: "Code".to_string(),
                color: "#f59e0b".to_string(),
                inputs: Some(vec![NodePort { name: "input".to_string(), r#type: "any".to_string(), optional: Some(true) }]),
                outputs: Some(vec![NodePort { name: "result".to_string(), r#type: "any".to_string(), optional: None }]),
            },
            NodeTypeDefinition {
                r#type: "template".to_string(),
                name: "Template".to_string(),
                description: "Text template rendering".to_string(),
                category: "processing".to_string(),
                icon: "FileText".to_string(),
                color: "#14b8a6".to_string(),
                inputs: Some(vec![NodePort { name: "variables".to_string(), r#type: "object".to_string(), optional: Some(true) }]),
                outputs: Some(vec![NodePort { name: "result".to_string(), r#type: "string".to_string(), optional: None }]),
            },
            // Integration
            NodeTypeDefinition {
                r#type: "http".to_string(),
                name: "HTTP Request".to_string(),
                description: "Call external API".to_string(),
                category: "integration".to_string(),
                icon: "Globe".to_string(),
                color: "#06b6d4".to_string(),
                inputs: Some(vec![NodePort { name: "url".to_string(), r#type: "string".to_string(), optional: Some(false) }]),
                outputs: Some(vec![
                    NodePort { name: "status".to_string(), r#type: "number".to_string(), optional: None },
                    NodePort { name: "body".to_string(), r#type: "any".to_string(), optional: None },
                ]),
            },
            // Data
            NodeTypeDefinition {
                r#type: "variable".to_string(),
                name: "Variable".to_string(),
                description: "Variable operations".to_string(),
                category: "data".to_string(),
                icon: "Database".to_string(),
                color: "#6366f1".to_string(),
                inputs: Some(vec![NodePort { name: "value".to_string(), r#type: "any".to_string(), optional: Some(true) }]),
                outputs: Some(vec![NodePort { name: "result".to_string(), r#type: "any".to_string(), optional: None }]),
            },
        ]
    }
}
