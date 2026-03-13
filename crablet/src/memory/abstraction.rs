//! Memory Layer Abstraction - 记忆层抽象
//!
//! 提供统一的记忆层接口，支持多种后端实现

use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// 记忆类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MemoryType {
    /// 情景记忆 - 具体事件
    Episodic,
    /// 语义记忆 - 事实和概念
    Semantic,
    /// 程序记忆 - 技能和过程
    Procedural,
    /// 工作记忆 - 短期信息
    Working,
}

/// 记忆条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub memory_type: MemoryType,
    pub content: String,
    pub embedding: Option<Vec<f32>>,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub access_count: u64,
    pub importance: f32,
    pub session_id: Option<String>,
    pub tags: Vec<String>,
}

impl MemoryEntry {
    pub fn new(content: String, memory_type: MemoryType) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            memory_type,
            content,
            embedding: None,
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
            access_count: 0,
            importance: 0.5,
            session_id: None,
            tags: Vec::new(),
        }
    }

    pub fn with_session(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }

    pub fn with_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.embedding = Some(embedding);
        self
    }

    pub fn with_importance(mut self, importance: f32) -> Self {
        self.importance = importance.clamp(0.0, 1.0);
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }
}

/// 查询条件
#[derive(Debug, Clone, Default)]
pub struct MemoryQuery {
    pub memory_type: Option<MemoryType>,
    pub session_id: Option<String>,
    pub tags: Option<Vec<String>>,
    pub content_contains: Option<String>,
    pub min_importance: Option<f32>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
    pub semantic_query: Option<String>,
}

impl MemoryQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_type(mut self, memory_type: MemoryType) -> Self {
        self.memory_type = Some(memory_type);
        self
    }

    pub fn with_session(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }

    pub fn with_content(mut self, content: String) -> Self {
        self.content_contains = Some(content);
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn with_semantic(mut self, query: String) -> Self {
        self.semantic_query = Some(query);
        self
    }
}

/// 记忆后端接口
#[async_trait]
pub trait MemoryBackend: Send + Sync {
    /// 存储记忆
    async fn store(&self, entry: MemoryEntry) -> Result<String>;

    /// 批量存储
    async fn store_batch(&self, entries: Vec<MemoryEntry>) -> Result<Vec<String>>;

    /// 检索记忆
    async fn retrieve(&self, query: &MemoryQuery) -> Result<Vec<MemoryEntry>>;

    /// 根据 ID 获取
    async fn get_by_id(&self, id: &str) -> Result<Option<MemoryEntry>>;

    /// 更新记忆
    async fn update(&self, id: &str, entry: MemoryEntry) -> Result<()>;

    /// 删除记忆
    async fn delete(&self, id: &str) -> Result<bool>;

    /// 语义搜索
    async fn semantic_search(&self, query: &str, top_k: usize) -> Result<Vec<(MemoryEntry, f32)>>;

    /// 获取会话历史
    async fn get_session_history(&self, session_id: &str, limit: usize) -> Result<Vec<MemoryEntry>>;

    /// 清理过期记忆
    async fn cleanup_expired(&self, max_age_days: i64) -> Result<usize>;

    /// 获取统计
    async fn stats(&self) -> Result<MemoryStats>;
}

/// 记忆统计
#[derive(Debug, Clone, Default)]
pub struct MemoryStats {
    pub total_entries: usize,
    pub by_type: HashMap<MemoryType, usize>,
    pub total_size_bytes: usize,
    pub avg_importance: f32,
}

/// 记忆层配置
#[derive(Debug, Clone)]
pub struct MemoryLayerConfig {
    pub enable_caching: bool,
    pub cache_size: usize,
    pub enable_compression: bool,
    pub max_entry_size: usize,
    pub default_ttl_days: i64,
}

impl Default for MemoryLayerConfig {
    fn default() -> Self {
        Self {
            enable_caching: true,
            cache_size: 1000,
            enable_compression: false,
            max_entry_size: 10 * 1024 * 1024, // 10MB
            default_ttl_days: 30,
        }
    }
}

/// 记忆层 - 统一接口
pub struct MemoryLayer {
    backend: Arc<dyn MemoryBackend>,
    config: MemoryLayerConfig,
}

