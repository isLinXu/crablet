use async_trait::async_trait;
use crate::error::Result;
use crate::types::{TraceStep, Message};
use crate::config::Config;
use std::sync::Arc;

pub mod router;
pub mod system1;
pub mod system2;
pub mod system3;
pub mod llm;
pub mod multimodal;
pub mod planner;
pub mod react;
pub mod middleware;
pub mod classifier;
pub mod lane;
pub mod tot;
pub mod mcts_tot;
pub mod meta_router;
pub mod streaming_pipeline;

#[async_trait]
pub trait CognitiveSystem: Send + Sync {
    /// Process the input and return a response with traces
    async fn process(&self, input: &str, context: &[Message]) -> Result<(String, Vec<TraceStep>)>;
    
    /// The name of this system (for logging/debugging)
    fn name(&self) -> &str;
}

pub async fn create_llm_client(config: &Config) -> Result<Arc<Box<dyn llm::LlmClient>>> {
    let client: Box<dyn llm::LlmClient> = if config.model_name.contains("mock") {
         Box::new(llm::MockClient)
    } else if config.model_name.contains("kimi") {
         Box::new(llm::KimiClient::new(&config.model_name)?)
    } else if config.model_name.contains("glm") {
         Box::new(llm::ZhipuClient::new(&config.model_name)?)
    } else if config.model_name.starts_with("ollama:") || config.ollama_model != "qwen2.5:14b" { 
         // If model name starts with ollama: or ollama_model is set (default is set in config, so this condition is tricky)
         // Let's simplify: if model_name is "ollama" or starts with "ollama:", use Ollama
         let model = if config.model_name.starts_with("ollama:") {
             config.model_name.trim_start_matches("ollama:")
         } else {
             &config.ollama_model
         };
         Box::new(llm::OllamaClient::new(model))
    } else {
         Box::new(llm::OpenAiClient::new(&config.model_name)?)
    };

    Ok(Arc::new(client))
}
