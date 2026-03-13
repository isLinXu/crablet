//! Multi-Level Cache Architecture - 多级缓存架构
//!
//! L1: In-memory LRU (hot data)
//! L2: Shared memory / local disk (warm data)
//! L3: Distributed cache (cold data)

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;
use lru::LruCache;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// 缓存层级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CacheLevel {
    L1, // 内存 LRU
    L2, // 本地存储
    L3, // 分布式
}

/// 缓存条目
#[derive(Debug, Clone)]
struct CacheEntry<V> {
    value: V,
    level: CacheLevel,
    created_at: Instant,
    expires_at: Instant,
    access_count: u64,
}

/// 缓存统计
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub hits: HashMap<CacheLevel, u64>,
    pub misses: u64,
    pub evictions: u64,
    pub size: HashMap<CacheLevel, usize>,
}

impl CacheStats {
    pub fn new() -> Self {
        Self {
            hits: HashMap::new(),
            misses: 0,
            evictions: 0,
            size: HashMap::new(),
        }
    }

    pub fn hit_rate(&self) -> f64 {
        let total_hits: u64 = self.hits.values().sum();
        let total = total_hits + self.misses;
        if total == 0 {
            0.0
        } else {
            total_hits as f64 / total as f64
        }
    }

    pub fn record_hit(&mut self, level: CacheLevel) {
        *self.hits.entry(level).or_insert(0) += 1;
    }

    pub fn record_miss(&mut self) {
        self.misses += 1;
    }

    pub fn record_eviction(&mut self) {
        self.evictions += 1;
    }
}

/// 多级缓存配置
#[derive(Debug, Clone)]
pub struct MultiLevelCacheConfig {
    /// L1 缓存大小
    pub l1_size: usize,
    /// L1 TTL
    pub l1_ttl: Duration,
    /// L2 缓存大小 (文件缓存条目数)
    pub l2_size: usize,
    /// L2 TTL
    pub l2_ttl: Duration,
    /// 是否启用 L2
    pub enable_l2: bool,
    /// 是否启用 L3
    pub enable_l3: bool,
    /// 预加载策略
    pub preload_policy: PreloadPolicy,
}

impl Default for MultiLevelCacheConfig {
    fn default() -> Self {
        Self {
            l1_size: 1000,
            l1_ttl: Duration::from_secs(300), // 5分钟
            l2_size: 10000,
            l2_ttl: Duration::from_secs(3600), // 1小时
            enable_l2: true,
            enable_l3: false,
            preload_policy: PreloadPolicy::OnDemand,
        }
    }
}

/// 预加载策略
#[derive(Debug, Clone)]
pub enum PreloadPolicy {
    /// 按需加载
    OnDemand,
    /// 预热加载
    WarmUp(Vec<String>),
    /// 预测加载
    Predictive,
}

/// L1 内存缓存
struct L1Cache<K, V> {
    cache: RwLock<LruCache<K, CacheEntry<V>>>,
    ttl: Duration,
}

impl<K: Eq + Hash, V: Clone> L1Cache<K, V> {
    fn new(size: usize, ttl: Duration) -> Self {
        Self {
            cache: RwLock::new(LruCache::new(std::num::NonZeroUsize::new(size).unwrap())),
            ttl,
        }
    }

    fn get(&self, key: &K) -> Option<V> {
        let mut cache = self.cache.write();
        let entry = cache.get(key)?;
        
        if Instant::now() > entry.expires_at {
            cache.pop(key);
            None
        } else {
            Some(entry.value.clone())
        }
    }

    fn put(&self, key: K, value: V) {
        let entry = CacheEntry {
            value,
            level: CacheLevel::L1,
            created_at: Instant::now(),
            expires_at: Instant::now() + self.ttl,
            access_count: 1,
        };
        self.cache.write().put(key, entry);
    }

    fn remove(&self, key: &K) -> Option<V> {
        self.cache.write().pop(key).map(|e| e.value)
    }

    fn len(&self) -> usize {
        self.cache.read().len()
    }

    fn clear(&self) {
        self.cache.write().clear();
    }

    /// 清理过期条目
    fn cleanup_expired(&self) -> usize {
        let now = Instant::now();
        let mut cache = self.cache.write();
        let mut removed = 0;
        
        // LruCache 不支持遍历，这里简化处理
        // 实际应该使用支持遍历的缓存结构
        while cache.len() > 0 {
            if let Some((key, entry)) = cache.pop_lru() {
                if entry.expires_at < now {
                    removed += 1;
                } else {
                    // 如果未过期，重新放回
                    cache.put(key, entry);
                    break;
                }
            }
        }
        
        removed
    }
}

/// L2 本地缓存 (简化版，实际可使用 sled/rocksdb)
struct L2Cache<K, V> {
    cache: RwLock<HashMap<K, CacheEntry<V>>>,
    max_size: usize,
    ttl: Duration,
}

impl<K: Eq + Hash + Clone, V: Clone> L2Cache<K, V> {
    fn new(max_size: usize, ttl: Duration) -> Self {
        Self {
            cache: RwLock::new(HashMap::with_capacity(max_size)),
            max_size,
            ttl,
        }
    }

