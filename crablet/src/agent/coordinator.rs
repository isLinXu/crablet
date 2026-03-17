use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use anyhow::Result;
use crate::agent::{Agent, AgentRole};
use crate::agent::task::{Task, TaskStatus};
use crate::agent::planning::TaskPlanner;
use crate::agent::swarm::{Swarm, AgentId, SwarmMessage, SwarmAgent};
use crate::cognitive::llm::LlmClient;
use crate::types::Message;
use dashmap::DashMap;
use async_trait::async_trait;

#[derive(Clone)]
pub struct CoordinatorAgent {
    id: AgentId,
    planner: TaskPlanner,
    pub llm: Arc<Box<dyn LlmClient>>,
    swarm: Arc<Swarm>,
    tasks: Arc<RwLock<HashMap<String, Task>>>,
    // Channel to notify execution loop about subtask completion
    // Key: subtask_id, Value: Sender<String> (result content)
    completion_notifiers: Arc<DashMap<String, mpsc::Sender<String>>>,
}

impl CoordinatorAgent {
    pub fn new(llm: Arc<Box<dyn LlmClient>>, swarm: Arc<Swarm>) -> Self {
        Self {
            id: AgentId::from_name("coordinator"),
            planner: TaskPlanner::new(llm.clone()),
            llm,
            swarm,
            tasks: Arc::new(RwLock::new(HashMap::new())),
            completion_notifiers: Arc::new(DashMap::new()),
        }
    }

    pub async fn submit_task(&self, description: String) -> String {
        let task = Task::new(description);
        let id = task.id.clone();
        let mut tasks = self.tasks.write().await;
        tasks.insert(id.clone(), task);
        id
    }

    pub async fn mark_subtask_completed(&self, subtask_id: &str, result: String) {
        if let Some((_, sender)) = self.completion_notifiers.remove(subtask_id) {
            let _ = sender.send(result).await;
        }
    }
    
    pub async fn mark_subtask_failed(&self, subtask_id: &str, error: String) {
         if let Some((_, sender)) = self.completion_notifiers.remove(subtask_id) {
            let _ = sender.send(format!("Failed: {}", error)).await;
        }
    }

    pub async fn decompose_task(&self, task_id: &str) -> Result<Vec<String>> {
        let description = {
            let tasks = self.tasks.read().await;
            let task = tasks.get(task_id).ok_or_else(|| anyhow::anyhow!("Task not found"))?;
            task.description.clone()
        };
        
        let plan = self.planner.decompose(&description).await?;
        
        let mut subtask_ids = Vec::new();
        {
            let mut tasks = self.tasks.write().await;
            if let Some(task) = tasks.get_mut(task_id) {
                task.status = TaskStatus::InProgress;
                
                let mut id_map = HashMap::new();
                for subplan in plan.subtasks {
                    let real_deps: Vec<String> = subplan.dependencies.iter()
                        .filter_map(|dep_id| id_map.get(dep_id).cloned())
                        .collect();
                        
                    let real_id = task.add_subtask(subplan.description, real_deps);
                    id_map.insert(subplan.id, real_id.clone());
                    subtask_ids.push(real_id.clone());
                    
                    if let Some(subtask) = task.subtasks.get_mut(&real_id) {
                        if let Some(cap) = subplan.required_capabilities.first() {
                            subtask.assigned_to = Some(cap.clone());
                        }
                    }
                }
            }
        }
        
        Ok(subtask_ids)
    }

