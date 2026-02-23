use anyhow::Result;
use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};
use sqlx::{sqlite::SqlitePool, Row};
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;
use serde_json::json;
use crate::knowledge::chunking::{Chunker, RecursiveCharacterChunker};

// #[cfg(feature = "qdrant-support")]
// use qdrant_client::prelude::*;
// #[cfg(feature = "qdrant-support")]
// use qdrant_client::qdrant::{PointStruct, vectors_config, VectorParams, Distance, CreateCollection, VectorsConfig, SearchPoints};

#[derive(Clone)]
struct CachedDocument {
    content: String,
    embedding: Vec<f32>,
    metadata: serde_json::Value,
}

enum StoreBackend {
    Sqlite {
        pool: SqlitePool,
        cache: RwLock<Vec<CachedDocument>>,
    },
    #[cfg(feature = "qdrant-support")]
    // Qdrant {
    //     client: Arc<QdrantClient>,
    //     collection: String,
    // },
    #[allow(dead_code)]
    QdrantStub,
}

pub struct VectorStore {
    backend: StoreBackend,
    // Use Arc<Mutex> for Embedder to allow cloning and moving into spawn_blocking
    embedder: Arc<Mutex<TextEmbedding>>,
}

impl VectorStore {
    pub async fn new(pool: SqlitePool) -> Result<Self> {
        // Initialize fastembed with a lightweight model
        let mut options = InitOptions::default();
        options.model_name = EmbeddingModel::AllMiniLML6V2;
        options.show_download_progress = true;
        
        // Move heavy initialization to blocking task
        let embedder = tokio::task::spawn_blocking(move || {
            TextEmbedding::try_new(options)
        }).await??;

        // Check for Qdrant config
        #[cfg(feature = "qdrant-support")]
        if let Ok(_url) = std::env::var("QDRANT_URL") {
            // tracing::info!("Initializing Qdrant Vector Store at {}", url);
            // let api_key = std::env::var("QDRANT_API_KEY").ok();
            // let collection = std::env::var("QDRANT_COLLECTION").unwrap_or_else(|_| "crablet_docs".to_string());
            
            // let mut config = QdrantClientConfig::from_url(&url);
            // if let Some(key) = api_key {
            //     config.set_api_key(&key);
            // }
            // let client = QdrantClient::new(Some(config))?;
            
            // // Ensure collection exists
            // if !client.collection_exists(&collection).await? {
            //     client.create_collection(&CreateCollection {
            //         collection_name: collection.clone(),
            //         vectors_config: Some(VectorsConfig {
            //             config: Some(vectors_config::Config::Params(VectorParams {
            //                 size: 384, // AllMiniLML6V2 size
            //                 distance: Distance::Cosine.into(),
            //                 ..Default::default()
            //             })),
            //         }),
            //         ..Default::default()
            //     }).await?;
            // }

            // return Ok(Self {
            //     backend: StoreBackend::Qdrant {
            //         client: Arc::new(client),
            //         collection,
            //     },
            //     embedder: Arc::new(Mutex::new(embedder)),
            // });
            return Err(anyhow::anyhow!("Qdrant support temporarily disabled"));
        }

        // Fallback to Sqlite
        // Create vector table
        let schema = r#"
        CREATE TABLE IF NOT EXISTS document_embeddings (
            id TEXT PRIMARY KEY,
            content TEXT NOT NULL,
            embedding JSON NOT NULL,
            metadata JSON
        );
        "#;
        
        sqlx::query(schema).execute(&pool).await?;

        // Load existing embeddings into cache
        let rows = sqlx::query("SELECT content, embedding, metadata FROM document_embeddings")
            .fetch_all(&pool)
            .await?;
            
        let mut cache = Vec::new();
        for row in rows {
            let content: String = row.get("content");
            let embedding_json: String = row.get("embedding");
            let metadata: serde_json::Value = row.get("metadata");
            // Handle potential JSON errors gracefully
            if let Ok(embedding) = serde_json::from_str::<Vec<f32>>(&embedding_json) {
                cache.push(CachedDocument { content, embedding, metadata });
            }
        }

        Ok(Self { 
            backend: StoreBackend::Sqlite { pool, cache: RwLock::new(cache) },
            embedder: Arc::new(Mutex::new(embedder)),
        })
    }