    fn get(&self, key: &K) -> Option<V> {
        let cache = self.cache.read();
        let entry = cache.get(key)?;
        
        if Instant::now() > entry.expires_at {
            drop(cache);
            self.cache.write().remove(key);
            None
        } else {
            Some(entry.value.clone())
        }
    }

    fn put(&self, key: K, value: V) {
        let mut cache = self.cache.write();
        
        // 如果满了，清理最旧的条目（简化实现）
        if cache.len() >= self.max_size {
            // 简单清理：移除一半条目
            let keys: Vec<_> = cache.keys().cloned().collect();
            for key in keys.iter().take(keys.len() / 2) {
                cache.remove(key);
            }
        }

        let entry = CacheEntry {
            value,
            level: CacheLevel::L2,
            created_at: Instant::now(),
            expires_at: Instant::now() + self.ttl,
            access_count: 1,
        };
        cache.insert(key, entry);
    }

    fn remove(&self, key: &K) -> Option<V> {
        self.cache.write().remove(key).map(|e| e.value)
    }

    fn len(&self) -> usize {
        self.cache.read().len()
    }

    fn clear(&self) {
        self.cache.write().clear();
    }
}

/// 多级缓存
pub struct MultiLevelCache<K, V> {
    l1: L1Cache<K, V>,
    l2: Option<L2Cache<K, V>>,
    stats: RwLock<CacheStats>,
    config: MultiLevelCacheConfig,
}

impl<K: Eq + Hash + Clone + Send + Sync, V: Clone + Send + Sync> MultiLevelCache<K, V> {
    /// 创建新的多级缓存
    pub fn new(config: MultiLevelCacheConfig) -> Self {
        let l1 = L1Cache::new(config.l1_size, config.l1_ttl);
        let l2 = if config.enable_l2 {
            Some(L2Cache::new(config.l2_size, config.l2_ttl))
        } else {
            None
        };

        Self {
            l1,
            l2,
            stats: RwLock::new(CacheStats::new()),
            config,
        }
    }

    /// 获取值
    pub fn get(&self, key: &K) -> Option<V> {
        // 先查 L1
        if let Some(value) = self.l1.get(key) {
            self.stats.write().record_hit(CacheLevel::L1);
            debug!("L1 cache hit");
            return Some(value);
        }

        // 再查 L2
        if let Some(ref l2) = self.l2 {
            if let Some(value) = l2.get(key) {
                // 回填 L1
                self.l1.put(key.clone(), value.clone());
                self.stats.write().record_hit(CacheLevel::L2);
                debug!("L2 cache hit, backfilled to L1");
                return Some(value);
            }
        }

        self.stats.write().record_miss();
        debug!("Cache miss");
        None
    }

    /// 设置值
    pub fn put(&self, key: K, value: V) {
        self.l1.put(key.clone(), value.clone());
        
        if let Some(ref l2) = self.l2 {
            l2.put(key, value);
        }
        
        debug!("Value cached at all levels");
    }

    /// 删除值
    pub fn remove(&self, key: &K) -> Option<V> {
        let l1_value = self.l1.remove(key);
        
        if let Some(ref l2) = self.l2 {
            let _ = l2.remove(key);
        }
        
        l1_value
    }

    /// 获取或计算
    pub fn get_or_insert<F>(&self, key: K, factory: F) -> V
    where
        F: FnOnce() -> V,
    {
        if let Some(value) = self.get(&key) {
            return value;
        }

        let value = factory();
        self.put(key, value.clone());
        value
    }

    /// 批量获取
    pub fn get_batch(&self, keys: &[K]) -> HashMap<K, V> {
        let mut results = HashMap::new();
        
        for key in keys {
            if let Some(value) = self.get(key) {
                results.insert(key.clone(), value);
            }
        }
        
        results
    }

    /// 批量设置
    pub fn put_batch(&self, entries: HashMap<K, V>) {
        for (key, value) in entries {
            self.put(key, value);
        }
    }

    /// 获取统计
    pub fn stats(&self) -> CacheStats {
        self.stats.read().clone()
    }

    /// 清理过期条目
    pub fn cleanup(&self) -> usize {
        let l1_cleaned = self.l1.cleanup_expired();
        info!("Cleaned {} expired entries from L1", l1_cleaned);
        l1_cleaned
    }

    /// 清空缓存
    pub fn clear(&self) {
        self.l1.clear();
        if let Some(ref l2) = self.l2 {
            l2.clear();
        }
        
        let mut stats = self.stats.write();
        *stats = CacheStats::new();
        
        info!("Cache cleared");
    }

    /// 获取缓存大小
    pub fn size(&self) -> HashMap<CacheLevel, usize> {
        let mut sizes = HashMap::new();
        sizes.insert(CacheLevel::L1, self.l1.len());
        
        if let Some(ref l2) = self.l2 {
            sizes.insert(CacheLevel::L2, l2.len());
        }
        
        sizes
    }

