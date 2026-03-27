//! Multi-Layer Cache Architecture
//!
//! Implements a 3-tier caching system for optimal performance:
//! - **L1 (In-Memory LRU)**: Fastest, limited capacity, TTL-based eviction
//! - **L2 (Redis)**: Fast, medium capacity, distributed cache
//! - **L3 (SQLite)**: Slowest, persistent, source of truth
//!
//! ## Cache Strategies
//! - **Write-Through**: Write goes to all layers synchronously
//! - **Read-Through**: Check L1 → L2 → L3, populate upper layers on miss
//! - **Write-Back**: Write to L1+L2, async flush to L3

use std::sync::Arc;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use lru::LruCache;
use anyhow::Result;

use super::redis_client::RedisClient;

/// L1 Cache configuration
#[derive(Debug, Clone)]
pub struct L1Config {
    /// Maximum number of entries
    pub capacity: usize,
    /// Time-to-live in seconds
    pub ttl_secs: u64,
}

impl Default for L1Config {
    fn default() -> Self {
        Self {
            capacity: 1000,     // 1000 entries max
            ttl_secs: 300,      // 5 minutes TTL
        }
    }
}

/// Unified multi-layer cache entry with metadata
#[derive(Debug, Clone)]
struct CacheEntry<V> {
    value: V,
    created_at: Instant,
    ttl: Duration,
    dirty: bool,  // Marked dirty for write-back
}

impl<V> CacheEntry<V> {
    fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }
}

/// Multi-layer cache with L1 (memory) + L2 (Redis) + L3 (SQLite fallback)
pub struct LayerCache<K, V>
where
    K: std::hash::Hash + Eq + Clone + std::fmt::Debug,
    V: Clone + Serialize + DeserializeOwned,
{
    name: String,
    l1: Arc<RwLock<LruCache<K, CacheEntry<V>>>>,
    l1_config: L1Config,
    redis: Option<Arc<RedisClient>>,
    redis_key_prefix: String,
    l3_getter: Option<Arc<dyn Fn(&K) -> Result<Option<V>> + Send + Sync>>,
    l3_setter: Option<Arc<dyn Fn(&K, &V) -> Result<()> + Send + Sync>>,
}

