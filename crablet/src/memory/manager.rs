use std::sync::Arc;
use std::path::PathBuf;
use dashmap::DashMap;
use tokio::task::JoinHandle;
use std::time::{Duration, Instant};
use parking_lot::Mutex;
use lru::LruCache;
use crate::memory::episodic::EpisodicMemory;
use crate::memory::working::WorkingMemory;
use crate::memory::shared::{SharedBlackboard, CrossAgentMessage};
use crate::memory::core::{CoreMemory, CoreMemoryBlock};
use crate::agent::AgentRole;
use tracing::{info, warn, error};
use crate::error::Result;

#[cfg(feature = "knowledge")]
use crate::memory::consolidator::MemoryConsolidator;

use tokio::sync::RwLock;
use moka::future::Cache as MokaCache;

/// Helper for managing working memory with O(1) LRU eviction
pub struct WorkingMemoryStore {
    data: DashMap<String, Arc<RwLock<WorkingMemory>>>,
    access_order: Mutex<LruCache<String, ()>>,
    max_entries: usize,
}

impl WorkingMemoryStore {
    pub fn new(max_entries: usize) -> Self {
        Self {
            data: DashMap::new(),
            access_order: Mutex::new(LruCache::new(std::num::NonZeroUsize::new(max_entries).unwrap())),
            max_entries,
        }
    }

    pub fn get(&self, key: &str) -> Option<Arc<RwLock<WorkingMemory>>> {
        if let Some(wm) = self.data.get(key) {
            let mut order = self.access_order.lock();
            order.put(key.to_string(), ());
            return Some(wm.clone());
        }
        None
    }

    pub fn insert(&self, key: String, value: Arc<RwLock<WorkingMemory>>) {
        let mut order = self.access_order.lock();
        if self.data.len() >= self.max_entries && !self.data.contains_key(&key) {
            if let Some((evict_key, _)) = order.pop_lru() {
                self.data.remove(&evict_key);
            }
        }
        self.data.insert(key.clone(), value);
        order.put(key, ());
    }

    pub fn remove(&self, key: &str) {
        self.data.remove(key);
        let mut order = self.access_order.lock();
        order.pop(key);
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    pub fn iter(&self) -> dashmap::iter::Iter<'_, String, Arc<RwLock<WorkingMemory>>> {
        self.data.iter()
    }
}

pub struct MemoryManager {
    pub episodic: Option<Arc<EpisodicMemory>>,
    pub working_store: Arc<WorkingMemoryStore>,
    /// L1 Cache for recently accessed working memories
    pub l1_cache: MokaCache<String, Arc<RwLock<WorkingMemory>>>,
    pub blackboard: SharedBlackboard,
    pub mailbox: Arc<DashMap<String, Vec<CrossAgentMessage>>>,
    /// Core Memory - Always visible persistent memory
    pub core_memory: Arc<RwLock<CoreMemory>>,
    /// Path for Core Memory persistence
    core_memory_path: Option<PathBuf>,
    /// Last user activity timestamp (for heartbeat detection)
    last_activity: Arc<RwLock<Instant>>,
    #[cfg(feature = "knowledge")]
    pub consolidator: Option<Arc<MemoryConsolidator>>,
    _ttl_cleaner: Option<JoinHandle<()>>,
    #[allow(dead_code)]
    memory_ttl: Duration,
    _hot_reloader: Option<crate::memory::hot_reload::CoreMemoryHotReloader>,
    pub distributed_sync: Option<Arc<crate::memory::distributed::DistributedCoreMemory>>,
}

impl MemoryManager {
    pub fn new(episodic: Option<Arc<EpisodicMemory>>, max_entries: usize, ttl: Duration) -> Self {
        Self::with_clean_interval(episodic, max_entries, ttl, Duration::from_secs(60))
    }

    #[cfg(feature = "knowledge")]
    pub fn with_consolidator(mut self, consolidator: Arc<MemoryConsolidator>) -> Self {
        self.consolidator = Some(consolidator);
        self
    }

