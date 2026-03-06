use anyhow::Result;
use crablet::cognitive::router::CognitiveRouter;
use crablet::cognitive::system2::System2;
use crablet::cognitive::llm::LlmClient;
use crablet::types::Message;
use crablet::events::EventBus;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use crablet::config::Config;

// Mock LLM
struct MockLlm {
    calls: Arc<Mutex<Vec<String>>>,
    response: String,
}

impl MockLlm {
    fn new(response: &str) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            response: response.to_string(),
        }
    }
}

#[async_trait]
impl LlmClient for MockLlm {
    async fn chat_complete(&self, _messages: &[Message]) -> Result<String> {
        self.calls.lock().unwrap().push("chat_complete".to_string());
        Ok(self.response.clone())
    }

    async fn chat_complete_with_tools(&self, _messages: &[Message], _tools: &[serde_json::Value]) -> Result<Message> {
        self.calls.lock().unwrap().push("chat_complete_with_tools".to_string());
        Ok(Message::new("assistant", &self.response))
    }

    fn model_name(&self) -> &str {
        "mock-router-test"
    }
}

#[tokio::test]
async fn test_router_system1() {
    let event_bus = Arc::new(EventBus::new(100));
    let config = Config::default();
    
    // We don't need real LLM for System 1 test
    let llm = Box::new(MockLlm::new("I am System 2"));
    let sys2 = System2::with_client(llm, event_bus.clone()).await;
    
    let router = CognitiveRouter::with_system2_async(&config, None, sys2, event_bus.clone()).await;
    
    // "Hello" should hit System 1
    let (response, traces) = router.process("Hello", "test_s1").await.unwrap();
    
    // System 1 response for "Hello" is usually "你好！..."
    assert!(response.contains("Crablet") || response.contains("你好"));
    assert!(traces[0].thought.contains("System 1"));
}

#[tokio::test]
async fn test_router_system2_force() {
    let event_bus = Arc::new(EventBus::new(100));
    let config = Config::default();
    
    let mock_llm = MockLlm::new("System 2 Response");
    let calls = mock_llm.calls.clone();
    
    let sys2 = System2::with_client(Box::new(mock_llm), event_bus.clone()).await;
    
    let router = CognitiveRouter::with_system2_async(&config, None, sys2, event_bus.clone()).await;
    
    // Force Cloud System 2
    let (response, _) = router.process("[FORCE_CLOUD] complex query", "test_s2").await.unwrap();
    
    assert_eq!(response, "System 2 Response");
    
    // Check if LLM was called (via System 2)
    // Note: System 2 calls chat_complete or chat_complete_with_tools depending on ReAct/Planner
    // ReAct engine usually calls chat_complete_with_tools
    let c = calls.lock().unwrap();
    assert!(!c.is_empty(), "System 2 LLM should be called");
}
