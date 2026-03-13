use crate::error::Result;
use crate::tools::bash::BashPlugin;
use crate::tools::file::FilePlugin;
use crate::tools::search::WebSearchPlugin;
use crate::tools::http::HttpPlugin;
use crate::tools::vision::VisionPlugin;
use crate::cognitive::llm::{LlmClient, OpenAiClient, OllamaClient};
use crate::cognitive::llm::cache::CachedLlmClient;
use crate::safety::oracle::{SafetyOracle, SafetyLevel};
use crate::types::{Message, TraceStep};
use crate::memory::semantic::KnowledgeGraph;
#[cfg(feature = "knowledge")]
use crate::knowledge::vector_store::VectorStore;
use tracing::{info, warn};
use std::sync::Arc;
use std::env;

use crate::skills::SkillRegistry;
use tokio::sync::RwLock;

use crate::cognitive::planner::TaskPlanner;
#[cfg(feature = "knowledge")]
use crate::memory::consolidator::MemoryConsolidator;
use crate::memory::episodic::EpisodicMemory;
use crate::cognitive::react::ReActEngine;
use crate::skills::watcher::SkillWatcher;
use crate::config::Config;
use crate::tools::manager::SkillManagerTool;
use crate::tools::management_plugin::{InstallSkillPlugin, CreateSkillPlugin};
use crate::tools::demo::{WeatherPlugin, CalculatorPlugin};
use crate::tools::mcp_plugins::{McpResourcePlugin, McpPromptPlugin};
use crate::tools::browser::BrowserPlugin;
use crate::cognitive::CognitiveSystem;
use async_trait::async_trait;

use crate::cognitive::middleware::{MiddlewarePipeline, MiddlewareState, PlanningMiddleware, RagMiddleware, SkillContextMiddleware, SafetyMiddleware, RoutingMiddleware, CostGuardMiddleware, SemanticCacheMiddleware};
use crate::events::EventBus;

use crate::cognitive::classifier::Classifier;
use crate::error::CrabletError;
use crate::cognitive::tot::{TreeOfThoughts, TotConfig, SearchStrategy};
use crate::cognitive::mcts_tot::{MCTSTreeOfThoughts, MCTSConfig};
use crate::events::AgentEvent;
use serde::Serialize;
#[cfg(feature = "knowledge")]
use std::str::FromStr;
#[cfg(feature = "knowledge")]
use crate::knowledge::graph_rag::EntityExtractorMode;

pub mod multimodal;
pub mod canvas;
pub mod post_process;

#[derive(Clone)]
pub struct System2 {
    pub llm: Arc<Box<dyn LlmClient>>,
    #[allow(dead_code)]
    oracle: SafetyOracle,
    pub kg: Option<Arc<dyn KnowledgeGraph>>,
    #[cfg(feature = "knowledge")]
    pub vector_store: Option<Arc<VectorStore>>,
    skill_manager: Arc<SkillManagerTool>,
    pub skills: Arc<RwLock<SkillRegistry>>,
    planner: Arc<TaskPlanner>,
    #[cfg(feature = "knowledge")]
    consolidator: Option<Arc<MemoryConsolidator>>,
    react_engine: Arc<ReActEngine>,
    skill_watcher: Option<Arc<SkillWatcher>>,
    event_bus: Arc<EventBus>,
    pipeline: Arc<MiddlewarePipeline>,
    hierarchical_config: Arc<RwLock<HierarchicalReasoningConfig>>,
    hierarchical_stats: Arc<RwLock<HierarchicalReasoningStats>>,
    #[cfg(feature = "knowledge")]
    graph_rag_entity_mode: Arc<RwLock<EntityExtractorMode>>,
}

#[derive(Clone, Debug)]
pub struct HierarchicalReasoningConfig {
    pub enabled: bool,
    pub deliberate_threshold: f32,
    pub meta_threshold: f32,
    pub mcts_simulations: u32,
    pub mcts_exploration_weight: f32,
}

impl Default for HierarchicalReasoningConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            deliberate_threshold: 0.58,
            meta_threshold: 0.82,
            mcts_simulations: 24,
            mcts_exploration_weight: 1.2,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct HierarchicalReasoningStats {
    pub total_requests: u64,
    pub deliberate_activations: u64,
    pub meta_activations: u64,
    pub strategy_switches: u64,
    pub bfs_runs: u64,
    pub dfs_runs: u64,
    pub mcts_runs: u64,
}

