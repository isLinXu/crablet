use crablet::memory::working::WorkingMemory;
#[cfg(feature = "knowledge")]
use crablet::memory::episodic::EpisodicMemory;
#[cfg(feature = "knowledge")]
use tokio::time::{sleep, Duration};

#[test]
fn test_working_memory_basic() {
    let mut wm = WorkingMemory::new(10, 1000);
    
    wm.add_message("user", "Hello");
    wm.add_message("assistant", "Hi there");
    
    let context = wm.get_context();
    assert_eq!(context.len(), 2);
    assert_eq!(context[0].role, "user");
    assert_eq!(context[1].role, "assistant");
}

#[test]
fn test_working_memory_compression_limit_messages() {
    // Capacity 3 messages
    let mut wm = WorkingMemory::new(3, 10000);
    
    // 1. System message (usually first)
    wm.add_message("system", "You are a bot");
    
    // 2. User/Assistant exchange
    wm.add_message("user", "1");
    wm.add_message("assistant", "2");
    
    // 3. Overflow
    wm.add_message("user", "3");
    
    wm.add_message("assistant", "4");
    wm.add_message("user", "5");
    wm.add_message("assistant", "6");
    
    let context = wm.get_context();
    // Verify system message is kept
    assert_eq!(context[0].role, "system");
    // Verify recent messages are kept
    assert_eq!(context.last().unwrap().text().unwrap(), "6");
}

#[test]
fn test_working_memory_token_limit() {
    // Max tokens very small
    let mut wm = WorkingMemory::new(10, 50); // 50 tokens is small
    
    wm.add_message("system", "System");
    
    // Add a long message
    let long_text = "word ".repeat(20); // ~20-30 tokens
    wm.add_message("user", &long_text);
    
    // Add another long message
    wm.add_message("assistant", &long_text);
    
    // Add third
    wm.add_message("user", "short");
    
    // Add more messages to force truncation logic if it respects preserve_recent.
    
    wm.add_message("assistant", "short");
    wm.add_message("user", "short");
    
    let context = wm.get_context();
    assert_eq!(context[0].role, "system");
}

#[cfg(feature = "knowledge")]
#[tokio::test]
async fn test_episodic_memory_sqlite() {
    // Use in-memory SQLite
    let db_url = "sqlite::memory:";
    
    let mem = EpisodicMemory::new(db_url).await.expect("Failed to create episodic memory");
    
    let session_id = "test-session-1";
    
    // 1. Save message directly (auto-create session)
    mem.save_message(session_id, "user", "Hello history").await.expect("Save failed");
    
    // Sleep to ensure timestamp difference (since we use seconds resolution)
    sleep(Duration::from_secs(1)).await;

    // 2. Retrieve
    let history = mem.get_history(session_id, 10).await.expect("Get failed");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].text().unwrap(), "Hello history");
    
    // 3. Add more
    mem.save_message(session_id, "assistant", "Hi back").await.expect("Save failed");
    
    let history = mem.get_history(session_id, 10).await.expect("Get failed");
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].role, "user");
    assert_eq!(history[1].role, "assistant");
    
    // 4. Limit
    let history = mem.get_history(session_id, 1).await.expect("Get failed");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].role, "assistant"); 
}
