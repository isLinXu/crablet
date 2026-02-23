use anyhow::Result;
use crate::tools::bash::BashPlugin;
use crate::tools::file::FilePlugin;
use crate::tools::search::WebSearchPlugin;
use crate::tools::http::HttpPlugin;
// use crate::tools::vision::VisionPlugin;
use crate::cognitive::llm::{LlmClient, OpenAiClient};
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
use crate::agent::coordinator::AgentCoordinator;
use crate::agent::researcher::ResearchAgent;
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
use crate::cognitive::llm::cache::CachedLlmClient;

use crate::cognitive::middleware::{MiddlewarePipeline, MiddlewareState, PlanningMiddleware, RagMiddleware, SkillContextMiddleware, SafetyMiddleware, RoutingMiddleware, CostGuardMiddleware, SemanticCacheMiddleware};
use crate::events::EventBus;

#[derive(Clone)]
pub struct System2 {
    llm: Arc<Box<dyn LlmClient>>,
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
    #[allow(dead_code)]
    coordinator: Arc<RwLock<AgentCoordinator>>,
    react_engine: Arc<ReActEngine>,
    skill_watcher: Option<Arc<SkillWatcher>>,
    event_bus: Arc<EventBus>,
    pipeline: Arc<MiddlewarePipeline>,
}

use crate::cognitive::llm::OllamaClient;

impl System2 {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
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
        // We create a "multimodal" client if needed, or stick to the primary one.
        // For simplicity, we assume the primary `llm_arc` handles everything or we rely on the specific `VisionPlugin`
        // to use its own client if needed. However, ReAct itself uses `llm_arc`.
        // If the user wants ReAct to be multimodal-aware (e.g. seeing images in context), 
        // `llm_arc` must support vision (e.g. gpt-4o).
        // 
        // If `model` is "gpt-4o-mini", it supports vision.
        // If it's a text-only model, we might have issues if we pass images in messages.
        // 
        // TODO: In a real robust system, we might switch `llm_arc` based on input content.
        // For now, we trust the configured model is capable or the tools handle it.
        
        let planner = Arc::new(TaskPlanner::new(llm_arc.clone()));

        let research_agent = Arc::new(ResearchAgent::new(llm_arc.clone(), event_bus.clone()));
        let coordinator = AgentCoordinator::new(research_agent.clone());
        let coordinator = Arc::new(RwLock::new(coordinator));
        
        let skills = Arc::new(RwLock::new(SkillRegistry::new()));
        
        // Use configured skills dir or default
        let skills_dir = std::path::PathBuf::from("skills");
        let skill_manager = Arc::new(SkillManagerTool::new(&skills_dir));

        // Register Native Plugins
        {
            let mut registry = skills.try_write().expect("Failed to lock registry for initialization");
            registry.register_plugin(Box::new(WebSearchPlugin::new()));
            registry.register_plugin(Box::new(HttpPlugin));
            // registry.register_plugin(Box::new(VisionPlugin::new(llm_arc.clone())));
            registry.register_plugin(Box::new(BashPlugin::new(oracle.clone())));
            registry.register_plugin(Box::new(FilePlugin::new(oracle.clone())));
            // Management Plugins
            registry.register_plugin(Box::new(InstallSkillPlugin::new(skill_manager.clone())));
            registry.register_plugin(Box::new(CreateSkillPlugin::new(skill_manager.clone())));
            
            // Browser Plugin
            registry.register_plugin(Box::new(BrowserPlugin));
            
            // Demo Plugins
            registry.register_plugin(Box::new(WeatherPlugin));
            registry.register_plugin(Box::new(CalculatorPlugin));
            
            // MCP Meta Plugins
            // We use skills.clone() which clones the Arc, which is safe even while holding the lock on the inner RwLock.
            registry.register_plugin(Box::new(McpResourcePlugin::new(skills.clone())));
            registry.register_plugin(Box::new(McpPromptPlugin::new(skills.clone())));
        }

        let react_engine = Arc::new(ReActEngine::new(llm_arc.clone(), skills.clone(), event_bus.clone()));