impl<K, V> LayerCache<K, V>
where
    K: std::hash::Hash + Eq + Clone + std::fmt::Debug + Send + 'static,
    V: Clone + Serialize + DeserializeOwned + Send + 'static,
{
    /// Create a new multi-layer cache
    pub fn new(
        name: &str,
        l1_config: L1Config,
        redis: Option<Arc<RedisClient>>,
        redis_key_prefix: &str,
    ) -> Self {
        let capacity = NonZeroUsize::new(l1_config.capacity.max(1))
            .expect("layer cache capacity must be non-zero");
        let cache = LruCache::new(capacity);
        
        Self {
            name: name.to_string(),
            l1: Arc::new(RwLock::new(cache)),
            l1_config,
            redis,
            redis_key_prefix: redis_key_prefix.to_string(),
            l3_getter: None,
            l3_setter: None,
        }
    }

    /// Configure L3 (SQLite) fallback
    pub fn with_l3(
        mut self,
        getter: impl Fn(&K) -> Result<Option<V>> + Send + Sync + 'static,
        setter: impl Fn(&K, &V) -> Result<()> + Send + Sync + 'static,
    ) -> Self {
        self.l3_getter = Some(Arc::new(getter));
        self.l3_setter = Some(Arc::new(setter));
        self
    }

    /// Make Redis key for L2
    fn make_l2_key(&self, key: &K) -> String {
        let key_str = format!("{:?}", key);
        format!("{}:{}", self.redis_key_prefix, key_str)
    }

    /// Get value from cache (L1 → L2 → L3)
    /// Returns (value, cache_hit_level)
    pub async fn get(&self, key: &K) -> Result<Option<(V, &'static str)>> {
        // L1: In-memory check
        {
            let mut l1 = self.l1.write().await;
            if let Some(entry) = l1.get(key) {
                if !entry.is_expired() {
                    tracing::trace!("[{}] L1 hit for key {:?}", self.name, key);
                    return Ok(Some((entry.value.clone(), "L1")));
                }
            }
        }

        // L2: Redis check
        if let Some(redis) = &self.redis {
            let l2_key = self.make_l2_key(key);
            if let Ok(Some(json)) = redis.get(&l2_key).await {
                if let Ok(value) = serde_json::from_str::<V>(&json) {
                    // Populate L1 on L2 hit
                    self.l1_put(key.clone(), value.clone(), false).await;
                    tracing::trace!("[{}] L2 hit for key {:?}", self.name, key);
                    return Ok(Some((value, "L2")));
                }
            }
        }

        // L3: SQLite fallback
        if let Some(l3_getter) = &self.l3_getter {
            if let Ok(Some(value)) = l3_getter(key) {
                // Populate L1 and L2 on L3 hit
                self.l1_put(key.clone(), value.clone(), false).await;
                if let Some(redis) = &self.redis {
                    let l2_key = self.make_l2_key(key);
                    let json = serde_json::to_string(&value).unwrap_or_default();
                    let _ = redis.set(&l2_key, &json, Some(3600)).await;
                }
                tracing::trace!("[{}] L3 hit for key {:?}", self.name, key);
                return Ok(Some((value, "L3")));
            }
        }

        Ok(None)
    }

    /// Put value to cache (write-through to all layers)
    pub async fn put(&self, key: K, value: V) -> Result<()> {
        self.l1_put(key.clone(), value.clone(), true).await;
        self.l2_put(&key, &value).await?;
        self.l3_put(&key, &value).await?;
        Ok(())
    }

    /// Put value to L1 only (internal)
    async fn l1_put(&self, key: K, value: V, dirty: bool) {
        let mut l1 = self.l1.write().await;
        let entry = CacheEntry {
            value,
            created_at: Instant::now(),
            ttl: Duration::from_secs(self.l1_config.ttl_secs),
            dirty,
        };
        l1.put(key, entry);
    }

    /// Put value to L2 (Redis)
    async fn l2_put(&self, key: &K, value: &V) -> Result<()> {
        if let Some(redis) = &self.redis {
            let l2_key = self.make_l2_key(key);
            let json = serde_json::to_string(value)?;
            // L2 TTL is 10x L1 TTL
            let ttl = self.l1_config.ttl_secs * 10;
            redis.set(&l2_key, &json, Some(ttl)).await?;
            tracing::trace!("[{}] L2 put for key {:?}", self.name, key);
        }
        Ok(())
    }

    /// Put value to L3 (SQLite)
    async fn l3_put(&self, key: &K, value: &V) -> Result<()> {
        if let Some(l3_setter) = &self.l3_setter {
            l3_setter(key, value)?;
            tracing::trace!("[{}] L3 put for key {:?}", self.name, key);
        }
        Ok(())
    }

    /// Delete from all layers
    pub async fn delete(&self, key: &K) -> Result<()> {
        // L1 delete
        {
            let mut l1 = self.l1.write().await;
            l1.pop(key);
        }

        // L2 delete
        if let Some(redis) = &self.redis {
            let l2_key = self.make_l2_key(key);
            let _ = redis.del(&l2_key).await;
        }

        // L3 delete (best effort)
        // Note: L3 delete requires separate implementation if needed

        Ok(())
    }

    /// Invalidate L1 only (useful when data is known to be stale)
    pub async fn invalidate_l1(&self) {
        let mut l1 = self.l1.write().await;
        l1.clear();
        tracing::debug!("[{}] L1 cache invalidated", self.name);
    }

    /// Get L1 stats
    pub async fn l1_stats(&self) -> L1Stats {
        let l1 = self.l1.read().await;
        L1Stats {
            len: l1.len(),
            capacity: self.l1_config.capacity,
        }
    }

    /// Periodic flush: write dirty L1 entries to L2 and L3
    pub async fn flush_dirty(&self) -> Result<usize> {
        let mut dirty_count = 0;
        let mut l1 = self.l1.write().await;

        // Note: This is a simplified flush. In production, you'd want
        // to iterate over dirty entries and write them back.
        for (key, entry) in l1.iter_mut() {
            if entry.dirty {
                // Write to L2
                if let Some(redis) = &self.redis {
                    let l2_key = self.make_l2_key(key);
                    let json = serde_json::to_string(&entry.value).unwrap_or_default();
                    let _ = redis.set(&l2_key, &json, Some(self.l1_config.ttl_secs * 10)).await;
                }
                // Write to L3
                if let Some(l3_setter) = &self.l3_setter {
                    let _ = l3_setter(key, &entry.value);
                }
                entry.dirty = false;
                dirty_count += 1;
            }
        }

        tracing::info!("[{}] Flushed {} dirty entries", self.name, dirty_count);
        Ok(dirty_count)
    }
}

