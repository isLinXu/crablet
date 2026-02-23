use crate::cognitive::system1::System1;
use crate::cognitive::system2::System2;
use crate::cognitive::system3::System3;
use crate::memory::episodic::EpisodicMemory;
use crate::memory::working::WorkingMemory;
use crate::memory::semantic::SharedKnowledgeGraph;
#[cfg(feature = "knowledge")]
use crate::knowledge::vector_store::VectorStore;
use crate::types::TraceStep;
use anyhow::Result;
use tracing::{info, error, instrument, warn};
use std::sync::Arc;
use std::path::Path;
use crate::cognitive::CognitiveSystem;
use crate::cognitive::llm::{LlmClient, OpenAiClient, OllamaClient};
use crate::cognitive::llm::cache::CachedLlmClient;
use std::env;

use crate::events::{AgentEvent, EventBus};

use dashmap::DashMap;

#[derive(Clone)]
pub struct CognitiveRouter {
    pub sys1: System1,
    pub sys2: System2,
    pub sys3: System3,
    memory: Option<Arc<EpisodicMemory>>,
    // working_memory: Arc<Mutex<WorkingMemory>>, // Replaced by DashMap
    working_memories: Arc<DashMap<String, WorkingMemory>>,
    pub event_bus: Arc<EventBus>,
}

impl CognitiveRouter {
    pub async fn new(memory: Option<Arc<EpisodicMemory>>, event_bus: Arc<EventBus>) -> Self {
        // Initialize LLM for System 3
        let model = env::var("OPENAI_MODEL_NAME").unwrap_or_else(|_| "gpt-4o-mini".to_string());
        let llm_inner: Box<dyn LlmClient> = match OpenAiClient::new(&model) {
            Ok(client) => Box::new(client),
            Err(_) => {
                 let ollama_model = env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen3:4b".to_string());
                 Box::new(OllamaClient::new(&ollama_model))
            }
        };
        let llm: Box<dyn LlmClient> = Box::new(CachedLlmClient::new(llm_inner, 100));
        let llm_arc = Arc::new(llm);

        Self {
            sys1: System1::new(),
            sys2: System2::new(event_bus.clone()),
            sys3: System3::new(llm_arc, event_bus.clone()).await,
            memory,
            working_memories: Arc::new(DashMap::new()),
            event_bus,
        }
    }

    pub fn with_system2(_memory: Option<Arc<EpisodicMemory>>, _sys2: System2, _event_bus: Arc<EventBus>) -> Self {
        // Initialize LLM for System 3 (duplicate logic for now)
        let model = env::var("OPENAI_MODEL_NAME").unwrap_or_else(|_| "gpt-4o-mini".to_string());
        let llm_inner: Box<dyn LlmClient> = match OpenAiClient::new(&model) {
            Ok(client) => Box::new(client),
            Err(_) => {
                let ollama_model = env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen3:4b".to_string());
                Box::new(OllamaClient::new(&ollama_model))
            }
        };
        let llm: Box<dyn LlmClient> = Box::new(CachedLlmClient::new(llm_inner, 100));
        let _llm_arc = Arc::new(llm);

        // System3 needs to be initialized async now, but this constructor is synchronous.
        // We can't await here.
        // This is problematic for tests.
        // We should make with_system2 async or fake System3 init.
        // However, System3::new is async because it registers agents.
        
        // Hack for tests: block on async init? No, we are in async runtime usually.
        // Correct fix: Make with_system2 async.
        
        // But we can't change signature easily if it's used in non-async context?
        // It is used in tests which are #[tokio::test] (async).
        // So making it async is fine.
        panic!("Use CognitiveRouter::with_system2_async instead");
    }

    pub async fn with_system2_async(memory: Option<Arc<EpisodicMemory>>, sys2: System2, event_bus: Arc<EventBus>) -> Self {
        // Initialize LLM for System 3
        let model = env::var("OPENAI_MODEL_NAME").unwrap_or_else(|_| "gpt-4o-mini".to_string());
        let llm_inner: Box<dyn LlmClient> = match OpenAiClient::new(&model) {
            Ok(client) => Box::new(client),
            Err(_) => {
                let ollama_model = env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen3:4b".to_string());
                Box::new(OllamaClient::new(&ollama_model))
            }
        };
        let llm: Box<dyn LlmClient> = Box::new(CachedLlmClient::new(llm_inner, 100));
        let llm_arc = Arc::new(llm);

        Self {
            sys1: System1::new(),
            sys2,
            sys3: System3::new(llm_arc, event_bus.clone()).await,
            memory,
            working_memories: Arc::new(DashMap::new()),
            event_bus,
        }
    }
    
