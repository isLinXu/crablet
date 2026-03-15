use crate::cognitive::system1_enhanced::System1Enhanced;
use crate::cognitive::system2::System2;
use crate::cognitive::system3::System3;
use crate::memory::episodic::EpisodicMemory;
use crate::memory::manager::MemoryManager;
use crate::memory::semantic::SharedKnowledgeGraph;
#[cfg(feature = "knowledge")]
use crate::knowledge::vector_store::VectorStore;
use crate::types::TraceStep;
use crate::error::Result;
#[cfg(not(feature = "knowledge"))]
use crate::error::CrabletError;
use tracing::{info, error, instrument};
use std::sync::Arc;
use std::path::Path;
use crate::cognitive::CognitiveSystem;
use crate::cognitive::llm::{LlmClient, OpenAiClient, OllamaClient};
use crate::cognitive::llm::cache::CachedLlmClient;
// use std::env;

use crate::events::{AgentEvent, EventBus};

use std::time::Duration;

use crate::cognitive::classifier::{Classifier, Intent};
use crate::cognitive::meta_router::{MetaCognitiveRouter, SystemChoice};
use crate::cognitive::system2::HierarchicalReasoningConfig;
use crate::skills::SkillRegistry;
use tokio::sync::RwLock;

// Define a RouterConfig struct to hold dynamic thresholds
#[derive(Clone, Debug)]
pub struct RouterConfig {
    pub system2_threshold: f32,
    pub system3_threshold: f32,
    pub enable_adaptive_routing: bool,
    pub bandit_exploration: f32,
    pub enable_hierarchical_reasoning: bool,
    pub deliberate_threshold: f32,
    pub meta_reasoning_threshold: f32,
    pub mcts_simulations: u32,
    pub mcts_exploration_weight: f32,
    pub graph_rag_entity_mode: String,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            system2_threshold: 0.3,
            system3_threshold: 0.7,
            enable_adaptive_routing: false,
            bandit_exploration: 0.55,
            enable_hierarchical_reasoning: true,
            deliberate_threshold: 0.58,
            meta_reasoning_threshold: 0.82,
            mcts_simulations: 24,
            mcts_exploration_weight: 1.2,
            graph_rag_entity_mode: "hybrid".to_string(),
        }
    }
}

#[derive(Clone)]
pub struct CognitiveRouter {
    pub shared_skills: Arc<RwLock<SkillRegistry>>, // Shared Registry
    pub sys1: System1Enhanced,
    pub sys2: System2,       // Cloud (OpenAI)
    pub sys2_local: System2, // Local (Ollama) - Permanent instance
    pub sys3: System3,
    pub memory_mgr: Arc<MemoryManager>,
    pub event_bus: Arc<EventBus>,
    pub config: Arc<RwLock<RouterConfig>>,
    pub meta_router: Arc<RwLock<MetaCognitiveRouter>>,
    pub complexity_analyzer: Arc<crate::cognitive::routing::complexity::ComplexityAnalyzer>,
    /// Fusion Memory System (optional, for OpenClaw-style memory)
    pub fusion_memory: Option<Arc<crate::memory::fusion::FusionMemorySystem>>,
    /// Skill trigger engine for automatic skill activation
    pub skill_trigger_engine: Option<Arc<crate::skills::SkillTriggerEngine>>,
    /// Skill execution enabled flag
    pub skill_execution_enabled: bool,
}

use crate::config::Config;

impl CognitiveRouter {
    fn router_config_from_app(config: &Config) -> RouterConfig {
        RouterConfig {
            system2_threshold: config.system2_threshold,
            system3_threshold: config.system3_threshold,
            enable_adaptive_routing: config.enable_adaptive_routing,
            bandit_exploration: config.bandit_exploration,
            enable_hierarchical_reasoning: config.enable_hierarchical_reasoning,
            deliberate_threshold: config.deliberate_threshold,
            meta_reasoning_threshold: config.meta_reasoning_threshold,
            mcts_simulations: config.mcts_simulations,
            mcts_exploration_weight: config.mcts_exploration_weight,
            graph_rag_entity_mode: config.graph_rag_entity_mode.clone(),
        }
    }

