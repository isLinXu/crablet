//! Smart Multi-Level Cache System for Knowledge Retrieval
//!
//! Provides L1 (memory), L2 (Redis), and L3 (disk) caching with
//! intelligent prefetching and cache warming capabilities.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::{Result, anyhow};
use dashmap::DashMap;
use moka::future::Cache as MokaCache;
use serde::{de::DeserializeOwned, Serialize};
use tokio::fs;
use tracing::{debug, info, warn};

/// Cache statistics for monitoring
#[derive(Debug, Default, Clone)]
pub struct CacheStats {
    pub hits_l1: u64,
    pub hits_l2: u64,
    pub hits_l3: u64,
    pub misses: u64,
    pub evictions: u64,
    pub total_requests: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        let hits = self.hits_l1 + self.hits_l2 + self.hits_l3;
        hits as f64 / self.total_requests as f64
    }

    pub fn l1_hit_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        self.hits_l1 as f64 / self.total_requests as f64
    }
}

/// Configuration for multi-level cache
#[derive(Clone, Debug)]
pub struct SmartCacheConfig {
    /// L1 cache capacity
    pub l1_capacity: u64,
    /// L1 TTL in seconds
    pub l1_ttl_secs: u64,
    /// L2 Redis URL (optional)
    pub l2_redis_url: Option<String>,
    /// L2 TTL in seconds
    pub l2_ttl_secs: u64,
    /// L3 disk cache path (optional)
    pub l3_path: Option<PathBuf>,
    /// L3 TTL in seconds
    pub l3_ttl_secs: u64,
    /// Enable prefetching
    pub enable_prefetch: bool,
    /// Prefetch threshold (cache hits before prefetch)
    pub prefetch_threshold: u32,
}

impl Default for SmartCacheConfig {
    fn default() -> Self {
        Self {
            l1_capacity: 10_000,
            l1_ttl_secs: 300, // 5 minutes
            l2_redis_url: None,
            l2_ttl_secs: 600, // 10 minutes
            l3_path: None,
            l3_ttl_secs: 3600, // 1 hour
            enable_prefetch: true,
            prefetch_threshold: 3,
        }
    }
}

/// Cache entry with metadata
#[derive(Clone, Debug)]
struct CacheEntry<V> {
    value: V,
    created_at: Instant,
    access_count: u32,
    last_accessed: Instant,
}

/// Smart multi-level cache
pub struct SmartCache<K, V>
where
    K: Clone + Send + Sync + std::hash::Hash + Eq + 'static + std::fmt::Display,
    V: Clone + Send + Sync + 'static + Serialize + DeserializeOwned,
{
    config: SmartCacheConfig,
    /// L1: In-memory cache (Moka)
    l1_cache: MokaCache<K, CacheEntry<V>>,
    /// L2: Redis cache (optional)
    l2_redis: Option<Arc<dyn RedisCache<K, V>>>,
    /// L3: Disk cache path
    l3_path: Option<PathBuf>,
    /// Access tracking for prefetching
    access_tracker: DashMap<K, u32>,
    /// Statistics
    stats: Arc<DashMap<String, CacheStats>>,
}

/// Trait for Redis cache abstraction
#[async_trait::async_trait]
pub trait RedisCache<K, V>: Send + Sync
where
    K: Clone + Send + Sync + std::hash::Hash + Eq + 'static,
    V: Clone + Send + Sync + 'static,
{
    async fn get(&self, key: &K) -> Result<Option<V>>;
    async fn set(&self, key: &K, value: &V, ttl: Duration) -> Result<()>;
    async fn delete(&self, key: &K) -> Result<()>;
}

