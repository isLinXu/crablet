use super::{CognitiveMiddleware, MiddlewareState};
use crate::types::{Message, TraceStep};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
#[cfg(feature = "knowledge")]
use tracing::info;

pub(crate) const SEMANTIC_CACHE_SOURCE: &str = "semantic_cache";
pub(crate) const QA_CACHE_TYPE: &str = "qa_cache";

pub(crate) fn is_semantic_cache_metadata(metadata: &Value) -> bool {
    let source_matches = metadata
        .get("source")
        .and_then(Value::as_str)
        .map(|source| source == SEMANTIC_CACHE_SOURCE)
        .unwrap_or(false);
    let cache_type = metadata.get("type").and_then(Value::as_str);

    source_matches && matches!(cache_type, Some(QA_CACHE_TYPE) | Some("qa_pair"))
}

fn cached_response_from_metadata(metadata: &Value, score: f32, threshold: f32) -> Option<String> {
    if score < threshold || !is_semantic_cache_metadata(metadata) {
        return None;
    }

    metadata
        .get("response")
        .and_then(Value::as_str)
        .filter(|response| !response.trim().is_empty())
        .map(str::to_string)
}

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
        _input: &str,
        _context: &mut Vec<Message>,
        _state: &MiddlewareState,
    ) -> Result<Option<(String, Vec<TraceStep>)>> {
        #[cfg(feature = "knowledge")]
        if let Some(vs) = &_state.vector_store {
            // Optimization: Search for top 3 results to increase hit chance if first is just below threshold
            // but semantically identical
            let search_result: Result<Vec<(String, f32, serde_json::Value)>> =
                vs.search(_input, 3).await;
            if let Ok(results) = search_result {
                for (_content, score, metadata) in results {
                    if let Some(response) =
                        cached_response_from_metadata(&metadata, score, self.threshold)
                    {
                        info!("Semantic Cache: High confidence match ({:.2})", score);
                        return Ok(Some((response, vec![TraceStep::cache_hit(score)])));
                    }
                }
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cache_metadata_accepts_current_type() {
        let metadata = json!({
            "source": SEMANTIC_CACHE_SOURCE,
            "type": QA_CACHE_TYPE,
            "response": "cached answer"
        });

        assert_eq!(
            cached_response_from_metadata(&metadata, 0.95, 0.92).as_deref(),
            Some("cached answer")
        );
    }

    #[test]
    fn cache_metadata_accepts_legacy_type() {
        let metadata = json!({
            "source": SEMANTIC_CACHE_SOURCE,
            "type": "qa_pair",
            "response": "legacy cached answer"
        });

        assert_eq!(
            cached_response_from_metadata(&metadata, 0.95, 0.92).as_deref(),
            Some("legacy cached answer")
        );
    }

    #[test]
    fn cache_metadata_rejects_normal_documents() {
        let metadata = json!({
            "source": "docs/readme.md",
            "type": QA_CACHE_TYPE,
            "response": "not a cache entry"
        });

        assert!(cached_response_from_metadata(&metadata, 0.99, 0.92).is_none());
    }

    #[test]
    fn cache_metadata_respects_threshold() {
        let metadata = json!({
            "source": SEMANTIC_CACHE_SOURCE,
            "type": QA_CACHE_TYPE,
            "response": "cached answer"
        });

        assert!(cached_response_from_metadata(&metadata, 0.91, 0.92).is_none());
    }
}