        // Initialize Middleware Pipeline
        let pipeline = MiddlewarePipeline::new()
            .add(SafetyMiddleware)
            .add(CostGuardMiddleware) // Add CostGuard early
            .add(SemanticCacheMiddleware) // Add Cache early
            .add(PlanningMiddleware)
            .add(RagMiddleware)
            .add(SkillContextMiddleware);
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
            coordinator,
            react_engine,
            skill_watcher: None,
            event_bus,
            pipeline,
        }
    }

    pub fn with_config(mut self, config: &Config) -> Self {
        // Update skill manager with correct path
        self.skill_manager = Arc::new(SkillManagerTool::new(&config.skills_dir));
        
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
                self.consolidator = Some(Arc::new(MemoryConsolidator::new(self.llm.clone(), vs)));
            }
        }
        self
    }

    pub fn with_client(llm_inner: Box<dyn LlmClient>, event_bus: Arc<EventBus>) -> Self {
        // Wrap with Cache
        let llm: Box<dyn LlmClient> = Box::new(CachedLlmClient::new(llm_inner, 100));
        let llm_arc = Arc::new(llm);
        
        let research_agent = Arc::new(ResearchAgent::new(llm_arc.clone(), event_bus.clone()));
        let coordinator = AgentCoordinator::new(research_agent.clone());

        let skills = Arc::new(RwLock::new(SkillRegistry::new()));
        let oracle = SafetyOracle::new(SafetyLevel::Strict);

        let planner = Arc::new(TaskPlanner::new(llm_arc.clone()));

        let skill_manager = Arc::new(SkillManagerTool::new(&std::path::PathBuf::from("skills")));

        // Register Native Plugins
        {
            let mut registry = skills.try_write().expect("Failed to lock registry for initialization");
            registry.register_plugin(Box::new(WebSearchPlugin::new()));
            registry.register_plugin(Box::new(HttpPlugin));
            // registry.register_plugin(Box::new(VisionPlugin::new(llm_arc.clone())));
            registry.register_plugin(Box::new(BashPlugin::new(oracle.clone())));
            registry.register_plugin(Box::new(FilePlugin::new(oracle.clone())));
            registry.register_plugin(Box::new(InstallSkillPlugin::new(skill_manager.clone())));
            registry.register_plugin(Box::new(CreateSkillPlugin::new(skill_manager.clone())));
        }

        let react_engine = Arc::new(ReActEngine::new(llm_arc.clone(), skills.clone(), event_bus.clone()));

        // Initialize Middleware Pipeline
        let pipeline = Arc::new(MiddlewarePipeline::new()
            .add(SafetyMiddleware)
            .add(CostGuardMiddleware) // Add CostGuard early
            .add(SemanticCacheMiddleware) // Add Cache early
            .add(RoutingMiddleware::new(llm_arc.clone()))
            .add(PlanningMiddleware)
            .add(RagMiddleware)
            .add(SkillContextMiddleware));

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
            coordinator: Arc::new(RwLock::new(coordinator)),
            react_engine,
            skill_watcher: None,
            event_bus,
            pipeline,
        }
    }

    pub async fn consolidate_memory(&self, memory: &EpisodicMemory, session_id: &str) -> Result<()> {
        #[cfg(feature = "knowledge")]
        {
            if let Some(consolidator) = &self.consolidator {
                 consolidator.consolidate(memory, session_id).await?;
            }
        }
        Ok(())
    }
}

use crate::error::CrabletError;

#[async_trait]
impl CognitiveSystem for System2 {
    fn name(&self) -> &str {
        "System 2 (Analytical)"
    }

    async fn process(&self, input: &str, context: &[Message]) -> Result<(String, Vec<TraceStep>)> {
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
        };

        // 2. Execute Middleware Pipeline
        let mut final_context = context.to_vec();
        
