use anyhow::Result;
use std::sync::Arc;
use crate::knowledge::vector_store::VectorStore;
use crate::knowledge::chunking::{Chunker, RecursiveCharacterChunker};
use crate::knowledge::multimodal::process_file;
use std::path::Path;
use tracing::info;
use uuid::Uuid;

pub struct IngestionService {
    vector_store: Arc<VectorStore>,
    chunker: Box<dyn Chunker>,
}

impl IngestionService {
    pub fn new(vector_store: Arc<VectorStore>) -> Self {
        Self {
            vector_store,
            chunker: Box::new(RecursiveCharacterChunker::new(1000, 200)),
        }
    }

    pub async fn ingest_text(&self, text: &str, metadata: serde_json::Value) -> Result<String> {
        let doc_id = Uuid::new_v4().to_string();
        info!("Ingesting document {} (length: {})", doc_id, text.len());

        // 1. Chunking
        let chunks = self.chunker.chunk(text)?;
        info!("Generated {} chunks", chunks.len());

        // 2. Prepare for Vector Store
        let mut ids = Vec::new();
        let mut payloads = Vec::new();
        let mut contents = Vec::new();

        for (i, chunk) in chunks.iter().enumerate() {
            let chunk_id = format!("{}_{}", doc_id, i);
            ids.push(chunk_id);
            contents.push(chunk.content.clone());
            
            let mut payload = metadata.as_object().cloned().unwrap_or_default();
            payload.insert("doc_id".to_string(), serde_json::Value::String(doc_id.clone()));
            payload.insert("chunk_index".to_string(), serde_json::Value::Number(serde_json::Number::from(i)));
            payload.insert("content".to_string(), serde_json::Value::String(chunk.content.clone()));
            
            // Merge chunk metadata
            for (k, v) in &chunk.metadata {
                payload.insert(k.clone(), serde_json::Value::String(v.clone()));
            }
            
            payloads.push(serde_json::Value::Object(payload));
        }

        // 3. Store in Vector DB
        if !contents.is_empty() {
            self.vector_store.add_documents(contents, payloads).await?;
        }

        Ok(doc_id)
    }

    pub async fn ingest_file(&self, path: &Path, metadata: serde_json::Value) -> Result<String> {
        let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        let processed = process_file(path, &extension).await?;
        let mut merged = metadata.as_object().cloned().unwrap_or_default();
        if let Some(extra) = processed.metadata.as_object() {
            for (k, v) in extra {
                merged.insert(k.clone(), v.clone());
            }
        }
        merged.insert("processed_at".to_string(), serde_json::Value::String(chrono::Utc::now().to_rfc3339()));
        self.ingest_text(&processed.text, serde_json::Value::Object(merged)).await
    }

    pub async fn list_documents(&self) -> Result<Vec<serde_json::Value>> {
        self.vector_store.list_documents().await
    }

    pub async fn get_document_chunks(&self, source: &str) -> Result<Vec<serde_json::Value>> {
        self.vector_store.get_document_chunks(source).await
    }

    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<serde_json::Value>> {
        let results = self.vector_store.search(query, limit).await?;
        Ok(results.into_iter().map(|(content, score, metadata)| {
            serde_json::json!({
                "content": content,
                "score": score,
                "metadata": metadata
            })
        }).collect())
    }
}