    pub fn with_knowledge(
        mut self,
        kg: Option<SharedKnowledgeGraph>,
        #[cfg(feature = "knowledge")]
        vector_store: Option<Arc<VectorStore>>
    ) -> Self {
        // Re-create System 2 with knowledge components
        #[cfg(feature = "knowledge")]
        {
            self.sys2 = self.sys2.with_knowledge(kg, vector_store);
        }
        #[cfg(not(feature = "knowledge"))]
        {
            self.sys2 = self.sys2.with_knowledge(kg);
        }
        self
    }

    pub fn with_config(mut self, config: &crate::config::Config) -> Self {
        self.sys2 = self.sys2.with_config(config);
        self
    }

    pub async fn load_skills<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut skills = self.sys2.skills.write().await;
        skills.load_from_dir(path).await
    }

    pub fn watch_skills<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.sys2 = self.sys2.watch_skills(path.as_ref());
        self
    }

    pub async fn consolidate_memory(&self, session_id: &str) -> Result<()> {
        #[cfg(feature = "knowledge")]
        if let Some(mem) = &self.memory {
            self.sys2.consolidate_memory(mem, session_id).await?;
        }
        Ok(())
    }

    pub async fn get_all_skills(&self) -> Vec<crate::skills::SkillManifest> {
        let skills = self.sys2.skills.read().await;
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
                    return Err(anyhow::anyhow!("Knowledge feature not enabled for PDF parsing"));
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
             warn!("Vector Store not available for ingestion");
        }
        
        Ok(())
    }

    #[instrument(skip(self), fields(session.id = %session_id, input.length = input.len()))]
    pub async fn process(&self, input: &str, session_id: &str) -> Result<(String, Vec<TraceStep>)> {
        self.event_bus.publish(AgentEvent::UserInput(input.to_string()));

        // 0. Save User Input to Episodic Memory
        if let Some(mem) = &self.memory {
            if let Err(e) = mem.save_message(session_id, "user", input).await {
                error!("Failed to save user message: {}", e);
            }
        }

        // 1. Update Working Memory with User Input
        {
            // Get or create working memory for this session
            let mut wm = self.working_memories.entry(session_id.to_string())
                .or_insert_with(|| WorkingMemory::new(10));
            wm.add_message("user", input);
            wm.compress_context(); // Trigger compression if needed
        }

        let start = std::time::Instant::now();
        let (response, traces) = self.route_and_process(input, session_id).await?;
        let latency = start.elapsed();

        // 2. Update Working Memory with Assistant Response
        {
            let mut wm = self.working_memories.entry(session_id.to_string())
                .or_insert_with(|| WorkingMemory::new(10));
            wm.add_message("assistant", &response);
            wm.compress_context();
        }

        // 3. Save Assistant Response to Episodic Memory
        if let Some(mem) = &self.memory {
            if let Err(e) = mem.save_message(session_id, "assistant", &response).await {
                error!("Failed to save assistant message: {}", e);
            }
        }

        info!("Total latency: {:?}", latency);
        Ok((response, traces))
    }

    #[instrument(skip(self), fields(session.id = %session_id))]
    async fn route_and_process(&self, input: &str, session_id: &str) -> Result<(String, Vec<TraceStep>)> {
        use tracing::warn;

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
        if !force_cloud && !force_local {
            if let Ok((response, traces)) = self.sys1.process(input_text, &[]).await {
                info!("System 1 hit: {:?}", input_text);
                return Ok((response, traces));
            }
        }

        // 2. Check for Deep Research (System 3)
        if input_text.to_lowercase().starts_with("research ") || input_text.contains("deep research") {
             let msg = format!("Routing to System 3 (Deep Research): {:?}", input_text);
             info!("{}", msg);
             self.event_bus.publish(AgentEvent::SystemLog(msg));
             
             let context = {
                let wm = self.working_memories.entry(session_id.to_string())
                    .or_insert_with(|| WorkingMemory::new(10));
                wm.get_context()
             };
             return self.sys3.process(input_text, &context).await;
        }

        // 3. Intelligent Routing (System 2)
        let complexity_score = self.assess_complexity(input_text);
        // Use cloud if forced OR (not forced local AND complexity is high)
        // Default threshold 0.3 might be too low for some tasks, let's bump it or refine.
        // Actually, for better UX, let's prefer Local unless complexity is explicitly HIGH (>0.6) or tools are involved?
        // Current logic: > 0.3 goes to cloud. "calculate" adds 0.2. "1+1" len < 100. Score = 0.
        // So "1+1" goes to Local. "calculate 1+1" score = 0.2 -> Local.
        // "Please explain quantum physics" -> "explain" +0.2 -> Local.
        // Maybe we want Cloud more often?
        // Let's adjust: Cloud if complexity > 0.5.
        // But also check if Ollama is available? If not, fallback to Cloud immediately.
        
        let use_cloud = force_cloud || (!force_local && complexity_score > 0.5); 

        let context = {
            let wm = self.working_memories.entry(session_id.to_string())
                .or_insert_with(|| WorkingMemory::new(10));
            wm.get_context()
        };

        if use_cloud {
             let msg = if force_cloud {
                 format!("Routing to Cloud System 2 (FORCED): {:?}", input_text)
             } else {
                 format!("Routing to Cloud System 2 (Complexity: {} > 0.5): {:?}", complexity_score, input_text)
             };
             info!("{}", msg);
             self.event_bus.publish(AgentEvent::SystemLog(msg));
             
             match self.sys2.process(input_text, &context).await {
                 Ok(res) => Ok(res),
                 Err(e) => {
                     let warn_msg = format!("Cloud System 2 failed: {}. Falling back to Local System 2.", e);
                     warn!("{}", warn_msg);
                     self.event_bus.publish(AgentEvent::SystemLog(warn_msg));
                     
                     // Fallback to Local System 2 (Ollama)
                     let ollama_model = env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen2.5:14b".to_string());
                     let local_llm_inner: Box<dyn LlmClient> = Box::new(OllamaClient::new(&ollama_model));
                     let local_llm = Box::new(CachedLlmClient::new(local_llm_inner, 100));
                     
                     let mut local_sys2 = System2::with_client(local_llm, self.event_bus.clone());
                     
                     #[cfg(feature = "knowledge")]
                     {
                         local_sys2 = local_sys2.with_knowledge(self.sys2.kg.clone(), self.sys2.vector_store.clone());
                     }
                     #[cfg(not(feature = "knowledge"))]
                     {
                         local_sys2 = local_sys2.with_knowledge(self.sys2.kg.clone());
                     }
                     
                     local_sys2.skills = self.sys2.skills.clone();
                     
                     local_sys2.process(input_text, &context).await
                 }
             }
        } else {
             // Local Routing (Ollama)
             let msg = if force_local {
                 format!("Routing to Local System 2 (FORCED): {:?}", input_text)
             } else {
                 format!("Routing to Local System 2 (Complexity: {} <= 0.5): {:?}", complexity_score, input_text)
             };
             info!("{}", msg);
             self.event_bus.publish(AgentEvent::SystemLog(msg));
             
             // Create a temporary System 2 instance using Ollama client but sharing knowledge
             let ollama_model = env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen2.5:14b".to_string());
             let local_llm_inner: Box<dyn LlmClient> = Box::new(OllamaClient::new(&ollama_model));
             let local_llm = Box::new(CachedLlmClient::new(local_llm_inner, 100));
             // Pass event_bus to the new System2 instance
             let mut local_sys2 = System2::with_client(local_llm, self.event_bus.clone());
             
             #[cfg(feature = "knowledge")]
             {
                 local_sys2 = local_sys2.with_knowledge(self.sys2.kg.clone(), self.sys2.vector_store.clone());
             }
             #[cfg(not(feature = "knowledge"))]
             {
                 local_sys2 = local_sys2.with_knowledge(self.sys2.kg.clone());
             }
             
             // Share skills registry
             local_sys2.skills = self.sys2.skills.clone();
                
             match local_sys2.process(input_text, &context).await {
                 Ok(res) => Ok(res),
                 Err(e) => {
                     let warn_msg = format!("Local System 2 failed (likely Ollama not running): {}. Falling back to Cloud System 2.", e);
                     warn!("{}", warn_msg);
                     self.event_bus.publish(AgentEvent::SystemLog(warn_msg));
                     self.sys2.process(input_text, &context).await
                 }
             }
        }
    }
    
    fn assess_complexity(&self, input: &str) -> f32 {
        let mut score: f32 = 0.0;
        
        // Length heuristic
        if input.len() > 100 { score += 0.3; } // Increased weight
        if input.len() > 500 { score += 0.4; }
        
        // Keyword heuristic
        // "code" is a strong indicator for cloud usually, unless local model is a coding model
        let complex_keywords = ["analyze", "compare", "reason", "explain", "design", "search", "calculate", "weather"];
        for keyword in complex_keywords {
            if input.to_lowercase().contains(keyword) {
                score += 0.2;
            }
        }
        
        // Code specific
        if input.to_lowercase().contains("function") || input.contains("```") {
            score += 0.4;
        }
        
        // Tool usage heuristic
        if input.starts_with("run ") || input.starts_with("read ") || input.starts_with("search ") {
            score += 0.6; // Strong push to cloud if tools are likely needed and local might fail tool calling
        }

        if score > 1.0 { 1.0 } else { score }
    }
}