        // Native Vision Enhancement: Scan for [System Note] with image paths and inject them as Image parts
        if input.contains("[System Note: The user has uploaded the following files") {
            // We need to inject images into the LAST user message, or append a new one if structure is weird.
            // But context is [User, Assistant, User, ...] usually.
            // Let's find the last message with role "user"
            
            if let Some(user_msg_idx) = final_context.iter().rposition(|m| m.role == "user") {
                let mut user_msg = final_context[user_msg_idx].clone();
                let mut new_parts = Vec::new();
                
                // Add existing content.
                // If it was just text, convert to Text part.
                // If it was already parts, keep them.
                if let Some(text) = user_msg.text() {
                    new_parts.push(crate::types::ContentPart::Text { text: text.clone() });
                } else if let Some(parts) = &user_msg.content {
                    new_parts.extend(parts.clone());
                }
                
                // Parse paths from input string
                for line in input.lines() {
                    if let Some(path) = line.trim().strip_prefix("- File: ") {
                        let path = path.trim();
                        // Try to read and encode
                        // Check if it's an image before injecting as ImageUrl
                        let ext = std::path::Path::new(path).extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                        let is_image = matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp");

                        if is_image {
                            if let Ok(bytes) = tokio::fs::read(path).await {
                                use base64::Engine;
                                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                                // Detect mime type simple (extension based)
                                let mime = if path.ends_with(".png") { "image/png" } 
                                    else if path.ends_with(".jpg") || path.ends_with(".jpeg") { "image/jpeg" }
                                    else if path.ends_with(".webp") { "image/webp" }
                                    else { "application/octet-stream" };
                                    
                                let data_url = format!("data:{};base64,{}", mime, b64);
                                
                                info!("Injecting image content for: {}", path);
                                new_parts.push(crate::types::ContentPart::ImageUrl { 
                                    image_url: crate::types::ImageUrl { url: data_url } 
                                });
                            } else {
                                warn!("Failed to read image for vision injection: {}", path);
                            }
                        } else {
                            // Non-image file (e.g. PDF), skip injection or handle differently
                            // For PDFs, we already ingested them into Knowledge Base in Router.
                            // We can optionally add a system note saying "Document X is available in knowledge base."
                            // But for now, just don't try to send it as an image to OpenAI.
                            info!("Skipping non-image file injection for: {}", path);
                            
                            // Inject a system prompt hint about RAG availability
                            if let Some(filename) = std::path::Path::new(path).file_name().and_then(|s| s.to_str()) {
                                new_parts.push(crate::types::ContentPart::Text { 
                                    text: format!("\n[System Hint] The file '{}' has been ingested into the knowledge base. Use retrieved context to answer questions about it.", filename)
                                });
                            }
                        }
                    }
                }
                
                if !new_parts.is_empty() {
                    user_msg.content = Some(new_parts);
                    final_context[user_msg_idx] = user_msg;
                    
                    // Add system prompt to inform the model about the image
                    final_context.insert(0, Message::new("system", 
                        "An image has been uploaded to the context. You can see it directly using your vision capabilities. \
                        Do NOT use tools to read or analyze the image file. Just describe what you see in the image provided in the user message."
                    ));
                }
            }
        }
        
        match self.pipeline.execute(input, &mut final_context, &state).await {
            Ok(Some(result)) => return Ok(result),
            Ok(None) => {}, // Continue
            Err(e) => {
                warn!("Middleware Error: {}", e);
                // Wrap in CrabletError? For now just propagate anyhow
                return Err(e);
            }
        }

        // 3. ReAct Loop (Execution Engine)
        let result = match self.react_engine.execute(&final_context).await {
            Ok((response, traces)) => Ok((response, traces)),
            Err(e) => {
                warn!("ReAct Engine Critical Failure: {}", e);
                Err(anyhow::anyhow!(CrabletError::Unknown(e.to_string())))
            }
        };

        // Post-processing: Detect artifacts and publish CanvasUpdate
        if let Ok((response, _)) = &result {
            // Simple heuristics for artifacts
            // 1. Mermaid Diagrams
            if response.contains("```mermaid") {
                if let Some(start) = response.find("```mermaid") {
                    let rest = &response[start..];
                    if let Some(end_code) = rest[10..].find("```") {
                        let content = &rest[10..10+end_code];
                        self.event_bus.publish(crate::events::AgentEvent::CanvasUpdate {
                            title: "Diagram Generated".to_string(),
                            content: content.trim().to_string(),
                            kind: "mermaid".to_string(),
                        });
                    }
                }
            }
            
            // 2. HTML Previews (e.g. for UI mockups)
            if response.contains("```html") {
                 if let Some(start) = response.find("```html") {
                    let rest = &response[start..];
                    if let Some(end_code) = rest[7..].find("```") {
                        let content = &rest[7..7+end_code];
                        // Heuristic: Only publish if it looks like a full page or component
                        if content.contains("<div") || content.contains("<html") || content.contains("<body") {
                            self.event_bus.publish(crate::events::AgentEvent::CanvasUpdate {
                                title: "HTML Preview".to_string(),
                                content: content.trim().to_string(),
                                kind: "html".to_string(),
                            });
                        }
                    }
                }
            }
            
            // 3. Significant Code Blocks (Rust/Python)
            // Iterate over types to find the longest block
            for lang in ["rust", "python", "javascript", "typescript", "json", "toml"] {
                let tag = format!("```{}", lang);
                if response.contains(&tag) {
                     if let Some(start) = response.find(&tag) {
                        let offset = tag.len();
                        let rest = &response[start..];
                        if let Some(end_code) = rest[offset..].find("```") {
                            let content = &rest[offset..offset+end_code];
                            // Only publish if significant length (> 5 lines or > 100 chars)
                            if content.lines().count() > 5 || content.len() > 100 {
                                self.event_bus.publish(crate::events::AgentEvent::CanvasUpdate {
                                    title: format!("{} Snippet", lang.to_uppercase()),
                                    content: content.trim().to_string(),
                                    kind: "code".to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
        
        result
    }
}