    pub async fn execute_task(&self, task_id: &str) -> Result<String> {
        let subtasks = self.decompose_task(task_id).await?;
        let mut results_map = HashMap::new();
        
        let mut pending_ids: HashSet<String> = subtasks.iter().cloned().collect();
        let mut completed_ids = HashSet::new();
        let mut running_ids = HashSet::new();
        
        let mut join_set = tokio::task::JoinSet::new();
        
        loop {
            // 1. Identify and Launch Ready Tasks
            let mut ready_tasks = Vec::new();
            {
                let tasks = self.tasks.read().await;
                if let Some(task) = tasks.get(task_id) {
                    for sub_id in &pending_ids {
                        if let Some(sub) = task.subtasks.get(sub_id) {
                            if sub.dependencies.iter().all(|d| completed_ids.contains(d)) {
                                ready_tasks.push((sub_id.clone(), sub.description.clone(), sub.assigned_to.clone()));
                            }
                        }
                    }
                }
            }

            for (sub_id, desc, capability_hint) in ready_tasks {
                pending_ids.remove(&sub_id);
                running_ids.insert(sub_id.clone());
                
                let swarm = self.swarm.clone();
                let notifiers = self.completion_notifiers.clone();
                let target_agent_name = capability_hint.unwrap_or_else(|| "researcher".to_string());
                let my_id = self.id.clone();
                let t_sub_id = sub_id.clone();
                
                join_set.spawn(async move {
                    let target_id = AgentId::from_name(&target_agent_name);
                    let msg = SwarmMessage::Task { 
                        task_id: t_sub_id.clone(), 
                        description: desc, 
                        context: vec![],
                        payload: None,
                    };
                    
                    let (tx, mut rx) = mpsc::channel(1);
                    notifiers.insert(t_sub_id.clone(), tx);
                    
                    if let Err(e) = swarm.send(&target_id, msg, &my_id).await {
                        return (t_sub_id, Err(anyhow::anyhow!("Failed to dispatch to {}: {}", target_agent_name, e)));
                    }
                    
                    // Configurable timeout? For now 120s
                    let result = match tokio::time::timeout(std::time::Duration::from_secs(120), rx.recv()).await {
                        Ok(Some(res)) => Ok(res),
                        Ok(None) => Err(anyhow::anyhow!("Channel closed")),
                        Err(_) => Err(anyhow::anyhow!("Timeout")),
                    };
                    
                    (t_sub_id, result)
                });
            }
            
            // 2. Check Termination
            if running_ids.is_empty() && pending_ids.is_empty() {
                break;
            }
            
            if running_ids.is_empty() && !pending_ids.is_empty() {
                return Err(anyhow::anyhow!("Deadlock detected: {} tasks pending but none ready.", pending_ids.len()));
            }
            
            // 3. Wait for next completion
            if let Some(res) = join_set.join_next().await {
                match res {
                    Ok((sub_id, task_res)) => {
                        running_ids.remove(&sub_id);
                        
                        match task_res {
                            Ok(output) => {
                                completed_ids.insert(sub_id.clone());
                                results_map.insert(sub_id.clone(), output.clone());
                                
                                // Update task status
                                let mut tasks = self.tasks.write().await;
                                if let Some(task) = tasks.get_mut(task_id) {
                                    if let Some(subtask) = task.subtasks.get_mut(&sub_id) {
                                        subtask.status = TaskStatus::Completed;
                                        subtask.result = Some(output);
                                    }
                                }
                            },
                            Err(e) => {
                                return Err(anyhow::anyhow!("Subtask {} failed: {}", sub_id, e));
                            }
                        }
                    }
                    Err(e) => {
                         return Err(anyhow::anyhow!("Task panic: {}", e));
                    }
                }
            }
        }

        // Aggregate results
        let mut report_builder = String::from("Subtask Results:\n");
        for sub_id in &subtasks {
            if let Some(res) = results_map.get(sub_id) {
                report_builder.push_str(&format!("- Subtask {}: {}\n", sub_id, res));
            }
        }

        // Use LLM to synthesize final answer if we have multiple subtasks or if it's complex
        let final_output = if subtasks.len() > 1 {
            let prompt = format!(
                 "You are a coordinator agent. Based on the following subtask results, synthesize a comprehensive answer to the original user request.\n\nOriginal Request: {}\n\n{}\n\nSynthesized Answer:",
                 {
                     let tasks = self.tasks.read().await;
                     tasks.get(task_id).map(|t| t.description.clone()).unwrap_or_default()
                 },
                 report_builder
             );
             
             let messages = vec![Message::new("user", prompt)];
             match self.llm.chat_complete(&messages).await {
                 Ok(summary) => summary,
                 Err(_) => report_builder // Fallback to raw report
             }
        } else {
            // Single task, just return the result (maybe cleaned up)
            results_map.values().next().cloned().unwrap_or(report_builder)
        };
        
        {
            let mut tasks = self.tasks.write().await;
            if let Some(task) = tasks.get_mut(task_id) {
                task.status = TaskStatus::Completed;
            }
        }

        Ok(final_output)
    }
}

#[async_trait]
impl Agent for CoordinatorAgent {
    fn name(&self) -> &str {
        "coordinator"
    }

    fn role(&self) -> AgentRole {
        AgentRole::Executor // Or Custom("Coordinator")
    }

    fn description(&self) -> &str {
        "Orchestrates complex tasks by decomposing them and assigning subtasks to other agents."
    }

    async fn execute(&self, task: &str, _context: &[Message]) -> Result<String> {
        let task_id = self.submit_task(task.to_string()).await;
        self.execute_task(&task_id).await
    }
}

#[async_trait]
impl SwarmAgent for CoordinatorAgent {
    fn id(&self) -> &AgentId {
        &self.id
    }
    
    fn name(&self) -> &str {
        "coordinator"
    }

    async fn receive(&mut self, message: SwarmMessage, _sender: AgentId) -> Option<SwarmMessage> {
        match message {
            SwarmMessage::Result { task_id, content, .. } => {
                self.mark_subtask_completed(&task_id, content).await;
            }
            SwarmMessage::StatusUpdate { task_id, status } => {
                 self.mark_subtask_failed(&task_id, status).await;
            }
            SwarmMessage::Error { task_id, error } => {
                 self.mark_subtask_failed(&task_id, error).await;
            }
            _ => {}
        }
        None
    }
}