    pub fn with_clean_interval(
        episodic: Option<Arc<EpisodicMemory>>, 
        max_entries: usize, 
        ttl: Duration,
        clean_interval: Duration,
    ) -> Self {
        Self::with_clean_interval_and_core_path(
            episodic,
            max_entries,
            ttl,
            clean_interval,
            None,
        )
    }

    /// Create MemoryManager with custom Core Memory persistence path
    pub fn with_clean_interval_and_core_path(
        episodic: Option<Arc<EpisodicMemory>>, 
        max_entries: usize, 
        ttl: Duration,
        clean_interval: Duration,
        core_memory_path: Option<PathBuf>,
    ) -> Self {
        let working_store = Arc::new(WorkingMemoryStore::new(max_entries));
        let cleaner = Self::start_ttl_cleaner(working_store.clone(), ttl, clean_interval);
        
        // Load or create Core Memory
        let core_memory = if let Some(ref path) = core_memory_path {
            match CoreMemory::load(path) {
                Ok(cm) => Arc::new(RwLock::new(cm)),
                Err(e) => {
                    warn!("Failed to load Core Memory, creating new: {}", e);
                    Arc::new(RwLock::new(CoreMemory::new()))
                }
            }
        } else {
            Arc::new(RwLock::new(CoreMemory::new()))
        };
        
        // Initialize HotReloader if path is provided
        let mut hot_reloader = None;
        let mut distributed_sync = None;
        if let Some(ref path) = core_memory_path {
            let mut reloader = crate::memory::hot_reload::CoreMemoryHotReloader::new(path.clone());
            if let Err(e) = reloader.start_watch(core_memory.clone()) {
                error!("Failed to start Core Memory hot-reloader: {}", e);
            } else {
                hot_reloader = Some(reloader);
            }
            
            distributed_sync = Some(Arc::new(crate::memory::distributed::DistributedCoreMemory::new(
                core_memory.clone(),
                path.clone()
            )));
        }
        
        let l1_cache = MokaCache::builder()
            .max_capacity(100)
            .time_to_live(Duration::from_secs(300))
            .build();
            
        Self {
            episodic,
            working_store,
            l1_cache,
            blackboard: SharedBlackboard::new(),
            mailbox: Arc::new(DashMap::new()),
            core_memory,
            core_memory_path,
            last_activity: Arc::new(RwLock::new(Instant::now())),
            #[cfg(feature = "knowledge")]
            consolidator: None,
            _ttl_cleaner: Some(cleaner),
            memory_ttl: ttl,
            _hot_reloader: hot_reloader,
            distributed_sync,
        }
    }

    fn start_ttl_cleaner(store: Arc<WorkingMemoryStore>, ttl: Duration, interval: Duration) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                let now = Instant::now();
                let initial_count = store.len();
                
                // Collect keys to remove to avoid holding locks too long
                let mut to_remove = Vec::new();
                for entry in store.iter() {
                    let wm = entry.value().read().await;
                    if now.duration_since(wm.last_accessed) >= ttl {
                        to_remove.push(entry.key().clone());
                    }
                }
                
                for key in to_remove {
                    store.remove(&key);
                }

