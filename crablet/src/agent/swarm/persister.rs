use crate::agent::harness_agent::HarnessExecutionState;
use crate::agent::swarm::types::{GraphStatus, TaskGraph, TaskGraphTemplate, TaskNode, TaskStatus};
use anyhow::Result;
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use std::collections::HashMap;

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
                let tasks = sqlx::query("SELECT id, agent_role, prompt, dependencies, status, result, logs, execution_state FROM swarm_tasks WHERE graph_id = ?")
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
                    let execution_state_json: Option<String> = task_row.get("execution_state");

                    let dependencies: Vec<String> =
                        serde_json::from_str(&deps_json).unwrap_or_default();
                    let mut status: TaskStatus =
                        serde_json::from_str(&status_json).unwrap_or(TaskStatus::Pending);
                    let mut logs: Vec<String> =
                        serde_json::from_str(&logs_json).unwrap_or_default();
                    let execution_state = execution_state_json
                        .and_then(|json| serde_json::from_str::<HarnessExecutionState>(&json).ok());

                    if matches!(status, TaskStatus::Running { .. }) {
                        status = TaskStatus::Pending;
                        logs.push(if execution_state.is_some() {
                            "Recovered interrupted task from persisted harness execution state."
                                .to_string()
                        } else {
                            "Recovered interrupted task after restart; rerunning from task input."
                                .to_string()
                        });
                    }

                    nodes.insert(
                        task_id.clone(),
                        TaskNode {
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
                            execution_state,
                        },
                    );
                }

                graphs.insert(
                    id,
                    TaskGraph {
                        nodes,
                        status,
                        goal,
                    },
                );
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
                let execution_state_json = match &node.execution_state {
                    Some(state) => Some(serde_json::to_string(state)?),
                    None => None,
                };

                sqlx::query("INSERT OR REPLACE INTO swarm_tasks (id, graph_id, agent_role, prompt, dependencies, status, result, logs, execution_state, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
                    .bind(&node.id)
                    .bind(id)
                    .bind(&node.agent_role)
                    .bind(&node.prompt)
                    .bind(deps_json)
                    .bind(status_json)
                    .bind(&node.result)
                    .bind(logs_json)
                    .bind(execution_state_json)
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
            let goal: Option<String> =
                sqlx::query_scalar("SELECT goal FROM swarm_graphs WHERE id = ?")
                    .bind(graph_id)
                    .fetch_optional(pool)
                    .await?;
            Ok(goal.unwrap_or_else(|| "Unknown Goal".to_string()))
        } else {
            Ok("Unknown Goal".to_string())
        }
    }

    pub async fn load_graph(&self, graph_id: &str) -> Result<Option<TaskGraph>> {
        if let Some(pool) = &self.pool {
            let graph_row = sqlx::query("SELECT goal, status FROM swarm_graphs WHERE id = ?")
                .bind(graph_id)
                .fetch_optional(pool)
                .await?;

            let Some(graph_row) = graph_row else {
                return Ok(None);
            };

            let goal: String = graph_row.get("goal");
            let status_str: String = graph_row.get("status");
            let status = match status_str.as_str() {
                "Active" => GraphStatus::Active,
                "Paused" => GraphStatus::Paused,
                "Completed" => GraphStatus::Completed,
                "Failed" => GraphStatus::Failed,
                _ => GraphStatus::Active,
            };

            let tasks = sqlx::query("SELECT id, agent_role, prompt, dependencies, status, result, logs, execution_state FROM swarm_tasks WHERE graph_id = ?")
                .bind(graph_id)
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
                let execution_state_json: Option<String> = task_row.get("execution_state");

                let dependencies: Vec<String> =
                    serde_json::from_str(&deps_json).unwrap_or_default();
                let task_status: TaskStatus =
                    serde_json::from_str(&status_json).unwrap_or(TaskStatus::Pending);
                let logs: Vec<String> = serde_json::from_str(&logs_json).unwrap_or_default();
                let execution_state = execution_state_json
                    .and_then(|json| serde_json::from_str::<HarnessExecutionState>(&json).ok());

                nodes.insert(
                    task_id.clone(),
                    TaskNode {
                        id: task_id,
                        agent_role: role,
                        prompt,
                        dependencies,
                        status: task_status,
                        result,
                        logs,
                        priority: 128,
                        timeout_ms: 30000,
                        max_retries: 3,
                        retry_count: 0,
                        execution_state,
                    },
                );
            }

            Ok(Some(TaskGraph {
                nodes,
                status,
                goal,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn delete_graph(&self, graph_id: &str) -> Result<bool> {
        if let Some(pool) = &self.pool {
            let mut tx = pool.begin().await?;

            sqlx::query("DELETE FROM swarm_tasks WHERE graph_id = ?")
                .bind(graph_id)
                .execute(&mut *tx)
                .await?;

            let deleted = sqlx::query("DELETE FROM swarm_graphs WHERE id = ?")
                .bind(graph_id)
                .execute(&mut *tx)
                .await?
                .rows_affected()
                > 0;

            tx.commit().await?;
            Ok(deleted)
        } else {
            Ok(false)
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
            let json: Option<String> =
                sqlx::query_scalar("SELECT graph_json FROM swarm_templates WHERE id = ?")
                    .bind(template_id)
                    .fetch_optional(pool)
                    .await?;
            Ok(json)
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::harness_agent::HarnessExecutionState;
    use crate::agent::swarm::types::{TaskGraph, TaskStatus};
    use sqlx::sqlite::SqlitePoolOptions;

    async fn init_test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::query(
            "CREATE TABLE swarm_graphs (
                id TEXT PRIMARY KEY,
                goal TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE swarm_tasks (
                id TEXT PRIMARY KEY,
                graph_id TEXT NOT NULL,
                agent_role TEXT NOT NULL,
                prompt TEXT NOT NULL,
                dependencies TEXT NOT NULL,
                status TEXT NOT NULL,
                result TEXT,
                logs TEXT NOT NULL,
                execution_state TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE swarm_templates (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                graph_json TEXT NOT NULL,
                created_at INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        pool
    }

    #[tokio::test]
    async fn persist_and_load_preserves_execution_state() {
        let pool = init_test_pool().await;
        let persister = SwarmPersister::new(Some(pool));

        let mut graph = TaskGraph::new();
        graph.add_task(
            "task-1".to_string(),
            "coder".to_string(),
            "implement the feature".to_string(),
            vec![],
        );
        {
            let node = graph.nodes.get_mut("task-1").unwrap();
            node.status = TaskStatus::Running { started_at: 42 };
            node.execution_state = Some(HarnessExecutionState::new(
                "implement the feature",
                &[],
                None,
            ));
        }

        persister
            .persist_graph("graph-1", &graph, "ship the feature")
            .await
            .unwrap();

        let loaded = persister.load_active_graphs().await.unwrap();
        let loaded_graph = loaded.get("graph-1").unwrap();
        let node = loaded_graph.nodes.get("task-1").unwrap();

        assert!(matches!(node.status, TaskStatus::Pending));
        assert!(node.execution_state.is_some());
        assert!(node
            .logs
            .iter()
            .any(|log| log.contains("Recovered interrupted task")));
    }

    #[tokio::test]
    async fn delete_graph_removes_graph_and_tasks() {
        let pool = init_test_pool().await;
        let persister = SwarmPersister::new(Some(pool.clone()));

        let mut graph = TaskGraph::new();
        graph.add_task(
            "task-1".to_string(),
            "coder".to_string(),
            "clean up state".to_string(),
            vec![],
        );
        graph.add_task(
            "task-2".to_string(),
            "reviewer".to_string(),
            "verify clean up".to_string(),
            vec!["task-1".to_string()],
        );

        persister
            .persist_graph("graph-delete", &graph, "delete graph")
            .await
            .unwrap();

        let deleted = persister.delete_graph("graph-delete").await.unwrap();
        assert!(deleted);

        let graph_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM swarm_graphs WHERE id = ?")
            .bind("graph-delete")
            .fetch_one(&pool)
            .await
            .unwrap();
        let task_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM swarm_tasks WHERE graph_id = ?")
                .bind("graph-delete")
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(graph_count, 0);
        assert_eq!(task_count, 0);
    }

    #[tokio::test]
    async fn load_graph_preserves_runtime_status_for_replay() {
        let pool = init_test_pool().await;
        let persister = SwarmPersister::new(Some(pool));

        let mut graph = TaskGraph::new();
        graph.add_task(
            "task-1".to_string(),
            "coder".to_string(),
            "replay this task".to_string(),
            vec![],
        );
        graph.status = GraphStatus::Paused;
        {
            let node = graph.nodes.get_mut("task-1").unwrap();
            node.status = TaskStatus::Running { started_at: 99 };
            node.logs.push("still running".to_string());
        }

        persister
            .persist_graph("graph-replay", &graph, "replay graph")
            .await
            .unwrap();

        let loaded = persister.load_graph("graph-replay").await.unwrap().unwrap();
        let node = loaded.nodes.get("task-1").unwrap();

        assert_eq!(loaded.status, GraphStatus::Paused);
        assert!(matches!(
            node.status,
            TaskStatus::Running { started_at: 99 }
        ));
        assert_eq!(node.logs, vec!["still running".to_string()]);
    }
}
