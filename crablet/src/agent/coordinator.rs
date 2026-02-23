use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use anyhow::Result;
use crate::agent::SharedAgent;
use crate::agent::task::{Task, TaskStatus};

pub struct AgentCoordinator {
    primary_agent: SharedAgent,
    subordinate_agents: Arc<RwLock<HashMap<String, SharedAgent>>>,
    tasks: Arc<RwLock<HashMap<String, Task>>>,
}

impl AgentCoordinator {
    pub fn new(primary_agent: SharedAgent) -> Self {
        Self {
            primary_agent,
            subordinate_agents: Arc::new(RwLock::new(HashMap::new())),
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register_agent(&self, agent: SharedAgent) {
        let mut agents = self.subordinate_agents.write().await;
        agents.insert(agent.name().to_string(), agent);
    }

    pub async fn get_agent(&self, name: &str) -> Option<SharedAgent> {
        let agents = self.subordinate_agents.read().await;
        agents.get(name).cloned()
    }

    pub async fn submit_task(&self, description: String) -> String {
        let task = Task::new(description);
        let id = task.id.clone();
        let mut tasks = self.tasks.write().await;
        tasks.insert(id.clone(), task);
        id
    }

    pub async fn decompose_task(&self, task_id: &str) -> Result<Vec<String>> {
        let mut tasks = self.tasks.write().await;
        let task = tasks.get_mut(task_id).ok_or_else(|| anyhow::anyhow!("Task not found"))?;
        
        // In a real implementation, we would use the Planner Agent to decompose the task
        // For now, we'll create a dummy subtask
        let subtask_id = task.add_subtask(format!("Analyze: {}", task.description), vec![]);
        task.status = TaskStatus::InProgress;
        
        Ok(vec![subtask_id])
    }

    pub async fn execute_task(&self, task_id: &str) -> Result<String> {
        // Simple execution logic for now:
        // 1. Decompose
        // 2. Assign to Primary Agent (or router logic)
        // 3. Aggregate
        
        let subtasks = self.decompose_task(task_id).await?;
        let mut results = Vec::new();

        for _subtask_id in subtasks {
            // Retrieve subtask details (read lock)
            let (desc, _agent_name) = {
                let tasks = self.tasks.read().await;
                let task = tasks.get(task_id).unwrap();
                let subtask = task.subtasks.get(&_subtask_id).unwrap();
                (subtask.description.clone(), subtask.assigned_to.clone())
            };

            // Execute
            let result = self.primary_agent.execute(&desc, &[]).await?;
            results.push(result.clone());

            // Update subtask (write lock)
            {
                let mut tasks = self.tasks.write().await;
                if let Some(task) = tasks.get_mut(task_id) {
                    if let Some(subtask) = task.subtasks.get_mut(&_subtask_id) {
                        subtask.status = TaskStatus::Completed;
                        subtask.result = Some(result);
                    }
                }
            }
        }

        let final_result = results.join("\n\n");
        
        // Update main task
        {
            let mut tasks = self.tasks.write().await;
            if let Some(task) = tasks.get_mut(task_id) {
                task.status = TaskStatus::Completed;
            }
        }

        Ok(final_result)
    }
}
