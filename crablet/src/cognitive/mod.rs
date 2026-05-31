use crate::config::Config;
use crate::error::Result;
use crate::types::{Message, TraceStep};
use async_trait::async_trait;
use std::sync::Arc;

pub mod classifier;
pub mod intent_classifier;
pub mod lane;
pub mod llm;
pub mod mcts_tot;
pub mod meta_router;
pub mod middleware;
pub mod multimodal;
pub mod planner;
pub mod react;
pub mod react_observable;
pub mod router;
pub mod routing;
#[deprecated(
    since = "0.6.0",
    note = "SpeculativeRouter is unused and will be removed in a future version. Use FusionRouter instead."
)]
pub mod speculative_router;
pub mod streaming_pipeline;
pub mod system1;
pub mod system1_enhanced;
pub mod system2;
pub mod system3;
pub mod system4;
pub mod thought_graph;
pub mod tot;
#[deprecated(
    since = "0.6.0",
    note = "UnifiedRouter is unused and will be removed in a future version. Use FusionRouter instead."
)]
pub mod unified_router;

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

#[async_trait]
pub trait CognitiveSystem: Send + Sync {
    /// Process the input and return a response with traces
    async fn process(&self, input: &str, context: &[Message]) -> Result<(String, Vec<TraceStep>)>;

    /// The name of this system (for logging/debugging)
    fn name(&self) -> &str;
}

pub async fn create_llm_client(config: &Config) -> Result<Arc<Box<dyn llm::LlmClient>>> {
    let vendor = config
        .llm_vendor
        .as_deref()
        .unwrap_or("openai")
        .to_lowercase();

    let client: Box<dyn llm::LlmClient> = match vendor.as_str() {
        "mock" => Box::new(llm::MockClient),
        "kimi" | "moonshot" => Box::new(llm::KimiClient::new(&config.model_name)?),
        "zhipu" | "glm" => Box::new(llm::ZhipuClient::new(&config.model_name)?),
        "ollama" => Box::new(llm::OllamaClient::new(&config.ollama_model)),
        "aliyun" | "dashscope" => Box::new(llm::OpenAiClient::new(&config.model_name)?),
        _ => {
            if config.model_name.contains("mock") {
                Box::new(llm::MockClient)
            } else if config.model_name.contains("kimi") {
                Box::new(llm::KimiClient::new(&config.model_name)?)
            } else if config.model_name.contains("glm") {
                Box::new(llm::ZhipuClient::new(&config.model_name)?)
            } else if config.model_name.starts_with("ollama:") {
                let model = config.model_name.trim_start_matches("ollama:");
                Box::new(llm::OllamaClient::new(model))
            } else {
                Box::new(llm::OpenAiClient::new(&config.model_name)?)
            }
        }
    };

    // Wrap network-backed clients with exponential-backoff retry so transient
    // API/network failures are recovered automatically. MockClient is left bare
    // to keep test behavior deterministic.
    let resilient: Box<dyn llm::LlmClient> = if vendor == "mock" || config.model_name.contains("mock")
    {
        client
    } else {
        Box::new(llm::RetryLlmClient::new(client))
    };

    // Wrap in cache (cache sits on top, so cache hits skip retries entirely)
    let cached: Box<dyn llm::LlmClient> =
        Box::new(llm::cache::CachedLlmClient::new(resilient, 100));
    Ok(Arc::new(cached))
}
