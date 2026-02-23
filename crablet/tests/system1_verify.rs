use crablet::cognitive::system1::System1;
use crablet::cognitive::CognitiveSystem;

#[tokio::test]
async fn verify_system1_regex() {
    let sys1 = System1::new();
    
    // Test Greeting (Regex: (hello|hi|hey|你好|您好).*)
    let (resp, _) = sys1.process("hello world", &[]).await.unwrap();
    assert!(resp.contains("你好"));

    // Test Case Insensitive
    let (resp, _) = sys1.process("Hi there", &[]).await.unwrap();
    assert!(resp.contains("你好"));

    // Test Help Command
    let (resp, _) = sys1.process("/help", &[]).await.unwrap();
    assert!(resp.contains("Available commands"));
    
    // Test Status
    let (resp, _) = sys1.process("status check", &[]).await.unwrap();
    assert!(resp.contains("ONLINE"));

    // Test Pattern
    // Note: We removed Regex support in Phase 1 and replaced it with Trie + Fuzzy Match.
    // "what time is it" is not in the hardcoded Trie in System 1 unless added.
    // System 1 currently only has "hello", "hi", "hey", "/help", "status", "ping".
    // "what time is it" should now be routed to System 2 (return Err/None from System 1).
    let result = sys1.process("what time is it", &[]).await;
    // assert!(resp.contains("clock")); // This was valid when using Regex
    assert!(result.is_err(), "System 1 should not handle complex queries like 'what time is it'");
    
    // Test No Match
    let result = sys1.process("calculate the mass of sun", &[]).await;
    assert!(result.is_err());
}
