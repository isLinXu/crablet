use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use anyhow::Result;
use tracing::info;
use uuid::Uuid;
use crate::cognitive::llm::LlmClient;

use crate::agent::swarm::types::{TaskGraph, TaskGraphTemplate, GraphStatus, TaskNode};
use crate::agent::swarm::persister::SwarmPersister;
use crate::agent::swarm::executor::SwarmExecutor;
use crate::types::Message as ChatMessage;

use serde::Deserialize;

pub struct SwarmCoordinator {
    pub llm: Arc<Box<dyn LlmClient>>,
    pub executor: Arc<SwarmExecutor>,
    pub persister: Arc<SwarmPersister>,
    pub active_graphs: Arc<RwLock<HashMap<String, TaskGraph>>>,
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
        Ok(())
    }

    pub async fn decompose_and_execute(&self, goal: &str) -> Result<String> {
        // Special case for Draft Mode
        if goal.to_lowercase().starts_with("draft ") {
            let topic = goal.chars().skip(6).collect::<String>();
            return self.start_draft_swarm(&topic).await;
        }

        // 1. Decompose goal into TaskGraph using LLM
        let graph = self.decompose_goal(goal).await?;
        
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
        
        // 2. Execute Graph
        // Note: executor needs reference to active_graphs lock
        self.executor.execute_graph(graph, &graph_id, goal, self.active_graphs.clone()).await
    }

    pub async fn start_draft_swarm(&self, topic: &str) -> Result<String> {
        let mut graph = TaskGraph::new();
        let t1 = "research_task".to_string();
        let t2 = "draft_task".to_string();
        let t3 = "critique_task".to_string();
        let t4 = "polish_task".to_string();

        graph.add_task(t1.clone(), "researcher".to_string(), format!("Deeply research the following topic and provide key facts and insights: {}", topic), vec![]);
        graph.add_task(t2.clone(), "drafter".to_string(), format!("Write a comprehensive first draft about: {}. Use the research findings provided.", topic), vec![t1]);
        graph.add_task(t3.clone(), "critic".to_string(), "Review the draft meticulously. Identify gaps, inconsistencies, and areas for improvement. Provide actionable feedback.".to_string(), vec![t2]);
        graph.add_task(t4.clone(), "drafter".to_string(), "Refine and polish the draft based on the critic's feedback. Ensure the final version is high-quality and professional.".to_string(), vec![t3]);

        let graph_id = Uuid::new_v4().to_string();
        {
            let mut graphs = self.active_graphs.write().await;
            graphs.insert(graph_id.clone(), graph.clone());
        }
        
        let goal = format!("Draft: {}", topic);
        self.persister.persist_graph(&graph_id, &graph, &goal).await?;
        
        // Execute in background
        let executor = self.executor.clone();
        let active_graphs = self.active_graphs.clone();
        let graph_id_str = graph_id.clone();
        
        tokio::spawn(async move {
            let _ = executor.execute_graph(graph, &graph_id_str, &goal, active_graphs).await;
        });

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

        let def: GraphDef = serde_json::from_str(json_str)
            .map_err(|e| anyhow::anyhow!("Failed to parse task graph JSON: {} (Response: {})", e, response))?;

        let mut graph = TaskGraph::new();
        for task in def.tasks {
            graph.add_task(task.id, task.role, task.prompt, task.dependencies);
        }
        
        Ok(graph)
    }

    pub async fn save_template(&self, name: &str, description: &str, graph: &TaskGraph) -> Result<String> {
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
                let new_id = id_map.get(old_id).ok_or_else(|| anyhow::anyhow!("Dependency mapping failed for {}", old_id))?;
                let mut new_node = node.clone();
                new_node.id = new_id.clone();
                new_node.status = crate::agent::swarm::types::TaskStatus::Pending;
                new_node.result = None;
                new_node.logs = Vec::new();
                new_node.dependencies = node.dependencies.iter()
                    .filter_map(|d| id_map.get(d).cloned())
                    .collect();
                    
                new_nodes.insert(new_id.clone(), new_node);
            }
            
            graph.nodes = new_nodes;
            graph.status = GraphStatus::Active;
            
            let graph_id = Uuid::new_v4().to_string();
            {
                let mut active = self.active_graphs.write().await;
                active.insert(graph_id.clone(), graph.clone());
            }
            
            self.persister.persist_graph(&graph_id, &graph, goal).await?;
            
            // Note: Execution must be spawned by caller as Coordinator doesn't spawn automatically here to allow flexibility
            // Or we could return graph_id and let caller call executor.execute_graph
            
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
            graphs.get(graph_id).cloned().ok_or_else(|| anyhow::anyhow!("Graph not found"))?
        };
        
        let goal = self.persister.get_goal(graph_id).await?;
        self.persister.persist_graph(graph_id, &graph, &goal).await?;
        Ok(())
    }
}
