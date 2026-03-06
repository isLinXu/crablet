#[cfg(feature = "knowledge")]
use crate::knowledge::vector_store::VectorStore;
#[cfg(feature = "knowledge")]
use std::sync::Arc;
#[cfg(feature = "knowledge")]
use tracing::warn;

#[cfg(feature = "knowledge")]
pub async fn update_semantic_cache(input: &str, response: &str, vector_store: &Option<Arc<VectorStore>>) {
    if let Some(ref vs) = vector_store {
        let metadata = serde_json::json!({
            "response": response,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "source": "semantic_cache",
            "type": "qa_pair"
        });
        // We await here, but ideally we should spawn this if we don't want to block response
        // For now, simple await is fine as embedding is offloaded
        if let Err(e) = vs.add_document(input, Some(metadata)).await {
            warn!("Failed to update semantic cache: {}", e);
        }
    }
}

#[cfg(not(feature = "knowledge"))]
pub async fn update_semantic_cache(_input: &str, _response: &str, _vector_store: &Option<()>) {
    // No-op without knowledge feature
}
