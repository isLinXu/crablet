//! Multimodal Memory System
//!
//! Provides unified memory storage and retrieval for multiple content types:
//! - Text (conversations, documents)
//! - Images (screenshots, generated images)
//! - Audio (voice transcriptions)
//! - Files (code, PDFs, etc.)

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{info, debug};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Content type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContentType {
    Text,
    Image,
    Audio,
    File,
    Structured, // JSON, etc.
}

impl ContentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContentType::Text => "text",
            ContentType::Image => "image",
            ContentType::Audio => "audio",
            ContentType::File => "file",
            ContentType::Structured => "structured",
        }
    }
}

/// Unified memory content wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryContent {
    /// Unique identifier
    pub id: String,
    /// Content type
    pub content_type: ContentType,
    /// The actual content (stored as string for all types)
    pub data: String,
    /// Metadata specific to content type
    pub metadata: ContentMetadata,
    /// Timestamp when created
    pub created_at: DateTime<Utc>,
    /// Importance score (0.0 - 1.0)
    pub importance: f32,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Reference count (for garbage collection)
    ref_count: usize,
}

impl MemoryContent {
    /// Create new text content
    pub fn new_text(content: String, importance: f32) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            content_type: ContentType::Text,
            data: content,
            metadata: ContentMetadata::default(),
            created_at: Utc::now(),
            importance,
            tags: vec![],
            ref_count: 0,
        }
    }

    /// Create new image content (base64 encoded)
    pub fn new_image(data: String, importance: f32, width: u32, height: u32) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            content_type: ContentType::Image,
            data,
            metadata: ContentMetadata {
                width: Some(width),
                height: Some(height),
                mime_type: Some("image/png".to_string()),
                ..Default::default()
            },
            created_at: Utc::now(),
            importance,
            tags: vec![],
            ref_count: 0,
        }
    }

    /// Create new audio content (base64 encoded or text transcript)
    pub fn new_audio(data: String, importance: f32, duration_secs: Option<f32>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            content_type: ContentType::Audio,
            data,
            metadata: ContentMetadata {
                duration_secs,
                mime_type: Some("audio/webm".to_string()),
                ..Default::default()
            },
            created_at: Utc::now(),
            importance,
            tags: vec![],
            ref_count: 0,
        }
    }

    /// Create new file content
    pub fn new_file(data: String, importance: f32, filename: String, mime_type: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            content_type: ContentType::File,
            data,
            metadata: ContentMetadata {
                filename: Some(filename),
                mime_type: Some(mime_type),
                ..Default::default()
            },
            created_at: Utc::now(),
            importance,
            tags: vec![],
            ref_count: 0,
        }
    }

    /// Add a tag
    pub fn with_tag(mut self, tag: String) -> Self {
        self.tags.push(tag);
        self
    }

    /// Increment reference count
    pub fn inc_ref(&mut self) {
        self.ref_count += 1;
    }

    /// Decrement reference count
    pub fn dec_ref(&mut self) {
        self.ref_count = self.ref_count.saturating_sub(1);
    }

    /// Check if content can be garbage collected
    pub fn can_collect(&self) -> bool {
        self.ref_count == 0 && self.importance < 0.3
    }
}

/// Metadata for memory content
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContentMetadata {
    /// For images: width in pixels
    pub width: Option<u32>,
    /// For images: height in pixels
    pub height: Option<u32>,
    /// For audio: duration in seconds
    pub duration_secs: Option<f32>,
    /// MIME type
    pub mime_type: Option<String>,
    /// Original filename (for files)
    pub filename: Option<String>,
    /// Source URL or path (for images, files)
    pub source: Option<String>,
    /// Parent memory ID (for derived content)
    pub parent_id: Option<String>,
    /// Associated skill name
    pub skill_name: Option<String>,
}

/// Multimodal memory query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultimodalQuery {
    /// Text query for semantic search
    pub text_query: Option<String>,
    /// Filter by content type
    pub content_types: Vec<ContentType>,
    /// Filter by tags
    pub required_tags: Vec<String>,
    /// Minimum importance score
    pub min_importance: f32,
    /// Time range filter
    pub time_range: Option<TimeRange>,
    /// Maximum results to return
    pub limit: usize,
}

