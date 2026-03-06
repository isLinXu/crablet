use crablet::cognitive::router::CognitiveRouter;
use crablet::cognitive::system2::System2;
use crablet::cognitive::llm::LlmClient;
use crablet::types::{Message, ContentPart};
use crablet::events::EventBus;
use async_trait::async_trait;
use anyhow::Result;
use std::sync::{Arc, Mutex};

// Mock LLM Client that captures context
struct SpyLlmClient {
    last_context: Arc<Mutex<Vec<Message>>>,
}

impl SpyLlmClient {
    fn new() -> Self {
        Self {
            last_context: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl LlmClient for SpyLlmClient {
    async fn chat_complete(&self, messages: &[Message]) -> Result<String> {
        let mut context = self.last_context.lock().unwrap();
        *context = messages.to_vec();
        Ok("Mock Response".to_string())
    }

    async fn chat_complete_with_tools(&self, messages: &[Message], _tools: &[serde_json::Value]) -> Result<Message> {
        let content = self.chat_complete(messages).await?;
        Ok(Message::new("assistant", &content))
    }

    fn model_name(&self) -> &str {
        "mock-integration-test"
    }
}

#[tokio::test]
async fn test_system1_hit() {
    let event_bus = Arc::new(EventBus::new(100));
    let config = crablet::config::Config::default();
    let router = CognitiveRouter::new(&config, None, event_bus).await;
    let (response, _) = router.process("hello", "test_session").await.unwrap();
    // System1 greeting logic might change, checking for typical greetings
    assert!(response.contains("Crablet") || response.to_lowercase().contains("hello") || response.to_lowercase().contains("hi"));
}

#[tokio::test]
async fn test_context_retention() {
    // 1. Setup Spy LLM
    let spy_client = SpyLlmClient::new();
    let last_context = spy_client.last_context.clone();
    
    let event_bus = Arc::new(EventBus::new(100));
    let config = crablet::config::Config::default();

    // 2. Setup System 2 with Spy Client
    let sys2 = System2::with_client(Box::new(spy_client), event_bus.clone()).await;
    
    // 3. Setup Router
    let router = CognitiveRouter::with_system2_async(&config, None, sys2, event_bus.clone()).await;

    // 4. Interaction 1: User says something
    // "My name is Alice" is short, but we want to force System 2 or ensure System 1 doesn't catch it.
    // System 1 catches "hello", "hi", "/help", "who are you".
    // "My name is Alice" should go to System 2.
    let _ = router.process("My name is Alice", "test_session").await.unwrap();
    
    // 5. Interaction 2: Ask question (System 2 fallback)
    let _ = router.process("What is my name?", "test_session").await.unwrap();

    // 6. Verify Context
    let context = last_context.lock().unwrap();
    
    // Context should contain:
    // 1. User: My name is Alice
    // 2. Assistant: Mock Response (since it went to System 2 SpyClient)
    // 3. User: What is my name?
    // 4. (Potentially System prompts if injected by Middleware)
    
    // We expect at least the user messages.
    // Let's find the message "My name is Alice"
    
    let has_alice = context.iter().any(|m| {
        m.role == "user" && m.content.as_ref().map_or(false, |parts| {
            parts.iter().any(|p| match p {
                ContentPart::Text { text } => text == "My name is Alice",
                _ => false,
            })
        })
    });
    
    let has_question = context.iter().any(|m| {
        m.role == "user" && m.content.as_ref().map_or(false, |parts| {
            parts.iter().any(|p| match p {
                ContentPart::Text { text } => text == "What is my name?",
                _ => false,
            })
        })
    });

    assert!(has_alice, "Context should contain 'My name is Alice'");
    assert!(has_question, "Context should contain 'What is my name?'");
}
