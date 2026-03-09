use std::sync::Arc;
use std::path::PathBuf;
use dashmap::DashMap;
use tokio::task::JoinHandle;
use std::time::{Duration, Instant};
use crate::memory::episodic::EpisodicMemory;
use crate::memory::working::WorkingMemory;
use crate::memory::shared::{SharedBlackboard, CrossAgentMessage};
use crate::memory::core::{CoreMemory, CoreMemoryBlock};
use crate::agent::AgentRole;
use tracing::{info, warn};
use crate::error::Result;

#[cfg(feature = "knowledge")]
use crate::memory::consolidator::MemoryConsolidator;

use tokio::sync::RwLock;

use moka::future::Cache as MokaCache;

pub struct MemoryManager {
    pub episodic: Option<Arc<EpisodicMemory>>,
    pub working_memories: Arc<DashMap<String, Arc<RwLock<WorkingMemory>>>>,
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
    max_memory_entries: usize,
    #[allow(dead_code)]
    memory_ttl: Duration,
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
        let working_memories = Arc::new(DashMap::new());
        let cleaner = Self::start_ttl_cleaner(working_memories.clone(), ttl, clean_interval);
        
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
        
        let l1_cache = MokaCache::builder()
            .max_capacity(100)
            .time_to_live(Duration::from_secs(300))
            .build();
            
        Self {
            episodic,
            working_memories,
            l1_cache,
            blackboard: SharedBlackboard::new(),
            mailbox: Arc::new(DashMap::new()),
            core_memory,
            core_memory_path,
            last_activity: Arc::new(RwLock::new(Instant::now())),
            #[cfg(feature = "knowledge")]
            consolidator: None,
            _ttl_cleaner: Some(cleaner),
            max_memory_entries: max_entries,
            memory_ttl: ttl,
        }
    }

    fn start_ttl_cleaner(memories: Arc<DashMap<String, Arc<RwLock<WorkingMemory>>>>, ttl: Duration, interval: Duration) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                let now = Instant::now();
                let initial_count = memories.len();
                
                // Collect keys to remove to avoid holding locks too long
                let mut to_remove = Vec::new();
                for entry in memories.iter() {
                    let wm = entry.value().read().await;
                    if now.duration_since(wm.last_accessed) >= ttl {
                        to_remove.push(entry.key().clone());
                    }
                }
                
                for key in to_remove {
                    memories.remove(&key);
                }

                let cleaned_count = initial_count - memories.len();
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

        // 2. Check Working Memories (L2)
        if let Some(wm_arc) = self.working_memories.get(session_id) {
            let wm_arc = wm_arc.clone();
            self.l1_cache.insert(session_id.to_string(), wm_arc.clone()).await;
            
            let wm_arc_clone = wm_arc.clone();
            tokio::spawn(async move {
                let mut wm = wm_arc_clone.write().await;
                wm.last_accessed = Instant::now();
            });
            return wm_arc;
        }

        // Evict if full (LRU-ish)
        if self.working_memories.len() >= self.max_memory_entries {
             // Find oldest accessed (Note: this is still O(N) but we only do it on creation)
             let mut oldest_key = None;
             let mut oldest_time = Instant::now();
             
             for entry in self.working_memories.iter() {
                 let wm = entry.value().read().await;
                 if wm.last_accessed < oldest_time {
                     oldest_time = wm.last_accessed;
                     oldest_key = Some(entry.key().clone());
                 }
             }
             
             if let Some(key) = oldest_key {
                 self.working_memories.remove(&key);
             }
        }

        // Determine capacity based on role
        let (capacity, max_tokens) = match role {
            Some(AgentRole::Researcher) => (5, 4000),
            Some(AgentRole::Coder) => (20, 16000),
            Some(AgentRole::Reviewer) => (10, 8000),
            Some(AgentRole::Custom(_)) => (10, 8000), // Default
            _ => (10, 8000), // Default
        };

        // Get or Create
        self.working_memories.entry(session_id.to_string())
            .or_insert_with(|| Arc::new(RwLock::new(WorkingMemory::new(capacity, max_tokens))))
            .value()
            .clone()
    }

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
        
        // Persist changes
        self.persist_core_memory(&core).await;
        
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
            self.persist_core_memory(&core).await;
        }
        
        Ok(replaced)
    }

    /// Clear a Core Memory block
    pub async fn core_memory_clear(&self, block: CoreMemoryBlock) {
        let mut core = self.core_memory.write().await;
        core.clear(block);
        
        self.persist_core_memory(&core).await;
    }

    /// Persist Core Memory to disk
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
        self.persist_core_memory(&core).await;
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
        self.working_memories.len()
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
        
        // Access s1 to make it recent
        let _ = manager.get_or_create_working_memory("s1", None).await;
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Add s3 -> s2 should be evicted (oldest accessed)
        manager.save_message_atomic("s3", "user", "msg3").await.unwrap();

        // Check
        assert!(manager.working_memories.contains_key("s1")); // Kept (accessed)
        assert!(manager.working_memories.contains_key("s3")); // New
        assert!(!manager.working_memories.contains_key("s2")); // Evicted
    }
}
