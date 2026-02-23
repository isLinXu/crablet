use async_trait::async_trait;
use anyhow::Result;
use crate::types::{TraceStep, Message};

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

#[async_trait]
pub trait CognitiveSystem: Send + Sync {
    /// Process the input and return a response with traces
    async fn process(&self, input: &str, context: &[Message]) -> Result<(String, Vec<TraceStep>)>;
    
    /// The name of this system (for logging/debugging)
    fn name(&self) -> &str;
}
