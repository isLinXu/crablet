use anyhow::Result;
use async_trait::async_trait;
use crablet::cognitive::llm::LlmClient;
use crablet::cognitive::router::CognitiveRouter;
use crablet::cognitive::system2::System2;
use crablet::config::Config;
use crablet::events::EventBus;
use crablet::memory::fusion::{
    daily_logs::DailyLogsConfig,
    layer_soul::{AgentIdentityConfig, CoreValue, ImmutableRule, SoulConfig, SoulMetadataConfig},
    layer_tools::ToolsConfig,
    layer_user::{Preference, UserCommunication, UserConfig},
    weaver::SemanticMemoryConfig,
    FusionConfig, MemoryConfig, WorkingMemoryConfig,
};
use crablet::types::Message;
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;

struct MockLlm {
    response: String,
}

#[async_trait]
impl LlmClient for MockLlm {
    async fn chat_complete(&self, _messages: &[Message]) -> Result<String> {
        Ok(self.response.clone())
    }

    async fn chat_complete_with_tools(
        &self,
        _messages: &[Message],
        _tools: &[serde_json::Value],
    ) -> Result<Message> {
        Ok(Message::new("assistant", &self.response))
    }

    fn model_name(&self) -> &str {
        "mock-fusion-test"
    }
}

fn test_config() -> Config {
    Config::for_test()
}

fn build_fusion_config(temp_dir: &TempDir) -> Arc<FusionConfig> {
    let user_storage = temp_dir.path().join("user");
    let daily_logs = temp_dir.path().join("daily_logs");

    Arc::new(FusionConfig {
        soul: SoulConfig {
            identity: AgentIdentityConfig {
                name: "Crablet".to_string(),
                description: "A fusion-memory test agent".to_string(),
                role: "assistant".to_string(),
                version: "1.0.0".to_string(),
            },
            core_values: vec![CoreValue {
                name: "User First".to_string(),
                description: "Prioritize useful help".to_string(),
                priority: 10,
                category: "ethics".to_string(),
            }],
            immutable_rules: vec![ImmutableRule {
                rule: "Do not harm the user".to_string(),
                reason: Some("Safety first".to_string()),
            }],
            metadata: SoulMetadataConfig {
                created_at: "2026-01-01".to_string(),
                updated_at: "2026-01-01".to_string(),
                author: "test".to_string(),
            },
        },
        tools: ToolsConfig {
            available_tools: vec![],
            permissions: vec![],
            tool_chains: vec![],
        },
        user: UserConfig {
            user_id: "test-user".to_string(),
            name: "Test User".to_string(),
            storage_path: user_storage.to_string_lossy().to_string(),
            preferences: HashMap::<String, Preference>::new(),
            communication: UserCommunication {
                tone: "friendly".to_string(),
                detail_level: "moderate".to_string(),
                languages: vec!["en".to_string()],
                format_preference: "markdown".to_string(),
            },
        },
        memory: MemoryConfig {
            working: WorkingMemoryConfig {
                max_tokens: 1024,
                capacity_messages: 32,
            },
            daily_logs: DailyLogsConfig {
                enabled: true,
                storage_path: daily_logs.to_string_lossy().to_string(),
                context_window_days: 7,
                auto_extract_memories: true,
            },
            semantic: SemanticMemoryConfig {
                backend: "memory".to_string(),
                enabled: true,
            },
        },
    })
}

#[tokio::test]
async fn router_process_creates_and_updates_fusion_session() {
    let temp_dir = TempDir::new().unwrap();
    let event_bus = Arc::new(EventBus::new(100));
    let config = test_config();
    let sys2 = System2::with_client(
        Arc::new(MockLlm {
            response: "fusion-response".to_string(),
        }),
        event_bus.clone(),
    )
    .await;

    let router = CognitiveRouter::with_system2_async(&config, None, sys2, event_bus.clone())
        .await
        .with_fusion_memory(build_fusion_config(&temp_dir))
        .await
        .unwrap();

    let session_id = "fusion-session-process";
    let (response, _traces) = router
        .process(
            "[FORCE_CLOUD] Explain the current memory routing design",
            session_id,
        )
        .await
        .unwrap();

    assert_eq!(response, "fusion-response");

    let fusion = router
        .fusion_memory
        .as_ref()
        .expect("fusion memory enabled");
    let session = fusion
        .get_session(session_id)
        .expect("process should create a fusion session");
    let messages = session.get_messages().await;

    assert_eq!(messages.len(), 3, "system + user + assistant");
    assert_eq!(messages[0].role, "system");
    assert_eq!(
        messages[1].text().unwrap_or_default(),
        "Explain the current memory routing design"
    );
    assert_eq!(messages[2].text().unwrap_or_default(), "fusion-response");
}
