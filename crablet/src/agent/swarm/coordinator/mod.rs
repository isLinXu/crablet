use crate::cognitive::llm::LlmClient;
use anyhow::Result;
use dashmap::DashMap;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{info, warn};
use uuid::Uuid;

use crate::agent::swarm::executor::SwarmExecutor;
use crate::agent::swarm::persister::SwarmPersister;
use crate::agent::swarm::types::{
    GraphStatus, NodeRecoveryOptions, TaskGraph, TaskGraphTemplate, TaskNode, TaskStatus,
};
use crate::types::Message as ChatMessage;

use serde::Deserialize;

pub struct SwarmCoordinator {
    pub llm: Arc<Box<dyn LlmClient>>,
    pub executor: Arc<SwarmExecutor>,
    pub persister: Arc<SwarmPersister>,
    pub active_graphs: Arc<RwLock<HashMap<String, TaskGraph>>>,
    pub running_graphs: Arc<DashMap<String, ()>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PauseGraphResult {
    pub quiesced: bool,
    pub running_tasks: usize,
}

impl SwarmCoordinator {
    pub fn new(
        llm: Arc<Box<dyn LlmClient>>,
        executor: Arc<SwarmExecutor>,
        persister: Arc<SwarmPersister>,
    ) -> Self {
        Self {
            llm,
            executor,
            persister,
            active_graphs: Arc::new(RwLock::new(HashMap::new())),
            running_graphs: Arc::new(DashMap::new()),
        }
    }

    pub async fn init(&self) -> Result<()> {
        self.persister.init_tables().await?;

        if let Ok(graphs) = self.persister.load_active_graphs().await {
            let mut active = self.active_graphs.write().await;
            for (id, graph) in graphs {
                active.insert(id, graph);
            }
            info!("Loaded {} active swarm graphs from database", active.len());
        }
        self.resume_loaded_graphs().await;
        Ok(())
    }

    pub async fn decompose_and_execute(&self, goal: &str) -> Result<String> {
        // Special case for Draft Mode
        if goal.to_lowercase().starts_with("draft ") {
            let topic = goal.chars().skip(6).collect::<String>();
            return self.start_draft_swarm(&topic).await;
        }

        // 1. Decompose goal into TaskGraph using LLM
        let graph = self.decompose_goal(goal).await?.with_goal(goal.to_string());

        // Store graph
        let graph_id = Uuid::new_v4().to_string();
        {
            let mut graphs = self.active_graphs.write().await;
            graphs.insert(graph_id.clone(), graph.clone());
        }

        // Persist initial state
        if let Err(e) = self.persister.persist_graph(&graph_id, &graph, goal).await {
            tracing::error!("Failed to persist swarm graph: {}", e);
        }

        self.executor.emit_control_activity(
            &graph_id,
            None,
            "GraphCreated",
            format!("Created swarm graph for goal: {}", goal),
        );
        self.executor
            .emit_state_transitions(&graph_id, None, &graph);

        // 2. Execute Graph
        // Note: executor needs reference to active_graphs lock
        self.executor
            .execute_graph(graph, &graph_id, goal, self.active_graphs.clone())
            .await
    }