/// Time range filter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl Default for MultimodalQuery {
    fn default() -> Self {
        Self {
            text_query: None,
            content_types: vec![],
            required_tags: vec![],
            min_importance: 0.0,
            time_range: None,
            limit: 20,
        }
    }
}

/// Query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultimodalResult {
    pub content: MemoryContent,
    pub relevance_score: f32,
    pub matched_on: Vec<String>, // Which fields matched
}

/// Multimodal Memory Store
pub struct MultimodalMemoryStore {
    /// Text memories
    text_memories: Arc<RwLock<HashMap<String, MemoryContent>>>,
    /// Image memories
    image_memories: Arc<RwLock<HashMap<String, MemoryContent>>>,
    /// Audio memories
    audio_memories: Arc<RwLock<HashMap<String, MemoryContent>>>,
    /// File memories
    file_memories: Arc<RwLock<HashMap<String, MemoryContent>>>,
    /// Index by tags
    tag_index: Arc<RwLock<HashMap<String, Vec<String>>>>, // tag -> memory_ids
    /// Index by time (for efficient time-range queries)
    time_index: Arc<RwLock<Vec<(DateTime<Utc>, String)>>>, // (timestamp, memory_id)
    /// Configuration
    config: MultimodalConfig,
}

/// Configuration for multimodal memory
#[derive(Debug, Clone)]
pub struct MultimodalConfig {
    /// Maximum total memories to store
    pub max_memories: usize,
    /// Maximum text memory size in bytes
    pub max_text_size: usize,
    /// Maximum image memory size in bytes
    pub max_image_size: usize,
    /// Enable automatic garbage collection
    pub enable_gc: bool,
    /// GC threshold (importance below this is collectible)
    pub gc_importance_threshold: f32,
    /// Enable semantic search
    pub enable_semantic_search: bool,
}

impl Default for MultimodalConfig {
    fn default() -> Self {
        Self {
            max_memories: 10000,
            max_text_size: 100_000, // 100KB
            max_image_size: 5_000_000, // 5MB
            enable_gc: true,
            gc_importance_threshold: 0.3,
            enable_semantic_search: true,
        }
    }
}

impl MultimodalMemoryStore {
    /// Create a new multimodal memory store
    pub fn new(config: MultimodalConfig) -> Self {
        Self {
            text_memories: Arc::new(RwLock::new(HashMap::new())),
            image_memories: Arc::new(RwLock::new(HashMap::new())),
            audio_memories: Arc::new(RwLock::new(HashMap::new())),
            file_memories: Arc::new(RwLock::new(HashMap::new())),
            tag_index: Arc::new(RwLock::new(HashMap::new())),
            time_index: Arc::new(RwLock::new(Vec::new())),
            config,
        }
    }

    /// Store content
    pub async fn store(&self, content: MemoryContent) -> Result<String, MemoryError> {
        let id = content.id.clone();
        let content_type = content.content_type;
        let tags = content.tags.clone();
        let timestamp = content.created_at;

        // Store in appropriate bucket
        match content_type {
            ContentType::Text => {
                let mut memories = self.text_memories.write().await;
                memories.insert(id.clone(), content);
            }
            ContentType::Image => {
                let mut memories = self.image_memories.write().await;
                memories.insert(id.clone(), content);
            }
            ContentType::Audio => {
                let mut memories = self.audio_memories.write().await;
                memories.insert(id.clone(), content);
            }
            ContentType::File | ContentType::Structured => {
                let mut memories = self.file_memories.write().await;
                memories.insert(id.clone(), content);
            }
        }

        // Update tag index
        {
            let mut tag_index = self.tag_index.write().await;
            for tag in tags {
                tag_index.entry(tag).or_default().push(id.clone());
            }
        }

        // Update time index
        {
            let mut time_index = self.time_index.write().await;
            time_index.push((timestamp, id.clone()));
            time_index.sort_by(|a, b| a.0.cmp(&b.0));
        }

        debug!("Stored {} memory: {}", content_type.as_str(), id);
        Ok(id)
    }

