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
#[cfg(feature = "knowledge")]
use crate::knowledge::graph_rag::EntityExtractorMode;

pub mod safety;
pub mod cost_guard;
pub mod semantic_cache;
pub mod planning;
pub mod rag;
pub mod skill_context;
pub mod routing;

pub use safety::SafetyMiddleware;
pub use cost_guard::CostGuardMiddleware;
pub use semantic_cache::SemanticCacheMiddleware;
pub use planning::PlanningMiddleware;
pub use rag::RagMiddleware;
pub use skill_context::SkillContextMiddleware;
pub use routing::RoutingMiddleware;

#[derive(Clone, Debug)]
pub struct RagTraceItem {
    pub source: String,
    pub score: f32,
    pub content: String,
}

#[derive(Clone, Debug)]
pub struct RagTracePayload {
    pub retrieval: String,
    pub refs: Vec<RagTraceItem>,
    pub graph_entities: Vec<String>,
}

pub struct MiddlewareState {
    pub llm: Arc<Box<dyn LlmClient>>,
    pub skills: Arc<RwLock<SkillRegistry>>,
    pub event_bus: Arc<EventBus>,
    pub kg: Option<SharedKnowledgeGraph>,
    #[cfg(feature = "knowledge")]
    pub vector_store: Option<Arc<VectorStore>>,
    pub planner: Arc<TaskPlanner>,
    pub skill_manager: Arc<SkillManagerTool>,
    #[cfg(feature = "knowledge")]
    pub graph_rag_entity_mode: EntityExtractorMode,
    pub rag_trace: Arc<RwLock<Option<RagTracePayload>>>,
}

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

impl Default for MiddlewarePipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl MiddlewarePipeline {
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
        }
    }

    pub fn with_middleware<M: CognitiveMiddleware + 'static>(mut self, middleware: M) -> Self {
        self.middlewares.push(Box::new(middleware));
        self
    }

    pub(crate) fn ensure_system_prompt(context: &mut Vec<Message>, content: &str) {
        if context.is_empty() {
            context.push(Message::new("system", content));
        } else if context[0].role == "system" {
            // Append to existing system prompt to avoid multiple system messages
            if let Some(text) = context[0].text() {
                let new_text = format!("{}\n\n{}", text, content);
                context[0] = Message::new("system", &new_text);
            }
        } else {
            // Insert at 0 if first message is not system
            context.insert(0, Message::new("system", content));
        }
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