    /// 预热缓存
    pub fn warm_up(&self, entries: HashMap<K, V>) {
        info!("Warming up cache with {} entries", entries.len());
        
        for (key, value) in entries {
            self.l1.put(key.clone(), value.clone());
            
            if let Some(ref l2) = self.l2 {
                l2.put(key, value);
            }
        }
        
        info!("Cache warm-up complete");
    }
}

/// 缓存键生成器
pub struct CacheKeyGenerator;

impl CacheKeyGenerator {
    /// 为查询生成缓存键
    pub fn for_query(query: &str, context: Option<&str>) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        query.hash(&mut hasher);
        if let Some(ctx) = context {
            ctx.hash(&mut hasher);
        }
        
        format!("query:{}", hasher.finish())
    }

    /// 为技能结果生成缓存键
    pub fn for_skill(skill_name: &str, params: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        skill_name.hash(&mut hasher);
        params.hash(&mut hasher);
        
        format!("skill:{}:{}", skill_name, hasher.finish())
    }

    /// 为 LLM 响应生成缓存键
    pub fn for_llm(prompt: &str, model: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        prompt.hash(&mut hasher);
        model.hash(&mut hasher);
        
        format!("llm:{}:{}", model, hasher.finish())
    }
}

/// 智能缓存 - 带预测和预加载
pub struct SmartCache<K, V> {
    cache: Arc<MultiLevelCache<K, V>>,
    access_pattern: RwLock<HashMap<K, Vec<Instant>>>,
    predictions: RwLock<HashMap<K, Vec<K>>>, // 基于访问模式预测下一个可能访问的键
}

impl<K: Eq + Hash + Clone + Send + Sync, V: Clone + Send + Sync> SmartCache<K, V> {
    pub fn new(config: MultiLevelCacheConfig) -> Self {
        Self {
            cache: Arc::new(MultiLevelCache::new(config)),
            access_pattern: RwLock::new(HashMap::new()),
            predictions: RwLock::new(HashMap::new()),
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        // 记录访问模式
        let mut patterns = self.access_pattern.write();
        patterns
            .entry(key.clone())
            .or_insert_with(Vec::new)
            .push(Instant::now());
        
        // 限制历史记录大小
        if let Some(history) = patterns.get_mut(key) {
            if history.len() > 100 {
                history.remove(0);
            }
        }
        
        drop(patterns);

        // 预加载预测的数据
        self.preload_predictions(key);

        self.cache.get(key)
    }

    pub fn put(&self, key: K, value: V) {
        self.cache.put(key, value);
    }

    /// 预加载预测的数据
    fn preload_predictions(&self, current_key: &K) {
        let predictions = self.predictions.read();
        if let Some(predicted_keys) = predictions.get(current_key) {
            for key in predicted_keys.iter().take(3) {
                // 异步预加载（这里简化处理）
                if self.cache.get(key).is_none() {
                    debug!("Preloading predicted key");
                }
            }
        }
    }

    /// 更新预测模型
    pub fn update_predictions(&self) {
        let patterns = self.access_pattern.read();
        let mut predictions = self.predictions.write();
        predictions.clear();

        // 简单的序列预测：如果 A 经常被 B 跟随，则预测 B
        for (key, history) in patterns.iter() {
            if history.len() < 2 {
                continue;
            }

            // 这里可以实现更复杂的预测算法
            // 简化版：基于时间接近度
            let recent: Vec<_> = history.iter().rev().take(10).collect();
            
            // 查找在相似时间访问的其他键
            for (other_key, other_history) in patterns.iter() {
                if other_key == key {
                    continue;
                }

                let common_times: Vec<_> = other_history
                    .iter()
                    .filter(|t| recent.iter().any(|rt| rt.duration_since(**t).as_secs() < 60))
                    .collect();

                if common_times.len() >= 3 {
                    predictions
                        .entry(key.clone())
                        .or_insert_with(Vec::new)
                        .push(other_key.clone());
                }
            }
        }

        info!("Updated predictions for {} keys", predictions.len());
    }

    pub fn stats(&self) -> CacheStats {
        self.cache.stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_level_cache() {
        let config = MultiLevelCacheConfig::default();
        let cache: MultiLevelCache<String, i32> = MultiLevelCache::new(config);
        
        cache.put("key".to_string(), 42);
        assert_eq!(cache.get(&"key".to_string()), Some(42));
        
        let stats = cache.stats();
        assert!(stats.hit_rate() > 0.0);
    }

    #[test]
    fn test_cache_key_generator() {
        let key1 = CacheKeyGenerator::for_query("hello world", None);
        let key2 = CacheKeyGenerator::for_query("hello world", None);
        assert_eq!(key1, key2);

        let key3 = CacheKeyGenerator::for_query("hello world", Some("context"));
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_cache_stats() {
        let mut stats = CacheStats::new();
        stats.record_hit(CacheLevel::L1);
        stats.record_hit(CacheLevel::L1);
        stats.record_miss();
        
        assert_eq!(stats.hit_rate(), 2.0 / 3.0);
    }
}