    // ... existing create_llm_client methods ...
    fn create_llm_client(config: &Config, model_hint: Option<&str>) -> Arc<Box<dyn LlmClient>> {
        let model = model_hint
            .map(|s| s.to_string())
            .unwrap_or_else(|| config.model_name.clone());

        let llm_inner: Box<dyn LlmClient> = match OpenAiClient::new(&model) {
            Ok(client) => Box::new(client),
            Err(_) => {
                 let ollama_model = config.ollama_model.clone();
                 Box::new(OllamaClient::new(&ollama_model))
            }
        };
        let llm: Box<dyn LlmClient> = Box::new(CachedLlmClient::new(llm_inner, 100));
        Arc::new(llm)
    }
    
    fn create_local_llm_client(config: &Config) -> Box<dyn LlmClient> {
        let ollama_model = config.ollama_model.clone();
        let local_llm_inner: Box<dyn LlmClient> = Box::new(OllamaClient::new(&ollama_model));
        Box::new(CachedLlmClient::new(local_llm_inner, 100))
    }

    pub async fn new(config: &Config, memory: Option<Arc<EpisodicMemory>>, event_bus: Arc<EventBus>) -> Self {
        // Initialize Shared Skill Registry
        let shared_skills = Arc::new(RwLock::new(SkillRegistry::new()));

        // Initialize LLM for System 3
        let llm_arc = Self::create_llm_client(config, None);
        let memory_mgr = Arc::new(MemoryManager::new(
            memory.clone(), 
            100, // max entries
            Duration::from_secs(3600) // TTL
        ));

        // Create System 2 (Cloud)
        let sys2 = System2::new(event_bus.clone()).await.with_shared_skills(shared_skills.clone());
        
        // Create System 2 Local (Ollama)
        let local_llm_cached = Self::create_local_llm_client(config);
        let sys2_local = System2::with_client(local_llm_cached, event_bus.clone()).await.with_shared_skills(shared_skills.clone());

        let pool = memory.as_ref().map(|m| m.pool.clone());

        let initial_config = Self::router_config_from_app(config);
        sys2.set_graph_rag_entity_mode(&initial_config.graph_rag_entity_mode).await;
        sys2_local.set_graph_rag_entity_mode(&initial_config.graph_rag_entity_mode).await;
        let mut meta = MetaCognitiveRouter::new();
        meta.set_exploration(initial_config.bandit_exploration);
        Self {
            shared_skills,
            sys1: System1Enhanced::new(),
            sys2,
            sys2_local,
            sys3: System3::new(llm_arc, event_bus.clone(), pool).await,
            memory_mgr,
            event_bus,
            config: Arc::new(RwLock::new(initial_config)),
            meta_router: Arc::new(RwLock::new(meta)),
            complexity_analyzer: Arc::new(crate::cognitive::routing::complexity::ComplexityAnalyzer::new()),
            fusion_memory: None,
            skill_trigger_engine: None,
            skill_execution_enabled: true,
        }
    }

    /// Initialize with Fusion Memory System
    pub async fn with_fusion_memory(
        mut self,
        fusion_config: Arc<crate::memory::fusion::FusionConfig>,
    ) -> crate::error::Result<Self> {
        let fusion = Arc::new(
            crate::memory::fusion::FusionMemorySystem::initialize(fusion_config).await
                .map_err(|e| crate::error::CrabletError::Config(format!("Failed to initialize Fusion Memory: {}", e)))?
        );
        self.fusion_memory = Some(fusion);
        Ok(self)
    }

    /// Initialize with Skill Trigger Engine
    pub fn with_skill_trigger_engine(
        mut self,
        engine: Arc<crate::skills::SkillTriggerEngine>,
    ) -> Self {
        self.skill_trigger_engine = Some(engine);
        self
    }

    /// Enable or disable skill execution
    pub fn set_skill_execution_enabled(mut self, enabled: bool) -> Self {
        self.skill_execution_enabled = enabled;
        self
    }