impl MemoryLayer {
    pub fn new(backend: Arc<dyn MemoryBackend>, config: MemoryLayerConfig) -> Self {
        Self { backend, config }
    }

    pub async fn store(&self, content: String, memory_type: MemoryType) -> Result<String> {
        let entry = MemoryEntry::new(content, memory_type);
        self.backend.store(entry).await
    }

    pub async fn store_with_context(
        &self,
        content: String,
        memory_type: MemoryType,
        session_id: Option<String>,
        importance: f32,
    ) -> Result<String> {
        let mut entry = MemoryEntry::new(content, memory_type)
            .with_importance(importance);
        
        if let Some(sid) = session_id {
            entry = entry.with_session(sid);
        }
        
        self.backend.store(entry).await
    }

    pub async fn recall(&self, query: &MemoryQuery) -> Result<Vec<MemoryEntry>> {
        self.backend.retrieve(query).await
    }

    pub async fn recall_recent(&self, session_id: &str, limit: usize) -> Result<Vec<MemoryEntry>> {
        self.backend.get_session_history(session_id, limit).await
    }

    pub async fn recall_similar(&self, query: &str, top_k: usize) -> Result<Vec<MemoryEntry>> {
        let results = self.backend.semantic_search(query, top_k).await?;
        Ok(results.into_iter().map(|(entry, _)| entry).collect())
    }

    pub async fn forget(&self, id: &str) -> Result<bool> {
        self.backend.delete(id).await
    }

    pub async fn stats(&self) -> Result<MemoryStats> {
        self.backend.stats().await
    }
}

/// 内存后端实现
pub mod backends {
    use super::*;
    use dashmap::DashMap;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// 内存后端
    pub struct InMemoryBackend {
        entries: DashMap<String, MemoryEntry>,
        counter: AtomicU64,
    }

    impl InMemoryBackend {
        pub fn new() -> Self {
            Self {
                entries: DashMap::new(),
                counter: AtomicU64::new(0),
            }
        }
    }

    #[async_trait]
    impl MemoryBackend for InMemoryBackend {
        async fn store(&self, entry: MemoryEntry) -> Result<String> {
            let id = entry.id.clone();
            self.entries.insert(id.clone(), entry);
            self.counter.fetch_add(1, Ordering::Relaxed);
            Ok(id)
        }

        async fn store_batch(&self, entries: Vec<MemoryEntry>) -> Result<Vec<String>> {
            let mut ids = Vec::new();
            for entry in entries {
                let id = self.store(entry).await?;
                ids.push(id);
            }
            Ok(ids)
        }

        async fn retrieve(&self, query: &MemoryQuery) -> Result<Vec<MemoryEntry>> {
            let limit = query.limit.unwrap_or(100);
            let mut results: Vec<_> = self
                .entries
                .iter()
                .filter(|e| {
                    if let Some(ref memory_type) = query.memory_type {
                        if e.memory_type != *memory_type {
                            return false;
                        }
                    }
                    if let Some(ref session_id) = query.session_id {
                        if e.session_id.as_ref() != Some(session_id) {
                            return false;
                        }
                    }
                    if let Some(ref content) = query.content_contains {
                        if !e.content.contains(content) {
                            return false;
                        }
                    }
                    if let Some(min_importance) = query.min_importance {
                        if e.importance < min_importance {
                            return false;
                        }
                    }
                    true
                })
                .map(|e| e.clone())
                .collect();

            // 按重要性排序
            results.sort_by(|a, b| b.importance.partial_cmp(&a.importance).unwrap());
            results.truncate(limit);

            Ok(results)
        }

        async fn get_by_id(&self, id: &str) -> Result<Option<MemoryEntry>> {
            Ok(self.entries.get(id).map(|e| e.clone()))
        }

        async fn update(&self, id: &str, entry: MemoryEntry) -> Result<()> {
            self.entries.insert(id.to_string(), entry);
            Ok(())
        }

        async fn delete(&self, id: &str) -> Result<bool> {
            Ok(self.entries.remove(id).is_some())
        }

