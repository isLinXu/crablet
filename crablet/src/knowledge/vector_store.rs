use anyhow::Result;
use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};
use sqlx::{sqlite::SqlitePool, Row};
use std::sync::{Arc, Mutex};
use serde_json::json;
use crate::knowledge::chunking::{Chunker, RecursiveCharacterChunker, MarkdownChunker};
use std::sync::atomic::{AtomicUsize, Ordering};
use futures::StreamExt; // For stream iteration

#[cfg(feature = "qdrant-support")]
use qdrant_client::qdrant::{PointStruct, Distance, CreateCollectionBuilder, VectorParamsBuilder, UpsertPointsBuilder, SearchPointsBuilder, DeletePointsBuilder, Filter, Condition, Value};
#[cfg(feature = "qdrant-support")]
use qdrant_client::Qdrant;

struct EmbedderPool {
    pool: Vec<Arc<Mutex<TextEmbedding>>>,
    next: AtomicUsize,
    mock_mode: bool,
}

impl EmbedderPool {
    fn new(size: usize, options: InitOptions) -> Result<Self> {
        let mut pool = Vec::with_capacity(size);
        for _ in 0..size {
             let opts = options.clone();
             let embedder = TextEmbedding::try_new(opts)?;
             pool.push(Arc::new(Mutex::new(embedder)));
        }
        Ok(Self { pool, next: AtomicUsize::new(0), mock_mode: false })
    }
    
    fn new_mock() -> Self {
        Self { pool: Vec::new(), next: AtomicUsize::new(0), mock_mode: true }
    }

    async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        if self.mock_mode {
            // Return dummy vectors of size 384 (all-MiniLM-L6-v2 size)
            return Ok(texts.iter().map(|_| vec![0.1; 384]).collect());
        }

        let idx = self.next.fetch_add(1, Ordering::Relaxed) % self.pool.len();
        let embedder = self.pool[idx].clone();
        
        tokio::task::spawn_blocking(move || {
            let mut embedder = embedder.lock().map_err(|e| anyhow::anyhow!("Embedder lock poisoned: {}", e))?;
            embedder.embed(texts, None)
        }).await?
    }
}

enum StoreBackend {
    Sqlite {
        pool: SqlitePool,
    },
    #[cfg(feature = "qdrant-support")]
    Qdrant {
        client: Arc<Qdrant>,
        collection: String,
        pool: SqlitePool, // Add pool for metadata
    },
    InMemory {
        docs: Arc<Mutex<Vec<(String, Vec<f32>, serde_json::Value)>>>,
    }
}

pub struct VectorStore {
    backend: StoreBackend,
    embedder: Arc<EmbedderPool>,
}

impl VectorStore {
    pub async fn new(pool: SqlitePool) -> Result<Self> {
        // Run migrations for metadata tables (document_embeddings)
        sqlx::migrate!("./migrations").run(&pool).await?;

        // Initialize fastembed with a lightweight model
        let mut options = InitOptions::default();
        options.model_name = EmbeddingModel::AllMiniLML6V2;
        options.show_download_progress = true;
        
        // Move heavy initialization to blocking task
        let embedder_pool = tokio::task::spawn_blocking(move || {
            // Create a pool of 2 embedders for concurrency
            EmbedderPool::new(2, options)
        }).await??;

        // Check for Qdrant config
        #[cfg(feature = "qdrant-support")]
        if let Ok(url) = std::env::var("QDRANT_URL") {
            let client = Qdrant::from_url(&url).build()?;
            let collection = "crablet_knowledge".to_string();
            
            if !client.collection_exists(&collection).await? {
                client.create_collection(
                    CreateCollectionBuilder::new(collection.clone())
                        .vectors_config(VectorParamsBuilder::new(384, Distance::Cosine))
                ).await?;
            }
            
            return Ok(Self {
                backend: StoreBackend::Qdrant { 
                    client: Arc::new(client),
                    collection,
                    pool: pool.clone(),
                },
                embedder: Arc::new(embedder_pool),
            });
        }

        Ok(Self { 
            backend: StoreBackend::Sqlite { pool },
            embedder: Arc::new(embedder_pool),
        })
    }
    
    pub fn new_in_memory() -> Self {
        Self {
            backend: StoreBackend::InMemory { docs: Arc::new(Mutex::new(Vec::new())) },
            embedder: Arc::new(EmbedderPool::new_mock()),
        }
    }