impl System2 {
    pub async fn new(event_bus: Arc<EventBus>) -> Self {
        // Read model from env, default to qwen-plus
        let model = env::var("OPENAI_MODEL_NAME")
            .unwrap_or_else(|_| "qwen-plus".to_string());
            
        let llm_inner: Box<dyn LlmClient> = match OpenAiClient::new(&model) {
            Ok(client) => {
                info!("System 2 initialized with OpenAI (model: {})", model);
                Box::new(client)
            }
            Err(_) => {
                let ollama_model = env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3".to_string());
                warn!("OpenAI API key not found, falling back to Ollama ({})", ollama_model);
                Box::new(OllamaClient::new(&ollama_model))
            }
        };
        
        // Wrap with Cache (Optimization 1)
        let llm: Box<dyn LlmClient> = Box::new(CachedLlmClient::new(llm_inner, 100));
        let llm_arc = Arc::new(llm);

        // Initialize Safety Oracle (Default to Strict for MVP)
        let oracle = SafetyOracle::new(SafetyLevel::Strict);

        // --- Dynamic Model Selection (System 2) ---
        
        let planner = Arc::new(TaskPlanner::new(llm_arc.clone()));

        let skills = Arc::new(RwLock::new(SkillRegistry::new()));
        
        // Use configured skills dir or default
        let skills_dir = std::path::PathBuf::from("skills");
        let skill_manager = Arc::new(SkillManagerTool::new(&skills_dir));

        // Register Native Plugins
        Self::register_plugins(skills.clone(), skill_manager.clone(), oracle.clone(), llm_arc.clone()).await;

        let react_engine = Arc::new(ReActEngine::new(llm_arc.clone(), skills.clone(), event_bus.clone()));

        // Initialize Middleware Pipeline
        let pipeline = MiddlewarePipeline::new()
            .with_middleware(SafetyMiddleware)
            .with_middleware(CostGuardMiddleware::new()) // Add CostGuard early
            .with_middleware(SemanticCacheMiddleware::new(0.92)) // Add Cache early
            .with_middleware(PlanningMiddleware)
            .with_middleware(RagMiddleware)
            .with_middleware(SkillContextMiddleware);
        let pipeline = Arc::new(pipeline);

        Self {
            llm: llm_arc,
            oracle,
            kg: None,
            #[cfg(feature = "knowledge")]
            vector_store: None,
            skill_manager,
            skills,
            planner,
            #[cfg(feature = "knowledge")]
            consolidator: None,
            react_engine,
            skill_watcher: None,
            event_bus,
            pipeline,
            hierarchical_config: Arc::new(RwLock::new(HierarchicalReasoningConfig::default())),
            hierarchical_stats: Arc::new(RwLock::new(HierarchicalReasoningStats::default())),
            #[cfg(feature = "knowledge")]
            graph_rag_entity_mode: Arc::new(RwLock::new(EntityExtractorMode::Hybrid)),
        }
    }
    
    async fn register_plugins(skills: Arc<RwLock<SkillRegistry>>, skill_manager: Arc<SkillManagerTool>, oracle: SafetyOracle, llm_arc: Arc<Box<dyn LlmClient>>) {
        let mut registry = skills.write().await;
        registry.register_plugin(Box::new(WebSearchPlugin::new()));
        registry.register_plugin(Box::new(HttpPlugin));
        registry.register_plugin(Box::new(VisionPlugin::new(llm_arc.clone())));
        registry.register_plugin(Box::new(BashPlugin::new(oracle.clone())));
        registry.register_plugin(Box::new(FilePlugin::new(oracle.clone())));
        // Management Plugins
        registry.register_plugin(Box::new(InstallSkillPlugin::new(skill_manager.clone())));
        registry.register_plugin(Box::new(CreateSkillPlugin::new(skill_manager.clone())));
        
        // Browser Plugin
        registry.register_plugin(Box::new(BrowserPlugin {}));
        
        // Demo Plugins
        registry.register_plugin(Box::new(WeatherPlugin));
        registry.register_plugin(Box::new(CalculatorPlugin));
        
        // MCP Meta Plugins
        registry.register_plugin(Box::new(McpResourcePlugin::new(skills.clone())));
        registry.register_plugin(Box::new(McpPromptPlugin::new(skills.clone())));
    }