    pub async fn add_document(&self, content: &str, metadata: Option<serde_json::Value>) -> Result<()> {
        let chunker = RecursiveCharacterChunker::new(500, 50);
        self.add_document_with_chunker(content, metadata, &chunker).await
    }

    pub async fn add_document_with_chunker(&self, content: &str, metadata: Option<serde_json::Value>, chunker: &dyn Chunker) -> Result<()> {
        let chunks_obj = chunker.chunk(content)?;
        let chunks: Vec<String> = chunks_obj.iter().map(|c| c.content.clone()).collect();
        
        // Offload embedding to blocking thread
        let embedder = self.embedder.clone();
        let chunks_clone = chunks.clone();
        
        let embeddings = tokio::task::spawn_blocking(move || {
            let mut embedder = embedder.lock().map_err(|e| anyhow::anyhow!("Embedder lock poisoned: {}", e))?;
            embedder.embed(chunks_clone, None)
        }).await??;
        
        match &self.backend {
            StoreBackend::Sqlite { pool, cache } => {
                // Batch insert to DB and Cache
                let mut tx = pool.begin().await?;
                
                for (i, chunk) in chunks.iter().enumerate() {
                    let embedding = &embeddings[i];
                    let embedding_json = serde_json::to_string(embedding)?;
                    
                    let id = uuid::Uuid::new_v4().to_string();
                    let mut meta = metadata.clone().unwrap_or(json!({}));
                    
                    if let Some(obj) = meta.as_object_mut() {
                        obj.insert("chunk_index".to_string(), json!(i));
                        obj.insert("total_chunks".to_string(), json!(chunks.len()));
                    }

                    sqlx::query("INSERT INTO document_embeddings (id, content, embedding, metadata) VALUES (?, ?, ?, ?)")
                        .bind(id)
                        .bind(chunk)
                        .bind(embedding_json)
                        .bind(meta.clone())
                        .execute(&mut *tx)
                        .await?;
                }
                
                tx.commit().await?;

                // Update Cache (Write Lock)
                {
                    let mut cache = cache.write().await;
                    for (i, chunk) in chunks.iter().enumerate() {
                        let mut meta = metadata.clone().unwrap_or(json!({}));
                        if let Some(obj) = meta.as_object_mut() {
                             obj.insert("chunk_index".to_string(), json!(i));
                             obj.insert("total_chunks".to_string(), json!(chunks.len()));
                        }
                        cache.push(CachedDocument {
                            content: chunk.clone(),
                            embedding: embeddings[i].clone(),
                            metadata: meta,
                        });
                    }
                }
            },
            // #[cfg(feature = "qdrant-support")]
            // StoreBackend::Qdrant { client, collection } => {
            //      let mut points = Vec::new();
            //      for (i, chunk) in chunks.iter().enumerate() {
            //         let embedding = &embeddings[i];
            //         let id = uuid::Uuid::new_v4().to_string();
            //         
            //         let mut meta = metadata.clone().unwrap_or(json!({}));
            //         if let Some(obj) = meta.as_object_mut() {
            //             obj.insert("chunk_index".to_string(), json!(i));
            //             obj.insert("total_chunks".to_string(), json!(chunks.len()));
            //             obj.insert("content".to_string(), json!(chunk)); // Store content in payload
            //         }
            //         
            //         // Convert serde Value to Payload
            //         let payload: Payload = meta.try_into()?;
            //
            //         let point = PointStruct::new(id, embedding.clone(), payload);
            //         points.push(point);
            //      }
            //      
            //      client.upsert_points_blocking(collection, None, points, None).await?;
            // }
            StoreBackend::QdrantStub => {}
        }

        Ok(())
    }

    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<(String, f32, serde_json::Value)>> {
        let query_vec = self.embed_query(query).await?;

        match &self.backend {
            StoreBackend::Sqlite { cache, .. } => {
                // 2. Search in Cache (Async Read Lock)
                let cache = cache.read().await;
                
                let mut results = Vec::new();
                for doc in cache.iter() {
                    let similarity = cosine_similarity(&query_vec, &doc.embedding);
                    results.push((doc.content.clone(), similarity, doc.metadata.clone()));
                }
                
                // 3. Sort (Top K)
                results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                results.truncate(limit);

                Ok(results)
            },
            // #[cfg(feature = "qdrant-support")]
            // StoreBackend::Qdrant { client, collection } => {
            //      let search_result = client.search_points(&SearchPoints {
            //          collection_name: collection.clone(),
            //          vector: query_vec,
            //          limit: limit as u64,
            //          with_payload: Some(true.into()),
            //          ..Default::default()
            //      }).await?;
            //      
            //      let mut results = Vec::new();
            //      for scored_point in search_result.result {
            //          let score = scored_point.score;
            //          let payload = scored_point.payload;
            //          // Convert payload back to serde Value
            //          let metadata = serde_json::to_value(payload)?;
            //          
            //          let content = metadata.get("content")
            //             .and_then(|v| v.as_str())
            //             .unwrap_or("")
            //             .to_string();
            //             
            //          results.push((content, score, metadata));
            //      }
            //      
            //      Ok(results)
            // }
            StoreBackend::QdrantStub => Ok(Vec::new())
        }
    }