    /// Retrieve content by ID
    pub async fn retrieve(&self, id: &str) -> Option<MemoryContent> {
        // Search all buckets
        if let Some(content) = self.text_memories.read().await.get(id) {
            return Some(content.clone());
        }
        if let Some(content) = self.image_memories.read().await.get(id) {
            return Some(content.clone());
        }
        if let Some(content) = self.audio_memories.read().await.get(id) {
            return Some(content.clone());
        }
        if let Some(content) = self.file_memories.read().await.get(id) {
            return Some(content.clone());
        }
        None
    }

    /// Query memories
    pub async fn query(&self, query: &MultimodalQuery) -> Vec<MultimodalResult> {
        let mut results = Vec::new();

        // Get matching IDs from tag index
        let tag_matches: Vec<String> = if !query.required_tags.is_empty() {
            let tag_index = self.tag_index.read().await;
            query.required_tags.iter()
                .filter_map(|tag| tag_index.get(tag).cloned())
                .flatten()
                .collect()
        } else {
            vec![]
        };

        // Filter by content type
        let buckets_to_search: Vec<(&str, Arc<RwLock<HashMap<String, MemoryContent>>>)> = {
            if query.content_types.is_empty() {
                vec![
                    ("text", self.text_memories.clone()),
                    ("image", self.image_memories.clone()),
                    ("audio", self.audio_memories.clone()),
                    ("file", self.file_memories.clone()),
                ]
            } else {
                query.content_types.iter().filter_map(|ct| {
                    match ct {
                        ContentType::Text => Some(("text", self.text_memories.clone())),
                        ContentType::Image => Some(("image", self.image_memories.clone())),
                        ContentType::Audio => Some(("audio", self.audio_memories.clone())),
                        ContentType::File | ContentType::Structured => Some(("file", self.file_memories.clone())),
                    }
                }).collect()
            }
        };

        // Search each bucket
        for (_, bucket) in buckets_to_search {
            let memories = bucket.read().await;
            for (id, content) in memories.iter() {
                // Skip if tag filtering is active and this doesn't match
                if !tag_matches.is_empty() && !tag_matches.contains(id) {
                    continue;
                }

                // Filter by importance
                if content.importance < query.min_importance {
                    continue;
                }

                // Filter by time range
                if let Some(ref range) = query.time_range {
                    if content.created_at < range.start || content.created_at > range.end {
                        continue;
                    }
                }

                // Calculate relevance score
                let mut relevance_score = 0.0;
                let mut matched_on = Vec::new();

                if let Some(ref text_query) = query.text_query {
                    if content.data.to_lowercase().contains(&text_query.to_lowercase()) {
                        relevance_score += 0.8;
                        matched_on.push("text_match".to_string());
                    }
                }

                // Boost by importance
                relevance_score += content.importance * 0.2;

                if relevance_score > 0.0 {
                    results.push(MultimodalResult {
                        content: content.clone(),
                        relevance_score,
                        matched_on,
                    });
                }
            }
        }

        // Sort by relevance
        results.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap_or(std::cmp::Ordering::Equal));