    pub fn with_shared_skills(mut self, skills: Arc<RwLock<SkillRegistry>>) -> Self {
        self.skills = skills;
        // Also update React Engine if possible, but it holds a clone.
        // We should recreate React Engine or update its reference.
        // Recreating is safer.
        self.react_engine = Arc::new(ReActEngine::new(self.llm.clone(), self.skills.clone(), self.event_bus.clone()));
        self
    }

    pub fn with_config(mut self, config: &Config, load_mcp: bool) -> Self {
        // Update skill manager with correct path
        self.skill_manager = Arc::new(SkillManagerTool::new(&config.skills_dir));
        if let Ok(mut cfg) = self.hierarchical_config.try_write() {
            cfg.enabled = config.enable_hierarchical_reasoning;
            cfg.deliberate_threshold = config.deliberate_threshold;
            cfg.meta_threshold = config.meta_reasoning_threshold;
            cfg.mcts_simulations = config.mcts_simulations;
            cfg.mcts_exploration_weight = config.mcts_exploration_weight;
        }
        
        if load_mcp {
            // Load MCP servers
            let skills = self.skills.clone();
            let mcp_servers = config.mcp_servers.clone();
            
            tokio::spawn(async move {
                for (name, server_config) in mcp_servers {
                    info!("Initializing MCP server: {}", name);
                    match crate::tools::mcp::McpClient::new(&server_config.command, &server_config.args).await {
                        Ok(client) => {
                            let client_arc = Arc::new(client);
                            match client_arc.list_tools().await {
                                Ok(tools) => {
                                    let mut registry = skills.write().await;
                                    for tool in tools {
                                        info!("Registering MCP tool: {} (from server {})", tool.name, name);
                                        registry.register_mcp_tool(tool.name, client_arc.clone(), tool.description, tool.input_schema);
                                    }
                                }
                                Err(e) => warn!("Failed to list tools from MCP server {}: {}", name, e),
                            }
                            
                            // Register Resources
                            match client_arc.list_resources().await {
                                Ok(resources) => {
                                    let mut registry = skills.write().await;
                                    for resource in resources {
                                        info!("Registering MCP resource: {} (from server {})", resource.name, name);
                                        registry.register_mcp_resource(resource, client_arc.clone());
                                    }
                                }
                                Err(e) => warn!("Failed to list resources from MCP server {}: {}", name, e),
                            }

                            // Register Prompts
                            match client_arc.list_prompts().await {
                                Ok(prompts) => {
                                    let mut registry = skills.write().await;
                                    for prompt in prompts {
                                        info!("Registering MCP prompt: {} (from server {})", prompt.name, name);
                                        registry.register_mcp_prompt(prompt, client_arc.clone());
                                    }
                                }
                                Err(e) => warn!("Failed to list prompts from MCP server {}: {}", name, e),
                            }
                        }
                        Err(e) => warn!("Failed to connect to MCP server {}: {}", name, e),
                    }
                }
            });
        }
        
        self
    }

    pub fn watch_skills(mut self, skills_dir: &std::path::Path) -> Self {
        match SkillWatcher::new(self.skills.clone(), skills_dir) {
            Ok(watcher) => {
                info!("SkillWatcher started for {:?}", skills_dir);
                self.skill_watcher = Some(Arc::new(watcher));
            }
            Err(e) => {
                warn!("Failed to start skill watcher: {}", e);
            }
        }
        self
    }
    
    pub fn with_knowledge(
        mut self, 
        kg: Option<Arc<dyn KnowledgeGraph>>,
        #[cfg(feature = "knowledge")]
        vector_store: Option<Arc<VectorStore>>
    ) -> Self {
        self.kg = kg;
        
        #[cfg(feature = "knowledge")]
        {
            self.vector_store = vector_store.clone();
            if let Some(vs) = vector_store {
                self.consolidator = Some(Arc::new(MemoryConsolidator::new(self.llm.clone(), Some(vs), Some(self.event_bus.clone()))));
            }
        }
        self
    }

