use async_trait::async_trait;
use anyhow::Result;
use crate::types::{Message, TraceStep};
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::cognitive::llm::LlmClient;
use crate::skills::SkillRegistry;
use crate::events::EventBus;
use crate::memory::semantic::SharedKnowledgeGraph;
#[cfg(feature = "knowledge")]
use crate::knowledge::vector_store::VectorStore;
use crate::cognitive::planner::TaskPlanner;
use crate::tools::manager::SkillManagerTool;
use tracing::{info, warn};

pub struct MiddlewareState {
    pub llm: Arc<Box<dyn LlmClient>>,
    pub skills: Arc<RwLock<SkillRegistry>>,
    pub event_bus: Arc<EventBus>,
    pub kg: Option<SharedKnowledgeGraph>,
    #[cfg(feature = "knowledge")]
    pub vector_store: Option<Arc<VectorStore>>,
    pub planner: Arc<TaskPlanner>,
    pub skill_manager: Arc<SkillManagerTool>,
}

pub struct PlanningMiddleware;

#[async_trait]
pub trait CognitiveMiddleware: Send + Sync {
    /// Execute the middleware logic.
    /// Returns Ok(Some((response, traces))) if the request is handled and should return early.
    /// Returns Ok(None) to continue to the next middleware.
    /// Modifies `context` in place (e.g. injecting system prompts).
    async fn execute(&self, input: &str, context: &mut Vec<Message>, state: &MiddlewareState) -> Result<Option<(String, Vec<TraceStep>)>>;
    
    fn name(&self) -> &str;
}

pub struct MiddlewarePipeline {
    middlewares: Vec<Box<dyn CognitiveMiddleware>>,
}

impl MiddlewarePipeline {
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
        }
    }

    pub fn add<M: CognitiveMiddleware + 'static>(mut self, middleware: M) -> Self {
        self.middlewares.push(Box::new(middleware));
        self
    }

    pub async fn execute(&self, input: &str, context: &mut Vec<Message>, state: &MiddlewareState) -> Result<Option<(String, Vec<TraceStep>)>> {
        for middleware in &self.middlewares {
            // tracing::info!("Executing middleware: {}", middleware.name());
            if let Some(result) = middleware.execute(input, context, state).await? {
                return Ok(Some(result));
            }
        }
        Ok(None)
    }
}

// --- Concrete Implementations ---

pub struct SafetyMiddleware;

#[async_trait]
impl CognitiveMiddleware for SafetyMiddleware {
    fn name(&self) -> &str {
        "Safety Check"
    }

    async fn execute(&self, input: &str, _context: &mut Vec<Message>, _state: &MiddlewareState) -> Result<Option<(String, Vec<TraceStep>)>> {
        // Basic Input Safety Checks
        if input.len() > 10000 {
            warn!("Input blocked: too long ({} chars)", input.len());
            return Ok(Some(("I cannot process this request because it is too long.".to_string(), vec![])));
        }
        
        // Check for obvious jailbreak patterns (very naive MVP)
        let lower = input.to_lowercase();
        if lower.contains("ignore all previous instructions") || lower.contains("ignore above instructions") {
             warn!("Input blocked: potential jailbreak detected");
             return Ok(Some(("I cannot comply with that request.".to_string(), vec![])));
        }
        
        Ok(None)
    }
}

pub struct CostGuardMiddleware;

#[async_trait]
impl CognitiveMiddleware for CostGuardMiddleware {
    fn name(&self) -> &str {
        "Cost Guard"
    }

