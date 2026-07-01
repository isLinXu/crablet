use crate::agent::capability::CapabilityRouter;
use crate::agent::factory::AgentFactory;
use crate::agent::handoff::Handoff;
use crate::agent::harness::{AgentHarnessContext, HarnessConfig};
use crate::agent::harness_agent::{
    HarnessAgent, HarnessAgentBuilder, HarnessAgentResult, HarnessExecutionState,
};
use crate::agent::hitl::{HumanDecision, HumanInTheLoop, ReviewType};
use crate::agent::smart_allocator::SmartTaskAllocator;
use crate::agent::swarm::persister::SwarmPersister;
use crate::agent::swarm::types::{AgentId, GraphStatus, TaskGraph, TaskNode, TaskStatus};
use crate::cognitive::llm::LlmClient;
use crate::events::{AgentEvent, EventBus};
use crate::types::Message as ChatMessage;
use anyhow::Result;
use dashmap::DashMap;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tokio::time::Duration;
use tracing::{debug, error, info, warn};

pub struct SwarmExecutor {
    pub llm: Arc<dyn LlmClient>,
    pub agent_factory: Arc<AgentFactory>,
    pub capability_router: Arc<CapabilityRouter>,
    pub smart_allocator: Arc<SmartTaskAllocator>,
    pub hitl: Arc<HumanInTheLoop>,
    pub event_bus: Option<Arc<EventBus>>,
    pub persister: Arc<SwarmPersister>,

    // Runtime state
    pub active_agents: Arc<DashMap<String, i64>>, // Track active agents by role
    running_harnesses: Arc<DashMap<String, RunningHarnessTask>>,
    pub limits: HashMap<String, i64>, // Concurrency limits by role
    pub task_semaphore: Arc<Semaphore>,

    /// Dynamic timeout engine (optional). When present, task timeouts are
    /// computed from historical performance, system load, and task complexity
    /// instead of using the static `task_node.timeout_ms`.
    pub dynamic_timeout: Option<Arc<super::dynamic_timeout::DynamicTimeoutEngine>>,
}

#[derive(Debug, Clone)]
struct RunningHarnessTask {
    graph_id: String,
    harness: Arc<RwLock<AgentHarnessContext>>,
}

#[derive(Debug, Clone)]
struct SwarmExecutionProfile {
    max_steps: usize,
    tool_timeout_ms: u64,
    step_timeout_ms: u64,
    max_memory_bytes: u64,
    max_cpu_time_ms: u64,
    enable_self_reflection: bool,
}

impl SwarmExecutionProfile {
    fn for_role(role: &str, task_timeout_ms: u64) -> Self {
        let base = match role {
            "researcher" => Self {
                max_steps: 4,
                tool_timeout_ms: 30_000,
                step_timeout_ms: task_timeout_ms,
                max_memory_bytes: 512 * 1024 * 1024,
                max_cpu_time_ms: 120_000,
                enable_self_reflection: true,
            },
            "coder" | "analyst" | "drafter" => Self {
                max_steps: 3,
                tool_timeout_ms: 20_000,
                step_timeout_ms: task_timeout_ms,
                max_memory_bytes: 384 * 1024 * 1024,
                max_cpu_time_ms: 90_000,
                enable_self_reflection: true,
            },
            "reviewer" | "security" | "planner" | "critic" => Self {
                max_steps: 2,
                tool_timeout_ms: 15_000,
                step_timeout_ms: task_timeout_ms,
                max_memory_bytes: 256 * 1024 * 1024,
                max_cpu_time_ms: 45_000,
                enable_self_reflection: false,
            },
            _ => Self {
                max_steps: 3,
                tool_timeout_ms: 20_000,
                step_timeout_ms: task_timeout_ms,
                max_memory_bytes: 384 * 1024 * 1024,
                max_cpu_time_ms: 60_000,
                enable_self_reflection: true,
            },
        };

        Self {
            step_timeout_ms: task_timeout_ms.max(5_000).max(base.step_timeout_ms),
            ..base
        }
    }
}

#[derive(Clone)]
struct SwarmHarnessAgentAdapter {
    agent: crate::agent::SharedAgent,
}

impl SwarmHarnessAgentAdapter {
    fn new(agent: crate::agent::SharedAgent) -> Self {
        Self { agent }
    }
}

#[async_trait::async_trait]
impl HarnessAgent for SwarmHarnessAgentAdapter {
    fn name(&self) -> &str {
        self.agent.name()
    }

    fn role(&self) -> crate::agent::AgentRole {
        self.agent.role()
    }

    fn tools(&self) -> Vec<String> {
        Vec::new()
    }

    async fn execute_step(
        &self,
        task: &str,
        context: &[ChatMessage],
        _step_number: usize,
    ) -> Result<(String, Option<String>, Option<serde_json::Value>)> {
        let result = self.agent.execute(task, context).await?;
        Ok((result, None, None))
    }
}

