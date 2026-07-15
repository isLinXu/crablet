use crate::config::Config;
use crate::error::Result;
use crate::types::{Message, TraceStep};
use async_trait::async_trait;
use std::sync::Arc;

pub mod audit_log;
pub mod classifier;
pub mod context_handler;
pub mod intent_classifier;
pub mod lane;
pub mod llm;
pub mod logical_expression;
pub mod mcts_tot;
pub mod meta_router;
pub mod middleware;
pub mod multimodal;
pub mod pattern_matcher;
pub mod planner;
pub mod react;
pub mod react_observable;
pub mod router;
pub mod routing;
pub mod streaming_pipeline;
pub mod system1;
pub mod system1_dynamic;
pub mod system1_enhanced;
pub mod system2;
pub mod system3;
pub mod system4;
pub mod thought_graph;
pub mod tot;

// Fusion Memory System integration
pub mod fusion_router;

// Meta-Cognitive System
pub mod meta_controller;

// Re-export fusion router types
pub use fusion_router::{FusionRouter, FusionRoutingContext, RouterConfig, SessionFusionRouter};

// Re-export meta-cognitive types
pub use meta_controller::{
    ExecutionRequest, ExecutionResult, MetaCognitiveController, MetaConfig, MetaStatistics,
};

// Re-export System 1 dynamic types
pub use system1_dynamic::{CommandMatch, ContextSnapshot, DynamicCommandRule, System1Dynamic};

#[async_trait]
pub trait CognitiveSystem: Send + Sync {
    /// Process the input and return a response with traces
    async fn process(&self, input: &str, context: &[Message]) -> Result<(String, Vec<TraceStep>)>;

    /// The name of this system (for logging/debugging)
    fn name(&self) -> &str;
}

pub async fn create_llm_client(config: &Config) -> Result<Arc<dyn llm::LlmClient>> {
    let vendor = config
        .llm_vendor
        .as_deref()
        .unwrap_or("openai")
        .to_lowercase();

    let client: Arc<dyn llm::LlmClient> = match vendor.as_str() {
        "mock" => Arc::new(llm::MockClient),
        "kimi" | "moonshot" => {
            Arc::new(llm::KimiClient::new(&config.model_name)?) as Arc<dyn llm::LlmClient>
        }
        "zhipu" | "glm" => {
            Arc::new(llm::ZhipuClient::new(&config.model_name)?) as Arc<dyn llm::LlmClient>
        }
        "ollama" => {
            Arc::new(llm::OllamaClient::new(&config.ollama_model)) as Arc<dyn llm::LlmClient>
        }
        "aliyun" | "dashscope" => {
            Arc::new(llm::OpenAiClient::new(&config.model_name)?) as Arc<dyn llm::LlmClient>
        }
        _ => {
            if config.model_name.contains("mock") {
                Arc::new(llm::MockClient) as Arc<dyn llm::LlmClient>
            } else if config.model_name.contains("kimi") {
                Arc::new(llm::KimiClient::new(&config.model_name)?) as Arc<dyn llm::LlmClient>
            } else if config.model_name.contains("glm") {
                Arc::new(llm::ZhipuClient::new(&config.model_name)?) as Arc<dyn llm::LlmClient>
            } else if config.model_name.starts_with("ollama:") {
                let model = config.model_name.trim_start_matches("ollama:");
                Arc::new(llm::OllamaClient::new(model)) as Arc<dyn llm::LlmClient>
            } else {
                Arc::new(llm::OpenAiClient::new(&config.model_name)?) as Arc<dyn llm::LlmClient>
            }
        }
    };

    // Wrap network-backed clients with exponential-backoff retry so transient
    // API/network failures are recovered automatically. MockClient is left bare
    // to keep test behavior deterministic.
    let resilient: Arc<dyn llm::LlmClient> =
        if vendor == "mock" || config.model_name.contains("mock") {
            client
        } else {
            Arc::new(llm::RetryLlmClient::new(client)) as Arc<dyn llm::LlmClient>
        };

    // Wrap in cache (cache sits on top, so cache hits skip retries entirely)
    let cached: Arc<dyn llm::LlmClient> =
        Arc::new(llm::cache::CachedLlmClient::new(resilient, 100)) as Arc<dyn llm::LlmClient>;
    Ok(cached)
}