    pub async fn with_client(llm_inner: Box<dyn LlmClient>, event_bus: Arc<EventBus>) -> Self {
        // Wrap with Cache
        let llm: Box<dyn LlmClient> = Box::new(CachedLlmClient::new(llm_inner, 100));
        let llm_arc = Arc::new(llm);
        
        let skills = Arc::new(RwLock::new(SkillRegistry::new()));
        let oracle = SafetyOracle::new(SafetyLevel::Strict);

        let planner = Arc::new(TaskPlanner::new(llm_arc.clone()));

        let skill_manager = Arc::new(SkillManagerTool::new(&std::path::PathBuf::from("skills")));

        // Register Native Plugins
        {
            let mut registry = skills.write().await;
            registry.register_plugin(Box::new(WebSearchPlugin::new()));
            registry.register_plugin(Box::new(HttpPlugin));
            registry.register_plugin(Box::new(VisionPlugin::new(llm_arc.clone())));
            registry.register_plugin(Box::new(BashPlugin::new(oracle.clone())));
            registry.register_plugin(Box::new(FilePlugin::new(oracle.clone())));
            registry.register_plugin(Box::new(InstallSkillPlugin::new(skill_manager.clone())));
            registry.register_plugin(Box::new(CreateSkillPlugin::new(skill_manager.clone())));
        }

        let react_engine = Arc::new(ReActEngine::new(llm_arc.clone(), skills.clone(), event_bus.clone()));

        // Initialize Middleware Pipeline
        let pipeline = Arc::new(MiddlewarePipeline::new()
            .with_middleware(SafetyMiddleware)
            .with_middleware(CostGuardMiddleware::new()) // Add CostGuard early
            .with_middleware(SemanticCacheMiddleware::new(0.92)) // Add Cache early
            .with_middleware(RoutingMiddleware::new(llm_arc.clone()))
            .with_middleware(PlanningMiddleware)
            .with_middleware(RagMiddleware)
            .with_middleware(SkillContextMiddleware));

        Self {
            llm: llm_arc,
            oracle,
            kg: None,
            #[cfg(feature = "knowledge")]
            vector_store: None,
            skill_manager,
            skills,
            planner,
            #[cfg(feature = "knowledge")]
            consolidator: None,
            react_engine,
            skill_watcher: None,
            event_bus,
            pipeline,
            hierarchical_config: Arc::new(RwLock::new(HierarchicalReasoningConfig::default())),
            hierarchical_stats: Arc::new(RwLock::new(HierarchicalReasoningStats::default())),
            #[cfg(feature = "knowledge")]
            graph_rag_entity_mode: Arc::new(RwLock::new(EntityExtractorMode::Hybrid)),
        }
    }

    pub async fn consolidate_memory(&self, _memory: &EpisodicMemory, _session_id: &str) -> Result<()> {
        #[cfg(feature = "knowledge")]
        {
            if let Some(consolidator) = &self.consolidator {
                 consolidator.consolidate(_memory, _session_id).await?;
            }
        }
        Ok(())
    }

    pub fn start_consolidation_loop(&self, _memory: Arc<EpisodicMemory>) {
        #[cfg(feature = "knowledge")]
        if let Some(consolidator) = &self.consolidator {
            consolidator.clone().start_background_loop(_memory);
        }
    }

    pub async fn set_hierarchical_config(&self, config: HierarchicalReasoningConfig) {
        let mut cfg = self.hierarchical_config.write().await;
        *cfg = config;
    }

    pub async fn hierarchical_stats(&self) -> HierarchicalReasoningStats {
        self.hierarchical_stats.read().await.clone()
    }

    pub async fn set_graph_rag_entity_mode(&self, _mode: &str) {
        #[cfg(feature = "knowledge")]
        {
        let parsed = EntityExtractorMode::from_str(_mode).unwrap_or(EntityExtractorMode::Hybrid);
        let mut cfg = self.graph_rag_entity_mode.write().await;
        *cfg = parsed;
        }
    }
}

#[async_trait]
impl CognitiveSystem for System2 {
    fn name(&self) -> &str {
        "System 2 (Analytical)"
    }