    pub async fn list_documents(&self) -> Result<Vec<serde_json::Value>> {
        match &self.backend {
            StoreBackend::Sqlite { pool, .. } => {
                // Return distinct sources from metadata
                // We use SQL because cache structure doesn't index by source efficiently
                // SQLite JSON extract: json_extract(metadata, '$.source')
                let rows = sqlx::query(
                    "SELECT DISTINCT json_extract(metadata, '$.source') as source, 
                            json_extract(metadata, '$.file_type') as file_type,
                            COUNT(*) as chunk_count
                     FROM document_embeddings 
                     GROUP BY source"
                )
                .fetch_all(pool)
                .await?;
                
                let mut docs = Vec::new();
                for row in rows {
                    let source: Option<String> = row.try_get("source").ok();
                    let file_type: Option<String> = row.try_get("file_type").ok();
                    let chunk_count: i64 = row.get("chunk_count");
                    
                    if let Some(src) = source {
                        docs.push(json!({
                            "source": src,
                            "file_type": file_type.unwrap_or("unknown".to_string()),
                            "chunks": chunk_count
                        }));
                    }
                }
                Ok(docs)
            },
            StoreBackend::QdrantStub => Ok(Vec::new()),
        }
    }

    pub async fn delete_document(&self, source: &str) -> Result<()> {
        match &self.backend {
            StoreBackend::Sqlite { pool, cache } => {
                // Delete from DB
                sqlx::query("DELETE FROM document_embeddings WHERE json_extract(metadata, '$.source') = ?")
                    .bind(source)
                    .execute(pool)
                    .await?;
                    
                // Update Cache (Write Lock)
                // This is O(N) but acceptable for MVP cache size
                let mut cache = cache.write().await;
                cache.retain(|doc| {
                    doc.metadata.get("source").and_then(|v| v.as_str()) != Some(source)
                });
                
                Ok(())
            },
            StoreBackend::QdrantStub => Ok(()),
        }
    }

    pub async fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        let embedder = self.embedder.clone();
        let query_str = query.to_string();

        let query_embeddings = tokio::task::spawn_blocking(move || {
            let mut embedder = embedder.lock().map_err(|e| anyhow::anyhow!("Embedder lock poisoned: {}", e))?;
            embedder.embed(vec![query_str], None)
        }).await??;
        
        Ok(query_embeddings[0].clone())
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    
    dot_product / (norm_a * norm_b)
}