/// L1 cache statistics
#[derive(Debug, Clone)]
pub struct L1Stats {
    pub len: usize,
    pub capacity: usize,
}

/// Specialized session context cache with multi-layer support
pub struct SessionContextCache {
    cache: LayerCache<String, SessionContextData>,
}

impl SessionContextCache {
    /// Create session context cache
    pub fn new(
        redis: Option<Arc<RedisClient>>,
        l1_config: L1Config,
    ) -> Self {
        let cache = LayerCache::new(
            "session_context",
            l1_config,
            redis,
            "session_ctx",
        );

        Self { cache }
    }

    /// Get session context
    pub async fn get(&self, session_id: &str) -> Result<Option<(SessionContextData, &'static str)>> {
        self.cache.get(&session_id.to_string()).await
    }

    /// Put session context
    pub async fn put(&self, session_id: &str, data: SessionContextData) -> Result<()> {
        self.cache.put(session_id.to_string(), data).await
    }

    /// Delete session context
    pub async fn delete(&self, session_id: &str) -> Result<()> {
        self.cache.delete(&session_id.to_string()).await
    }

    /// Invalidate L1 cache
    pub async fn invalidate(&self) {
        self.cache.invalidate_l1().await;
    }
}

/// Session context data structure for caching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContextData {
    pub session_id: String,
    pub token_count: u32,
    pub max_tokens: u32,
    pub compressed: bool,
    pub last_updated: i64,
    pub messages_json: String,
}

/// Cache statistics for monitoring
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub l1_hits: u64,
    pub l2_hits: u64,
    pub l3_hits: u64,
    pub l1_misses: u64,
    pub l2_misses: u64,
    pub l3_misses: u64,
}

impl CacheStats {
    pub fn total_hits(&self) -> u64 {
        self.l1_hits + self.l2_hits + self.l3_hits
    }

    pub fn total_misses(&self) -> u64 {
        self.l1_misses + self.l2_misses + self.l3_misses
    }

    pub fn hit_rate(&self) -> f64 {
        let total = self.total_hits() + self.total_misses();
        if total == 0 {
            0.0
        } else {
            self.total_hits() as f64 / total as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_l1_cache_basic() {
        let l1_config = L1Config {
            capacity: 10,
            ttl_secs: 60,
        };

        let cache = LayerCache::new(
            "test",
            l1_config,
            None,  // No Redis for test
            "test",
        );

        // Put and get
        cache.put("key1".to_string(), "value1".to_string()).await.unwrap();
        let result = cache.get(&"key1".to_string()).await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, "value1");

        // Stats
        let stats = cache.l1_stats().await;
        assert_eq!(stats.len, 1);
    }

    #[tokio::test]
    async fn test_l1_eviction() {
        let l1_config = L1Config {
            capacity: 2,
            ttl_secs: 60,
        };

        let cache = LayerCache::new(
            "test",
            l1_config,
            None,
            "test",
        );

        // Fill beyond capacity
        cache.put("k1".to_string(), "v1".to_string()).await.unwrap();
        cache.put("k2".to_string(), "v2".to_string()).await.unwrap();
        cache.put("k3".to_string(), "v3".to_string()).await.unwrap();

        let stats = cache.l1_stats().await;
        assert_eq!(stats.len, 2); // k1 should be evicted
    }
}