impl<K, V> SmartCache<K, V>
where
    K: Clone + Send + Sync + std::hash::Hash + Eq + 'static + std::fmt::Display,
    V: Clone + Send + Sync + 'static + Serialize + DeserializeOwned,
{
    /// Create a new smart cache with default configuration
    pub fn new() -> Self {
        Self::with_config(SmartCacheConfig::default())
    }

    /// Create a new smart cache with custom configuration
    pub fn with_config(config: SmartCacheConfig) -> Self {
        let l1_cache = MokaCache::builder()
            .max_capacity(config.l1_capacity)
            .time_to_live(Duration::from_secs(config.l1_ttl_secs))
            .eviction_listener(|key, _, cause| {
                debug!("L1 cache eviction: key={}, cause={:?}", key, cause);
            })
            .build();

        // Ensure L3 directory exists
        if let Some(ref path) = config.l3_path {
            if let Err(e) = std::fs::create_dir_all(path) {
                warn!("Failed to create L3 cache directory: {}", e);
            }
        }

        Self {
            config: config.clone(),
            l1_cache,
            l2_redis: None,
            l3_path: config.l3_path,
            access_tracker: DashMap::new(),
            stats: Arc::new(DashMap::new()),
        }
    }

    /// Set L2 Redis cache
    pub fn with_redis(mut self, redis: Arc<dyn RedisCache<K, V>>) -> Self {
        self.l2_redis = Some(redis);
        self
    }

    /// Get value from cache (L1 -> L2 -> L3)
    pub async fn get(&self, key: &K) -> Option<V> {
        let mut stats = self.stats.entry("default".to_string()).or_default().clone();
        stats.total_requests += 1;

        // Try L1 cache first
        if let Some(entry) = self.l1_cache.get(key).await {
            debug!("L1 cache hit for key: {}", key);
            stats.hits_l1 += 1;
            self.update_stats("default".to_string(), stats);
            self.track_access(key.clone());
            return Some(entry.value);
        }

        // Try L2 cache (Redis)
        if let Some(ref redis) = self.l2_redis {
            match redis.get(key).await {
                Ok(Some(value)) => {
                    debug!("L2 cache hit for key: {}", key);
                    stats.hits_l2 += 1;
                    
                    // Promote to L1
                    self.put_l1(key.clone(), value.clone()).await;
                    self.update_stats("default".to_string(), stats);
                    self.track_access(key.clone());
                    return Some(value);
                }
                Ok(None) => {}
                Err(e) => {
                    warn!("L2 cache error: {}", e);
                }
            }
        }

        // Try L3 cache (Disk)
        if let Some(ref path) = self.l3_path {
            match self.get_l3(key, path).await {
                Ok(Some(value)) => {
                    debug!("L3 cache hit for key: {}", key);
                    stats.hits_l3 += 1;
                    
                    // Promote to L1 and L2
                    self.put_l1(key.clone(), value.clone()).await;
                    if let Some(ref redis) = self.l2_redis {
                        let _ = redis.set(key, &value, Duration::from_secs(self.config.l2_ttl_secs)).await;
                    }
                    
                    self.update_stats("default".to_string(), stats);
                    self.track_access(key.clone());
                    return Some(value);
                }
                Ok(None) => {}
                Err(e) => {
                    warn!("L3 cache error: {}", e);
                }
            }
        }

        // Cache miss
        debug!("Cache miss for key: {}", key);
        stats.misses += 1;
        self.update_stats("default".to_string(), stats);
        None
    }

    /// Put value into cache (all levels)
    pub async fn put(&self, key: K, value: V) {
        // Put into L1
        self.put_l1(key.clone(), value.clone()).await;

        // Put into L2
        if let Some(ref redis) = self.l2_redis {
            let _ = redis.set(&key, &value, Duration::from_secs(self.config.l2_ttl_secs)).await;
        }

        // Put into L3
        if let Some(ref path) = self.l3_path {
            let _ = self.put_l3(&key, &value, path).await;
        }
    }

    /// Invalidate cache entry
    pub async fn invalidate(&self, key: &K) {
        self.l1_cache.invalidate(key).await;
        
        if let Some(ref redis) = self.l2_redis {
            let _ = redis.delete(key).await;
        }

        if let Some(ref path) = self.l3_path {
            let _ = self.delete_l3(key, path).await;
        }

        self.access_tracker.remove(key);
    }

    /// Invalidate cache entries by pattern (if supported)
    pub async fn invalidate_pattern(&self, _pattern: &str) {
        // For now, just clear L1
        self.l1_cache.invalidate_all();
        
        // L2 and L3 pattern invalidation would require additional implementation
        warn!("Pattern invalidation not fully implemented for L2/L3");
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        self.stats
            .get("default")
            .map(|s| s.clone())
            .unwrap_or_default()
    }

    /// Warm cache with values
    pub async fn warm_cache(&self, entries: Vec<(K, V)>) {
        info!("Warming cache with {} entries", entries.len());
        for (key, value) in entries {
            self.put(key, value).await;
        }
    }

    /// Check if key should be prefetched
    pub fn should_prefetch(&self, key: &K) -> bool {
        if !self.config.enable_prefetch {
            return false;
        }

        self.access_tracker
            .get(key)
            .map(|count| *count >= self.config.prefetch_threshold)
            .unwrap_or(false)
    }

    // Private helper methods

    async fn put_l1(&self, key: K, value: V) {
        let entry = CacheEntry {
            value,
            created_at: Instant::now(),
            access_count: 1,
            last_accessed: Instant::now(),
        };
        self.l1_cache.insert(key, entry).await;
    }

    async fn get_l3(&self, key: &K, base_path: &PathBuf) -> Result<Option<V>> {
        let file_path = base_path.join(format!("{}.bin", key));
        
        if !file_path.exists() {
            return Ok(None);
        }

        // Check TTL
        let metadata = fs::metadata(&file_path).await?;
        let modified = metadata.modified()?;
        let age = Instant::now().duration_since(
            modified.duration_since(std::time::UNIX_EPOCH)
                .map(|d| Instant::now() - d)
                .unwrap_or_default()
        );

        if age > Duration::from_secs(self.config.l3_ttl_secs) {
            // Expired
            let _ = fs::remove_file(&file_path).await;
            return Ok(None);
        }

        let data = fs::read(&file_path).await?;
        let value: V = bincode::deserialize(&data)
            .map_err(|e| anyhow!("Failed to deserialize L3 cache: {}", e))?;
        
        Ok(Some(value))
    }

    async fn put_l3(&self, key: &K, value: &V, base_path: &PathBuf) -> Result<()> {
        let file_path = base_path.join(format!("{}.bin", key));
        let data = bincode::serialize(value)
            .map_err(|e| anyhow!("Failed to serialize L3 cache: {}", e))?;
        fs::write(&file_path, data).await?;
        Ok(())
    }

    async fn delete_l3(&self, key: &K, base_path: &PathBuf) -> Result<()> {
        let file_path = base_path.join(format!("{}.bin", key));
        if file_path.exists() {
            fs::remove_file(&file_path).await?;
        }
        Ok(())
    }

    fn track_access(&self, key: K) {
        self.access_tracker
            .entry(key)
            .and_modify(|count| *count += 1)
            .or_insert(1);
    }

    fn update_stats(&self, key: String, stats: CacheStats) {
        self.stats.insert(key, stats);
    }
}