    async fn execute(&self, input: &str, context: &mut Vec<Message>, _state: &MiddlewareState) -> Result<Option<(String, Vec<TraceStep>)>> {
        // Simple token estimation (approx 4 chars per token)
        let input_tokens = input.len() / 4;
        let context_tokens: usize = context.iter().map(|m| m.text().map(|s| s.len() / 4).unwrap_or(0)).sum();
        let total_tokens = input_tokens + context_tokens;
        
        // Hard limit for MVP: 8k tokens context window safety
        if total_tokens > 8000 {
            warn!("CostGuard: Context too large ({} tokens). Truncating oldest messages.", total_tokens);
            // Truncate oldest messages, keeping system prompt (usually index 0)
            // Strategy: Keep index 0, remove 1..N until size fits
            
            let _current_tokens = total_tokens;
            let _remove_count = 0;
            
            // Start checking from index 1 (skip system)
            // But we can't easily iterate and remove.
            // Let's just create a new vector
            
            if context.len() > 2 {
                let mut new_context = Vec::new();
                if let Some(sys) = context.first() {
                    new_context.push(sys.clone());
                }
                
                // Add recent messages until we hit limit (reverse order then reverse back)
                let mut added_tokens = new_context[0].text().map(|s| s.len() / 4).unwrap_or(0);
                let limit = 6000; // Target size after truncation
                
                for msg in context.iter().skip(1).rev() {
                    let msg_tokens = msg.text().map(|s| s.len() / 4).unwrap_or(0);
                    if added_tokens + msg_tokens < limit {
                        new_context.push(msg.clone());
                        added_tokens += msg_tokens;
                    } else {
                        break;
                    }
                }
                
                // Correct order (except system which is at 0)
                // new_context: [System, Latest, Prev, ...] -> [System, ..., Prev, Latest]
                // Actually the above logic pushes system first, then latest, then prev.
                // So new_context is [System, Latest, Prev...]
                // We need to keep System at 0, and reverse 1..end.
                
                if new_context.len() > 1 {
                    let mut tail = new_context.split_off(1);
                    tail.reverse();
                    new_context.extend(tail);
                }
                
                *context = new_context;
                info!("CostGuard: Truncated context to {} messages (approx {} tokens)", context.len(), added_tokens);
            }
        }
        
        Ok(None)
    }
}

pub struct SemanticCacheMiddleware;

#[async_trait]
impl CognitiveMiddleware for SemanticCacheMiddleware {
    fn name(&self) -> &str {
        "Semantic Cache"
    }

    async fn execute(&self, input: &str, _context: &mut Vec<Message>, state: &MiddlewareState) -> Result<Option<(String, Vec<TraceStep>)>> {
        #[cfg(feature = "knowledge")]
        if let Some(vs) = &state.vector_store {
            // Check for similar queries in vector store
            // We need a specific collection or metadata filter for "Q&A Cache"
            // For MVP, let's assume if we find a very high match (0.95+) that is a "conversation_summary" or "past_answer", we might use it.
            // But VectorStore currently stores chunks of documents.
            // We need to store Q&A pairs.
            
            // Skip for now if no dedicated cache collection.
            // But we can simulate "System 1.5" here: if input matches a known FAQ in docs with high confidence.
            
            if let Ok(results) = vs.search(input, 1).await {
                if let Some((_content, score, _metadata)) = results.first() {
                    if *score > 0.92 {
                        // High confidence match.
                        // Check if it looks like an answer or a fact.
                        info!("Semantic Cache: High confidence match ({:.2})", score);
                        
                        // If it's a "conversation_summary", it might not be a direct answer.
                        // But if we index FAQs, this would work.
                        
                        // For now, let's just log it. Returning directly might be risky without verified QA pairs.
                        // return Ok(Some((format!("(Cached) Based on my memory: {}", content), vec![])));
                    }
                }
            }
        }
        Ok(None)
    }
}


#[async_trait]
impl CognitiveMiddleware for PlanningMiddleware {
    fn name(&self) -> &str {
        "Planning"
    }

    async fn execute(&self, input: &str, context: &mut Vec<Message>, state: &MiddlewareState) -> Result<Option<(String, Vec<TraceStep>)>> {
        // Planning Phase (for complex queries)
        if input.len() > 100 || input.contains(" and ") || input.contains(" then ") {
             info!("Complex query detected, invoking Task Planner...");
             if let Ok(plan) = state.planner.create_plan(input).await {
                 info!("Plan generated: {} steps", plan.tasks.len());
                 if let Ok(plan_str) = serde_json::to_string_pretty(&plan.tasks) {
                     let plan_context = format!("\n[CURRENT PLAN]\n{}\nFollow this plan to answer the user request.", plan_str);
                     // Inject into system prompt or as a new system message
                     context.insert(0, Message::new("system", &plan_context));
                 }
             }
        }
        Ok(None)
    }
}

