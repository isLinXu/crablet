#[allow(unused_imports)]
use anyhow::Result;
use crablet::cognitive::system1::System1;
use crablet::cognitive::CognitiveSystem;
use crablet::memory::working::WorkingMemory;
use crablet::skills::SkillRegistry;
// use crablet::types::Message; // Not needed if we use role/content strings

#[tokio::test]
async fn test_system1_exact_match() {
    let sys1 = System1::new();
    
    // 1. Exact Match (Trie)
    let result = sys1.process("hello", &[]).await;
    assert!(result.is_ok(), "System1 should match 'hello' exactly");
    let (response, _) = result.unwrap();
    assert!(response.contains("Crablet"), "Response should contain identity");

    // 2. Exact Match (Alias)
    let result = sys1.process("hi", &[]).await;
    assert!(result.is_ok(), "System1 should match 'hi' (alias)");

    // 3. No Match
    let result = sys1.process("complex philosophical question", &[]).await;
    assert!(result.is_err(), "System1 should NOT match complex query");
}

#[tokio::test]
async fn test_working_memory_operations() {
    // new(capacity_messages: usize, max_tokens: usize)
    let mut memory = WorkingMemory::new(10, 4000); 

    // 1. Add System Message (Important: WorkingMemory preserves index 0)
    memory.add_message("system", "You are a bot");

    // 2. Add Messages
    memory.add_message("user", "Hello");
    memory.add_message("assistant", "Hi there");
    
    assert_eq!(memory.get_context().len(), 3);
    // Use .text() helper and unwrap option
    assert_eq!(memory.get_context()[1].text().unwrap(), "Hello");

    // 3. Clear
    memory.clear();
    assert_eq!(memory.get_context().len(), 0);

    // 4. Overflow (FIFO with System preservation)
    memory.add_message("system", "You are a bot"); // Restore system
    for i in 0..15 {
        memory.add_message("user", &format!("Msg {}", i));
    }
    
    // Capacity 10. System (1) + 9 recent messages?
    // Logic: while len > 10 && len > 5: remove(1)
    // It stops when len == 10.
    // So we have System + 9 messages.
    // The messages added were 0..14 (15 msgs).
    // It removed 6 messages (index 1).
    // Removed: Msg 0, Msg 1, Msg 2, Msg 3, Msg 4, Msg 5.
    // Remaining: Msg 6..14 (9 msgs) + System = 10 total.
    
    let context = memory.get_context();
    assert_eq!(context.len(), 10);
    assert_eq!(context[0].text().unwrap(), "You are a bot");
    assert_eq!(context[1].text().unwrap(), "Msg 6"); // 0..5 dropped
    assert_eq!(context[9].text().unwrap(), "Msg 14");
}

#[tokio::test]
async fn test_tool_registry() {
    let registry = SkillRegistry::new();
    
    // Verify initial state
    assert_eq!(registry.len(), 0);
    assert!(registry.list_skills().is_empty());
    
    // We can't easily register a plugin without implementing one here, 
    // but we can check if it handles empty state correctly.
    let tools = registry.to_tool_definitions();
    assert!(tools.is_empty());
    
    // If we had a mock plugin, we could register it:
    /*
    struct MockPlugin;
    impl Plugin for MockPlugin { ... }
    let mut registry = registry; // Make mutable if needed later
    registry.register_plugin(Box::new(MockPlugin));
    assert_eq!(registry.len(), 1);
    */
}