    /// Refresh skill triggers from the registry
    pub async fn refresh_skill_triggers(&mut self) {
        if let Some(ref engine) = self.skill_trigger_engine {
            let registry = self.shared_skills.read().await;
            let engine = Arc::clone(engine);
            // Note: This requires SkillTriggerEngine to be mutable, which it isn't with Arc
            // In practice, we'd need to use RwLock or similar for the engine
            drop(registry);
            drop(engine);
        }
    }

    pub async fn with_system2_async(config: &Config, memory: Option<Arc<EpisodicMemory>>, sys2: System2, event_bus: Arc<EventBus>) -> Self {
        // Initialize Shared Skill Registry
        let shared_skills = Arc::new(RwLock::new(SkillRegistry::new()));

        // Initialize LLM for System 3
        let llm_arc = Self::create_llm_client(config, None);
        let memory_mgr = Arc::new(MemoryManager::new(
            memory.clone(), 
            100, 
            Duration::from_secs(3600)
        ));

        // Create System 2 Local (Ollama)
        let local_llm_cached = Self::create_local_llm_client(config);
        let sys2_local = System2::with_client(local_llm_cached, event_bus.clone()).await.with_shared_skills(shared_skills.clone());
        
        let sys2 = sys2.with_shared_skills(shared_skills.clone());

        let pool = memory.as_ref().map(|m| m.pool.clone());

        let initial_config = Self::router_config_from_app(config);
        sys2.set_graph_rag_entity_mode(&initial_config.graph_rag_entity_mode).await;
        sys2_local.set_graph_rag_entity_mode(&initial_config.graph_rag_entity_mode).await;
        let mut meta = MetaCognitiveRouter::new();
        meta.set_exploration(initial_config.bandit_exploration);
        Self {
            shared_skills,
            sys1: System1Enhanced::new(),
            sys2,
            sys2_local,
            sys3: System3::new(llm_arc, event_bus.clone(), pool).await,
            memory_mgr,
            event_bus,
            config: Arc::new(RwLock::new(initial_config)),
            meta_router: Arc::new(RwLock::new(meta)),
            complexity_analyzer: Arc::new(crate::cognitive::routing::complexity::ComplexityAnalyzer::new()),
            fusion_memory: None,
            skill_trigger_engine: None,
            skill_execution_enabled: true,
        }
    }
    
    // ... with_knowledge, with_config, load_skills, watch_skills ...
    pub fn with_knowledge(
        mut self,
        kg: Option<SharedKnowledgeGraph>,
        #[cfg(feature = "knowledge")]
        vector_store: Option<Arc<VectorStore>>
    ) -> Self {
        // Re-create System 2 with knowledge components
        #[cfg(feature = "knowledge")]
        {
            self.sys2 = self.sys2.with_knowledge(kg.clone(), vector_store.clone());
            self.sys2_local = self.sys2_local.with_knowledge(kg, vector_store);
            
            // Start consolidation loops if memory is available
            if let Some(mem) = &self.memory_mgr.episodic {
                self.sys2.start_consolidation_loop(mem.clone());
                self.sys2_local.start_consolidation_loop(mem.clone());
            }
        }
        #[cfg(not(feature = "knowledge"))]
        {
            self.sys2 = self.sys2.with_knowledge(kg.clone());
            self.sys2_local = self.sys2_local.with_knowledge(kg);
        }
        self
    }

    pub fn with_config(mut self, config: &crate::config::Config) -> Self {
        self.sys2 = self.sys2.with_config(config, true);
        self.sys2_local = self.sys2_local.with_config(config, false);
        self
    }

    pub async fn load_skills<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path_ref = path.as_ref();
        
        // Load skills into Shared Registry
        let mut skills = self.shared_skills.write().await;
        if let Err(e) = skills.load_from_dir(path_ref).await {
            error!("Failed to load skills into Shared Registry from {:?}: {}", path_ref, e);
        } else {
            info!("Loaded {} skills into Shared Registry", skills.len());
        }
        
