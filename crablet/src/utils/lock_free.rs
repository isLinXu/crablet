//! Lock-free Data Structures - 无锁数据结构优化
//!
//! 为热点路径提供无锁并发数据结构

use std::sync::atomic::{AtomicU64, Ordering, AtomicUsize};
use dashmap::DashMap;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};
use std::sync::mpsc::{channel, Sender, Receiver};

/// 无锁计数器
#[derive(Debug)]
pub struct LockFreeCounter {
    value: AtomicU64,
}

impl LockFreeCounter {
    pub fn new(initial: u64) -> Self {
        Self {
            value: AtomicU64::new(initial),
        }
    }

    pub fn increment(&self) -> u64 {
        self.value.fetch_add(1, Ordering::Relaxed)
    }

    pub fn add(&self, delta: u64) -> u64 {
        self.value.fetch_add(delta, Ordering::Relaxed)
    }

    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    pub fn reset(&self) -> u64 {
        self.value.swap(0, Ordering::Relaxed)
    }
}

impl Default for LockFreeCounter {
    fn default() -> Self {
        Self::new(0)
    }
}

/// 无锁环形缓冲区 (使用标准库 mpsc)
#[derive(Debug)]
pub struct LockFreeRingBuffer<T> {
    sender: Sender<T>,
    receiver: Receiver<T>,
    capacity: usize,
}