    async fn process(&self, input: &str, context: &[Message]) -> Result<(String, Vec<TraceStep>)> {
        let rag_trace = Arc::new(RwLock::new(None));
        // 1. Create Middleware State
        let state = MiddlewareState {
            llm: self.llm.clone(),
            skills: self.skills.clone(),
            event_bus: self.event_bus.clone(),
            kg: self.kg.clone(),
            #[cfg(feature = "knowledge")]
            vector_store: self.vector_store.clone(),
            planner: self.planner.clone(),
            skill_manager: self.skill_manager.clone(),
            #[cfg(feature = "knowledge")]
            graph_rag_entity_mode: *self.graph_rag_entity_mode.read().await,
            rag_trace: rag_trace.clone(),
        };

        // 2. Execute Middleware Pipeline
        let mut final_context = context.to_vec();
        
        // Use helper for multimodal injection
        multimodal::inject_vision_content(input, &mut final_context).await;
        
        match self.pipeline.execute(input, &mut final_context, &state).await {
            Ok(Some(result)) => return Ok(result),
            Ok(None) => {}, // Continue
            Err(e) => {
                warn!("Middleware Error: {}", e);
                // Wrap in CrabletError
                return Err(CrabletError::Other(e));
            }
        }

        // 3. Calculate dynamic steps based on complexity
        let complexity = Classifier::assess_complexity(input);
        // Base: 5 steps. 
        // Low complexity (<0.3) -> 3 steps.
        // High complexity (>0.8) -> 10 steps.
        let max_steps = if complexity < 0.3 {
            3
        } else if complexity > 0.8 {
            10
        } else {
            5
        };
        
        info!("Dynamic ReAct Config: Complexity={:.2}, MaxSteps={}", complexity, max_steps);
        {
            let mut stats = self.hierarchical_stats.write().await;
            stats.total_requests += 1;
        }
        let hierarchical_cfg = self.hierarchical_config.read().await.clone();

        let mut meta_traces: Vec<TraceStep> = Vec::new();
        if let Some(step) = build_rag_trace_step(input, &rag_trace).await {
            meta_traces.push(step);
        }
        if hierarchical_cfg.enabled && complexity >= hierarchical_cfg.meta_threshold {
            self.event_bus.publish(AgentEvent::CognitiveLayerChanged { layer: "meta_reasoning".to_string() });
            {
                let mut stats = self.hierarchical_stats.write().await;
                stats.meta_activations += 1;
                stats.mcts_runs += 1;
            }
            let mcts = MCTSTreeOfThoughts::new(
                self.llm.clone(),
                MCTSConfig {
                    simulations: hierarchical_cfg.mcts_simulations as usize,
                    exploration_weight: hierarchical_cfg.mcts_exploration_weight as f64,
                    ..MCTSConfig::default()
                },
            );
            if let Ok(guidance) = mcts.solve(input).await {
                final_context.push(Message::system(format!("Meta reasoning guidance (MCTS): {}", guidance)));
                meta_traces.push(TraceStep {
                    step: 0,
                    thought: "Layer3 Meta-Reasoning".to_string(),
                    action: Some("mcts_tot".to_string()),
                    action_input: Some(input.to_string()),
                    observation: Some(guidance),
                });
            }
        } else if hierarchical_cfg.enabled && complexity >= hierarchical_cfg.deliberate_threshold {
            self.event_bus.publish(AgentEvent::CognitiveLayerChanged { layer: "deliberate_reasoning".to_string() });
            let strategy = if should_use_dfs(input) { SearchStrategy::DFS } else { SearchStrategy::BFS };
            {
                let mut stats = self.hierarchical_stats.write().await;
                stats.deliberate_activations += 1;
                if strategy == SearchStrategy::DFS {
                    stats.dfs_runs += 1;
                } else {
                    stats.bfs_runs += 1;
                }
            }
            let tot = TreeOfThoughts::new(
                self.llm.clone(),
                TotConfig {
                    strategy,
                    ..TotConfig::default()
                },
            );
            if let Ok(guidance) = tot.solve(input).await {
                final_context.push(Message::system(format!("Deliberate reasoning guidance (ToT): {}", guidance)));
                meta_traces.push(TraceStep {
                    step: 0,
                    thought: "Layer2 Deliberate-Reasoning".to_string(),
                    action: Some("tot".to_string()),
                    action_input: Some(input.to_string()),
                    observation: Some(guidance),
                });
            }
        }

        // ReAct Loop (Execution Engine)
        let result = match self.react_engine.execute(&final_context, max_steps).await {
            Ok((response, traces)) => {
                if hierarchical_cfg.enabled && needs_meta_switch(&response, &traces) {
                    self.event_bus.publish(AgentEvent::CognitiveLayerChanged { layer: "meta_reasoning_switch".to_string() });
                    {
                        let mut stats = self.hierarchical_stats.write().await;
                        stats.strategy_switches += 1;
                    }
                    let mut retry_context = final_context.clone();
                    retry_context.push(Message::system("当前策略效果不佳，请切换思路：避免重复工具调用，优先综合已有观察给出结论。"));
                    match self.react_engine.execute(&retry_context, (max_steps + 2).min(12)).await {
                        Ok((retry_response, mut retry_traces)) => {
                            let mut combined = meta_traces.clone();
                            combined.extend(traces);
                            combined.push(TraceStep {
                                step: max_steps + 1,
                                thought: "Layer3 Strategy Switch".to_string(),
                                action: Some("react_retry".to_string()),
                                action_input: None,
                                observation: Some("Switched from initial ReAct run to revised strategy".to_string()),
                            });
                            combined.append(&mut retry_traces);
                            #[cfg(feature = "knowledge")]
                            post_process::update_semantic_cache(input, &retry_response, &self.vector_store).await;
                            Ok((retry_response, combined))
                        }
                        Err(_) => {
                            let mut combined = meta_traces.clone();
                            combined.extend(traces);
                            #[cfg(feature = "knowledge")]
                            post_process::update_semantic_cache(input, &response, &self.vector_store).await;
                            Ok((response, combined))
                        }
                    }
                } else {
                    let mut combined = meta_traces.clone();
                    combined.extend(traces);
                    #[cfg(feature = "knowledge")]
                    post_process::update_semantic_cache(input, &response, &self.vector_store).await;
                    Ok((response, combined))
                }
            },
            Err(e) => {
                warn!("ReAct Engine Critical Failure: {}", e);
                Err(CrabletError::Unknown(e.to_string()))
            }
        };

        // Post-processing: Detect artifacts and publish CanvasUpdate
        if let Ok((response, _)) = &result {
            canvas::detect_and_publish_canvas(response, &self.event_bus);
        }
        
        result
    }
}