                let cleaned_count = initial_count - store.len();
                if cleaned_count > 0 {
                    info!("MemoryManager: Cleaned {} expired working memories", cleaned_count);
                }
            }
        })
    }

    pub async fn get_or_create_working_memory(&self, session_id: &str, role: Option<&AgentRole>) -> Arc<RwLock<WorkingMemory>> {
        // 1. Check L1 Cache
        if let Some(wm_arc) = self.l1_cache.get(session_id).await {
            let wm_arc_clone = wm_arc.clone();
            tokio::spawn(async move {
                let mut wm = wm_arc_clone.write().await;
                wm.last_accessed = Instant::now();
            });
            return wm_arc;
        }

        // 2. Check Working Store (L2)
        if let Some(wm_arc) = self.working_store.get(session_id) {
            self.l1_cache.insert(session_id.to_string(), wm_arc.clone()).await;
            
            let wm_arc_clone = wm_arc.clone();
            tokio::spawn(async move {
                let mut wm = wm_arc_clone.write().await;
                wm.last_accessed = Instant::now();
            });
            return wm_arc;
        }

        // Determine capacity based on role
        let (capacity, max_tokens) = match role {
            Some(AgentRole::Researcher) => (5, 4000),
            Some(AgentRole::Coder) => (20, 16000),
            Some(AgentRole::Reviewer) => (10, 8000),
            Some(AgentRole::Custom(_)) => (10, 8000), // Default
            _ => (10, 8000), // Default
        };

        // Create New
        let wm_arc = Arc::new(RwLock::new(WorkingMemory::new(capacity, max_tokens)));
        self.working_store.insert(session_id.to_string(), wm_arc.clone());
        self.l1_cache.insert(session_id.to_string(), wm_arc.clone()).await;

        wm_arc
    }

    /// 启动预热: 加载最近活跃的 sessions
    pub async fn warmup(&self, recent_session_ids: &[String]) -> Result<()> {
        let mut handles = Vec::new();
        
        for session_id in recent_session_ids {
            let wm = self.get_or_create_working_memory(session_id, None).await;
            
            // 从 Episodic Memory 预加载历史
            if let Some(episodic) = &self.episodic {
                let wm_clone = wm.clone();
                let sid = session_id.to_string();
                let episodic_clone = episodic.clone();
                
                handles.push(tokio::spawn(async move {
                    if let Ok(history) = episodic_clone.get_context(&sid, 10).await {
                        let mut w = wm_clone.write().await;
                        for msg in history {
                            w.add_full_message(msg);
                        }
                    }
                }));
            }
        }
        
        for h in handles {
            let _ = h.await;
        }
        
        info!("Warmup completed: {} sessions preloaded", recent_session_ids.len());
        Ok(())
    }

    /// Save Message to both Episodic and Working Memory
    pub async fn save_message_atomic(&self, session_id: &str, role: &str, content: &str) -> Result<()> {
        // 1. Transactional Save to Episodic Memory
        if let Some(mem) = &self.episodic {
            mem.save_message_transactional(session_id, role, content).await?;
        }

        // 2. Update Working Memory atomically
        let wm_arc = self.get_or_create_working_memory(session_id, None).await;
        let mut wm = wm_arc.write().await;
        wm.add_message(role, content);
        
        // Trigger consolidation logic
        #[cfg(feature = "knowledge")]
        if let Some(consolidator) = &self.consolidator {
            consolidator.on_message_added(session_id).await;
        }

        Ok(())
    }

    pub async fn save_message(&self, session_id: &str, role: &str, content: &str) {
        if let Err(e) = self.save_message_atomic(session_id, role, content).await {
            warn!("Failed to save message atomically: {}", e);
        }
    }
    
    pub async fn get_context(&self, session_id: &str) -> Vec<crate::types::Message> {
        let wm_arc = self.get_or_create_working_memory(session_id, None).await;
        let wm = wm_arc.read().await;
        wm.get_context()
    }

    // ==================== Core Memory Methods ====================

    /// Get the Core Memory system prompt for LLM injection
    pub async fn get_core_memory_prompt(&self) -> String {
        let core = self.core_memory.read().await;
        core.to_system_prompt()
    }

    /// Get a snapshot of the Core Memory
    pub async fn get_core_memory(&self) -> CoreMemory {
        self.core_memory.read().await.clone()
    }

    /// Append to Core Memory block
    pub async fn core_memory_append(
        &self,
        block: CoreMemoryBlock,
        content: &str,
    ) -> Result<usize> {
        let mut core = self.core_memory.write().await;
        let added = core.append(block, content)?;
        
        // Immediate persistence (Async, non-blocking)
        let path = self.core_memory_path.clone();
        let core_clone = core.clone();
        tokio::spawn(async move {
            if let Some(ref p) = path {
                if let Err(e) = core_clone.save(p) {
                    warn!("Failed to persist Core Memory (async): {}", e);
                }
            }
        });
        
        Ok(added)
    }

    /// Replace content in Core Memory block
    pub async fn core_memory_replace(
        &self,
        block: CoreMemoryBlock,
        old_content: &str,
        new_content: &str,
    ) -> Result<bool> {
        let mut core = self.core_memory.write().await;
        let replaced = core.replace(block, old_content, new_content)?;
        
        if replaced {
            // Immediate persistence (Async, non-blocking)
            let path = self.core_memory_path.clone();
            let core_clone = core.clone();
            tokio::spawn(async move {
                if let Some(ref p) = path {
                    if let Err(e) = core_clone.save(p) {
                        warn!("Failed to persist Core Memory (async): {}", e);
                    }
                }
            });
        }
        
        Ok(replaced)
    }

    /// Clear a Core Memory block
    pub async fn core_memory_clear(&self, block: CoreMemoryBlock) {
        let mut core = self.core_memory.write().await;
        core.clear(block);
        
        // Immediate persistence (Async, non-blocking)
        let path = self.core_memory_path.clone();
        let core_clone = core.clone();
        tokio::spawn(async move {
            if let Some(ref p) = path {
                if let Err(e) = core_clone.save(p) {
                    warn!("Failed to persist Core Memory (async): {}", e);
                }
            }
        });
    }

    /// Persist Core Memory to disk (Sync-ish but internally calls disk I/O)
    #[allow(dead_code)]
    async fn persist_core_memory(&self, core: &CoreMemory) {
        if let Some(ref path) = self.core_memory_path {
            if let Err(e) = core.save(path) {
                warn!("Failed to persist Core Memory: {}", e);
            }
        }
    }

    /// Save Core Memory (public interface)
    pub async fn save_core_memory(&self) -> Result<()> {
        let core = self.core_memory.read().await;
        if let Some(ref path) = self.core_memory_path {
            core.save(path)?;
        }
        Ok(())
    }

    // ==================== Activity Tracking ====================

    /// Update last activity timestamp (for heartbeat detection)
    pub fn touch_activity(&self) {
        if let Ok(mut last) = self.last_activity.try_write() {
            *last = Instant::now();
        }
    }

    /// Get time since last user activity
    pub async fn idle_duration(&self) -> Duration {
        let last = self.last_activity.read().await;
        last.elapsed()
    }

    /// Check if user is idle (no activity for threshold duration)
    pub async fn is_idle(&self, threshold: Duration) -> bool {
        self.idle_duration().await >= threshold
    }

    /// Get count of active working memory sessions
    pub fn active_session_count(&self) -> usize {
        self.working_store.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_memory_manager_concurrency() {
        let manager = Arc::new(MemoryManager::new(None, 100, Duration::from_secs(60)));
        let session_id = "concurrent_session";
        let mut handles = vec![];

        for i in 0..10 {
            let m = manager.clone();
            let session = session_id.to_string();
            handles.push(tokio::spawn(async move {
                m.save_message_atomic(&session, "user", &format!("Message {}", i)).await.unwrap();
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        let wm_arc = manager.get_or_create_working_memory(session_id, None).await;
        let wm = wm_arc.read().await;
        let context = wm.get_context();
        assert_eq!(context.len(), 10);
    }

    #[tokio::test]
    async fn test_memory_manager_eviction() {
        // Max 2 entries
        let manager = Arc::new(MemoryManager::new(None, 2, Duration::from_secs(60)));
        
        // Add 3 sessions
        manager.save_message_atomic("s1", "user", "msg1").await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        manager.save_message_atomic("s2", "user", "msg2").await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Access s1 to make it recent in LRU order
        let _ = manager.working_store.get("s1");
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Add s3 -> s2 should be evicted (oldest accessed)
        manager.save_message_atomic("s3", "user", "msg3").await.unwrap();

        // Check
        assert!(manager.working_store.contains_key("s1")); // Kept (accessed)
        assert!(manager.working_store.contains_key("s3")); // New
        assert!(!manager.working_store.contains_key("s2")); // Evicted
    }
}
