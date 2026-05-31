#[cfg(feature = "knowledge")]
use crate::cognitive::middleware::semantic_cache::{QA_CACHE_TYPE, SEMANTIC_CACHE_SOURCE};
#[cfg(feature = "knowledge")]
use crate::knowledge::vector_store::VectorStore;
#[cfg(feature = "knowledge")]
use std::sync::Arc;
#[cfg(feature = "knowledge")]
use tracing::warn;

#[cfg(feature = "knowledge")]
pub async fn update_semantic_cache(
    input: &str,
    response: &str,
    vector_store: &Option<Arc<VectorStore>>,
) {
    if let Some(ref vs) = vector_store {
        let metadata = serde_json::json!({
            "response": response,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "source": SEMANTIC_CACHE_SOURCE,
            "type": QA_CACHE_TYPE
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

#[cfg(all(test, feature = "knowledge"))]
mod tests {
    use super::*;
    use crate::knowledge::vector_store::VectorStore;
    use std::sync::Arc;

    #[tokio::test]
    async fn update_semantic_cache_writes_qa_cache_metadata() {
        let store = Some(Arc::new(VectorStore::new_in_memory()));

        update_semantic_cache("What is Crablet?", "A cognitive agent OS.", &store).await;

        let results = store
            .as_ref()
            .expect("vector store should exist")
            .search("What is Crablet?", 1)
            .await
            .expect("semantic cache entry should be searchable");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].2["source"], SEMANTIC_CACHE_SOURCE);
        assert_eq!(results[0].2["type"], QA_CACHE_TYPE);
        assert_eq!(results[0].2["response"], "A cognitive agent OS.");
    }
}
