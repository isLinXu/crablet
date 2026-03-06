use std::sync::Arc;
use dashmap::DashMap;
use tokio::task::JoinHandle;
use std::time::{Duration, Instant};
use crate::memory::episodic::EpisodicMemory;
use crate::memory::working::WorkingMemory;
use crate::memory::shared::{SharedBlackboard, CrossAgentMessage};
use crate::agent::AgentRole;
use tracing::{info, warn};
use crate::error::Result;

#[cfg(feature = "knowledge")]
use crate::memory::consolidator::MemoryConsolidator;

pub struct MemoryManager {
    pub episodic: Option<Arc<EpisodicMemory>>,
    pub working_memories: Arc<DashMap<String, WorkingMemory>>,
    pub blackboard: SharedBlackboard,
    pub mailbox: Arc<DashMap<String, Vec<CrossAgentMessage>>>,
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
        let working_memories = Arc::new(DashMap::new());
        let cleaner = Self::start_ttl_cleaner(working_memories.clone(), ttl, clean_interval);
        
        Self {
            episodic,
            working_memories,
            blackboard: SharedBlackboard::new(),
            mailbox: Arc::new(DashMap::new()),
            #[cfg(feature = "knowledge")]
            consolidator: None,
            _ttl_cleaner: Some(cleaner),
            max_memory_entries: max_entries,
            memory_ttl: ttl,
        }
    }

    fn start_ttl_cleaner(memories: Arc<DashMap<String, WorkingMemory>>, ttl: Duration, interval: Duration) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                let now = Instant::now();
                let initial_count = memories.len();
                memories.retain(|_, wm| now.duration_since(wm.last_accessed) < ttl);
                let cleaned_count = initial_count - memories.len();
                if cleaned_count > 0 {
                    info!("MemoryManager: Cleaned {} expired working memories", cleaned_count);
                }
            }
        })
    }

    pub fn get_or_create_working_memory(&self, session_id: &str, role: Option<&AgentRole>) -> WorkingMemory {
        // If exists, return clone (fast path)
        if let Some(mut wm) = self.working_memories.get_mut(session_id) {
            wm.last_accessed = Instant::now();
            return wm.clone();
        }

        // Evict if full (LRU-ish)
        // Use entry API for atomicity where possible, but eviction is separate.
        // We use a simple check-then-act which has a race, but it's acceptable for cache.
        if self.working_memories.len() >= self.max_memory_entries {
             // Find oldest accessed
             if let Some(oldest_key) = self.working_memories.iter()
                .min_by_key(|r| r.value().last_accessed)
                .map(|r| r.key().clone()) 
             {
                 self.working_memories.remove(&oldest_key);
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
            .or_insert_with(|| WorkingMemory::new(capacity, max_tokens))
            .clone()
    }

    pub async fn save_message_atomic(&self, session_id: &str, role: &str, content: &str) -> Result<()> {
        // 1. Transactional Save to Episodic Memory
        if let Some(mem) = &self.episodic {
            mem.save_message_transactional(session_id, role, content).await?;
        }

        // 2. Update Working Memory atomically
        // First try to update existing entry
        let mut updated = false;
        if let Some(mut wm) = self.working_memories.get_mut(session_id) {
            wm.add_message(role, content);
            updated = true;
        }

        // If not found, we need to create it
        if !updated {
            // Eviction Logic (similar to get_or_create)
            if self.working_memories.len() >= self.max_memory_entries {
                 if let Some(oldest_key) = self.working_memories.iter()
                    .min_by_key(|r| r.value().last_accessed)
                    .map(|r| r.key().clone()) 
                 {
                     self.working_memories.remove(&oldest_key);
                 }
            }

            // Create new
            // We assume default capacity since we don't have role info here
            let mut wm = WorkingMemory::new(10, 8000); 
            wm.add_message(role, content);
            
            // Insert (race condition here is minor: if someone inserted in between, we overwrite or we use entry)
            // Use entry to be safe(er)
            self.working_memories.entry(session_id.to_string())
                .and_modify(|w| w.add_message(role, content))
                .or_insert(wm);
        }
        
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
    
    pub fn get_context(&self, session_id: &str) -> Vec<crate::types::Message> {
        let wm = self.get_or_create_working_memory(session_id, None);
        wm.get_context()
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

        let wm = manager.get_or_create_working_memory(session_id, None);
        let context = wm.get_context();
        assert_eq!(context.len(), 10);
    }

    #[tokio::test]
    async fn test_memory_manager_eviction() {
        // Max 2 entries
        let manager = Arc::new(MemoryManager::new(None, 2, Duration::from_secs(60)));
        
        // Add 3 sessions
        manager.save_message_atomic("s1", "user", "msg1").await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        manager.save_message_atomic("s2", "user", "msg2").await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        // Access s1 to make it recent
        let _ = manager.get_or_create_working_memory("s1", None);
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Add s3 -> s2 should be evicted (oldest accessed)
        manager.save_message_atomic("s3", "user", "msg3").await.unwrap();

        // Check
        assert!(manager.working_memories.contains_key("s1")); // Kept (accessed)
        assert!(manager.working_memories.contains_key("s3")); // New
        assert!(!manager.working_memories.contains_key("s2")); // Evicted
    }
}
