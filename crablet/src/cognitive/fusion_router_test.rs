//! FusionRouter Unit Tests
//!
//! Comprehensive test suite for the Fusion Cognitive Router.

#[cfg(test)]
mod tests {
    use super::super::*;
    use std::sync::Arc;
    use async_trait::async_trait;
    use crate::types::{Message, Role};
    use crate::error::Result;

    /// Mock cognitive system for testing
    struct MockCognitiveSystem {
        name: String,
        delay_ms: u64,
    }

    impl MockCognitiveSystem {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                delay_ms: 0,
            }
        }

        fn with_delay(name: &str, delay_ms: u64) -> Self {
            Self {
                name: name.to_string(),
                delay_ms,
            }
        }
    }

    #[async_trait]
    impl CognitiveSystem for MockCognitiveSystem {
        async fn process(&self, input: &str, _context: &[Message]) -> Result<(String, Vec<TraceStep>)> {
            // Simulate processing delay
            if self.delay_ms > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(self.delay_ms)).await;
            }

            let response = format!("[{}] Processed: {}", self.name, input);
            let traces = vec![TraceStep {
                system: self.name.clone(),
                action: "process".to_string(),
                input: input.to_string(),
                output: response.clone(),
            }];

            Ok((response, traces))
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    /// Helper to create a test FusionAdapter
    async fn create_test_adapter() -> Arc<FusionAdapter> {
        use crate::memory::fusion::{FusionConfig, adapter::{FusionAdapter, AdapterConfig, MigrationMode}};
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().to_path_buf();

        // Create minimal config files
        let soul_content = r#"---
version: "1.0.0"
---

# Identity

**Name**: TestAgent
**Description**: A test agent
**Role**: assistant

## Core Values

- **Test Value** (Priority: 10)
  - Description: Testing is important
"#;

        tokio::fs::write(workspace_path.join("SOUL.md"), soul_content)
            .await
            .unwrap();

        let user_content = r#"---
user_id: test-user
name: Test User
---

# User Profile

## Preferences

- **theme**: dark
"#;

        tokio::fs::write(workspace_path.join("USER.md"), user_content)
            .await
            .unwrap();

        let tools_content = r#"---
version: "1.0.0"
---

# Available Tools

- name: test_tool
  description: A test tool
  category: test
"#;

        tokio::fs::write(workspace_path.join("TOOLS.md"), tools_content)
            .await
            .unwrap();

        let config = FusionConfig::from_workspace(&workspace_path)
            .await
            .expect("Failed to load test config");

        let mut adapter_config = AdapterConfig::default();
        adapter_config.migration_mode = MigrationMode::FusionOnly;

        Arc::new(
            FusionAdapter::new(Arc::new(config), None, adapter_config)
                .await
                .expect("Failed to create adapter")
        )
    }

    #[tokio::test]
    async fn test_router_creation() {
        let adapter = create_test_adapter().await;
        let system1 = Arc::new(MockCognitiveSystem::new("System1"));

        let router = FusionRouter::new(adapter, system1, RouterConfig::default());

        assert_eq!(router.name(), "FusionRouter");
    }

    #[tokio::test]
    async fn test_complexity_calculation() {
        let adapter = create_test_adapter().await;
        let system1 = Arc::new(MockCognitiveSystem::new("System1"));

        let router = FusionRouter::new(adapter, system1, RouterConfig::default());

        // Simple input
        let score1 = router.calculate_complexity("Hello");
        assert!(score1 < 0.3, "Simple input should have low complexity");

        // Complex input with analysis keywords
        let score2 = router.calculate_complexity("Can you analyze and compare these approaches?");
        assert!(score2 > 0.4, "Complex input should have higher complexity");

        // Input with tool intent
        let score3 = router.calculate_complexity("Search for information about Rust");
        assert!(score3 > 0.3, "Input with tool intent should have moderate complexity");
    }

    #[tokio::test]
    async fn test_system_selection() {
        let adapter = create_test_adapter().await;
        let system1 = Arc::new(MockCognitiveSystem::new("System1"));
        let system2 = Arc::new(MockCognitiveSystem::new("System2"));
        let system3 = Arc::new(MockCognitiveSystem::new("System3"));

        let config = RouterConfig {
            system2_threshold: 0.6,
            system3_threshold: 0.8,
            ..Default::default()
        };

        let router = FusionRouter::new(adapter, system1, config)
            .with_system2(system2)
            .with_system3(system3);

        // Low complexity -> System1
        let context1 = FusionRoutingContext {
            session_id: "test-1".to_string(),
            input: "Hello".to_string(),
            system_prompt: "Test".to_string(),
            relevant_memories: vec![],
            recent_context: vec![],
            available_tools: vec![],
            complexity_score: 0.3,
        };
        let route1 = router.determine_system(&context1);
        assert!(matches!(route1, Route::System1));

        // Medium complexity -> System2
        let context2 = FusionRoutingContext {
            session_id: "test-2".to_string(),
            input: "Analyze this".to_string(),
            system_prompt: "Test".to_string(),
            relevant_memories: vec![],
            recent_context: vec![],
            available_tools: vec![],
            complexity_score: 0.7,
        };
        let route2 = router.determine_system(&context2);
        assert!(matches!(route2, Route::System2));

        // High complexity -> System3
        let context3 = FusionRoutingContext {
            session_id: "test-3".to_string(),
            input: "Complex analysis".to_string(),
            system_prompt: "Test".to_string(),
            relevant_memories: vec![],
            recent_context: vec![],
            available_tools: vec![],
            complexity_score: 0.9,
        };
        let route3 = router.determine_system(&context3);
        assert!(matches!(route3, Route::System3));
    }

    #[tokio::test]
    async fn test_tool_decision() {
        let adapter = create_test_adapter().await;
        let system1 = Arc::new(MockCognitiveSystem::new("System1"));

        let config = RouterConfig {
            enable_tools: true,
            ..Default::default()
        };

        let router = FusionRouter::new(adapter, system1, config);

        // Input with tool intent
        let context1 = FusionRoutingContext {
            session_id: "test-1".to_string(),
            input: "Search for Rust documentation".to_string(),
            system_prompt: "Test".to_string(),
            relevant_memories: vec![],
            recent_context: vec![],
            available_tools: vec!["web_search".to_string()],
            complexity_score: 0.5,
        };
        assert!(router.should_use_tools(&context1), "Should use tools for search intent");

        // Input without tool intent
        let context2 = FusionRoutingContext {
            session_id: "test-2".to_string(),
            input: "Hello, how are you?".to_string(),
            system_prompt: "Test".to_string(),
            relevant_memories: vec![],
            recent_context: vec![],
            available_tools: vec!["web_search".to_string()],
            complexity_score: 0.2,
        };
        assert!(!router.should_use_tools(&context2), "Should not use tools for greeting");

        // No tools available
        let context3 = FusionRoutingContext {
            session_id: "test-3".to_string(),
            input: "Search for something".to_string(),
            system_prompt: "Test".to_string(),
            relevant_memories: vec![],
            recent_context: vec![],
            available_tools: vec![],
            complexity_score: 0.5,
        };
        assert!(!router.should_use_tools(&context3), "Should not use tools if none available");
    }

    #[tokio::test]
    async fn test_basic_processing() {
        let adapter = create_test_adapter().await;
        let system1 = Arc::new(MockCognitiveSystem::new("System1"));

        let router = FusionRouter::new(adapter, system1, RouterConfig::default());

        let (response, traces) = router.process("Hello", &[])
            .await
            .expect("Failed to process");

        assert!(response.contains("System1"));
        assert!(response.contains("Processed"));
        assert!(!traces.is_empty());
    }

    #[tokio::test]
    async fn test_routing_context_building() {
        let adapter = create_test_adapter().await;
        let system1 = Arc::new(MockCognitiveSystem::new("System1"));

        let router = FusionRouter::new(adapter.clone(), system1, RouterConfig::default());

        let context = router.build_context("test-session", "Test input")
            .await
            .expect("Failed to build context");

        assert_eq!(context.session_id, "test-session");
        assert_eq!(context.input, "Test input");
        assert!(!context.system_prompt.is_empty());
        assert!(context.complexity_score >= 0.0 && context.complexity_score <= 1.0);
    }

    #[tokio::test]
    async fn test_session_aware_router() {
        let adapter = create_test_adapter().await;
        let system1 = Arc::new(MockCognitiveSystem::new("System1"));

        let router = FusionRouter::new(adapter, system1, RouterConfig::default());
        let session_router = SessionFusionRouter::new(router);

        // Start session
        session_router.start_session("test-session".to_string())
            .await
            .expect("Failed to start session");

        // Process multiple messages
        let (resp1, _) = session_router.process_in_session("Message 1")
            .await
            .expect("Failed to process message 1");

        let (resp2, _) = session_router.process_in_session("Message 2")
            .await
            .expect("Failed to process message 2");

        assert!(!resp1.is_empty());
        assert!(!resp2.is_empty());

        // End session
        session_router.end_session()
            .await
            .expect("Failed to end session");
    }

    #[tokio::test]
    async fn test_concurrent_processing() {
        let adapter = create_test_adapter().await;
        let system1 = Arc::new(MockCognitiveSystem::with_delay("System1", 10));

        let router = Arc::new(FusionRouter::new(adapter, system1, RouterConfig::default()));

        let mut handles = vec![];

        // Process multiple requests concurrently
        for i in 0..5 {
            let router_clone = router.clone();
            let handle = tokio::spawn(async move {
                router_clone.process(&format!("Request {}", i), &[])
                    .await
                    .expect("Failed to process")
            });
            handles.push(handle);
        }

        // Wait for all to complete
        let results = futures::future::join_all(handles).await;

        for result in results {
            let (response, traces) = result.expect("Task failed");
            assert!(!response.is_empty());
            assert!(!traces.is_empty());
        }
    }

    #[tokio::test]
    async fn test_memory_extraction() {
        let adapter = create_test_adapter().await;
        let system1 = Arc::new(MockCognitiveSystem::new("System1"));

        let config = RouterConfig {
            enable_memory_extraction: true,
            ..Default::default()
        };

        let router = FusionRouter::new(adapter.clone(), system1, config);

        // Process a message that should trigger memory extraction
        let _ = router.process("I prefer dark mode and use Python", &[])
            .await
            .expect("Failed to process");

        // Give time for async extraction
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Check if memories were extracted
        let memories = adapter.search_memories(10)
            .await
            .expect("Failed to search memories");

        // Should have extracted at least one memory
        assert!(!memories.is_empty(), "Should have extracted memories from input");
    }

    #[tokio::test]
    async fn test_router_config_defaults() {
        let config = RouterConfig::default();

        assert_eq!(config.system2_threshold, 0.6);
        assert_eq!(config.system3_threshold, 0.8);
        assert!(config.enable_tools);
        assert!(config.enable_memory_extraction);
        assert_eq!(config.max_tool_calls, 5);
    }

    #[tokio::test]
    async fn test_empty_input_handling() {
        let adapter = create_test_adapter().await;
        let system1 = Arc::new(MockCognitiveSystem::new("System1"));

        let router = FusionRouter::new(adapter, system1, RouterConfig::default());

        let (response, traces) = router.process("", &[])
            .await
            .expect("Should handle empty input");

        assert!(!response.is_empty());
        assert!(!traces.is_empty());
    }

    #[tokio::test]
    async fn test_long_input_handling() {
        let adapter = create_test_adapter().await;
        let system1 = Arc::new(MockCognitiveSystem::new("System1"));

        let router = FusionRouter::new(adapter, system1, RouterConfig::default());

        let long_input = "a".repeat(10000);
        let (response, traces) = router.process(&long_input, &[])
            .await
            .expect("Should handle long input");

        assert!(!response.is_empty());

        // Complexity should be high for long input
        let complexity = router.calculate_complexity(&long_input);
        assert!(complexity > 0.5, "Long input should have high complexity");
    }
}