pub struct RagMiddleware;

#[async_trait]
impl CognitiveMiddleware for RagMiddleware {
    fn name(&self) -> &str {
        "RAG Retrieval"
    }

    async fn execute(&self, input: &str, context: &mut Vec<Message>, state: &MiddlewareState) -> Result<Option<(String, Vec<TraceStep>)>> {
        let mut rag_context = String::new();
        
        // Graph Retrieval
        if let Some(kg) = &state.kg {
            let words: Vec<&str> = input.split_whitespace().collect();
            for word in words {
                let clean_word = word.trim_matches(|c: char| !c.is_alphanumeric());
                if clean_word.len() > 3 {
                    let relations_result: Result<Vec<(String, String, String)>> = kg.find_related(clean_word).await;
                    if let Ok(relations) = relations_result {
                        if !relations.is_empty() {
                            rag_context.push_str(&format!("\n[Knowledge Graph about '{}']:\n", clean_word));
                            for (dir, rel, target) in relations.iter().take(5) {
                                if dir == "->" {
                                    rag_context.push_str(&format!("- {} {} {}\n", clean_word, rel, target));
                                } else {
                                    rag_context.push_str(&format!("- {} {} {}\n", target, rel, clean_word));
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Vector Retrieval
        #[cfg(feature = "knowledge")]
        if let Some(vs) = &state.vector_store {
            if let Ok(results) = vs.search(input, 3).await {
                if !results.is_empty() {
                    rag_context.push_str("\n[Semantic Search Results]:\n");
                    for (i, (content, score, metadata)) in results.iter().enumerate() {
                        let _source_name = metadata.get("source").and_then(|v| v.as_str()).unwrap_or("Unknown Document");
                        let _chunk_id = metadata.get("chunk_index").and_then(|v| v.as_u64()).unwrap_or(0);
                        rag_context.push_str(&format!("- [Ref {}] (score: {:.2}) {}\n", i + 1, score, content));
                    }
                }
            }
        }
        
        if !rag_context.is_empty() {
            info!("Injecting RAG context");
            let msg = format!("\n[KNOWLEDGE]\nUse the following retrieved knowledge to answer the user's question if relevant.\nImportant: When using information from the context, cite the source using [Ref X] notation or mention the Knowledge Graph.\n{}\n", rag_context);
            context.insert(0, Message::new("system", &msg));
        }
        
        Ok(None)
    }
}

pub struct SkillContextMiddleware;

#[async_trait]
impl CognitiveMiddleware for SkillContextMiddleware {
    fn name(&self) -> &str {
        "Skill Context Injection"
    }

    async fn execute(&self, _input: &str, context: &mut Vec<Message>, state: &MiddlewareState) -> Result<Option<(String, Vec<TraceStep>)>> {
        let registry = state.skills.read().await;
        let skills_list = registry.list_skills();
        
        if !skills_list.is_empty() {
            let mut skills_desc = String::from("You have access to the following tools:\n");
            for skill in &skills_list {
                skills_desc.push_str(&format!("- {}: {} (Args: {})\n", skill.name, skill.description, skill.parameters));
            }
            
            let msg = format!("\n[TOOLS]\n{}\nIf you need to use a tool, please generate a tool call.\n", skills_desc);
            context.insert(0, Message::new("system", &msg));
        }
        
        Ok(None)
    }
}

use crate::cognitive::classifier::{IntentClassifier, Intent};
#[cfg(feature = "knowledge")]
use tokio::sync::OnceCell;

pub struct RoutingMiddleware {
    classifier: IntentClassifier,
    #[cfg(feature = "knowledge")]
    prototypes: OnceCell<Vec<(Intent, Vec<f32>)>>,
}

impl RoutingMiddleware {
    pub fn new(llm: Arc<Box<dyn LlmClient>>) -> Self {
        Self {
            classifier: IntentClassifier::new(llm),
            #[cfg(feature = "knowledge")]
            prototypes: OnceCell::new(),
        }
    }

    #[cfg(feature = "knowledge")]
    async fn get_prototypes(&self, vs: &VectorStore) -> Option<&Vec<(Intent, Vec<f32>)>> {
        self.prototypes.get_or_try_init(|| async {
            let mut protos = Vec::new();
            // Define prototypes
            let seeds = vec![
                (Intent::ChitChat, "hello hi greetings good morning how are you who are you"),
                (Intent::Research, "research find information search web deep dive look up news"),
                (Intent::Coding, "write code python rust function script programming implement fix bug"),
                (Intent::Reasoning, "plan analyze think step by step reason logic solve problem strategy"),
            ];

            for (intent, text) in seeds {
                if let Ok(emb) = vs.embed_query(text).await {
                    protos.push((intent, emb));
                }
            }
            if protos.is_empty() {
                Err(anyhow::anyhow!("Failed to embed prototypes"))
            } else {
                Ok(protos)
            }
        }).await.ok()
    }
}

#[async_trait]
impl CognitiveMiddleware for RoutingMiddleware {
    fn name(&self) -> &str {
        "Intent Routing"
    }

    async fn execute(&self, input: &str, context: &mut Vec<Message>, state: &MiddlewareState) -> Result<Option<(String, Vec<TraceStep>)>> {
        let mut intent = Intent::Unknown;
        
        // 1. Try Semantic Classification (Zero-Shot) if VectorStore is available
        #[cfg(feature = "knowledge")]
        if let Some(vs) = &state.vector_store {
            if let Some(protos) = self.get_prototypes(vs).await {
                if let Ok(query_emb) = vs.embed_query(input).await {
                    let mut max_score = -1.0;
                    let mut best_intent = Intent::Unknown;
                    
                    for (proto_intent, proto_emb) in protos {
                        let score = cosine_similarity(&query_emb, proto_emb);
                        if score > max_score {
                            max_score = score;
                            best_intent = proto_intent.clone();
                        }
                    }
                    
                    if max_score > 0.82 { // High confidence threshold
                        info!("Semantic Router: Matched {:?} with score {:.2}", best_intent, max_score);
                        intent = best_intent;
                    }
                }
            }
        }

        // 2. Fallback to LLM Classifier if Semantic failed
        if matches!(intent, Intent::Unknown) {
             if let Ok(classified) = self.classifier.classify(input).await {
                 intent = classified;
             }
        }

        match intent {
            Intent::ChitChat => {
                // Inject system prompt for concise, friendly response
                context.insert(0, Message::new("system", "You are Crablet, a helpful AI assistant. The user is engaging in small talk. Be friendly, concise, and helpful. Do not use tools unless explicitly asked."));
            },
            Intent::Research => {
                // System 3 should handle this, but if we are here (System 2 pipeline), 
                // we can inject a prompt to encourage thoroughness or delegate if we had dynamic routing.
                // For now, System 2 can also do research via tools.
                context.insert(0, Message::new("system", "You are Crablet. The user wants deep research. Use available search tools extensively to provide a comprehensive answer."));
            },
            Intent::Coding => {
                context.insert(0, Message::new("system", "You are Crablet. The user is asking for code. Provide clean, well-commented code blocks. Use the file tool if you need to read existing files."));
            },
            Intent::Reasoning => {
                // Default behavior
                context.insert(0, Message::new("system", "You are Crablet. Use your reasoning capabilities to solve the user's problem step-by-step."));
            },
            Intent::Unknown => {
                context.insert(0, Message::new("system", "You are Crablet, an advanced autonomous AI agent."));
            },
        }
        
        Ok(None)
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 { return 0.0; }
    dot_product / (norm_a * norm_b)
}
