use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{RwLock, Semaphore};
use dashmap::DashMap;
use anyhow::Result;
use tracing::{info, warn, debug, error};
use tokio::time::Duration;
use serde_json::json;
use crate::cognitive::llm::LlmClient;
use crate::agent::factory::AgentFactory;
use crate::agent::capability::CapabilityRouter;
use crate::agent::hitl::{HumanDecision, HumanInTheLoop, ReviewType};
use crate::agent::smart_allocator::SmartTaskAllocator;
use crate::agent::swarm::types::{AgentId, TaskGraph, TaskStatus, GraphStatus};
use crate::agent::swarm::persister::SwarmPersister;
use crate::events::{EventBus, AgentEvent};
use crate::types::Message as ChatMessage;

pub struct SwarmExecutor {
    pub llm: Arc<Box<dyn LlmClient>>,
    pub agent_factory: Arc<AgentFactory>,
    pub capability_router: Arc<CapabilityRouter>,
    pub smart_allocator: Arc<SmartTaskAllocator>,
    pub hitl: Arc<HumanInTheLoop>,
    pub event_bus: Option<Arc<EventBus>>,
    pub persister: Arc<SwarmPersister>,
    
    // Runtime state
    pub active_agents: Arc<DashMap<String, i64>>, // Track active agents by role
    pub limits: HashMap<String, i64>, // Concurrency limits by role
    pub task_semaphore: Arc<Semaphore>,
}