impl<K, V> Default for SmartCache<K, V>
where
    K: Clone + Send + Sync + std::hash::Hash + Eq + 'static + std::fmt::Display,
    V: Clone + Send + Sync + 'static + Serialize + DeserializeOwned,
{
    fn default() -> Self {
        Self::new()
    }
}

/// Cache warmer for proactive cache population
pub struct CacheWarmer<K, V> {
    cache: Arc<SmartCache<K, V>>,
    warming_queue: Arc<DashMap<K, ()>>,
}

impl<K, V> CacheWarmer<K, V>
where
    K: Clone + Send + Sync + std::hash::Hash + Eq + 'static + std::fmt::Display,
    V: Clone + Send + Sync + 'static + Serialize + DeserializeOwned,
{
    pub fn new(cache: Arc<SmartCache<K, V>>) -> Self {
        Self {
            cache,
            warming_queue: Arc::new(DashMap::new()),
        }
    }

    /// Schedule a key for warming
    pub fn schedule(&self, key: K) {
        self.warming_queue.insert(key, ());
    }

    /// Execute warming with provided loader function
    pub async fn warm<F, Fut>(&self, loader: F)
    where
        F: Fn(K) -> Fut,
        Fut: std::future::Future<Output = Option<V>>,
    {
        let keys: Vec<K> = self.warming_queue.iter().map(|e| e.key().clone()).collect();
        
        for key in keys {
            if let Some(value) = loader(key.clone()).await {
                self.cache.put(key.clone(), value).await;
            }
            self.warming_queue.remove(&key);
        }
    }
}

/// Semantic cache for similar queries
pub struct SemanticCache {
    /// Query embeddings cache
    embedding_cache: MokaCache<String, Vec<f32>>,
    /// Similarity threshold for cache hit
    similarity_threshold: f32,
}

impl SemanticCache {
    pub fn new() -> Self {
        Self {
            embedding_cache: MokaCache::builder()
                .max_capacity(1000)
                .time_to_live(Duration::from_secs(600))
                .build(),
            similarity_threshold: 0.95,
        }
    }

    /// Find semantically similar cached query
    pub async fn find_similar(&self, embedding: &[f32]) -> Option<(String, Vec<f32>)> {
        for entry in self.embedding_cache.iter() {
            let similarity = cosine_similarity(embedding, entry.value());
            if similarity >= self.similarity_threshold {
                return Some((entry.key().clone(), entry.value().clone()));
            }
        }
        None
    }

    /// Store query embedding
    pub async fn store(&self, query: String, embedding: Vec<f32>) {
        self.embedding_cache.insert(query, embedding).await;
    }
}

impl Default for SemanticCache {
    fn default() -> Self {
        Self::new()
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    
    dot / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_smart_cache_basic() {
        let cache: SmartCache<String, String> = SmartCache::new();
        
        // Put and get
        cache.put("key1".to_string(), "value1".to_string()).await;
        let value = cache.get(&"key1".to_string()).await;
        assert_eq!(value, Some("value1".to_string()));
        
        // Non-existent key
        let value = cache.get(&"key2".to_string()).await;
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_cache_invalidation() {
        let cache: SmartCache<String, String> = SmartCache::new();
        
        cache.put("key1".to_string(), "value1".to_string()).await;
        cache.invalidate(&"key1".to_string()).await;
        
        let value = cache.get(&"key1".to_string()).await;
        assert_eq!(value, None);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);
        
        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c)).abs() < 0.001);
    }
}