    pub async fn add_documents(&self, contents: Vec<String>, payloads: Vec<serde_json::Value>) -> Result<()> {
        if contents.is_empty() { return Ok(()); }
        
        let embeddings = self.embedder.embed(contents.clone()).await?;
        
        match &self.backend {
            StoreBackend::Sqlite { pool } => {
                let mut tx = pool.begin().await?;
                for (i, _content) in contents.iter().enumerate() {
                    let embedding = &embeddings[i];
                    let embedding_json = serde_json::to_string(embedding)?;
                    let id = uuid::Uuid::new_v4().to_string();
                    let metadata = &payloads[i];
                    
                    sqlx::query("INSERT INTO document_embeddings (id, content, embedding, metadata) VALUES (?, ?, ?, ?)")
                        .bind(id)
                        .bind(&contents[i])
                        .bind(embedding_json)
                        .bind(metadata)
                        .execute(&mut *tx)
                        .await?;
                }
                tx.commit().await?;
            },
            #[cfg(feature = "qdrant-support")]
            StoreBackend::Qdrant { client, collection, pool } => {
                let mut points = Vec::new();
                
                // ALSO insert into SQLite for metadata tracking
                let mut tx = pool.begin().await?;
                
                for (i, _content) in contents.iter().enumerate() {
                    let embedding = &embeddings[i];
                    let id = uuid::Uuid::new_v4().to_string();
                    let metadata = &payloads[i];
                    
                    // 1. Qdrant Payload
                    let mut payload = std::collections::HashMap::new();
                    payload.insert("content".to_string(), json!(&contents[i]));
                    
                    if let Some(obj) = metadata.as_object() {
                        for (k, v) in obj {
                            payload.insert(k.clone(), v.clone());
                        }
                    }

                    points.push(PointStruct::new(
                        id.clone(),
                        embedding.clone(),
                        payload.into_iter().map(|(k, v)| (k, v.into())).collect::<std::collections::HashMap<String, Value>>()
                    ));
                    
                    // 2. SQLite Metadata
                    let embedding_json = serde_json::to_string(embedding)?;
                     sqlx::query("INSERT INTO document_embeddings (id, content, embedding, metadata) VALUES (?, ?, ?, ?)")
                        .bind(id)
                        .bind(&contents[i])
                        .bind(embedding_json)
                        .bind(metadata)
                        .execute(&mut *tx)
                        .await?;
                }
                
                tx.commit().await?;
                client.upsert_points(UpsertPointsBuilder::new(collection, points)).await?;
            },
            StoreBackend::InMemory { docs } => {
                let mut guard = docs.lock().map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
                for (i, content) in contents.iter().enumerate() {
                    guard.push((content.clone(), embeddings[i].clone(), payloads[i].clone()));
                }
            }
        }
        Ok(())
    }

    pub async fn add_document(&self, content: &str, metadata: Option<serde_json::Value>) -> Result<()> {
        // Auto-detect type
        let is_markdown = if let Some(meta) = &metadata {
            meta.get("file_type").and_then(|v| v.as_str()).map(|s| s == "markdown").unwrap_or(false)
        } else {
            false
        };
        
        if is_markdown {
            let chunker = MarkdownChunker::new(1000);
            self.add_document_with_chunker(content, metadata, &chunker).await
        } else {
            let chunker = RecursiveCharacterChunker::new(500, 50);
            self.add_document_with_chunker(content, metadata, &chunker).await
        }
    }

