use crablet::cognitive::react::ReActEngine;
use crablet::cognitive::llm::LlmClient;
use crablet::skills::SkillRegistry;
use crablet::events::EventBus;
use crablet::types::{Message, ContentPart};
use crablet::plugins::Plugin;
use std::sync::Arc;
use tokio::sync::RwLock;
use anyhow::Result;

struct MockCalculatorPlugin;

#[async_trait::async_trait]
impl Plugin for MockCalculatorPlugin {
    fn name(&self) -> &str { "calculator" }
    fn description(&self) -> &str { "Calculates stuff" }
    async fn initialize(&mut self) -> Result<()> { Ok(()) }
    async fn execute(&self, _command: &str, _args: serde_json::Value) -> Result<String> {
        Ok("Observation: 108".to_string())
    }
    async fn shutdown(&mut self) -> Result<()> { Ok(()) }
}

struct LocalMockClient;

#[async_trait::async_trait]
impl LlmClient for LocalMockClient {
    async fn chat_complete(&self, messages: &[Message]) -> Result<String> {
        // Find if we have the observation in ANY message (Tool output)
        let has_observation = messages.iter().any(|m| {
            m.role == "tool" && m.content.as_ref().map_or(false, |parts| {
                parts.iter().any(|p| match p {
                    ContentPart::Text { text } => text.contains("108"),
                    _ => false
                })
            })
        });

        if has_observation {
             return Ok("The result is 108.".to_string());
        }
        
        // Check for calculation request in User message
        let has_calc_request = messages.iter().any(|m| {
             m.role == "user" && m.content.as_ref().map_or(false, |parts| {
                parts.iter().any(|p| match p {
                    ContentPart::Text { text } => text.contains("Calculate 15 * 7 + 3"),
                    _ => false
                })
            })
        });
        
        if has_calc_request {
            return Ok("Action: use calculator {\"expression\": \"15 * 7 + 3\"}".to_string());
        }

        Ok("I don't know.".to_string())
    }

    async fn chat_complete_with_tools(&self, messages: &[Message], _tools: &[serde_json::Value]) -> Result<Message> {
        let text = self.chat_complete(messages).await?;
        Ok(Message::new("assistant", &text))
    }

    fn model_name(&self) -> &str {
        "mock-react-test"
    }
}

#[tokio::test]
async fn test_react_engine_flow() {
    // 1. Setup
    let llm = Arc::new(Box::new(LocalMockClient) as Box<dyn LlmClient>);
    let skills = Arc::new(RwLock::new(SkillRegistry::new()));
    let event_bus = Arc::new(EventBus::new(100));
    
    // 2. Register Plugin
    {
        let mut registry = skills.write().await;
        registry.register_plugin(Box::new(MockCalculatorPlugin));
    }
    
    let engine = ReActEngine::new(llm, skills.clone(), event_bus);
    
    // 3. Execute
    let initial_context = vec![Message::new("user", "Calculate 15 * 7 + 3")];
    let (response, traces) = engine.execute(&initial_context, 5).await.expect("Execution failed");
    
    // 4. Verify
    assert_eq!(response, "The result is 108.");
    
    // Check traces
    assert!(traces.len() >= 1);
    
    let action_trace = traces.iter().find(|t| t.action.as_deref() == Some("calculator"));
    assert!(action_trace.is_some(), "Should have a calculator action trace");
    let obs = action_trace.unwrap().observation.as_ref().unwrap();
    assert!(obs.contains("108"));
}