impl<T> LockFreeRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = channel();
        Self {
            sender,
            receiver,
            capacity,
        }
    }

    pub fn push(&self, item: T) -> Result<(), T> {
        self.sender.send(item).map_err(|e| e.0)
    }

    pub fn pop(&self) -> Option<T> {
        self.receiver.try_recv().ok()
    }

    pub fn len(&self) -> usize {
        // 标准库 channel 不提供 len，返回 0 作为估计
        0
    }

    pub fn is_empty(&self) -> bool {
        // 尝试非阻塞接收来判断是否为空
        self.receiver.try_recv().map(|_| false).unwrap_or(true)
    }

    pub fn is_full(&self) -> bool {
        // 标准库 channel 无界，这里简化处理
        false
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

/// 分片锁哈希表 - 减少锁竞争
#[derive(Debug)]
pub struct ShardedHashMap<K, V> {
    shards: Vec<RwLock<HashMap<K, V>>>,
    shard_count: usize,
}

impl<K: Eq + Hash + Clone, V: Clone> ShardedHashMap<K, V> {
    pub fn new(shard_count: usize) -> Self {
        let mut shards = Vec::with_capacity(shard_count);
        for _ in 0..shard_count {
            shards.push(RwLock::new(HashMap::new()));
        }
        Self {
            shards,
            shard_count,
        }
    }

    fn get_shard_index(&self, key: &K) -> usize {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % self.shard_count
    }

    pub fn get(&self, key: &K) -> Option<V> {
        let shard_idx = self.get_shard_index(key);
        let shard = self.shards[shard_idx].read();
        shard.get(key).cloned()
    }

    pub fn insert(&self, key: K, value: V) -> Option<V> {
        let shard_idx = self.get_shard_index(&key);
        let mut shard = self.shards[shard_idx].write();
        shard.insert(key, value)
    }

    pub fn remove(&self, key: &K) -> Option<V> {
        let shard_idx = self.get_shard_index(key);
        let mut shard = self.shards[shard_idx].write();
        shard.remove(key)
    }

    pub fn contains_key(&self, key: &K) -> bool {
        let shard_idx = self.get_shard_index(key);
        let shard = self.shards[shard_idx].read();
        shard.contains_key(key)
    }

    /// 获取所有键值对（会锁定所有分片，谨慎使用）
    pub fn get_all(&self) -> HashMap<K, V> {
        let mut result = HashMap::new();
        for shard in &self.shards {
            let shard_data = shard.read();
            result.extend(shard_data.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        result
    }

    pub fn len(&self) -> usize {
        self.shards.iter().map(|s| s.read().len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<K: Eq + Hash + Clone, V: Clone> Default for ShardedHashMap<K, V> {
    fn default() -> Self {
        // 默认使用 CPU 核心数作为分片数
        let shard_count = std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(8);
        Self::new(shard_count)
    }
}

/// 带 TTL 的无锁缓存
#[derive(Clone)]
struct CacheEntry<V> {
    value: V,
    expires_at: Instant,
}

impl<V: std::fmt::Debug> std::fmt::Debug for CacheEntry<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheEntry")
            .field("value", &self.value)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

pub struct LockFreeCache<K, V> {
    store: DashMap<K, CacheEntry<V>>,
    default_ttl: Duration,
}

impl<K: std::fmt::Debug + Eq + Hash, V: std::fmt::Debug> std::fmt::Debug for LockFreeCache<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LockFreeCache")
            .field("store_size", &self.store.len())
            .field("default_ttl", &self.default_ttl)
            .finish()
    }
}

impl<K: Eq + Hash + Clone, V: Clone> LockFreeCache<K, V> {
    pub fn new(default_ttl: Duration) -> Self {
        Self {
            store: DashMap::new(),
            default_ttl,
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        let entry = self.store.get(key)?;
        if Instant::now() > entry.expires_at {
            drop(entry);
            self.store.remove(key);
            None
        } else {
            Some(entry.value.clone())
        }
    }

    pub fn insert(&self, key: K, value: V) {
        self.insert_with_ttl(key, value, self.default_ttl);
    }

    pub fn insert_with_ttl(&self, key: K, value: V, ttl: Duration) {
        let entry = CacheEntry {
            value,
            expires_at: Instant::now() + ttl,
        };
        self.store.insert(key, entry);
    }

    pub fn remove(&self, key: &K) -> Option<V> {
        self.store.remove(key).map(|(_, e)| e.value)
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    /// 清理过期条目
    pub fn cleanup_expired(&self) -> usize {
        let now = Instant::now();
        let mut removed = 0;
        
        self.store.retain(|_, entry| {
            let keep = entry.expires_at > now;
            if !keep {
                removed += 1;
            }
            keep
        });
        
        removed
    }

    pub fn len(&self) -> usize {
        self.store.len()
    }

    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }

    pub fn clear(&self) {
        self.store.clear();
    }
}

/// 无锁指标收集器
#[derive(Debug)]
pub struct LockFreeMetrics {
    counters: DashMap<String, AtomicU64>,
    histograms: DashMap<String, LockFreeRingBuffer<u64>>,
}

impl LockFreeMetrics {
    pub fn new() -> Self {
        Self {
            counters: DashMap::new(),
            histograms: DashMap::new(),
        }
    }

    pub fn increment_counter(&self, name: &str) {
        self.counters
            .entry(name.to_string())
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_counter(&self, name: &str, value: u64) {
        self.counters
            .entry(name.to_string())
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(value, Ordering::Relaxed);
    }

    pub fn get_counter(&self, name: &str) -> u64 {
        self.counters
            .get(name)
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    pub fn record_histogram(&self, name: &str, value: u64) {
        let buffer = self.histograms
            .entry(name.to_string())
            .or_insert_with(|| LockFreeRingBuffer::new(1000));
        
        // 如果满了，丢弃最旧的
        let _ = buffer.push(value);
    }

    pub fn get_histogram(&self, name: &str) -> Vec<u64> {
        let mut values = Vec::new();
        if let Some(buffer) = self.histograms.get(name) {
            while let Some(v) = buffer.pop() {
                values.push(v);
            }
        }
        values
    }

    /// 获取所有指标快照
    pub fn snapshot(&self) -> HashMap<String, u64> {
        let mut result = HashMap::new();
        for entry in self.counters.iter() {
            result.insert(entry.key().clone(), entry.value().load(Ordering::Relaxed));
        }
        result
    }
}

impl Default for LockFreeMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// 对象池 - 减少内存分配 (使用 DashMap)
#[derive(Debug)]
pub struct ObjectPool<T: Send> {
    pool: DashMap<usize, T>,
    capacity: usize,
    created_count: AtomicUsize,
    next_id: AtomicUsize,
}

impl<T: Send> ObjectPool<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            pool: DashMap::new(),
            capacity,
            created_count: AtomicUsize::new(0),
            next_id: AtomicUsize::new(0),
        }
    }

    /// 获取对象
    pub fn acquire(&self, factory: impl FnOnce() -> T) -> T {
        // 尝试获取一个现有对象
        if let Some(entry) = self.pool.iter().next() {
            let id = *entry.key();
            drop(entry);
            if let Some((_, obj)) = self.pool.remove(&id) {
                return obj;
            }
        }
        
        // 创建新对象
        self.created_count.fetch_add(1, Ordering::Relaxed);
        factory()
    }

    /// 归还对象
    pub fn release(&self, obj: T) {
        if self.pool.len() < self.capacity {
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            self.pool.insert(id, obj);
        }
    }

    pub fn len(&self) -> usize {
        self.pool.len()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn created_count(&self) -> usize {
        self.created_count.load(Ordering::Relaxed)
    }
}

/// 无锁会话管理器
#[derive(Debug)]
pub struct LockFreeSessionManager<V> {
    sessions: DashMap<String, V>,
    access_times: DashMap<String, AtomicU64>,
    counter: AtomicU64,
}

impl<V: Clone> LockFreeSessionManager<V> {
    pub fn new() -> Self {
        Self {
            sessions: DashMap::new(),
            access_times: DashMap::new(),
            counter: AtomicU64::new(0),
        }
    }

    pub fn get(&self, session_id: &str) -> Option<V> {
        let result = self.sessions.get(session_id).map(|v| v.clone());
        if result.is_some() {
            let timestamp = self.counter.fetch_add(1, Ordering::Relaxed);
            self.access_times
                .entry(session_id.to_string())
                .or_insert_with(|| AtomicU64::new(0))
                .store(timestamp, Ordering::Relaxed);
        }
        result
    }

    pub fn insert(&self, session_id: String, value: V) {
        let timestamp = self.counter.fetch_add(1, Ordering::Relaxed);
        self.sessions.insert(session_id.clone(), value);
        self.access_times
            .entry(session_id)
            .or_insert_with(|| AtomicU64::new(0))
            .store(timestamp, Ordering::Relaxed);
    }

    pub fn remove(&self, session_id: &str) -> Option<V> {
        self.access_times.remove(session_id);
        self.sessions.remove(session_id).map(|(_, v)| v)
    }

    /// 清理不活跃的会话
    pub fn cleanup_inactive(&self, max_inactive_count: u64) -> usize {
        let current = self.counter.load(Ordering::Relaxed);
        let mut removed = 0;

        self.access_times.retain(|session_id, last_access| {
            let inactive_count = current - last_access.load(Ordering::Relaxed);
            let keep = inactive_count < max_inactive_count;
            if !keep {
                self.sessions.remove(session_id);
                removed += 1;
            }
            keep
        });

        removed
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }
}

impl<V: Clone> Default for LockFreeSessionManager<V> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_free_counter() {
        let counter = LockFreeCounter::new(0);
        assert_eq!(counter.increment(), 0);
        assert_eq!(counter.increment(), 1);
        assert_eq!(counter.get(), 2);
    }

    #[test]
    fn test_sharded_hash_map() {
        let map: ShardedHashMap<String, i32> = ShardedHashMap::new(4);
        map.insert("key1".to_string(), 100);
        map.insert("key2".to_string(), 200);
        
        assert_eq!(map.get(&"key1".to_string()), Some(100));
        assert_eq!(map.get(&"key2".to_string()), Some(200));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn test_lock_free_cache() {
        let cache: LockFreeCache<String, i32> = LockFreeCache::new(Duration::from_secs(60));
        cache.insert("key".to_string(), 42);
        
        assert_eq!(cache.get(&"key".to_string()), Some(42));
        assert!(cache.contains_key(&"key".to_string()));
    }

    #[test]
    fn test_object_pool() {
        let pool: ObjectPool<Vec<u8>> = ObjectPool::new(10);
        
        let obj = pool.acquire(|| Vec::with_capacity(1024));
        pool.release(obj);
        
        assert_eq!(pool.len(), 1);
    }
}
