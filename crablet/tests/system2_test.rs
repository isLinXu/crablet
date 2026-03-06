use anyhow::Result;
use crablet::cognitive::system2::System2;
use crablet::cognitive::CognitiveSystem;
use crablet::events::EventBus;
use crablet::cognitive::llm::LlmClient;
use crablet::types::Message;
use async_trait::async_trait;
use std::sync::Arc;
use serde_json::Value;

// Mock LLM Client
struct MockLlmClient;

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn chat_complete(&self, _messages: &[Message]) -> Result<String> {
        Ok("Mock response".to_string())
    }

    async fn chat_complete_with_tools(&self, _messages: &[Message], _tools: &[Value]) -> Result<Message> {
        Ok(Message::new("assistant", "Mock response"))
    }

    async fn chat_complete_with_reasoning(&self, _messages: &[Message]) -> Result<(String, String)> {
        Ok(("Mock thought".to_string(), "Mock response".to_string()))
    }

    fn model_name(&self) -> &str {
        "mock-system2-test"
    }
}

#[tokio::test]
async fn test_system2_async_initialization() -> Result<()> {
    // Setup
    let event_bus = Arc::new(EventBus::new(10));
    
    // Test default initialization
    let sys2 = System2::new(event_bus.clone()).await;
    assert_eq!(sys2.name(), "System 2 (Analytical)");
    
    Ok(())
}

#[tokio::test]
async fn test_system2_with_client_async_initialization() -> Result<()> {
    // Setup
    let event_bus = Arc::new(EventBus::new(10));
    let mock_llm: Box<dyn LlmClient> = Box::new(MockLlmClient);
    
    // Test initialization with custom client
    let sys2 = System2::with_client(mock_llm, event_bus.clone()).await;
    
    // Verify skill registry is initialized and accessible
    let skills = sys2.skills.read().await;
    assert!(skills.len() > 0, "Default plugins should be registered");
    
    Ok(())
}

#[tokio::test]
async fn test_system2_concurrency_safe_access() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(10));
    let sys2 = Arc::new(System2::new(event_bus.clone()).await);
    
    let mut handles = vec![];
    
    // Spawn multiple tasks accessing System 2 concurrently
    for _ in 0..5 {
        let sys2_clone = sys2.clone();
        handles.push(tokio::spawn(async move {
            let skills = sys2_clone.skills.read().await;
            // Just read access to simulate concurrent usage
            skills.len()
        }));
    }
    
    for handle in handles {
        handle.await?;
    }
    
    Ok(())
}
