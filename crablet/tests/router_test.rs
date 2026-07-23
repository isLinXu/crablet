use anyhow::Result;
use async_trait::async_trait;
use crablet::cognitive::llm::LlmClient;
use crablet::cognitive::router::CognitiveRouter;
use crablet::cognitive::system2::System2;
use crablet::config::Config;
use crablet::events::EventBus;
use crablet::plugins::Plugin;
use crablet::skills::{SkillManifest, SkillTrigger, SkillType};
use crablet::types::Message;
use serde_json::json;
use std::sync::{Arc, Mutex};

fn test_config() -> Config {
    Config::for_test()
}

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

    async fn chat_complete_with_tools(
        &self,
        _messages: &[Message],
        _tools: &[serde_json::Value],
    ) -> Result<Message> {
        self.calls
            .lock()
            .unwrap()
            .push("chat_complete_with_tools".to_string());
        Ok(Message::new("assistant", &self.response))
    }

    fn model_name(&self) -> &str {
        "mock-router-test"
    }
}

struct TestPlugin {
    name: &'static str,
    response: std::result::Result<String, String>,
}

#[async_trait]
impl Plugin for TestPlugin {
    fn name(&self) -> &str {
        self.name
    }

    fn description(&self) -> &str {
        "Router trigger test plugin"
    }

    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    async fn execute(&self, _command: &str, _args: serde_json::Value) -> Result<String> {
        match &self.response {
            Ok(output) => Ok(output.clone()),
            Err(error) => Err(anyhow::anyhow!(error.clone())),
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

fn plugin_skill_type(plugin: TestPlugin, trigger: SkillTrigger) -> (String, SkillType) {
    let name = plugin.name().to_string();
    let manifest = SkillManifest {
        name: name.clone(),
        description: "Intent trigger test skill".to_string(),
        version: "1.0.0".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "additionalProperties": true
        }),
        entrypoint: "plugin".to_string(),
        env: Default::default(),
        requires: vec![],
        runtime: None,
        dependencies: None,
        resources: None,
        permissions: vec![],
        conflicts: vec![],
        min_crablet_version: None,
        author: None,
        triggers: vec![trigger],
    };

    let plugin: Arc<Box<dyn Plugin>> = Arc::new(Box::new(plugin) as Box<dyn Plugin>);
    (name.clone(), SkillType::Plugin(manifest, plugin))
}

#[tokio::test]
async fn test_router_system1() {
    let event_bus = Arc::new(EventBus::new(100));
    let config = test_config();

    // We don't need real LLM for System 1 test
    let llm = Arc::new(MockLlm::new("I am System 2"));
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
    let config = test_config();

    let mock_llm = MockLlm::new("System 2 Response");
    let calls = mock_llm.calls.clone();

    let sys2 = System2::with_client(Arc::new(mock_llm), event_bus.clone()).await;

    let router = CognitiveRouter::with_system2_async(&config, None, sys2, event_bus.clone()).await;

    // Force Cloud System 2
    let (response, _) = router
        .process("[FORCE_CLOUD] complex query", "test_s2")
        .await
        .unwrap();

    assert_eq!(response, "System 2 Response");

    // Check if LLM was called (via System 2)
    // Note: System 2 calls chat_complete or chat_complete_with_tools depending on ReAct/Planner
    // ReAct engine usually calls chat_complete_with_tools
    let c = calls.lock().unwrap();
    assert!(!c.is_empty(), "System 2 LLM should be called");
}

#[tokio::test]
async fn router_executes_intent_trigger_before_cognitive_routing() {
    let event_bus = Arc::new(EventBus::new(100));
    let config = test_config();

    let mock_llm = MockLlm::new("System 2 should not run");
    let calls = mock_llm.calls.clone();
    let sys2 = System2::with_client(Arc::new(mock_llm), event_bus.clone()).await;

    let router = CognitiveRouter::with_system2_async(&config, None, sys2, event_bus.clone()).await;

    {
        let mut registry = router.shared_skills.write().await;
        let (name, skill_type) = plugin_skill_type(
            TestPlugin {
                name: "intent_coder",
                response: Ok("skill:intent_coder".to_string()),
            },
            SkillTrigger::Intent {
                intent: "coding".to_string(),
                confidence_threshold: 0.7,
            },
        );
        registry.insert_skill(name, skill_type);
    }

    let engine = {
        let registry = router.shared_skills.read().await;
        Arc::new(registry.build_trigger_engine())
    };
    let router = router.with_skill_trigger_engine(engine);

    let (response, traces) = router
        .process(
            "Please implement a Rust function for fibonacci",
            "intent_trigger",
        )
        .await
        .unwrap();

    assert_eq!(response, "skill:intent_coder");
    assert!(traces[0].thought.contains("via intent trigger"));
    assert!(
        calls.lock().unwrap().is_empty(),
        "intent trigger should bypass cognitive routing when the skill succeeds"
    );
}

#[tokio::test]
async fn router_falls_back_to_cognitive_routing_when_skill_execution_fails() {
    let event_bus = Arc::new(EventBus::new(100));
    let config = test_config();

    let mock_llm = MockLlm::new("fallback:cognitive-route");
    let calls = mock_llm.calls.clone();
    let sys2 = System2::with_client(Arc::new(mock_llm), event_bus.clone()).await;

    let router = CognitiveRouter::with_system2_async(&config, None, sys2, event_bus.clone()).await;

    {
        let mut registry = router.shared_skills.write().await;
        let (name, skill_type) = plugin_skill_type(
            TestPlugin {
                name: "broken_intent_coder",
                response: Err("boom".to_string()),
            },
            SkillTrigger::Intent {
                intent: "coding".to_string(),
                confidence_threshold: 0.7,
            },
        );
        registry.insert_skill(name, skill_type);
    }

    let engine = {
        let registry = router.shared_skills.read().await;
        Arc::new(registry.build_trigger_engine())
    };
    let router = router.with_skill_trigger_engine(engine);

    let (response, _traces) = router
        .process(
            "Implement a Rust function that sorts numbers",
            "intent_fallback",
        )
        .await
        .unwrap();

    assert_eq!(response, "fallback:cognitive-route");
    assert!(
        !calls.lock().unwrap().is_empty(),
        "fallback path should continue into cognitive routing after skill failure"
    );
}
