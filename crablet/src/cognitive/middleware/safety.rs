use async_trait::async_trait;
use anyhow::Result;
use crate::types::{Message, TraceStep};
use super::{CognitiveMiddleware, MiddlewareState};
use tracing::warn;
use crate::constants::limits;

pub struct SafetyMiddleware;

#[async_trait]
impl CognitiveMiddleware for SafetyMiddleware {
    fn name(&self) -> &str {
        "Safety Check"
    }

    async fn execute(&self, input: &str, _context: &mut Vec<Message>, _state: &MiddlewareState) -> Result<Option<(String, Vec<TraceStep>)>> {
        // Basic Input Safety Checks
        if input.len() > limits::MAX_INPUT_SIZE {
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