    pub async fn start_draft_swarm(&self, topic: &str) -> Result<String> {
        let goal = format!("Draft: {}", topic);
        let mut graph = TaskGraph::new().with_goal(goal.clone());
        let t1 = "research_task".to_string();
        let t2 = "draft_task".to_string();
        let t3 = "critique_task".to_string();
        let t4 = "polish_task".to_string();

        graph.add_task(
            t1.clone(),
            "researcher".to_string(),
            format!(
                "Deeply research the following topic and provide key facts and insights: {}",
                topic
            ),
            vec![],
        );
        graph.add_task(
            t2.clone(),
            "drafter".to_string(),
            format!(
                "Write a comprehensive first draft about: {}. Use the research findings provided.",
                topic
            ),
            vec![t1],
        );
        graph.add_task(t3.clone(), "critic".to_string(), "Review the draft meticulously. Identify gaps, inconsistencies, and areas for improvement. Provide actionable feedback.".to_string(), vec![t2]);
        graph.add_task(t4.clone(), "drafter".to_string(), "Refine and polish the draft based on the critic's feedback. Ensure the final version is high-quality and professional.".to_string(), vec![t3]);

        let graph_id = Uuid::new_v4().to_string();
        {
            let mut graphs = self.active_graphs.write().await;
            graphs.insert(graph_id.clone(), graph.clone());
        }

        self.persister
            .persist_graph(&graph_id, &graph, &goal)
            .await?;

        self.executor.emit_control_activity(
            &graph_id,
            None,
            "GraphCreated",
            format!("Started draft swarm for topic: {}", topic),
        );
        self.executor
            .emit_state_transitions(&graph_id, None, &graph);

        self.spawn_graph_execution(graph_id.clone(), graph, goal);

        Ok(format!("Draft Mode Swarm started (Graph ID: {})", graph_id))
    }

    async fn decompose_goal(&self, goal: &str) -> Result<TaskGraph> {
        let prompt = format!(
            "Decompose the following goal into a dependency graph of subtasks for a swarm of agents.\n\
            Goal: \"{}\"\n\
            \n\
            Available Agent Roles: researcher, coder, analyst, reviewer, security, planner, drafter, critic\n\
            \n\
            Output JSON format ONLY:\n\
            {{\n\
              \"tasks\": [\n\
                {{ \"id\": \"t1\", \"role\": \"researcher\", \"prompt\": \"Search for...\", \"dependencies\": [] }},\n\
                {{ \"id\": \"t2\", \"role\": \"drafter\", \"prompt\": \"Draft the content based on...\", \"dependencies\": [\"t1\"] }},\n\
                {{ \"id\": \"t3\", \"role\": \"critic\", \"prompt\": \"Review the draft for...\", \"dependencies\": [\"t2\"] }}\n\
              ]\n\
            }}",
            goal
        );

        let messages = vec![ChatMessage::user(&prompt)];
        let response = self.llm.chat_complete(&messages).await?;

        // Extract JSON
        let json_str = if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                &response[start..=end]
            } else {
                &response
            }
        } else {
            &response
        };

        #[derive(Deserialize)]
        struct TaskDef {
            id: String,
            role: String,
            prompt: String,
            dependencies: Vec<String>,
        }

        #[derive(Deserialize)]
        struct GraphDef {
            tasks: Vec<TaskDef>,
        }