        // Limit results
        results.truncate(query.limit);
        results
    }

    /// Delete content
    pub async fn delete(&self, id: &str) -> bool {
        // Search all buckets
        let mut found = false;

        if let Some(mut content) = self.text_memories.write().await.remove(id) {
            content.dec_ref();
            found = true;
        }
        if let Some(mut content) = self.image_memories.write().await.remove(id) {
            content.dec_ref();
            found = true;
        }
        if let Some(mut content) = self.audio_memories.write().await.remove(id) {
            content.dec_ref();
            found = true;
        }
        if let Some(mut content) = self.file_memories.write().await.remove(id) {
            content.dec_ref();
            found = true;
        }

        // Remove from tag index
        {
            let mut tag_index = self.tag_index.write().await;
            for ids in tag_index.values_mut() {
                ids.retain(|i| i != id);
            }
        }

        // Remove from time index
        {
            let mut time_index = self.time_index.write().await;
            time_index.retain(|(_, i)| i != id);
        }

        if found {
            info!("Deleted memory: {}", id);
        }
        found
    }

    /// Get statistics
    pub async fn stats(&self) -> MultimodalStats {
        MultimodalStats {
            text_count: self.text_memories.read().await.len(),
            image_count: self.image_memories.read().await.len(),
            audio_count: self.audio_memories.read().await.len(),
            file_count: self.file_memories.read().await.len(),
            total_count: {
                let t = self.text_memories.read().await.len();
                let i = self.image_memories.read().await.len();
                let a = self.audio_memories.read().await.len();
                let f = self.file_memories.read().await.len();
                t + i + a + f
            },
            tag_count: self.tag_index.read().await.len(),
        }
    }

    /// Perform garbage collection
    pub async fn garbage_collect(&self) -> usize {
        if !self.config.enable_gc {
            return 0;
        }

        let mut removed = 0;
        let threshold = self.config.gc_importance_threshold;

        // Check text memories
        {
            let mut memories = self.text_memories.write().await;
            let to_remove: Vec<String> = memories.iter()
                .filter(|(_, c)| c.can_collect() && c.importance < threshold)
                .map(|(id, _)| id.clone())
                .collect();
            for id in to_remove {
                memories.remove(&id);
                removed += 1;
            }
        }

        // Check image memories
        {
            let mut memories = self.image_memories.write().await;
            let to_remove: Vec<String> = memories.iter()
                .filter(|(_, c)| c.can_collect() && c.importance < threshold)
                .map(|(id, _)| id.clone())
                .collect();
            for id in to_remove {
                memories.remove(&id);
                removed += 1;
            }
        }

        // Check audio memories
        {
            let mut memories = self.audio_memories.write().await;
            let to_remove: Vec<String> = memories.iter()
                .filter(|(_, c)| c.can_collect() && c.importance < threshold)
                .map(|(id, _)| id.clone())
                .collect();
            for id in to_remove {
                memories.remove(&id);
                removed += 1;
            }
        }

        // Check file memories
        {
            let mut memories = self.file_memories.write().await;
            let to_remove: Vec<String> = memories.iter()
                .filter(|(_, c)| c.can_collect() && c.importance < threshold)
                .map(|(id, _)| id.clone())
                .collect();
            for id in to_remove {
                memories.remove(&id);
                removed += 1;
            }
        }

        if removed > 0 {
            info!("Garbage collection removed {} low-importance memories", removed);
        }

        removed
    }
}

/// Statistics for multimodal memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultimodalStats {
    pub text_count: usize,
    pub image_count: usize,
    pub audio_count: usize,
    pub file_count: usize,
    pub total_count: usize,
    pub tag_count: usize,
}

/// Memory operation errors
#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("Content too large: {0} bytes (max: {1})")]
    ContentTooLarge(usize, usize),

    #[error("Memory not found: {0}")]
    NotFound(String),

    #[error("Invalid content type: {0}")]
    InvalidContentType(String),

    #[error("Storage error: {0}")]
    StorageError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_store_and_retrieve() {
        let store = MultimodalMemoryStore::new(MultimodalConfig::default());

        let content = MemoryContent::new_text("Hello, world!".to_string(), 0.8);
        let id = content.id.clone();

        store.store(content).await.unwrap();

        let retrieved = store.retrieve(&id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().data, "Hello, world!");
    }

    #[tokio::test]
    async fn test_query_by_tag() {
        let store = MultimodalMemoryStore::new(MultimodalConfig::default());

        let content = MemoryContent::new_text("Test content".to_string(), 0.8)
            .with_tag("test".to_string());

        let id = content.id.clone();
        store.store(content).await.unwrap();

        let query = MultimodalQuery {
            required_tags: vec!["test".to_string()],
            limit: 10,
            ..Default::default()
        };

        let results = store.query(&query).await;
        assert!(!results.is_empty());
        assert_eq!(results[0].content.id, id);
    }

    #[tokio::test]
    async fn test_garbage_collection() {
        let mut config = MultimodalConfig::default();
        config.enable_gc = true;

        let store = MultimodalMemoryStore::new(config);

        // High importance - should not be collected
        let content1 = MemoryContent::new_text("Important".to_string(), 0.8);
        store.store(content1).await.unwrap();

        // Low importance - should be collected
        let content2 = MemoryContent::new_text("Unimportant".to_string(), 0.2);
        store.store(content2).await.unwrap();

        let removed = store.garbage_collect().await;
        assert_eq!(removed, 1);
    }
}