        Ok(())
    }

    pub fn watch_skills<P: AsRef<Path>>(mut self, path: P) -> Self {
        let path_ref = path.as_ref();
        self.sys2 = self.sys2.watch_skills(path_ref);
        self.sys2_local = self.sys2_local.watch_skills(path_ref);
        self
    }

    pub async fn consolidate_memory(&self, _session_id: &str) -> Result<()> {
        #[cfg(feature = "knowledge")]
        if let Some(mem) = &self.memory_mgr.episodic {
            self.sys2.consolidate_memory(mem, _session_id).await?;
        }
        Ok(())
    }

    pub async fn get_all_skills(&self) -> Vec<crate::skills::SkillManifest> {
        let skills = self.shared_skills.read().await;
        skills.list_skills()
    }

    pub async fn ingest_file(&self, path: &str) -> Result<()> {
        let path_obj = Path::new(path);
        let ext = path_obj.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        
        let content = match ext.as_str() {
            "pdf" => {
                #[cfg(feature = "knowledge")]
                {
                    crate::knowledge::pdf::PdfParser::extract_text(path)?
                }
                #[cfg(not(feature = "knowledge"))]
                {
                    return Err(CrabletError::Config("Knowledge feature not enabled for PDF parsing".to_string()));
                }
            },
            "txt" | "md" | "rs" | "py" | "js" | "json" | "toml" | "yaml" | "xml" | "html" | "css" | "sh" | "sql" => {
                tokio::fs::read_to_string(path).await?
            },
            _ => {
                // Skip unsupported files (e.g. images) without error
                return Ok(());
            }
        };
        
        if content.trim().is_empty() {
            return Ok(());
        }

        #[cfg(feature = "knowledge")]
        if let Some(vs) = &self.sys2.vector_store {
            let metadata = serde_json::json!({
                "source": path,
                "file_type": ext
            });
            vs.add_document(&content, Some(metadata)).await?;
            info!("Ingested file into knowledge base: {}", path);
        } else {
            tracing::warn!("Vector Store not available for ingestion");
        }
        
        Ok(())
    }

    // New method to update router config at runtime
    pub async fn update_config(&self, new_config: RouterConfig) {
        let mut config = self.config.write().await;
        let prev_mode = config.graph_rag_entity_mode.clone();
        *config = new_config.clone();
        let exploration = config.bandit_exploration;
        drop(config);
        let mut meta = self.meta_router.write().await;
        meta.set_exploration(exploration);
        drop(meta);
        let hierarchical = HierarchicalReasoningConfig {
            enabled: new_config.enable_hierarchical_reasoning,
            deliberate_threshold: new_config.deliberate_threshold,
            meta_threshold: new_config.meta_reasoning_threshold,
            mcts_simulations: new_config.mcts_simulations,
            mcts_exploration_weight: new_config.mcts_exploration_weight,
        };
        self.sys2.set_hierarchical_config(hierarchical.clone()).await;
        self.sys2_local.set_hierarchical_config(hierarchical).await;
        self.sys2.set_graph_rag_entity_mode(&new_config.graph_rag_entity_mode).await;
        self.sys2_local.set_graph_rag_entity_mode(&new_config.graph_rag_entity_mode).await;
        if prev_mode != new_config.graph_rag_entity_mode {
            self.event_bus.publish(AgentEvent::GraphRagEntityModeChanged {
                from_mode: prev_mode,
                to_mode: new_config.graph_rag_entity_mode.clone(),
            });
        }
        let config = self.config.read().await;
        info!("Updated Router Config: {:?}", *config);
    }

    // Enhanced complexity assessment
    fn assess_complexity_enhanced(&self, input: &str) -> f32 {
        let message = crate::types::Message::user(input);
        let characteristics = self.complexity_analyzer.extract_characteristics(&[message]).unwrap_or_else(|_| {
            // Fallback to basic word count if analyzer fails
            let words: Vec<&str> = input.split_whitespace().collect();
            crate::cognitive::routing::complexity::TaskCharacteristics {
                word_count: words.len(),
                sentence_count: 1,
                technical_terms: 0,
                question_count: 0,
                instruction_count: 0,
                creativity_score: 0.0,
                detected_domains: vec![],
            }
        });

        let base_score = self.complexity_analyzer.calculate_complexity_score(&characteristics);
        
        // Normalize 0.0-10.0+ to 0.0-1.0
        let normalized_score = (base_score / 10.0).min(1.0);
        
        // Dynamic adjustment based on current config (e.g., adaptive threshold)
        normalized_score
    }

    #[instrument(skip(self), fields(session.id = %session_id, input.length = input.len()))]
    pub async fn process(&self, input: &str, session_id: &str) -> Result<(String, Vec<TraceStep>)> {
        self.event_bus.publish(AgentEvent::UserInput(input.to_string()));

        // 0. Check for Skill Trigger match first (before cognitive routing)
        if self.skill_execution_enabled {
            if let Some(ref engine) = self.skill_trigger_engine {
                if let Some(trigger_match) = engine.match_best(input, 0.7) {
                    info!("Skill trigger matched: {} (confidence: {})", trigger_match.skill_name, trigger_match.confidence);
                    return self.execute_skill_route(&trigger_match, input, session_id).await;
                }
            }
        }

        // 0. Save User Input (via MemoryManager)
        self.memory_mgr.save_message(session_id, "user", input).await;

        // 0.5. Create or get Fusion Memory session if available
        let fusion_context: Option<Arc<crate::memory::fusion::layer_session::SessionLayer>> = None;
        // Note: Fusion Memory integration temporarily disabled due to API changes
        // if let Some(ref fusion) = self.fusion_memory {
        //     match fusion.get_session(session_id) {
        //         Some(session) => {
        //             // Session exists, add user message
        //             Some(session)
        //         }
        //         None => {
        //             // Create new session
        //             match fusion.create_session(session_id.to_string()).await {
        //                 Ok(session) => {
        //                     Some(session)
        //                 }
        //                 Err(e) => {
        //                     tracing::warn!("Failed to create Fusion session: {}", e);
        //                     None
        //                 }
        //             }
        //         }
        //     }
        // } else {
        //     None
        // };

        let start = std::time::Instant::now();
        let (response, traces, system_choice) = self.route_and_process(input, session_id, fusion_context.clone()).await?;
        let latency = start.elapsed();

        // 1. Save Assistant Response (via MemoryManager)
        self.memory_mgr.save_message(session_id, "assistant", &response).await;

        // 1.5. Save to Fusion Memory if available
        // Note: Temporarily disabled
        // if let Some(ref session) = fusion_context {
        // }

        // Adaptive Routing Feedback (Simple Implementation)
        {
            let config = self.config.read().await;
            if config.enable_adaptive_routing {
                let quality_score = Self::estimate_quality_score(&response, &traces);
                let mut meta = self.meta_router.write().await;
                meta.record_feedback(session_id, input, latency, quality_score);
                info!(
                    "Adaptive routing feedback recorded: choice={:?}, quality={:.3}, latency_ms={}",
                    system_choice,
                    quality_score,
                    latency.as_millis()
                );
            }
        }

        info!("Total latency: {:?}", latency);
        Ok((response, traces))
    }

    fn estimate_quality_score(response: &str, traces: &[TraceStep]) -> f32 {
        let mut score = 0.35f32;
        if !response.trim().is_empty() {
            score += 0.25;
        }
        if response.chars().count() >= 32 {
            score += 0.15;
        }
        if traces.iter().any(|t| t.observation.as_ref().map(|v| !v.is_empty()).unwrap_or(false)) {
            score += 0.15;
        }
        score.min(0.95)
    }

    /// Execute a skill-based route
    async fn execute_skill_route(
        &self,
        trigger_match: &crate::skills::TriggerMatch,
        input: &str,
        session_id: &str,
    ) -> Result<(String, Vec<TraceStep>)> {
        use crate::skills::context::SkillContext;
        
        let start_time = std::time::Instant::now();
        
        // Create skill context
        let mut context = SkillContext::new(session_id, input)
            .with_args(trigger_match.extracted_args.clone().unwrap_or(serde_json::json!({})));
        
        // Execute the skill
        let registry = self.shared_skills.read().await;
        let skill_name = trigger_match.skill_name.clone();
        
        let result = match registry.execute(&skill_name, context.extracted_args.clone()).await {
            Ok(output) => {
                let duration = start_time.elapsed();
                context.record_execution(
                    &skill_name,
                    context.extracted_args.clone(),
                    &output,
                    true,
                    duration.as_millis() as u64,
                );
                
                let trace = vec![TraceStep {
                    step: 1,
                    thought: format!("Executed skill '{}' via {} trigger", skill_name, trigger_match.trigger_type),
                    action: Some(skill_name.clone()),
                    action_input: Some(serde_json::to_string(&context.extracted_args).unwrap_or_default()),
                    observation: Some(output.clone()),
                }];
                
                self.event_bus.publish(AgentEvent::CognitiveLayerChanged { layer: "skill".to_string() });
                
                Ok((output, trace))
            }
            Err(e) => {
                let error_msg = format!("Skill execution failed: {}", e);
                tracing::error!("{}", error_msg);
                
                // Fallback to cognitive routing if skill fails
                drop(registry);
                let msg = format!("Skill '{}' failed, falling back to cognitive routing", skill_name);
                info!("{}", msg);
                self.event_bus.publish(AgentEvent::SystemLog(msg));
                
                // Continue with normal routing
                let fusion_context = if let Some(ref fusion) = self.fusion_memory {
                    fusion.get_session(session_id)
                } else {
                    None
                };
                
                let (response, traces, _) = self.route_and_process(input, session_id, fusion_context).await?;
                Ok((response, traces))
            }
        };
        
        result
    }

    async fn route_and_process(
        &self,
        input: &str,
        session_id: &str,
        _fusion_context: Option<Arc<crate::memory::fusion::layer_session::SessionLayer>>,
    ) -> crate::error::Result<(String, Vec<TraceStep>, SystemChoice)> {
        let mut input_text = input;
        let mut force_cloud = false;
        let mut force_local = false;

        if input.starts_with("[FORCE_CLOUD]") {
            force_cloud = true;
            input_text = input.strip_prefix("[FORCE_CLOUD]").unwrap_or(input).trim();
        } else if input.starts_with("[FORCE_LOCAL]") {
            force_local = true;
            input_text = input.strip_prefix("[FORCE_LOCAL]").unwrap_or(input).trim();
        }

        // 1. Try System 1 (Fast Intuition)
        // Skip System 1 if forced routing is active to allow debugging System 2 directly
        let intent = Classifier::classify_intent(input_text);
        
        if !force_cloud && !force_local {
            match intent {
                Intent::Greeting | Intent::Help | Intent::Status | Intent::Persona | Intent::Chat => {
                    if let Ok((response, traces)) = self.sys1.process(input_text, &[]).await {
                        info!("System 1 hit (Intent: {:?}): {:?}", intent, input_text);
                        self.event_bus.publish(AgentEvent::CognitiveLayerChanged { layer: "system1".to_string() });
                        return Ok((response, traces, SystemChoice::System1));
                    }
                }
                _ => {}
            }
        }

        // 2. Check for Deep Research or High Complexity (System 3)
        let complexity_score = self.assess_complexity_enhanced(input_text);
        let config = self.config.read().await.clone();
        let context = self.memory_mgr.get_context(session_id).await;

        if config.enable_adaptive_routing && !force_cloud && !force_local {
            let (choice, features) = {
                let mut meta = self.meta_router.write().await;
                meta.route(input_text, &context, complexity_score, intent.clone())
            };
            {
                let mut meta = self.meta_router.write().await;
                meta.begin_route(session_id, input, choice.clone(), features);
            }
            match choice {
                SystemChoice::System1 => {
                    if let Ok((response, traces)) = self.sys1.process(input_text, &[]).await {
                        let msg = format!("Adaptive Router -> System 1 (Intent: {:?}, Complexity: {}): {:?}", intent, complexity_score, input_text);
                        info!("{}", msg);
                        self.event_bus.publish(AgentEvent::SystemLog(msg));
                        self.event_bus.publish(AgentEvent::CognitiveLayerChanged { layer: "system1".to_string() });
                        return Ok((response, traces, SystemChoice::System1));
                    }
                }
                SystemChoice::System3 => {
                    let msg = format!("Adaptive Router -> System 3 (Intent: {:?}, Complexity: {}): {:?}", intent, complexity_score, input_text);
                    info!("{}", msg);
                    self.event_bus.publish(AgentEvent::SystemLog(msg));
                    self.event_bus.publish(AgentEvent::CognitiveLayerChanged { layer: "system3".to_string() });
                    let result = self.sys3.process(input_text, &context).await?;
                    return Ok((result.0, result.1, SystemChoice::System3));
                }
                SystemChoice::System2 => {}
            }
        }
        
        if (matches!(intent, Intent::DeepResearch) || matches!(intent, Intent::MultiStep)) && complexity_score > config.system3_threshold {
             let msg = format!("Routing to System 3 (Intent: {:?}, Complexity: {}): {:?}", intent, complexity_score, input_text);
             info!("{}", msg);
             self.event_bus.publish(AgentEvent::SystemLog(msg));
             self.event_bus.publish(AgentEvent::CognitiveLayerChanged { layer: "system3".to_string() });
             
             let result = self.sys3.process(input_text, &context).await?;
             return Ok((result.0, result.1, SystemChoice::System3));
        }

        // 3. Intelligent Routing (System 2)
        // Use cloud if forced OR (not forced local AND complexity is high)
        let use_cloud = force_cloud || (!force_local && complexity_score > config.system2_threshold); 

        if use_cloud {
             let msg = if force_cloud {
                 format!("Routing to Cloud System 2 (FORCED): {:?}", input_text)
             } else {
                 format!("Routing to Cloud System 2 (Complexity: {} > {}): {:?}", complexity_score, config.system2_threshold, input_text)
             };
             info!("{}", msg);
             self.event_bus.publish(AgentEvent::SystemLog(msg));
             self.event_bus.publish(AgentEvent::CognitiveLayerChanged { layer: "system2".to_string() });
             
             match self.sys2.process(input_text, &context).await {
                Ok(res) => Ok((res.0, res.1, SystemChoice::System2)),
                 Err(e) => {
                     let warn_msg = format!("Cloud System 2 failed: {}. Falling back to Local System 2.", e);
                     tracing::warn!("{}", warn_msg);
                     self.event_bus.publish(AgentEvent::SystemLog(warn_msg));
                     
                     // Fallback to Local System 2 (Ollama) - REUSING INSTANCE
                    let res = self.sys2_local.process(input_text, &context).await?;
                    Ok((res.0, res.1, SystemChoice::System2))
                 }
             }
        } else {
             // Local Routing (Ollama)
             let msg = if force_local {
                 format!("Routing to Local System 2 (FORCED): {:?}", input_text)
             } else {
                 format!("Routing to Local System 2 (Complexity: {} <= {}): {:?}", complexity_score, config.system2_threshold, input_text)
             };
             info!("{}", msg);
             self.event_bus.publish(AgentEvent::SystemLog(msg));
             self.event_bus.publish(AgentEvent::CognitiveLayerChanged { layer: "system2".to_string() });
             
             // REUSING INSTANCE
             match self.sys2_local.process(input_text, &context).await {
                Ok(res) => Ok((res.0, res.1, SystemChoice::System2)),
                 Err(e) => {
                     let warn_msg = format!("Local System 2 failed (likely Ollama not running): {}. Falling back to Cloud System 2.", e);
                     tracing::warn!("{}", warn_msg);
                     self.event_bus.publish(AgentEvent::SystemLog(warn_msg));
                    let res = self.sys2.process(input_text, &context).await?;
                    Ok((res.0, res.1, SystemChoice::System2))
                 }
             }
        }
    }
    
}

#[cfg(test)]
mod tests {
    use crate::cognitive::classifier::{Classifier, Intent};

    #[test]
    fn test_router_classification_delegation() {
        // We now test Classifier directly in classifier.rs
        // This test just ensures Router delegates correctly conceptually
        assert_eq!(Classifier::classify_intent("Hello"), Intent::Greeting);
    }
}