impl SwarmExecutor {
    pub fn new(
        llm: Arc<Box<dyn LlmClient>>,
        agent_factory: Arc<AgentFactory>,
        capability_router: Arc<CapabilityRouter>,
        event_bus: Option<Arc<EventBus>>,
        persister: Arc<SwarmPersister>,
    ) -> Self {
        let mut limits = HashMap::new();
        // Default limits
        limits.insert("coder".to_string(), 2);
        limits.insert("researcher".to_string(), 5);
        
        let max_concurrent = 20;

        Self {
            llm,
            agent_factory,
            smart_allocator: Arc::new(SmartTaskAllocator::new(capability_router.clone())),
            hitl: Arc::new(HumanInTheLoop::new()),
            capability_router,
            event_bus,
            persister,
            active_agents: Arc::new(DashMap::new()),
            limits,
            task_semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }

    pub async fn execute_graph(&self, mut graph: TaskGraph, graph_id: &str, goal: &str, active_graphs_lock: Arc<RwLock<HashMap<String, TaskGraph>>>) -> Result<String> {
        let mut completed_count = 0;
        let mut total_tasks = graph.nodes.len();
        
        loop {
            // Check global graph status (Pause/Resume)
            // Refresh local graph from shared state to catch external updates (e.g. pause/resume/prompt update)
            {
                let active = active_graphs_lock.read().await;
                if let Some(shared_graph) = active.get(graph_id) {
                    graph = shared_graph.clone();
                }
            }
            
            if matches!(graph.status, GraphStatus::Paused) {
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }

            if completed_count >= total_tasks {
                break;
            }

            let ready_tasks = graph.get_ready_tasks();
            
            if ready_tasks.is_empty() {
                 // Check for failure or deadlock
                 let failed_info = graph.nodes.values().find_map(|n| {
                     if let TaskStatus::Failed { error, .. } = &n.status {
                         Some((n.id.clone(), error.clone()))
                     } else {
                         None
                     }
                 });

                 if let Some((failed_id, error)) = failed_info {
                     // --- REPLANNING LOGIC ---
                     warn!("Task {} failed: {}. Initiating replanning...", failed_id, error);
                     
                     match self.replan_graph(&mut graph, &failed_id, &error).await {
                         Ok(new_graph) => {
                             info!("Replanning successful. Resuming execution.");
                             graph = new_graph;
                             
                             // Update shared state
                             {
                                 let mut active = active_graphs_lock.write().await;
                                 active.insert(graph_id.to_string(), graph.clone());
                             }
                             // Persist
                             let _ = self.persister.persist_graph(graph_id, &graph, goal).await;
                             
                             // Recalculate totals
                             total_tasks = graph.nodes.len(); 
                             completed_count = graph.nodes.values().filter(|n| matches!(n.status, TaskStatus::Completed { .. })).count();
                             continue;
                         }
                         Err(e) => {
                             error!("Replanning failed: {}", e);
                             graph.status = GraphStatus::Failed;
                             
                             {
                                 let mut active = active_graphs_lock.write().await;
                                 active.insert(graph_id.to_string(), graph.clone());
                             }
                             let _ = self.persister.persist_graph(graph_id, &graph, goal).await;
                             
                             return Err(anyhow::anyhow!("Task execution failed and replanning failed: {}", e));
                         }
                     }
                 }
                 
                 // Simple wait
                 tokio::time::sleep(Duration::from_millis(500)).await;
                 continue;
            }
            
            for task_id in ready_tasks {
                let task_node = if let Some(node) = graph.nodes.get(&task_id) {
                    node.clone()
                } else {
                    continue;
                };
                let role = task_node.agent_role.clone();
                let prompt = task_node.prompt.clone();
                let is_fail_test = task_node.prompt.contains("fail_me");

                let selected_role = if is_fail_test {
                    role.clone()
                } else {
                    let candidate_roles = self.smart_allocator.suggest_candidate_roles(&task_node);
                    let available_agents: Vec<AgentId> = candidate_roles
                        .iter()
                        .map(|r| AgentId::from_name(r))
                        .collect();
                    let decision = self.smart_allocator.allocate_with_decision(&task_node, &available_agents).await;
                    if let Some(bus) = &self.event_bus {
                        let payload = json!({
                            "task_id": task_id,
                            "requested_role": role,
                            "selected_role": decision.selected_role,
                            "selected_agent_id": decision.selected_agent_id,
                            "candidates": decision.candidates,
                        });
                        bus.publish(AgentEvent::SwarmActivity {
                            task_id: task_id.clone(),
                            graph_id: graph_id.to_string(),
                            from: "SmartAllocator".to_string(),
                            to: decision.selected_role.clone(),
                            message_type: "AllocatorDecision".to_string(),
                            content: payload.to_string(),
                            timestamp: chrono::Utc::now().timestamp_millis(),
                        });
                    }
                    decision.selected_role
                };

                // CHECK LIMITS
                let limit = *self.limits.get(&selected_role).unwrap_or(&100); 
                let current_active = self.active_agents.get(&selected_role).map(|v| *v).unwrap_or(0);
                
                if current_active >= limit {
                    debug!("Rate limit reached for role {}: {}/{}", selected_role, current_active, limit);
                    continue; 
                }
                
                let _permit = self.task_semaphore.acquire().await.map_err(|_| anyhow::anyhow!("Semaphore closed"))?;

                // 1. Mark as Running
                if let Some(node) = graph.nodes.get_mut(&task_id) {
                    node.status = TaskStatus::Running { started_at: chrono::Utc::now().timestamp() };
                }
                
                // Increment active count & Load
                *self.active_agents.entry(selected_role.clone()).or_insert(0) += 1;
                self.capability_router.update_load(&selected_role, 1);

                // 2. Update shared state & Persist
                {
                    let mut active = active_graphs_lock.write().await;
                    active.insert(graph_id.to_string(), graph.clone());
                }
                let _ = self.persister.persist_graph(graph_id, &graph, goal).await;
                
                // 3. Execute Agent
                let start_time = std::time::Instant::now();
                let agent_result = if is_fail_test {
                    Err(anyhow::anyhow!("Simulated failure triggered"))
                } else {
                    // Create/Get Agent
                    match self.agent_factory.create_agent_by_role(&selected_role) {
                        Ok(agent) => {
                            // Prepare context from dependencies
                            let mut context = Vec::new();
                            
                            if let Some(node) = graph.nodes.get(&task_id) {
                                for dep_id in &node.dependencies {
                                    if let Some(dep_node) = graph.nodes.get(dep_id) {
                                        if let Some(res) = &dep_node.result {
                                            context.push(ChatMessage::user(format!(
                                                "Result from previous task ({} - {}):\n{}", 
                                                dep_node.agent_role, dep_id, res
                                            )));
                                        }
                                    }
                                }
                            }
                            
                            // Execute with timeout
                            let timeout_ms = graph.nodes.get(&task_id).map(|n| n.timeout_ms).unwrap_or(30000);
                            
                            match tokio::time::timeout(
                                Duration::from_millis(timeout_ms), 
                                agent.execute(&prompt, &context)
                            ).await {
                                Ok(res) => res,
                                Err(_) => Err(anyhow::anyhow!("Agent execution timed out")),
                            }
                        },
                        Err(e) => Err(e),
                    }
                };
                
                // Decrement active count
                *self.active_agents.entry(selected_role.clone()).or_insert(0) -= 1;
                self.capability_router.update_load(&selected_role, -1);
                
                // Record metrics
                let elapsed = start_time.elapsed().as_millis() as u64;
                self.capability_router.record_result(&selected_role, agent_result.is_ok(), elapsed);

                let agent_result = match agent_result {
                    Ok(content) => {
                        if self.should_request_hitl_review(&selected_role, &prompt, &content) {
                            let review_type = self.review_type_for(&selected_role, &prompt, &content);
                            if let Some(bus) = &self.event_bus {
                                let payload = json!({
                                    "task_id": task_id,
                                    "graph_id": graph_id,
                                    "review_type": review_type,
                                    "role": selected_role,
                                    "prompt": prompt,
                                    "output_preview": content.chars().take(600).collect::<String>(),
                                });
                                bus.publish(AgentEvent::SwarmActivity {
                                    task_id: task_id.clone(),
                                    graph_id: graph_id.to_string(),
                                    from: "HITL".to_string(),
                                    to: "HumanReviewer".to_string(),
                                    message_type: "HITLReviewRequested".to_string(),
                                    content: payload.to_string(),
                                    timestamp: chrono::Utc::now().timestamp_millis(),
                                });
                            }

                            let decision = self.hitl.request_review(
                                graph_id,
                                &task_id,
                                &content,
                                review_type,
                            ).await;

                            if let Some(bus) = &self.event_bus {
                                let payload = json!({
                                    "task_id": task_id,
                                    "graph_id": graph_id,
                                    "decision": decision,
                                    "role": selected_role,
                                });
                                bus.publish(AgentEvent::SwarmActivity {
                                    task_id: task_id.clone(),
                                    graph_id: graph_id.to_string(),
                                    from: "HumanReviewer".to_string(),
                                    to: "HITL".to_string(),
                                    message_type: "HITLDecision".to_string(),
                                    content: payload.to_string(),
                                    timestamp: chrono::Utc::now().timestamp_millis(),
                                });
                            }

                            match decision {
                                HumanDecision::Approved => Ok(content),
                                HumanDecision::Edited(edited) => Ok(edited),
                                HumanDecision::Feedback(feedback) => Ok(format!("{}\n\n[HumanFeedback]\n{}", content, feedback)),
                                HumanDecision::Rejected(reason) => Err(anyhow::anyhow!("Rejected by human reviewer: {}", reason)),
                                HumanDecision::Selected(_) => Ok(content),
                                HumanDecision::Timeout => Ok(content),
                            }
                        } else {
                            Ok(content)
                        }
                    }
                    Err(e) => Err(e),
                };

                // 4. Update Result
                if let Some(node) = graph.nodes.get_mut(&task_id) {
                    match agent_result {
                        Ok(content) => {
                            node.status = TaskStatus::Completed { duration: 500 }; 
                            node.result = Some(content.clone());
                            
                            // Simulate communication events
                            if let Some(bus) = &self.event_bus {
                                 bus.publish(AgentEvent::SwarmActivity {
                                     task_id: task_id.clone(),
                                     graph_id: graph_id.to_string(),
                                     from: "Orchestrator".to_string(),
                                     to: selected_role.clone(),
                                     message_type: "Task".to_string(),
                                     content: format!("Execute: {}", prompt),
                                     timestamp: chrono::Utc::now().timestamp_millis(),
                                 });
                                 
                                 bus.publish(AgentEvent::SwarmActivity {
                                     task_id: task_id.clone(),
                                     graph_id: graph_id.to_string(),
                                     from: selected_role.clone(),
                                     to: "Orchestrator".to_string(),
                                     message_type: "Result".to_string(),
                                     content: "Task completed successfully".to_string(),
                                     timestamp: chrono::Utc::now().timestamp_millis(),
                                 });
                            }
                            
                            let log1 = "Task started.".to_string();
                            self.emit_log(graph_id, &task_id, log1.clone()).await;
                            node.logs.push(log1);
                            
                            let log3 = "Task completed successfully.".to_string();
                            self.emit_log(graph_id, &task_id, log3.clone()).await;
                            node.logs.push(log3);
                            
                            completed_count += 1;
                        },
                        Err(e) => {
                             node.status = TaskStatus::Failed { error: e.to_string(), retries: 0 };
                             let log_msg = format!("Error: {}", e);
                             self.emit_log(graph_id, &task_id, log_msg.clone()).await;
                             node.logs.push(log_msg);
                        }
                    }
                }
                
                // 5. Update shared state & Persist again
                {
                    let mut active = active_graphs_lock.write().await;
                    active.insert(graph_id.to_string(), graph.clone());
                }
                let _ = self.persister.persist_graph(graph_id, &graph, goal).await;
            }
        }
        
        graph.status = GraphStatus::Completed;
        {
            let mut active = active_graphs_lock.write().await;
            active.insert(graph_id.to_string(), graph.clone());
        }
        let _ = self.persister.persist_graph(graph_id, &graph, goal).await;
        
        // Aggregate results from Leaf Nodes
        let all_dependencies: std::collections::HashSet<_> = graph.nodes.values()
            .flat_map(|n| &n.dependencies)
            .collect();
            
        let final_result = graph.nodes.values()
            .filter(|n| !all_dependencies.contains(&n.id))
            .map(|n| n.result.clone().unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n");
            
        Ok(if final_result.is_empty() { "Done".to_string() } else { final_result })
    }

    async fn emit_log(&self, graph_id: &str, task_id: &str, content: String) {
        if let Some(bus) = &self.event_bus {
            bus.publish(AgentEvent::SwarmLog {
                graph_id: graph_id.to_string(),
                task_id: task_id.to_string(),
                content,
                timestamp: chrono::Utc::now().timestamp_millis(),
            });
        }
    }

    fn should_request_hitl_review(&self, role: &str, prompt: &str, output: &str) -> bool {
        let role_hitl = matches!(role, "reviewer" | "security" | "planner");
        let p = prompt.to_lowercase();
        let risky = p.contains("delete") || p.contains("deploy") || p.contains("migration") || p.contains("security") || p.contains("生产");
        role_hitl || risky || output.len() > 1600
    }

    fn review_type_for(&self, role: &str, prompt: &str, output: &str) -> ReviewType {
        if role == "security" {
            return ReviewType::Approval;
        }
        if output.len() > 2000 || prompt.to_lowercase().contains("rewrite") {
            return ReviewType::Edit;
        }
        if prompt.to_lowercase().contains("choose") || prompt.contains("选") {
            return ReviewType::Selection;
        }
        ReviewType::FreeformFeedback
    }

    async fn replan_graph(&self, graph: &mut TaskGraph, failed_task_id: &str, error: &str) -> Result<TaskGraph> {
        // Layer 1: Task-Level Retry
        if let Some(node) = graph.nodes.get_mut(failed_task_id) {
            if node.retry_count < node.max_retries {
                node.retry_count += 1;
                let backoff_secs = 2u64.pow(node.retry_count);
                info!("Task {} failed. Retrying ({}/{}) in {}s...", failed_task_id, node.retry_count, node.max_retries, backoff_secs);
                
                node.status = TaskStatus::Pending;
                node.logs.push(format!("Retry {}/{}: {}", node.retry_count, node.max_retries, error));
                
                return Ok(graph.clone());
            }
        }

        // Layer 2: LLM Replanning
        let prompt = format!(
            "A task in the swarm execution failed after retries. Please modify the plan to recover.\n\
            Failed Task ID: {}\n\
            Error: {}\n\
            \n\
            Current Graph State (JSON): {}\n\
            \n\
            Instructions:\n\
            1. Analyze the failure.\n\
            2. Return a MODIFIED JSON task graph.\n\
            3. STRATEGY: Try to split the task into smaller steps, or change the agent role if appropriate.\n\
            4. Mark the failed task as 'Pending' if you want to retry it with changes, or remove it and add new ones.\n\
            \n\
            Output JSON format ONLY (same structure as before).",
            failed_task_id,
            error,
            serde_json::to_string(&graph.nodes).unwrap_or_default()
        );

        let messages = vec![ChatMessage::user(&prompt)];
        let response = self.llm.chat_complete(&messages).await?;
        
        let json_str = if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                &response[start..=end]
            } else {
                &response
            }
        } else {
            &response
        };
        
        #[derive(serde::Deserialize)]
        struct TaskDef {
            id: String,
            role: String,
            prompt: String,
            dependencies: Vec<String>,
        }
        #[derive(serde::Deserialize)]
        struct GraphDef {
            tasks: Vec<TaskDef>,
        }

        if let Ok(def) = serde_json::from_str::<GraphDef>(json_str) {
             let mut new_graph = TaskGraph::new();
             for task in def.tasks {
                 let id = task.id.clone();
                 // Preserve status if it was already completed
                 let existing_status = if let Some(old_node) = graph.nodes.get(&id) {
                     if matches!(old_node.status, TaskStatus::Completed { .. }) {
                         old_node.status.clone()
                     } else {
                         TaskStatus::Pending
                     }
                 } else {
                     TaskStatus::Pending
                 };
                 
                 let result = graph.nodes.get(&id).and_then(|n| n.result.clone());
                 let logs = graph.nodes.get(&id).map(|n| n.logs.clone()).unwrap_or_default();

                 new_graph.nodes.insert(id.clone(), crate::agent::swarm::types::TaskNode {
                    id: id.clone(),
                    agent_role: task.role,
                    prompt: task.prompt,
                    dependencies: task.dependencies,
                    status: existing_status,
                    result,
                    logs,
                    priority: 128,
                    timeout_ms: 30000,
                    max_retries: 3,
                    retry_count: 0,
                 });
             }
             Ok(new_graph)
        } else {
             warn!("LLM replanning parsing failed. Applying simple retry.");
             let mut new_graph = graph.clone();
             if let Some(node) = new_graph.nodes.get_mut(failed_task_id) {
                 node.status = TaskStatus::Pending; 
                 node.logs.push("Replanning: Simple retry triggered.".to_string());
             }
             Ok(new_graph)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::mocks::MockLlmClient;
    use crate::agent::factory::AgentFactory;
    use crate::agent::capability::CapabilityRouter;
    use crate::agent::swarm::persister::SwarmPersister;
    use crate::agent::swarm::types::{TaskGraph, TaskStatus};
    
    #[tokio::test]
    async fn test_swarm_executor_retry_logic() {
        // Setup Mocks
        let llm = Arc::new(Box::new(MockLlmClient::new()) as Box<dyn crate::cognitive::llm::LlmClient>);
        let event_bus = Arc::new(crate::events::EventBus::new(100));
        let factory = Arc::new(AgentFactory::new(llm.clone(), event_bus.clone()));
        let router = Arc::new(CapabilityRouter::new());
        let persister = Arc::new(SwarmPersister::new(None));
        
        let executor = SwarmExecutor::new(
            llm.clone(),
            factory,
            router,
            None,
            persister
        );
        
        // Setup Graph with 1 task that fails
        let mut graph = TaskGraph::new();
        graph.add_task("T1".to_string(), "coder".to_string(), "fail_me".to_string(), vec![]);
        
        // We can't easily test the loop in execute_graph without mocking agents to fail deterministically
        // But replan_graph is testable
        
        // Simulate failure
        let new_graph = executor.replan_graph(&mut graph, "T1", "simulated error").await.unwrap();
        
        // Should be pending (retry)
        let node = new_graph.nodes.get("T1").unwrap();
        assert!(matches!(node.status, TaskStatus::Pending));
        assert_eq!(node.retry_count, 1);
        assert!(node.logs.last().unwrap().contains("Retry 1/3"));
    }
}