fn should_use_dfs(input: &str) -> bool {
    let lower = input.to_lowercase();
    ["debug", "fix", "repair", "trace", "定位", "修复", "排查", "实现"]
        .iter()
        .any(|k| lower.contains(k))
}

fn needs_meta_switch(response: &str, traces: &[TraceStep]) -> bool {
    if response.to_lowercase().contains("maximum steps") {
        return true;
    }
    if traces.len() < 3 {
        return false;
    }
    let mut repeat_count = 0usize;
    for w in traces.windows(2) {
        let a = w[0].action.clone().unwrap_or_default();
        let b = w[1].action.clone().unwrap_or_default();
        if !a.is_empty() && a == b {
            repeat_count += 1;
        }
    }
    repeat_count >= 2
}

async fn build_rag_trace_step(input: &str, rag_trace: &Arc<RwLock<Option<crate::cognitive::middleware::RagTracePayload>>>) -> Option<TraceStep> {
    let payload = rag_trace.read().await.clone()?;
    let refs_count = payload.refs.len();
    let observation = serde_json::json!({
        "retrieval": payload.retrieval,
        "refs_count": refs_count,
        "graph_entities": payload.graph_entities,
        "refs": payload.refs.iter().map(|x| {
            serde_json::json!({
                "source": x.source,
                "score": x.score,
                "content": x.content,
            })
        }).collect::<Vec<_>>()
    });
    Some(TraceStep {
        step: 0,
        thought: format!("RAG 检索完成，命中 {} 条参考", refs_count),
        action: Some("rag_retrieve".to_string()),
        action_input: Some(serde_json::json!({ "query": input }).to_string()),
        observation: Some(observation.to_string()),
    })
}