impl SwarmExecutor {
    pub fn new(
        llm: Arc<dyn LlmClient>,
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
            running_harnesses: Arc::new(DashMap::new()),
            limits,
            task_semaphore: Arc::new(Semaphore::new(max_concurrent)),
            dynamic_timeout: None,
        }
    }

    /// Inject a dynamic timeout engine to enable adaptive timeout calculation
    pub fn with_dynamic_timeout(mut self, engine: Arc<super::dynamic_timeout::DynamicTimeoutEngine>) -> Self {
        self.dynamic_timeout = Some(engine);
        self
    }

    pub async fn execute_graph(
        &self,
        graph: TaskGraph,
        graph_id: &str,
        goal: &str,
        active_graphs_lock: Arc<RwLock<HashMap<String, TaskGraph>>>,
    ) -> Result<String> {
        let mut graph = graph;
        let mut should_refresh_from_shared = false;

        {
            let mut active = active_graphs_lock.write().await;
            active
                .entry(graph_id.to_string())
                .or_insert_with(|| graph.clone());
        }

        loop {
            // Check global graph status (Pause/Resume)
            // Refresh local graph from shared state to catch external updates (e.g. pause/resume/prompt update)
            if should_refresh_from_shared {
                graph = self
                    .refresh_shared_graph(&active_graphs_lock, graph_id)
                    .await?;
            } else {
                should_refresh_from_shared = true;
            }

            let total_tasks = graph.nodes.len();
            let completed_count = graph
                .nodes
                .values()
                .filter(|node| matches!(node.status, TaskStatus::Completed { .. }))
                .count();
            let running_tasks = graph
                .nodes
                .values()
                .filter(|node| matches!(node.status, TaskStatus::Running { .. }))
                .count();

            if completed_count >= total_tasks {
                break;
            }

            if matches!(graph.status, GraphStatus::Paused) {
                if running_tasks == 0 {
                    return Ok("Paused".to_string());
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
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
                    warn!(
                        "Task {} failed: {}. Initiating replanning...",
                        failed_id, error
                    );

                    match self.replan_graph(&mut graph, &failed_id, &error).await {
                        Ok(new_graph) => {
                            info!("Replanning successful. Resuming execution.");
                            graph = new_graph;

                            // Update shared state
                            self.sync_graph_state(&active_graphs_lock, graph_id, &graph, goal)
                                .await?;
                            continue;
                        }
                        Err(e) => {
                            error!("Replanning failed: {}", e);
                            graph.status = GraphStatus::Failed;

                            self.sync_graph_state(&active_graphs_lock, graph_id, &graph, goal)
                                .await?;

                            return Err(anyhow::anyhow!(
                                "Task execution failed and replanning failed: {}",
                                e
                            ));
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
                    let decision = self
                        .smart_allocator
                        .allocate_with_decision(&task_node, &available_agents)
                        .await;
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
                let current_active = self
                    .active_agents
                    .get(&selected_role)
                    .map(|v| *v)
                    .unwrap_or(0);

                if current_active >= limit {
                    debug!(
                        "Rate limit reached for role {}: {}/{}",
                        selected_role, current_active, limit
                    );
                    continue;
                }

                let _permit = self
                    .task_semaphore
                    .acquire()
                    .await
                    .map_err(|_| anyhow::anyhow!("Semaphore closed"))?;

                // Compute timeout: use dynamic engine if available, else fall back to static
                let timeout_ms = if let Some(ref engine) = self.dynamic_timeout {
                    // Infer task type from the prompt content
                    let prompt_lower = task_node.prompt.to_lowercase();
                    let task_type = if prompt_lower.contains("code") || prompt_lower.contains("implement") {
                        "coding"
                    } else if prompt_lower.contains("research") || prompt_lower.contains("search") {
                        "research"
                    } else if prompt_lower.contains("analyz") || prompt_lower.contains("evaluat") {
                        "analysis"
                    } else if prompt_lower.contains("draft") || prompt_lower.contains("write") {
                        "draft"
                    } else {
                        "general"
                    };
                    // Estimate complexity from prompt length (0.1 - 1.0)
                    let complexity = (task_node.prompt.len() as f32 / 500.0).clamp(0.1, 1.0);
                    engine.compute_timeout(
                        &selected_role,
                        task_type,
                        complexity,
                        task_node.priority,
                        false, // is_burst
                    ).await.as_millis() as u64
                } else {
                    task_node.timeout_ms
                };
                let context = self.build_dependency_context(&graph, &task_node, &selected_role);
                let execution_state = self.prepare_task_execution_state(&task_node, &context);

                // 1. Mark as Running
                if let Some(node) = graph.nodes.get_mut(&task_id) {
                    node.status = TaskStatus::Running {
                        started_at: chrono::Utc::now().timestamp(),
                    };
                    node.execution_state = Some(execution_state.clone());
                    if task_node.execution_state.is_some() {
                        node.logs.push(format!(
                            "Recovered harness execution state with {} prior trace steps.",
                            execution_state.trace.len()
                        ));
                    }
                }

                // Increment active count & Load
                *self.active_agents.entry(selected_role.clone()).or_insert(0) += 1;
                self.capability_router.update_load(&selected_role, 1);

                // 2. Update shared state & Persist
                self.sync_graph_state(&active_graphs_lock, graph_id, &graph, goal)
                    .await?;

                // 3. Execute Agent
                let start_time = std::time::Instant::now();
                let agent_result = if is_fail_test {
                    Err(anyhow::anyhow!("Simulated failure triggered"))
                } else {
                    // Create/Get Agent
                    match self.agent_factory.create_agent_by_role(&selected_role) {
                        Ok(agent) => {
                            let profile =
                                SwarmExecutionProfile::for_role(&selected_role, timeout_ms);
                            let harness_config = self.build_harness_config(
                                graph_id,
                                goal,
                                &task_node,
                                &selected_role,
                                profile,
                            );
                            let shared_harness = Arc::new(RwLock::new(AgentHarnessContext::new(
                                harness_config.clone(),
                            )));
                            self.register_running_harness(
                                graph_id,
                                &task_id,
                                shared_harness.clone(),
                            );

                            let result = match tokio::time::timeout(
                                Duration::from_millis(timeout_ms),
                                self.execute_agent_via_harness(
                                    agent,
                                    execution_state.clone(),
                                    harness_config,
                                    shared_harness.clone(),
                                ),
                            )
                            .await
                            {
                                Ok(res) => res,
                                Err(_) => {
                                    {
                                        let harness = shared_harness.read().await;
                                        harness.cancel();
                                    }
                                    Err(anyhow::anyhow!("Agent execution timed out"))
                                }
                            };
                            self.unregister_running_harness(graph_id, &task_id);
                            result
                        }
                        Err(e) => Err(e),
                    }
                };

                // Decrement active count
                *self.active_agents.entry(selected_role.clone()).or_insert(0) -= 1;
                self.capability_router.update_load(&selected_role, -1);

                // Record metrics
                let elapsed = start_time.elapsed().as_millis() as u64;
                let agent_succeeded = matches!(&agent_result, Ok(result) if result.success);
                self.capability_router
                    .record_result(&selected_role, agent_succeeded, elapsed);

                // Record to dynamic timeout engine for future predictions
                if let Some(ref engine) = self.dynamic_timeout {
                    let prompt_lower = task_node.prompt.to_lowercase();
                    let task_type = if prompt_lower.contains("code") || prompt_lower.contains("implement") {
                        "coding".to_string()
                    } else if prompt_lower.contains("research") || prompt_lower.contains("search") {
                        "research".to_string()
                    } else if prompt_lower.contains("analyz") || prompt_lower.contains("evaluat") {
                        "analysis".to_string()
                    } else if prompt_lower.contains("draft") || prompt_lower.contains("write") {
                        "draft".to_string()
                    } else {
                        "general".to_string()
                    };
                    engine.record_execution(super::dynamic_timeout::ExecutionRecord {
                        role: selected_role.clone(),
                        task_type,
                        duration_ms: elapsed,
                        success: agent_succeeded,
                        token_count: None,
                        timestamp: chrono::Utc::now(),
                    }).await;
                }

                let mut latest_execution_state = execution_state.clone();
                let agent_result = match agent_result {
                    Ok(result) if result.success => {
                        let mut content = result.output.clone();
                        let final_execution_state =
                            execution_state.clone().with_trace(result.trace.clone());
                        latest_execution_state = final_execution_state.clone();
                        if self.should_request_hitl_review(&selected_role, &prompt, &content) {
                            let review_type =
                                self.review_type_for(&selected_role, &prompt, &content);
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

                            let decision = self
                                .hitl
                                .request_review(graph_id, &task_id, &content, review_type)
                                .await;

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
                                HumanDecision::Approved => Ok((content, final_execution_state)),
                                HumanDecision::Edited(edited) => {
                                    content = edited;
                                    Ok((content, final_execution_state))
                                }
                                HumanDecision::Feedback(feedback) => {
                                    content =
                                        format!("{}\n\n[HumanFeedback]\n{}", content, feedback);
                                    Ok((content, final_execution_state))
                                }
                                HumanDecision::Rejected(reason) => {
                                    Err(anyhow::anyhow!("Rejected by human reviewer: {}", reason))
                                }
                                HumanDecision::Selected(_) => Ok((content, final_execution_state)),
                                HumanDecision::Timeout => Ok((content, final_execution_state)),
                            }
                        } else {
                            Ok((content, final_execution_state))
                        }
                    }
                    Ok(result) => {
                        latest_execution_state =
                            execution_state.clone().with_trace(result.trace.clone());
                        let message = result
                            .errors
                            .first()
                            .map(ToString::to_string)
                            .unwrap_or(result.output);
                        Err(anyhow::anyhow!(message))
                    }
                    Err(e) => Err(e),
                };

                if !self
                    .shared_task_is_still_running(&active_graphs_lock, graph_id, &task_id)
                    .await
                {
                    graph = self
                        .refresh_shared_graph(&active_graphs_lock, graph_id)
                        .await?;
                    break;
                }

                // 4. Update Result
                if let Some(node) = graph.nodes.get_mut(&task_id) {
                    match agent_result {
                        Ok((content, final_execution_state)) => {
                            node.status = TaskStatus::Completed { duration: 500 };
                            node.result = Some(content.clone());
                            node.execution_state = Some(final_execution_state);

                            // Draft Mode Enhancement: Update Canvas if role is drafter
                            if selected_role == "drafter" {
                                if let Some(bus) = &self.event_bus {
                                    bus.publish(AgentEvent::CanvasUpdate {
                                        title: format!("Draft Update: {}", task_id),
                                        content: content.clone(),
                                        kind: "markdown".to_string(),
                                    });
                                }
                            }

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
                        }
                        Err(e) => {
                            node.status = TaskStatus::Failed {
                                error: e.to_string(),
                                retries: 0,
                            };
                            node.execution_state = Some(latest_execution_state.clone());
                            let log_msg = format!("Error: {}", e);
                            self.emit_log(graph_id, &task_id, log_msg.clone()).await;
                            node.logs.push(log_msg);
                        }
                    }
                }

                // 5. Update shared state & Persist again
                self.sync_graph_state(&active_graphs_lock, graph_id, &graph, goal)
                    .await?;
            }
        }

        graph.status = GraphStatus::Completed;
        self.sync_graph_state(&active_graphs_lock, graph_id, &graph, goal)
            .await?;

        // Aggregate results from Leaf Nodes
        let all_dependencies: std::collections::HashSet<_> =
            graph.nodes.values().flat_map(|n| &n.dependencies).collect();

        let final_result = graph
            .nodes
            .values()
            .filter(|n| !all_dependencies.contains(&n.id))
            .map(|n| n.result.clone().unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n");

        Ok(if final_result.is_empty() {
            "Done".to_string()
        } else {
            final_result
        })
    }

    async fn refresh_shared_graph(
        &self,
        active_graphs_lock: &Arc<RwLock<HashMap<String, TaskGraph>>>,
        graph_id: &str,
    ) -> Result<TaskGraph> {
        let active = active_graphs_lock.read().await;
        active
            .get(graph_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Swarm graph {} has been removed", graph_id))
    }

    async fn sync_graph_state(
        &self,
        active_graphs_lock: &Arc<RwLock<HashMap<String, TaskGraph>>>,
        graph_id: &str,
        graph: &TaskGraph,
        goal: &str,
    ) -> Result<()> {
        let mut graph_to_store = graph.clone();
        let previous_graph;
        {
            let mut active = active_graphs_lock.write().await;
            let shared_graph = active
                .get(graph_id)
                .ok_or_else(|| anyhow::anyhow!("Swarm graph {} has been removed", graph_id))?;
            previous_graph = shared_graph.clone();
            if matches!(shared_graph.status, GraphStatus::Paused)
                && !matches!(
                    graph_to_store.status,
                    GraphStatus::Completed | GraphStatus::Failed
                )
            {
                graph_to_store.status = GraphStatus::Paused;
            }
            if !shared_graph.goal.is_empty() && graph_to_store.goal.is_empty() {
                graph_to_store.goal = shared_graph.goal.clone();
            }
            active.insert(graph_id.to_string(), graph_to_store.clone());
        }

        if let Err(error) = self
            .persister
            .persist_graph(graph_id, &graph_to_store, goal)
            .await
        {
            warn!(
                "Failed to persist swarm graph {} during execution: {}",
                graph_id, error
            );
        }

        self.emit_state_transitions(graph_id, Some(&previous_graph), &graph_to_store);

        Ok(())
    }

    fn register_running_harness(
        &self,
        graph_id: &str,
        task_id: &str,
        harness: Arc<RwLock<AgentHarnessContext>>,
    ) {
        self.running_harnesses.insert(
            Self::running_harness_key(graph_id, task_id),
            RunningHarnessTask {
                graph_id: graph_id.to_string(),
                harness,
            },
        );
    }

    fn unregister_running_harness(&self, graph_id: &str, task_id: &str) {
        self.running_harnesses
            .remove(&Self::running_harness_key(graph_id, task_id));
    }

    pub async fn cancel_graph_tasks(&self, graph_id: &str) -> usize {
        let harnesses = self
            .running_harnesses
            .iter()
            .filter(|entry| entry.value().graph_id == graph_id)
            .map(|entry| entry.value().harness.clone())
            .collect::<Vec<_>>();

        for harness in &harnesses {
            let harness = harness.read().await;
            harness.cancel();
        }

        harnesses.len()
    }

    async fn shared_task_is_still_running(
        &self,
        active_graphs_lock: &Arc<RwLock<HashMap<String, TaskGraph>>>,
        graph_id: &str,
        task_id: &str,
    ) -> bool {
        let active = active_graphs_lock.read().await;
        let Some(graph) = active.get(graph_id) else {
            return false;
        };

        matches!(
            graph.nodes.get(task_id).map(|node| &node.status),
            Some(TaskStatus::Running { .. })
        )
    }

    fn running_harness_key(graph_id: &str, task_id: &str) -> String {
        format!("{graph_id}:{task_id}")
    }

    #[cfg(test)]
    pub(crate) fn register_running_harness_for_test(
        &self,
        graph_id: &str,
        task_id: &str,
        harness: Arc<RwLock<AgentHarnessContext>>,
    ) {
        self.register_running_harness(graph_id, task_id, harness);
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

    pub(crate) fn emit_control_activity(
        &self,
        graph_id: &str,
        task_id: Option<&str>,
        message_type: &str,
        content: String,
    ) {
        if let Some(bus) = &self.event_bus {
            bus.publish(AgentEvent::SwarmActivity {
                task_id: task_id.unwrap_or("__graph__").to_string(),
                graph_id: graph_id.to_string(),
                from: "ControlPlane".to_string(),
                to: task_id.unwrap_or(graph_id).to_string(),
                message_type: message_type.to_string(),
                content,
                timestamp: chrono::Utc::now().timestamp_millis(),
            });
        }
    }

    pub(crate) fn emit_graph_update(&self, graph_id: &str, status: &GraphStatus) {
        if let Some(bus) = &self.event_bus {
            bus.publish(AgentEvent::SwarmGraphUpdate {
                graph_id: graph_id.to_string(),
                status: status.as_str().to_string(),
                timestamp: chrono::Utc::now().timestamp_millis(),
            });
        }
    }

    pub(crate) fn emit_task_update(&self, graph_id: &str, node: &TaskNode) {
        if let Some(bus) = &self.event_bus {
            let result_preview = node.result.as_ref().map(|result| {
                let mut preview = result.chars().take(500).collect::<String>();
                if result.chars().count() > 500 {
                    preview.push_str("...[truncated]");
                }
                preview
            });

            bus.publish(AgentEvent::SwarmTaskUpdate {
                graph_id: graph_id.to_string(),
                task_id: node.id.clone(),
                status: node.status.as_str().to_string(),
                result: result_preview,
                timestamp: chrono::Utc::now().timestamp_millis(),
            });
        }
    }

    pub(crate) fn emit_state_transitions(
        &self,
        graph_id: &str,
        previous_graph: Option<&TaskGraph>,
        current_graph: &TaskGraph,
    ) {
        let graph_status_changed = previous_graph
            .map(|graph| graph.status != current_graph.status)
            .unwrap_or(true);
        if graph_status_changed {
            self.emit_graph_update(graph_id, &current_graph.status);
        }

        for (task_id, node) in &current_graph.nodes {
            let previous_node = previous_graph.and_then(|graph| graph.nodes.get(task_id));
            let task_status_changed = previous_node
                .map(|previous| previous.status != node.status)
                .unwrap_or(true);
            if task_status_changed {
                self.emit_task_update(graph_id, node);
            }
        }
    }

    fn build_dependency_handoffs(
        &self,
        graph: &TaskGraph,
        task_node: &TaskNode,
        selected_role: &str,
    ) -> Vec<Handoff> {
        let mut handoffs = Vec::new();

        for dep_id in &task_node.dependencies {
            let Some(dep_node) = graph.nodes.get(dep_id) else {
                continue;
            };
            let Some(result) = &dep_node.result else {
                continue;
            };

            let preview = result.chars().take(800).collect::<String>();
            let full_result = if result.chars().count() > 4_000 {
                let truncated = result.chars().take(4_000).collect::<String>();
                format!("{}\n\n[truncated]", truncated)
            } else {
                result.clone()
            };

            let handoff = Handoff::new(
                &dep_node.agent_role,
                selected_role,
                "Task dependency completed",
                &format!(
                    "Dependency {} ({}) finished for task {}.\nResult preview:\n{}",
                    dep_id, dep_node.agent_role, task_node.id, preview
                ),
            )
            .with_artifact("dependency_result", &full_result)
            .with_variable("dependency_task_id", json!(dep_id))
            .with_variable("dependency_role", json!(dep_node.agent_role))
            .with_variable("target_task_id", json!(task_node.id))
            .with_variable("target_role", json!(selected_role));

            handoffs.push(handoff);
        }

        handoffs
    }

    fn build_dependency_context(
        &self,
        graph: &TaskGraph,
        task_node: &TaskNode,
        selected_role: &str,
    ) -> Vec<ChatMessage> {
        let handoffs = self.build_dependency_handoffs(graph, task_node, selected_role);
        let mut context = Vec::new();

        for handoff in handoffs {
            let artifacts = handoff
                .context
                .artifacts
                .iter()
                .map(|artifact| {
                    format!(
                        "- {} ({})\n{}",
                        artifact.name, artifact.content_type, artifact.content
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");

            context.push(ChatMessage::user(format!(
                "Structured handoff from {} to {}.\nReason: {}\nSummary:\n{}\n\nArtifacts:\n{}\n",
                handoff.from_agent,
                handoff.to_agent,
                handoff.reason,
                handoff.context.conversation_summary,
                artifacts
            )));
        }

        context
    }

    fn build_harness_config(
        &self,
        graph_id: &str,
        goal: &str,
        task_node: &TaskNode,
        selected_role: &str,
        profile: SwarmExecutionProfile,
    ) -> HarnessConfig {
        let mut metadata = HashMap::new();
        metadata.insert("execution_mode".to_string(), "swarm_harness".to_string());
        metadata.insert("graph_id".to_string(), graph_id.to_string());
        metadata.insert("task_id".to_string(), task_node.id.clone());
        metadata.insert("task_role".to_string(), selected_role.to_string());
        metadata.insert("goal".to_string(), goal.to_string());
        metadata.insert(
            "dependency_count".to_string(),
            task_node.dependencies.len().to_string(),
        );
        metadata.insert(
            "tool_timeout_ms".to_string(),
            profile.tool_timeout_ms.to_string(),
        );

        HarnessConfig {
            max_steps: profile.max_steps,
            tool_timeout: Duration::from_millis(profile.tool_timeout_ms),
            step_timeout: Duration::from_millis(profile.step_timeout_ms),
            enable_self_reflection: profile.enable_self_reflection,
            circuit_breaker: None,
            max_memory_bytes: Some(profile.max_memory_bytes),
            max_cpu_time_ms: Some(profile.max_cpu_time_ms),
            metadata,
        }
    }

    fn prepare_task_execution_state(
        &self,
        task_node: &TaskNode,
        context: &[ChatMessage],
    ) -> HarnessExecutionState {
        let mut state = task_node
            .execution_state
            .clone()
            .unwrap_or_else(|| HarnessExecutionState::new(task_node.prompt.clone(), context, None));
        state.task = task_node.prompt.clone();
        if state.context.is_empty() {
            state.context = context.to_vec();
        }
        state
    }

    async fn execute_agent_via_harness(
        &self,
        agent: crate::agent::SharedAgent,
        state: HarnessExecutionState,
        config: HarnessConfig,
        shared_harness: Arc<RwLock<AgentHarnessContext>>,
    ) -> Result<HarnessAgentResult> {
        let adapter = SwarmHarnessAgentAdapter::new(agent);
        HarnessAgentBuilder::new(Arc::new(adapter))
            .with_config(config)
            .with_shared_harness(shared_harness)
            .execute_from_state(state)
            .await
    }

    fn should_request_hitl_review(&self, role: &str, prompt: &str, output: &str) -> bool {
        let role_hitl = matches!(role, "reviewer" | "security" | "planner");
        let p = prompt.to_lowercase();
        let risky = p.contains("delete")
            || p.contains("deploy")
            || p.contains("migration")
            || p.contains("security")
            || p.contains("生产");
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

    async fn replan_graph(
        &self,
        graph: &mut TaskGraph,
        failed_task_id: &str,
        error: &str,
    ) -> Result<TaskGraph> {
        // Layer 1: Task-Level Retry
        if let Some(node) = graph.nodes.get_mut(failed_task_id) {
            if node.retry_count < node.max_retries {
                node.retry_count += 1;
                let backoff_secs = 2u64.pow(node.retry_count);
                info!(
                    "Task {} failed. Retrying ({}/{}) in {}s...",
                    failed_task_id, node.retry_count, node.max_retries, backoff_secs
                );

                node.status = TaskStatus::Pending;
                node.logs.push(format!(
                    "Retry {}/{}: {}",
                    node.retry_count, node.max_retries, error
                ));

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
                let logs = graph
                    .nodes
                    .get(&id)
                    .map(|n| n.logs.clone())
                    .unwrap_or_default();
                let execution_state = graph.nodes.get(&id).and_then(|n| n.execution_state.clone());

                new_graph.nodes.insert(
                    id.clone(),
                    crate::agent::swarm::types::TaskNode {
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
                        execution_state,
                    },
                );
            }
            Ok(new_graph)
        } else {
            warn!("LLM replanning parsing failed. Applying simple retry.");
            let mut new_graph = graph.clone();
            if let Some(node) = new_graph.nodes.get_mut(failed_task_id) {
                node.status = TaskStatus::Pending;
                node.logs
                    .push("Replanning: Simple retry triggered.".to_string());
            }
            Ok(new_graph)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::capability::CapabilityRouter;
    use crate::agent::factory::AgentFactory;
    use crate::agent::swarm::persister::SwarmPersister;
    use crate::agent::swarm::types::{TaskGraph, TaskStatus};
    use crate::testing::mocks::MockLlmClient;
    use async_trait::async_trait;
    use parking_lot::Mutex;
    use std::collections::VecDeque;

    #[derive(Clone, Default)]
    struct SequenceLlm {
        responses: Arc<Mutex<VecDeque<String>>>,
        prompts: Arc<Mutex<Vec<String>>>,
    }

    impl SequenceLlm {
        fn new(responses: &[&str]) -> Self {
            Self {
                responses: Arc::new(Mutex::new(
                    responses
                        .iter()
                        .map(|response| response.to_string())
                        .collect(),
                )),
                prompts: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn last_prompt(&self) -> String {
            self.prompts.lock().last().cloned().unwrap_or_default()
        }
    }

    fn bias_executor_toward_role(executor: &SwarmExecutor, preferred_role: &str) {
        for role in [
            "researcher",
            "analyst",
            "reviewer",
            "planner",
            "security",
            "drafter",
            "critic",
        ] {
            if role != preferred_role {
                executor.capability_router.update_load(role, 50);
            }
        }
    }

    #[async_trait]
    impl crate::cognitive::llm::LlmClient for SequenceLlm {
        async fn chat_complete(&self, messages: &[crate::types::Message]) -> Result<String> {
            let prompt = messages
                .iter()
                .map(|message| {
                    message
                        .content
                        .as_ref()
                        .map(|parts| {
                            parts
                                .iter()
                                .map(|part| match part {
                                    crate::types::ContentPart::Text { text } => text.clone(),
                                    _ => String::new(),
                                })
                                .collect::<Vec<_>>()
                                .join("")
                        })
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
                .join("\n");

            self.prompts.lock().push(prompt);

            Ok(self
                .responses
                .lock()
                .pop_front()
                .unwrap_or_else(|| "Sequence default".to_string()))
        }

        async fn chat_complete_with_tools(
            &self,
            messages: &[crate::types::Message],
            _tools: &[serde_json::Value],
        ) -> Result<crate::types::Message> {
            let content = self.chat_complete(messages).await?;
            Ok(crate::types::Message::new("assistant", &content))
        }

        fn model_name(&self) -> &str {
            "sequence-test"
        }
    }

    #[tokio::test]
    async fn test_swarm_executor_retry_logic() {
        // Setup Mocks
        let llm =
            Arc::new(MockLlmClient::new()) as Arc<dyn crate::cognitive::llm::LlmClient>;
        let event_bus = Arc::new(crate::events::EventBus::new(100));
        let factory = Arc::new(AgentFactory::new(llm.clone(), event_bus.clone()));
        let router = Arc::new(CapabilityRouter::new());
        let persister = Arc::new(SwarmPersister::new(None));

        let executor = SwarmExecutor::new(llm.clone(), factory, router, None, persister);

        // Setup Graph with 1 task that fails
        let mut graph = TaskGraph::new();
        graph.add_task(
            "T1".to_string(),
            "coder".to_string(),
            "fail_me".to_string(),
            vec![],
        );

        // We can't easily test the loop in execute_graph without mocking agents to fail deterministically
        // But replan_graph is testable

        // Simulate failure
        let new_graph = executor
            .replan_graph(&mut graph, "T1", "simulated error")
            .await
            .unwrap();

        // Should be pending (retry)
        let node = new_graph.nodes.get("T1").unwrap();
        assert!(matches!(node.status, TaskStatus::Pending));
        assert_eq!(node.retry_count, 1);
        assert!(node.logs.last().unwrap().contains("Retry 1/3"));
    }

    #[test]
    fn test_swarm_execution_profile_prefers_stricter_review_roles() {
        let reviewer = SwarmExecutionProfile::for_role("reviewer", 30_000);
        let coder = SwarmExecutionProfile::for_role("coder", 30_000);

        assert!(reviewer.max_steps < coder.max_steps);
        assert!(reviewer.max_memory_bytes < coder.max_memory_bytes);
        assert!(!reviewer.enable_self_reflection);
    }

    #[test]
    fn test_build_dependency_context_uses_structured_handoff() {
        let llm =
            Arc::new(MockLlmClient::new()) as Arc<dyn crate::cognitive::llm::LlmClient>;
        let event_bus = Arc::new(crate::events::EventBus::new(100));
        let factory = Arc::new(AgentFactory::new(llm.clone(), event_bus.clone()));
        let router = Arc::new(CapabilityRouter::new());
        let persister = Arc::new(SwarmPersister::new(None));
        let executor = SwarmExecutor::new(llm, factory, router, None, persister);
        bias_executor_toward_role(&executor, "coder");

        let mut graph = TaskGraph::new();
        graph.add_task(
            "T1".to_string(),
            "coder".to_string(),
            "build alpha".to_string(),
            vec![],
        );
        graph.add_task(
            "T2".to_string(),
            "coder".to_string(),
            "build beta".to_string(),
            vec!["T1".to_string()],
        );
        graph.nodes.get_mut("T1").unwrap().result = Some("alpha result".to_string());

        let task_node = graph.nodes.get("T2").unwrap().clone();
        let context = executor.build_dependency_context(&graph, &task_node, "coder");

        assert_eq!(context.len(), 1);
        let prompt = context[0].content.as_ref().unwrap();
        let text = prompt
            .iter()
            .map(|part| match part {
                crate::types::ContentPart::Text { text } => text.clone(),
                _ => String::new(),
            })
            .collect::<Vec<_>>()
            .join("");
        assert!(text.contains("Structured handoff from coder to coder"));
        assert!(text.contains("alpha result"));
    }

    #[tokio::test]
    async fn test_execute_graph_uses_harness_bridge_without_step_prefix() {
        let llm_client = SequenceLlm::new(&["plain harness result"]);
        let llm =
            Arc::new(llm_client.clone()) as Arc<dyn crate::cognitive::llm::LlmClient>;
        let event_bus = Arc::new(crate::events::EventBus::new(100));
        let factory = Arc::new(AgentFactory::new(llm.clone(), event_bus.clone()));
        let router = Arc::new(CapabilityRouter::new());
        let persister = Arc::new(SwarmPersister::new(None));
        let executor = SwarmExecutor::new(llm, factory, router, None, persister);
        bias_executor_toward_role(&executor, "coder");

        let mut graph = TaskGraph::new();
        graph.add_task(
            "T1".to_string(),
            "coder".to_string(),
            "implement feature".to_string(),
            vec![],
        );

        let active_graphs = Arc::new(RwLock::new(HashMap::new()));
        let result = executor
            .execute_graph(graph, "graph-1", "ship feature", active_graphs.clone())
            .await
            .unwrap();

        assert_eq!(result, "plain harness result");
        assert!(!result.starts_with("Step 1:"));
        assert!(llm_client.last_prompt().contains("implement feature"));

        let stored = active_graphs.read().await;
        let graph = stored.get("graph-1").unwrap();
        let node = graph.nodes.get("T1").unwrap();
        assert!(matches!(node.status, TaskStatus::Completed { .. }));
        assert_eq!(node.result.as_deref(), Some("plain harness result"));
    }

    #[test]
    fn test_build_harness_config_marks_swarm_execution_metadata() {
        let llm =
            Arc::new(MockLlmClient::new()) as Arc<dyn crate::cognitive::llm::LlmClient>;
        let event_bus = Arc::new(crate::events::EventBus::new(100));
        let factory = Arc::new(AgentFactory::new(llm.clone(), event_bus.clone()));
        let router = Arc::new(CapabilityRouter::new());
        let persister = Arc::new(SwarmPersister::new(None));
        let executor = SwarmExecutor::new(llm, factory, router, None, persister);

        let mut graph = TaskGraph::new();
        graph.add_task(
            "T2".to_string(),
            "coder".to_string(),
            "build beta".to_string(),
            vec!["T1".to_string()],
        );

        let task_node = graph.nodes.get("T2").unwrap().clone();
        let profile = SwarmExecutionProfile::for_role("coder", task_node.timeout_ms);
        let config =
            executor.build_harness_config("graph-2", "ship beta", &task_node, "coder", profile);

        assert_eq!(
            config.metadata.get("execution_mode").map(String::as_str),
            Some("swarm_harness")
        );
        assert_eq!(
            config.metadata.get("graph_id").map(String::as_str),
            Some("graph-2")
        );
        assert_eq!(
            config.metadata.get("task_id").map(String::as_str),
            Some("T2")
        );
        assert_eq!(
            config.metadata.get("dependency_count").map(String::as_str),
            Some("1")
        );
        assert_eq!(
            config.step_timeout,
            Duration::from_millis(task_node.timeout_ms)
        );
    }
}