    pub async fn add_document_with_chunker(&self, content: &str, metadata: Option<serde_json::Value>, chunker: &dyn Chunker) -> Result<()> {
        let chunks_obj = chunker.chunk(content)?;
        let chunks: Vec<String> = chunks_obj.iter().map(|c| c.content.clone()).collect();
        
        let embeddings = self.embedder.embed(chunks.clone()).await?;
        
        match &self.backend {
            StoreBackend::Sqlite { pool } => {
                // Batch insert to DB
                let mut tx = pool.begin().await?;
                
                for (i, chunk) in chunks.iter().enumerate() {
                    let embedding = &embeddings[i];
                    let embedding_json = serde_json::to_string(embedding)?;
                    
                    let id = uuid::Uuid::new_v4().to_string();
                    let mut meta = metadata.clone().unwrap_or(json!({}));
                    
                    if let Some(obj) = meta.as_object_mut() {
                        obj.insert("chunk_index".to_string(), json!(i));
                        obj.insert("total_chunks".to_string(), json!(chunks.len()));
                        // Add chunk specific metadata
                        for (k, v) in &chunks_obj[i].metadata {
                            obj.insert(k.clone(), json!(v));
                        }
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
            },
            #[cfg(feature = "qdrant-support")]
            StoreBackend::Qdrant { client, collection, pool } => {
                let mut points = Vec::new();
                
                // ALSO insert into SQLite for metadata tracking
                let mut tx = pool.begin().await?;
                
                for (i, chunk) in chunks.iter().enumerate() {
                    let embedding = &embeddings[i];
                    let id = uuid::Uuid::new_v4().to_string();
                    
                    // 1. Qdrant Payload
                    let mut payload = std::collections::HashMap::new();
                    payload.insert("content".to_string(), json!(chunk));
                    
                    if let Some(meta) = &metadata {
                        if let Some(obj) = meta.as_object() {
                            for (k, v) in obj {
                                payload.insert(k.clone(), v.clone());
                            }
                        }
                    }
                    // Chunk metadata
                    payload.insert("chunk_index".to_string(), json!(i));
                    payload.insert("total_chunks".to_string(), json!(chunks.len()));
                    // Add chunk specific metadata
                    for (k, v) in &chunks_obj[i].metadata {
                        payload.insert(k.clone(), json!(v));
                    }

                    points.push(PointStruct::new(
                        id.clone(),
                        embedding.clone(),
                        payload.into_iter().map(|(k, v)| (k, v.into())).collect::<std::collections::HashMap<String, Value>>()
                    ));
                    
                    // 2. SQLite Metadata
                    let embedding_json = serde_json::to_string(embedding)?;
                    let mut meta = metadata.clone().unwrap_or(json!({}));
                    
                    if let Some(obj) = meta.as_object_mut() {
                        obj.insert("chunk_index".to_string(), json!(i));
                        obj.insert("total_chunks".to_string(), json!(chunks.len()));
                        for (k, v) in &chunks_obj[i].metadata {
                            obj.insert(k.clone(), json!(v));
                        }
                    }

                    sqlx::query("INSERT INTO document_embeddings (id, content, embedding, metadata) VALUES (?, ?, ?, ?)")
                        .bind(id)
                        .bind(chunk)
                        .bind(embedding_json)
                        .bind(meta)
                        .execute(&mut *tx)
                        .await?;
                }
                
                tx.commit().await?;
                client.upsert_points(UpsertPointsBuilder::new(collection, points)).await?;
            },
            StoreBackend::InMemory { docs } => {
                 let mut guard = docs.lock().map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
                 for (i, chunk) in chunks.iter().enumerate() {
                    let mut meta = metadata.clone().unwrap_or(json!({}));
                    if let Some(obj) = meta.as_object_mut() {
                        obj.insert("chunk_index".to_string(), json!(i));
                        obj.insert("total_chunks".to_string(), json!(chunks.len()));
                        for (k, v) in &chunks_obj[i].metadata {
                            obj.insert(k.clone(), json!(v));
                        }
                    }
                    guard.push((chunk.clone(), embeddings[i].clone(), meta));
                 }
            }
        }

        Ok(())
    }

    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<(String, f32, serde_json::Value)>> {
        let query_vec = self.embed_query(query).await?;

        match &self.backend {
            StoreBackend::Sqlite { pool } => {
                let mut rows = sqlx::query("SELECT content, embedding, metadata FROM document_embeddings")
                    .fetch(pool);
                
                let mut results: Vec<(String, f32, serde_json::Value)> = Vec::with_capacity(limit * 2);
                
                while let Some(row) = rows.next().await {
                    let row = row?;
                    let embedding_json: String = row.get("embedding");
                    
                    if let Ok(embedding) = serde_json::from_str::<Vec<f32>>(&embedding_json) {
                        let similarity = cosine_similarity(&query_vec, &embedding);
                        
                        let content: String = row.get("content");
                        let metadata: serde_json::Value = row.get("metadata");
                        
                        results.push((content, similarity, metadata));
                        
                        if results.len() >= limit * 4 {
                             results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                             results.truncate(limit);
                        }
                    }
                }
                
                results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                results.truncate(limit);

                Ok(results)
            },
            #[cfg(feature = "qdrant-support")]
            StoreBackend::Qdrant { client, collection, .. } => {
                let search_result = client.search_points(
                    SearchPointsBuilder::new(collection.clone(), query_vec, limit as u64)
                        .with_payload(true)
                ).await?;

                let mut results = Vec::new();
                for point in search_result.result {
                    let score = point.score;
                    let payload = point.payload;
                    let content = payload.get("content")
                        .and_then(|v| match v.kind {
                            Some(qdrant_client::qdrant::value::Kind::StringValue(ref s)) => Some(s.as_str()),
                            _ => None
                        })
                        .unwrap_or("")
                        .to_string();
                    let metadata = json!(payload);
                    results.push((content, score, metadata));
                }
                Ok(results)
            },
            StoreBackend::InMemory { docs } => {
                let guard = docs.lock().map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
                let mut results: Vec<(String, f32, serde_json::Value)> = guard.iter()
                    .map(|(content, embedding, meta)| {
                        let similarity = cosine_similarity(&query_vec, embedding);
                        (content.clone(), similarity, meta.clone())
                    })
                    .collect();
                
                results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                results.truncate(limit);
                Ok(results)
            }
        }
    }
    
    pub async fn list_documents(&self) -> Result<Vec<serde_json::Value>> {
        match &self.backend {
            StoreBackend::Sqlite { pool } => {
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
            #[cfg(feature = "qdrant-support")]
            StoreBackend::Qdrant { pool, .. } => {
                // Same as Sqlite
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
            StoreBackend::InMemory { docs } => {
                 // In memory listing logic
                 // We need to group by source
                 let guard = docs.lock().map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
                 let mut map: std::collections::HashMap<String, (String, usize)> = std::collections::HashMap::new();
                 
                 for (_, _, meta) in guard.iter() {
                     if let Some(src) = meta.get("source").and_then(|v| v.as_str()) {
                         let ftype = meta.get("file_type").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                         let entry = map.entry(src.to_string()).or_insert((ftype, 0));
                         entry.1 += 1;
                     }
                 }
                 
                 let docs = map.into_iter().map(|(src, (ftype, count))| {
                     json!({
                         "source": src,
                         "file_type": ftype,
                         "chunks": count
                     })
                 }).collect();
                 Ok(docs)
            }
        }
    }

    pub async fn get_document_chunks(&self, source: &str) -> Result<Vec<serde_json::Value>> {
        match &self.backend {
            StoreBackend::Sqlite { pool } => {
                let rows = sqlx::query(
                    "SELECT content, metadata FROM document_embeddings WHERE json_extract(metadata, '$.source') = ?"
                )
                .bind(source)
                .fetch_all(pool)
                .await?;
                
                let mut chunks = Vec::new();
                for row in rows {
                    let content: String = row.get("content");
                    let metadata: serde_json::Value = row.get("metadata");
                    chunks.push(json!({
                        "content": content,
                        "metadata": metadata
                    }));
                }
                Ok(chunks)
            },
            #[cfg(feature = "qdrant-support")]
            StoreBackend::Qdrant { pool, .. } => {
                // Use metadata pool for retrieval as Qdrant might be slower for full scan
                let rows = sqlx::query(
                    "SELECT content, metadata FROM document_embeddings WHERE json_extract(metadata, '$.source') = ?"
                )
                .bind(source)
                .fetch_all(pool)
                .await?;
                
                let mut chunks = Vec::new();
                for row in rows {
                    let content: String = row.get("content");
                    let metadata: serde_json::Value = row.get("metadata");
                    chunks.push(json!({
                        "content": content,
                        "metadata": metadata
                    }));
                }
                Ok(chunks)
            },
            StoreBackend::InMemory { docs } => {
                let guard = docs.lock().map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
                let chunks = guard.iter()
                    .filter(|(_, _, meta)| meta.get("source").and_then(|v| v.as_str()) == Some(source))
                    .map(|(content, _, meta)| json!({
                        "content": content,
                        "metadata": meta
                    }))
                    .collect();
                Ok(chunks)
            }
        }
    }

    pub async fn delete_document(&self, source: &str) -> Result<()> {
        match &self.backend {
            StoreBackend::Sqlite { pool } => {
                sqlx::query("DELETE FROM document_embeddings WHERE json_extract(metadata, '$.source') = ?")
                    .bind(source)
                    .execute(pool)
                    .await?;
            },
            #[cfg(feature = "qdrant-support")]
            StoreBackend::Qdrant { client, collection, pool } => {
                sqlx::query("DELETE FROM document_embeddings WHERE json_extract(metadata, '$.source') = ?")
                    .bind(source)
                    .execute(pool)
                    .await?;
                
                client.delete_points(
                    DeletePointsBuilder::new(collection)
                        .points(Filter::all([Condition::matches("source", source.to_string())]))
                ).await?;
            },
            StoreBackend::InMemory { docs } => {
                let mut guard = docs.lock().map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
                guard.retain(|(_, _, meta)| {
                    meta.get("source").and_then(|v| v.as_str()) != Some(source)
                });
            }
        }

        Ok(())
    }

    pub async fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        let embeddings = self.embedder.embed(vec![query.to_string()]).await?;
        Ok(embeddings[0].clone())
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
