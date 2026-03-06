use anyhow::Result;
use crate::types::{Message, TraceStep};
use super::{CognitiveMiddleware, MiddlewareState};
#[cfg(feature = "knowledge")]
use tracing::info;
use async_trait::async_trait;

pub struct SemanticCacheMiddleware {
    threshold: f32,
}

impl SemanticCacheMiddleware {
    pub fn new(threshold: f32) -> Self {
        Self { threshold }
    }
}

#[async_trait]
impl CognitiveMiddleware for SemanticCacheMiddleware {
    fn name(&self) -> &str {
        "Semantic Cache"
    }

    async fn execute(
        &self,
        input: &str,
        _context: &mut Vec<Message>,
        state: &MiddlewareState,
    ) -> Result<Option<(String, Vec<TraceStep>)>> {
        #[cfg(feature = "knowledge")]
        if let Some(vs) = &state.vector_store {
            // Optimization: Search for top 3 results to increase hit chance if first is just below threshold
            // but semantically identical
            let search_result: Result<Vec<(String, f32, serde_json::Value)>> = vs.search(input, 3).await;
            if let Ok(results) = search_result {
                for (_content, score, metadata) in results {
                    // Check if metadata indicates it's a cached Q&A pair
                    let is_qa_cache = metadata.get("type")
                        .and_then(|v: &serde_json::Value| v.as_str())
                        .map(|s: &str| s == "qa_cache")
                        .unwrap_or(false);

                    if is_qa_cache && score > self.threshold {
                        let cached_response = metadata.get("response")
                            .and_then(|v: &serde_json::Value| v.as_str())
                            .map(|s: &str| s.to_string());
                            
                        if let Some(response) = cached_response {
                            info!("Semantic Cache: High confidence match ({:.2})", score);
                            return Ok(Some((
                                response, 
                                vec![TraceStep::cache_hit(score)]
                            )));
                        }
                    }
                }
            }
        }
        Ok(None)
    }
}
