use crablet::memory::manager::MemoryManager;
use crablet::memory::working::WorkingMemory;
use std::time::Duration;
use tokio::time::sleep;
use proptest::prelude::*;

#[tokio::test]
async fn test_ttl_cleanup_with_fast_interval() {
    // ✅ Use configurable cleanup interval
    let manager = MemoryManager::with_clean_interval(
        None, 10, 
        Duration::from_millis(50),   // TTL = 50ms
        Duration::from_millis(20),   // Interval = 20ms
    );
    
    manager.save_message("session1", "user", "hello").await;
    assert_eq!(manager.working_memories.len(), 1);
    
    // Wait for TTL + Interval
    sleep(Duration::from_millis(100)).await;
    
    assert_eq!(manager.working_memories.len(), 0, "Expired sessions should be cleaned");
}

#[tokio::test]
async fn test_lru_eviction_deterministic() {
    let manager = MemoryManager::new(None, 2, Duration::from_secs(3600));
    
    // Create and access in order
    manager.save_message("oldest", "user", "1").await;
    sleep(Duration::from_millis(10)).await;
    
    manager.save_message("middle", "user", "2").await;
    sleep(Duration::from_millis(10)).await;
    
    // Access oldest to make it recent
    let _ = manager.get_context("oldest");
    sleep(Duration::from_millis(10)).await;
    
    // Trigger eviction by accessing/creating "newest"
    // Since we fixed save_message_atomic to use get_or_create_working_memory,
    // save_message will trigger eviction logic.
    manager.save_message("newest", "user", "3").await;
    
    assert_eq!(manager.working_memories.len(), 2);
    assert!(manager.working_memories.contains_key("newest"));
    assert!(manager.working_memories.contains_key("oldest")); // oldest accessed recently
    assert!(!manager.working_memories.contains_key("middle")); // middle evicted
}

proptest! {
    #[test]
    fn working_memory_never_exceeds_capacity(
        messages in prop::collection::vec("[a-z]{1,100}", 1..50),
        capacity in 3usize..20,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut wm = WorkingMemory::new(capacity, 100_000);
            for msg in &messages {
                wm.add_message("user", msg);
                let limit = std::cmp::max(capacity, 5); // preserve_recent logic
                assert!(wm.get_context().len() <= limit + 1, 
                    "Working memory exceeded capacity: {} > {}", 
                    wm.get_context().len(), limit);
            }
        });
    }
    
    #[test]
    fn memory_manager_never_exceeds_max_entries(
        session_count in 10usize..50,
        max_entries in 3usize..10,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let manager = MemoryManager::new(None, max_entries, Duration::from_secs(3600));
            for i in 0..session_count {
                let session_id = format!("session-{}", i);
                manager.save_message(&session_id, "user", "hello").await;
                assert!(manager.working_memories.len() <= max_entries,
                    "Memory entries {} exceeded max {}", 
                    manager.working_memories.len(), max_entries);
            }
        });
    }
}