        async fn semantic_search(&self, query: &str, top_k: usize) -> Result<Vec<(MemoryEntry, f32)>> {
            // 简化实现：基于关键词匹配
            let query_lower = query.to_lowercase();
            let mut results: Vec<_> = self
                .entries
                .iter()
                .filter_map(|e| {
                    let content_lower = e.content.to_lowercase();
                    if content_lower.contains(&query_lower) {
                        let score = Self::calculate_similarity(&query_lower, &content_lower);
                        Some((e.clone(), score))
                    } else {
                        None
                    }
                })
                .collect();

            results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            results.truncate(top_k);
            Ok(results)
        }

        async fn get_session_history(&self, session_id: &str, limit: usize) -> Result<Vec<MemoryEntry>> {
            let mut results: Vec<_> = self
                .entries
                .iter()
                .filter(|e| e.session_id.as_deref() == Some(session_id))
                .map(|e| e.clone())
                .collect();

            results.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            results.truncate(limit);
            Ok(results)
        }

        async fn cleanup_expired(&self, max_age_days: i64) -> Result<usize> {
            let cutoff = Utc::now() - chrono::Duration::days(max_age_days);
            let mut removed = 0;

            self.entries.retain(|_, entry| {
                let keep = entry.updated_at > cutoff;
                if !keep {
                    removed += 1;
                }
                keep
            });

            Ok(removed)
        }

        async fn stats(&self) -> Result<MemoryStats> {
            let total = self.entries.len();
            let mut by_type = HashMap::new();
            let mut total_size = 0;
            let mut total_importance = 0.0;

            for entry in self.entries.iter() {
                *by_type.entry(entry.memory_type.clone()).or_insert(0) += 1;
                total_size += entry.content.len();
                total_importance += entry.importance;
            }

            Ok(MemoryStats {
                total_entries: total,
                by_type,
                total_size_bytes: total_size,
                avg_importance: if total > 0 {
                    total_importance / total as f32
                } else {
                    0.0
                },
            })
        }
    }

    impl InMemoryBackend {
        fn calculate_similarity(query: &str, content: &str) -> f32 {
            // 简化的相似度计算
            let query_words: std::collections::HashSet<_> = query.split_whitespace().collect();
            let content_words: std::collections::HashSet<_> = content.split_whitespace().collect();
            
            let intersection: std::collections::HashSet<_> = query_words
                .intersection(&content_words)
                .collect();
            
            if query_words.is_empty() {
                return 0.0;
            }
            
            intersection.len() as f32 / query_words.len() as f32
        }
    }

    impl Default for InMemoryBackend {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::backends::InMemoryBackend;

    #[tokio::test]
    async fn test_memory_layer() {
        let backend = Arc::new(InMemoryBackend::new());
        let layer = MemoryLayer::new(backend, MemoryLayerConfig::default());

        // 存储记忆
        let id = layer.store("Hello, world!".to_string(), MemoryType::Episodic).await.unwrap();
        assert!(!id.is_empty());

        // 检索记忆
        let query = MemoryQuery::new().with_type(MemoryType::Episodic);
        let results = layer.recall(&query).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "Hello, world!");
    }

    #[tokio::test]
    async fn test_session_history() {
        let backend = Arc::new(InMemoryBackend::new());
        let layer = MemoryLayer::new(backend, MemoryLayerConfig::default());

        let session_id = "test-session".to_string();
        
        layer.store_with_context(
            "Message 1".to_string(),
            MemoryType::Episodic,
            Some(session_id.clone()),
            0.8,
        ).await.unwrap();

        layer.store_with_context(
            "Message 2".to_string(),
            MemoryType::Episodic,
            Some(session_id.clone()),
            0.9,
        ).await.unwrap();

        let history = layer.recall_recent(&session_id, 10).await.unwrap();
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn test_memory_entry_builder() {
        let entry = MemoryEntry::new("Test content".to_string(), MemoryType::Semantic)
            .with_session("session-1".to_string())
            .with_importance(0.9)
            .with_tags(vec!["tag1".to_string(), "tag2".to_string()]);

        assert_eq!(entry.content, "Test content");
        assert_eq!(entry.session_id, Some("session-1".to_string()));
        assert_eq!(entry.importance, 0.9);
        assert_eq!(entry.tags.len(), 2);
    }
}