        let def: GraphDef = serde_json::from_str(json_str).map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse task graph JSON: {} (Response: {})",
                e,
                response
            )
        })?;

        let mut graph = TaskGraph::new();
        for task in def.tasks {
            graph.add_task(task.id, task.role, task.prompt, task.dependencies);
        }

        Ok(graph)
    }

    pub async fn save_template(
        &self,
        name: &str,
        description: &str,
        graph: &TaskGraph,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let template = TaskGraphTemplate {
            id: id.clone(),
            name: name.to_string(),
            description: description.to_string(),
            graph: graph.clone(),
            created_at: chrono::Utc::now().timestamp(),
        };
        self.persister.save_template(&template).await?;
        Ok(id)
    }

    pub async fn list_templates(&self) -> Result<Vec<TaskGraphTemplate>> {
        self.persister.list_templates().await
    }

    pub async fn instantiate_template(&self, template_id: &str, goal: &str) -> Result<String> {
        if let Some(json_str) = self.persister.get_template_json(template_id).await? {
            let mut graph: TaskGraph = serde_json::from_str(&json_str)?;

            // Assign new IDs to tasks to avoid conflicts
            let mut id_map = HashMap::new();
            let mut new_nodes = HashMap::new();

            for old_id in graph.nodes.keys() {
                let new_id = Uuid::new_v4().to_string();
                id_map.insert(old_id.clone(), new_id.clone());
            }

            for (old_id, node) in &graph.nodes {
                let new_id = id_map
                    .get(old_id)
                    .ok_or_else(|| anyhow::anyhow!("Dependency mapping failed for {}", old_id))?;
                let mut new_node = node.clone();
                new_node.id = new_id.clone();
                new_node.status = crate::agent::swarm::types::TaskStatus::Pending;
                new_node.result = None;
                new_node.logs = Vec::new();
                new_node.dependencies = node
                    .dependencies
                    .iter()
                    .filter_map(|d| id_map.get(d).cloned())
                    .collect();

                new_nodes.insert(new_id.clone(), new_node);
            }

            graph.nodes = new_nodes;
            graph.status = GraphStatus::Active;
            graph.goal = goal.to_string();

            let graph_id = Uuid::new_v4().to_string();
            {
                let mut active = self.active_graphs.write().await;
                active.insert(graph_id.clone(), graph.clone());
            }

            self.persister
                .persist_graph(&graph_id, &graph, goal)
                .await?;

            self.executor.emit_control_activity(
                &graph_id,
                None,
                "GraphInstantiated",
                format!(
                    "Instantiated template {} into active graph for goal: {}",
                    template_id, goal
                ),
            );
            self.executor
                .emit_state_transitions(&graph_id, None, &graph);

            self.spawn_graph_execution(graph_id.clone(), graph, goal.to_string());

            Ok(graph_id)
        } else {
            Err(anyhow::anyhow!("Template not found"))
        }
    }

    pub async fn add_task_to_graph(&self, graph_id: &str, task: TaskNode) -> Result<()> {
        // 1. Update active graph
        {
            let mut graphs = self.active_graphs.write().await;
            if let Some(graph) = graphs.get_mut(graph_id) {
                graph.nodes.insert(task.id.clone(), task.clone());
            } else {
                return Err(anyhow::anyhow!("Graph not found"));
            }
        }

        // 2. Persist
        let graph = {
            let graphs = self.active_graphs.read().await;
            graphs
                .get(graph_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Graph not found"))?
        };

        self.persist_graph_state(graph_id, &graph).await?;
        self.executor.emit_control_activity(
            graph_id,
            Some(&task.id),
            "NodeAdded",
            format!("Added task {} with role {}", task.id, task.agent_role),
        );
        self.executor.emit_task_update(graph_id, &task);
        Ok(())
    }

    pub async fn pause_graph(&self, graph_id: &str) -> Result<PauseGraphResult> {
        let (previous_graph, graph) = {
            let mut graphs = self.active_graphs.write().await;
            let graph = graphs
                .get_mut(graph_id)
                .ok_or_else(|| anyhow::anyhow!("Graph not found"))?;
            let previous_graph = graph.clone();
            graph.status = GraphStatus::Paused;
            (previous_graph, graph.clone())
        };

        self.persist_graph_state(graph_id, &graph).await?;
        self.executor
            .emit_state_transitions(graph_id, Some(&previous_graph), &graph);

        let running_tasks = Self::count_running_tasks(&graph);
        if running_tasks == 0 {
            self.executor.emit_control_activity(
                graph_id,
                None,
                "PauseRequested",
                "Paused graph and reached quiescence immediately.".to_string(),
            );
            return Ok(PauseGraphResult {
                quiesced: true,
                running_tasks: 0,
            });
        }

        let remaining_running_tasks = self
            .wait_for_graph_quiescence(graph_id, Self::pause_wait_budget(&graph))
            .await?;
        self.executor.emit_control_activity(
            graph_id,
            None,
            "PauseRequested",
            if remaining_running_tasks == 0 {
                "Pause requested; graph is now quiesced.".to_string()
            } else {
                format!(
                    "Pause requested; {} running task(s) are still draining.",
                    remaining_running_tasks
                )
            },
        );
        Ok(PauseGraphResult {
            quiesced: remaining_running_tasks == 0,
            running_tasks: remaining_running_tasks,
        })
    }

    pub async fn resume_graph(&self, graph_id: &str) -> Result<bool> {
        let (previous_graph, graph) = {
            let mut graphs = self.active_graphs.write().await;
            let graph = graphs
                .get_mut(graph_id)
                .ok_or_else(|| anyhow::anyhow!("Graph not found"))?;
            let previous_graph = graph.clone();
            Self::reactivate_recoverable_tasks(graph);
            graph.status = GraphStatus::Active;
            (previous_graph, graph.clone())
        };

        let goal = self.persist_graph_state(graph_id, &graph).await?;
        self.executor.emit_control_activity(
            graph_id,
            None,
            "ResumeRequested",
            "Resumed graph and re-queued recoverable tasks.".to_string(),
        );
        self.executor
            .emit_state_transitions(graph_id, Some(&previous_graph), &graph);
        Ok(self.spawn_graph_execution(graph_id.to_string(), graph, goal))
    }

    pub async fn retry_node(&self, graph_id: &str, node_id: &str) -> Result<bool> {
        self.recover_node(graph_id, node_id, NodeRecoveryOptions::default())
            .await
    }

    pub async fn recover_node(
        &self,
        graph_id: &str,
        node_id: &str,
        options: NodeRecoveryOptions,
    ) -> Result<bool> {
        let (previous_graph, graph, should_spawn, reset_count) = {
            let mut graphs = self.active_graphs.write().await;
            let graph = graphs
                .get_mut(graph_id)
                .ok_or_else(|| anyhow::anyhow!("Graph not found"))?;
            let previous_graph = graph.clone();

            if matches!(graph.status, GraphStatus::Active)
                && self.running_graphs.contains_key(graph_id)
            {
                return Err(anyhow::anyhow!(
                    "Graph is actively executing; pause it before retrying nodes"
                ));
            }
            let running_tasks = Self::count_running_tasks(graph);
            if running_tasks > 0 {
                return Err(anyhow::anyhow!(
                    "Graph pause is still draining; wait for {} running task(s) to finish before retrying nodes",
                    running_tasks
                ));
            }

            let node = graph
                .nodes
                .get(node_id)
                .ok_or_else(|| anyhow::anyhow!("Node not found"))?;
            if !node.status.is_retriable() {
                return Err(anyhow::anyhow!("Node is not in a retriable state"));
            }
            Self::validate_recovery_options(graph, node_id, &options)?;

            let retry_ids = Self::collect_retry_set(graph, node_id);
            let reset_count = retry_ids.len();
            if retry_ids.iter().any(|candidate_id| {
                graph
                    .nodes
                    .get(candidate_id)
                    .map(|candidate| matches!(candidate.status, TaskStatus::Running { .. }))
                    .unwrap_or(false)
            }) {
                return Err(anyhow::anyhow!(
                    "Cannot retry while affected nodes are still running"
                ));
            }

            for retry_id in retry_ids {
                if let Some(node) = graph.nodes.get_mut(&retry_id) {
                    let reset_reason = if retry_id == node_id {
                        Self::apply_recovery_overrides(node, &options)?
                    } else {
                        format!(
                            "Reset to Pending because upstream task {} is being recovered.",
                            node_id
                        )
                    };
                    node.status = TaskStatus::Pending;
                    node.result = None;
                    node.execution_state = None;
                    node.logs.push(reset_reason);
                }
            }

            if matches!(graph.status, GraphStatus::Completed | GraphStatus::Failed)
                || (options.resume_graph && matches!(graph.status, GraphStatus::Paused))
            {
                graph.status = GraphStatus::Active;
            }

            let should_spawn = matches!(graph.status, GraphStatus::Active);
            (previous_graph, graph.clone(), should_spawn, reset_count)
        };

        let goal = self.persist_graph_state(graph_id, &graph).await?;
        self.executor.emit_control_activity(
            graph_id,
            Some(node_id),
            "NodeRecoveryScheduled",
            Self::describe_recovery_action(node_id, reset_count, &options, should_spawn),
        );
        self.executor
            .emit_state_transitions(graph_id, Some(&previous_graph), &graph);
        Ok(if should_spawn {
            self.spawn_graph_execution(graph_id.to_string(), graph, goal)
        } else {
            false
        })
    }

    pub async fn update_node(
        &self,
        graph_id: &str,
        node_id: &str,
        prompt: String,
        dependencies: Option<Vec<String>>,
    ) -> Result<()> {
        let graph = {
            let mut graphs = self.active_graphs.write().await;
            let graph = graphs
                .get_mut(graph_id)
                .ok_or_else(|| anyhow::anyhow!("Graph not found"))?;

            if graph.status != GraphStatus::Paused {
                return Err(anyhow::anyhow!("Graph must be paused to update nodes"));
            }
            let running_tasks = Self::count_running_tasks(graph);
            if running_tasks > 0 {
                return Err(anyhow::anyhow!(
                    "Graph pause is still draining; wait for {} running task(s) to finish before updating nodes",
                    running_tasks
                ));
            }

            if let Some(deps) = &dependencies {
                if !deps
                    .iter()
                    .all(|dependency| graph.nodes.contains_key(dependency))
                {
                    return Err(anyhow::anyhow!("Invalid dependencies"));
                }
                if graph.detects_cycle(node_id, deps) {
                    return Err(anyhow::anyhow!("Cycle detected in dependencies"));
                }
            }

            let node = graph
                .nodes
                .get_mut(node_id)
                .ok_or_else(|| anyhow::anyhow!("Node not found"))?;
            if !matches!(node.status, TaskStatus::Pending) {
                return Err(anyhow::anyhow!("Only pending tasks can be updated"));
            }

            node.prompt = prompt;
            if let Some(deps) = dependencies {
                node.dependencies = deps;
            }

            graph.clone()
        };

        self.persist_graph_state(graph_id, &graph).await?;
        self.executor.emit_control_activity(
            graph_id,
            Some(node_id),
            "NodeUpdated",
            format!(
                "Updated task {} prompt/dependencies while graph was paused.",
                node_id
            ),
        );
        Ok(())
    }

    pub async fn delete_graph(&self, graph_id: &str) -> Result<()> {
        self.executor.cancel_graph_tasks(graph_id).await;
        let removed_from_memory = {
            let mut graphs = self.active_graphs.write().await;
            graphs.remove(graph_id)
        };
        self.running_graphs.remove(graph_id);

        let removed_from_db = self.persister.delete_graph(graph_id).await?;
        if removed_from_memory.is_some() || removed_from_db {
            self.executor.emit_control_activity(
                graph_id,
                None,
                "GraphDeleted",
                "Deleted graph state from memory and persistence.".to_string(),
            );
            Ok(())
        } else {
            Err(anyhow::anyhow!("Graph not found"))
        }
    }

    pub async fn cancel_graph(&self, graph_id: &str) -> Result<usize> {
        let cancelled_harnesses = self.executor.cancel_graph_tasks(graph_id).await;
        let cancelled_at = chrono::Utc::now().timestamp();

        let (previous_graph, updated_graph, cancelled_tasks) = {
            let mut graphs = self.active_graphs.write().await;
            let graph = graphs
                .get_mut(graph_id)
                .ok_or_else(|| anyhow::anyhow!("Graph not found"))?;
            let previous_graph = graph.clone();
            graph.status = GraphStatus::Paused;
            let mut cancelled_tasks = 0;

            for node in graph.nodes.values_mut() {
                if matches!(node.status, TaskStatus::Running { .. }) {
                    node.status = TaskStatus::Cancelled {
                        cancelled_at,
                        reason: "Cancelled by swarm control plane".to_string(),
                    };
                    node.result = None;
                    node.execution_state = None;
                    node.logs.push(
                        "Execution cancelled; resume or retry to rerun the task.".to_string(),
                    );
                    cancelled_tasks += 1;
                }
            }

            (previous_graph, graph.clone(), cancelled_tasks)
        };

        self.persist_graph_state(graph_id, &updated_graph).await?;
        self.executor.emit_control_activity(
            graph_id,
            None,
            "CancelRequested",
            format!(
                "Cancelled {} running task(s) and paused the graph.",
                cancelled_tasks.max(cancelled_harnesses)
            ),
        );
        self.executor
            .emit_state_transitions(graph_id, Some(&previous_graph), &updated_graph);
        Ok(cancelled_tasks.max(cancelled_harnesses))
    }

    async fn resume_loaded_graphs(&self) {
        let graphs = { self.active_graphs.read().await.clone() };
        for (graph_id, graph) in graphs {
            if matches!(graph.status, GraphStatus::Active) {
                let goal = graph.goal.clone();
                if self.spawn_graph_execution(graph_id.clone(), graph, goal) {
                    info!("Resumed active swarm graph {}", graph_id);
                }
            }
        }
    }

    fn spawn_graph_execution(&self, graph_id: String, graph: TaskGraph, goal: String) -> bool {
        if graph.status != GraphStatus::Active {
            return false;
        }
        if !graph.nodes.values().any(|node| {
            matches!(
                node.status,
                TaskStatus::Pending | TaskStatus::Running { .. }
            )
        }) {
            return false;
        }
        if self.running_graphs.contains_key(&graph_id) {
            return false;
        }

        self.running_graphs.insert(graph_id.clone(), ());
        let executor = self.executor.clone();
        let active_graphs = self.active_graphs.clone();
        let running_graphs = self.running_graphs.clone();
        let graph_id_clone = graph_id.clone();

        tokio::spawn(async move {
            let result = executor
                .execute_graph(graph, &graph_id_clone, &goal, active_graphs)
                .await;
            if let Err(error) = result {
                warn!(
                    "Swarm graph {} execution ended with error: {}",
                    graph_id_clone, error
                );
            }
            running_graphs.remove(&graph_id_clone);
        });

        true
    }

    async fn wait_for_graph_quiescence(&self, graph_id: &str, timeout: Duration) -> Result<usize> {
        let deadline = std::time::Instant::now() + timeout;

        loop {
            let running_tasks = {
                let graphs = self.active_graphs.read().await;
                let graph = graphs
                    .get(graph_id)
                    .ok_or_else(|| anyhow::anyhow!("Graph not found"))?;
                Self::count_running_tasks(graph)
            };

            if running_tasks == 0 {
                return Ok(0);
            }

            if std::time::Instant::now() >= deadline {
                return Ok(running_tasks);
            }

            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    fn pause_wait_budget(graph: &TaskGraph) -> Duration {
        let max_running_timeout_ms = graph
            .nodes
            .values()
            .filter(|node| matches!(node.status, TaskStatus::Running { .. }))
            .map(|node| node.timeout_ms)
            .max()
            .unwrap_or(0);

        Duration::from_millis(max_running_timeout_ms.clamp(250, 30_000))
    }

    fn count_running_tasks(graph: &TaskGraph) -> usize {
        graph.running_task_count()
    }

    fn reactivate_recoverable_tasks(graph: &mut TaskGraph) -> usize {
        let mut reset_count = 0;

        for node in graph.nodes.values_mut() {
            let Some(reset_log) = (match &node.status {
                TaskStatus::Cancelled { reason, .. } => {
                    node.result = None;
                    node.execution_state = None;
                    Some(format!(
                        "Task reset to Pending when graph resumed after cancellation: {}",
                        reason
                    ))
                }
                TaskStatus::Paused { .. } => {
                    Some("Task reset to Pending when graph resumed from paused state.".to_string())
                }
                _ => None,
            }) else {
                continue;
            };

            node.status = TaskStatus::Pending;
            node.logs.push(reset_log);
            reset_count += 1;
        }

        reset_count
    }

    async fn persist_graph_state(&self, graph_id: &str, graph: &TaskGraph) -> Result<String> {
        let goal = self.resolve_graph_goal(graph_id, graph).await?;
        self.persister.persist_graph(graph_id, graph, &goal).await?;
        Ok(goal)
    }

    async fn resolve_graph_goal(&self, graph_id: &str, graph: &TaskGraph) -> Result<String> {
        if graph.goal.is_empty() {
            self.persister.get_goal(graph_id).await
        } else {
            Ok(graph.goal.clone())
        }
    }

    fn validate_recovery_options(
        graph: &TaskGraph,
        node_id: &str,
        options: &NodeRecoveryOptions,
    ) -> Result<()> {
        if let Some(role) = &options.agent_role {
            if role.trim().is_empty() {
                return Err(anyhow::anyhow!("agent_role cannot be empty"));
            }
        }

        if let Some(prompt) = &options.prompt {
            if prompt.trim().is_empty() {
                return Err(anyhow::anyhow!("prompt cannot be empty"));
            }
        }

        if let Some(dependencies) = &options.dependencies {
            if !dependencies
                .iter()
                .all(|dependency| graph.nodes.contains_key(dependency))
            {
                return Err(anyhow::anyhow!("Invalid dependencies"));
            }
            if graph.detects_cycle(node_id, dependencies) {
                return Err(anyhow::anyhow!("Cycle detected in dependencies"));
            }
        }

        Ok(())
    }

    fn apply_recovery_overrides(
        node: &mut TaskNode,
        options: &NodeRecoveryOptions,
    ) -> Result<String> {
        let mut changes = Vec::new();

        if let Some(role) = &options.agent_role {
            node.agent_role = role.clone();
            changes.push(format!("role -> {}", role));
        }

        if let Some(prompt) = &options.prompt {
            node.prompt = prompt.clone();
            changes.push("prompt updated".to_string());
        }

        if let Some(dependencies) = &options.dependencies {
            node.dependencies = dependencies.clone();
            changes.push(format!("dependencies -> {}", dependencies.len()));
        }

        if changes.is_empty() {
            Ok("Status reset to Pending for recovery.".to_string())
        } else {
            Ok(format!(
                "Recovery overrides applied: {}.",
                changes.join(", ")
            ))
        }
    }

    fn describe_recovery_action(
        node_id: &str,
        reset_count: usize,
        options: &NodeRecoveryOptions,
        resumed: bool,
    ) -> String {
        let mut parts = vec![format!(
            "Recovered node {} across {} task(s)",
            node_id, reset_count
        )];

        if let Some(role) = &options.agent_role {
            parts.push(format!("role={}", role));
        }
        if options.prompt.is_some() {
            parts.push("prompt updated".to_string());
        }
        if let Some(dependencies) = &options.dependencies {
            parts.push(format!("deps={}", dependencies.len()));
        }
        if options.resume_graph || resumed {
            parts.push("graph resumed".to_string());
        }

        parts.join("; ")
    }

    fn collect_retry_set(graph: &TaskGraph, node_id: &str) -> Vec<String> {
        let mut retry_ids = HashSet::from([node_id.to_string()]);
        let mut frontier = vec![node_id.to_string()];

        while let Some(current) = frontier.pop() {
            for candidate in graph.nodes.values() {
                if candidate
                    .dependencies
                    .iter()
                    .any(|dependency| dependency == &current)
                    && retry_ids.insert(candidate.id.clone())
                {
                    frontier.push(candidate.id.clone());
                }
            }
        }

        let mut ordered_retry_ids = vec![node_id.to_string()];
        let mut descendants = retry_ids
            .into_iter()
            .filter(|candidate_id| candidate_id != node_id)
            .collect::<Vec<_>>();
        descendants.sort();
        ordered_retry_ids.extend(descendants);
        ordered_retry_ids
    }
}

#[cfg(test)]
mod tests;
