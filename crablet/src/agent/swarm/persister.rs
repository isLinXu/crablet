use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use std::collections::HashMap;
use anyhow::Result;
use crate::agent::swarm::types::{TaskGraph, TaskNode, TaskStatus, GraphStatus, TaskGraphTemplate};


pub struct SwarmPersister {
    pub pool: Option<SqlitePool>,
}

impl SwarmPersister {
    pub fn new(pool: Option<SqlitePool>) -> Self {
        Self { pool }
    }
    
    pub async fn init_tables(&self) -> Result<()> {
        if let Some(_pool) = &self.pool {
            // Check if tables exist, or rely on schema.sql?
            // For now, let's assume schema.sql is run, or we create them if missing as fallback.
            // But the instruction is to unify schema. 
            // So we should NOT create tables here ideally.
            // But for now we keep the logic or rely on schema migration.
            // Let's assume schema migration handles it.
            Ok(())
        } else {
            Ok(())
        }
    }

    pub async fn load_active_graphs(&self) -> Result<HashMap<String, TaskGraph>> {
        if let Some(pool) = &self.pool {
            let rows = sqlx::query("SELECT id, goal, status FROM swarm_graphs WHERE status != 'Completed' AND status != 'Failed'")
                .fetch_all(pool)
                .await?;
                
            let mut graphs = HashMap::new();
            
            for row in rows {
                let id: String = row.get("id");
                let goal: String = row.get("goal");
                let status_str: String = row.get("status");
                let status = match status_str.as_str() {
                    "Active" => GraphStatus::Active,
                    "Paused" => GraphStatus::Paused,
                    _ => GraphStatus::Active,
                };
                
                // Load tasks
                let tasks = sqlx::query("SELECT id, agent_role, prompt, dependencies, status, result, logs FROM swarm_tasks WHERE graph_id = ?")
                    .bind(&id)
                    .fetch_all(pool)
                    .await?;
                    
                let mut nodes = HashMap::new();
                for task_row in tasks {
                    let task_id: String = task_row.get("id");
                    let role: String = task_row.get("agent_role");
                    let prompt: String = task_row.get("prompt");
                    let deps_json: String = task_row.get("dependencies");
                    let status_json: String = task_row.get("status");
                    let result: Option<String> = task_row.get("result");
                    let logs_json: String = task_row.get("logs");
                    
                    let dependencies: Vec<String> = serde_json::from_str(&deps_json).unwrap_or_default();
                    let status: TaskStatus = serde_json::from_str(&status_json).unwrap_or(TaskStatus::Pending);
                    let logs: Vec<String> = serde_json::from_str(&logs_json).unwrap_or_default();
                    
                    nodes.insert(task_id.clone(), TaskNode {
                        id: task_id,
                        agent_role: role,
                        prompt,
                        dependencies,
                        status,
                        result,
                        logs,
                        priority: 128, // Default
                        timeout_ms: 30000,
                        max_retries: 3,
                        retry_count: 0,
                    });
                }
                
                graphs.insert(id, TaskGraph { nodes, status, goal });
            }
            Ok(graphs)
        } else {
            Ok(HashMap::new())
        }
    }

    pub async fn persist_graph(&self, id: &str, graph: &TaskGraph, goal: &str) -> Result<()> {
        if let Some(pool) = &self.pool {
            // Save Graph
            let status_str = match graph.status {
                GraphStatus::Active => "Active",
                GraphStatus::Paused => "Paused",
                GraphStatus::Completed => "Completed",
                GraphStatus::Failed => "Failed",
            };
            
            let now = chrono::Utc::now().timestamp();
            
            sqlx::query("INSERT OR REPLACE INTO swarm_graphs (id, goal, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)")
                .bind(id)
                .bind(goal)
                .bind(status_str)
                .bind(now) // Created at - simplifying
                .bind(now)
                .execute(pool)
                .await?;
                
            // Save Tasks
            // Use transaction for atomicity?
            let mut tx = pool.begin().await?;
            
            for node in graph.nodes.values() {
                let deps_json = serde_json::to_string(&node.dependencies)?;
                let status_json = serde_json::to_string(&node.status)?;
                let logs_json = serde_json::to_string(&node.logs)?;
                
                sqlx::query("INSERT OR REPLACE INTO swarm_tasks (id, graph_id, agent_role, prompt, dependencies, status, result, logs, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
                    .bind(&node.id)
                    .bind(id)
                    .bind(&node.agent_role)
                    .bind(&node.prompt)
                    .bind(deps_json)
                    .bind(status_json)
                    .bind(&node.result)
                    .bind(logs_json)
                    .bind(now)
                    .bind(now)
                    .execute(&mut *tx)
                    .await?;
            }
            
            tx.commit().await?;
        }
        Ok(())
    }
    
    pub async fn get_goal(&self, graph_id: &str) -> Result<String> {
        if let Some(pool) = &self.pool {
             let goal: Option<String> = sqlx::query_scalar("SELECT goal FROM swarm_graphs WHERE id = ?")
                .bind(graph_id)
                .fetch_optional(pool)
                .await?;
            Ok(goal.unwrap_or_else(|| "Unknown Goal".to_string()))
        } else {
            Ok("Unknown Goal".to_string())
        }
    }
    
    pub async fn save_template(&self, template: &TaskGraphTemplate) -> Result<()> {
        if let Some(pool) = &self.pool {
            let graph_json = serde_json::to_string(&template.graph)?;
            sqlx::query("INSERT INTO swarm_templates (id, name, description, graph_json, created_at) VALUES (?, ?, ?, ?, ?)")
                .bind(&template.id)
                .bind(&template.name)
                .bind(&template.description)
                .bind(graph_json)
                .bind(template.created_at)
                .execute(pool)
                .await?;
        }
        Ok(())
    }
    
    pub async fn list_templates(&self) -> Result<Vec<TaskGraphTemplate>> {
        if let Some(pool) = &self.pool {
            let rows = sqlx::query("SELECT id, name, description, graph_json, created_at FROM swarm_templates ORDER BY created_at DESC")
                .fetch_all(pool)
                .await?;
                
            let mut templates = Vec::new();
            for row in rows {
                let id: String = row.get("id");
                let name: String = row.get("name");
                let desc: String = row.get("description");
                let json: String = row.get("graph_json");
                let created_at: i64 = row.get("created_at");
                
                let graph: TaskGraph = serde_json::from_str(&json).unwrap_or(TaskGraph::new());
                templates.push(TaskGraphTemplate {
                    id,
                    name,
                    description: desc,
                    graph,
                    created_at,
                });
            }
            Ok(templates)
        } else {
            Ok(vec![])
        }
    }
    
    pub async fn get_template_json(&self, template_id: &str) -> Result<Option<String>> {
        if let Some(pool) = &self.pool {
            let json: Option<String> = sqlx::query_scalar("SELECT graph_json FROM swarm_templates WHERE id = ?")
                .bind(template_id)
                .fetch_optional(pool)
                .await?;
            Ok(json)
        } else {
            Ok(None)
        }
    }
}
