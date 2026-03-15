//! FusionAdapter Unit Tests
//!
//! Comprehensive test suite for the Fusion Memory Adapter.

#[cfg(test)]
mod tests {
    use super::super::*;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::test;

    /// Helper to create a test FusionConfig
    async fn create_test_config() -> (Arc<FusionConfig>, TempDir) {
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
  - Category: testing

## Immutable Rules

- **Test Rule**: Never break tests
  - Reason: Tests ensure quality
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
- **language**: zh-CN

## Communication Style

- **Tone**: friendly
- **Detail Level**: moderate
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
        
        (Arc::new(config), temp_dir)
    }

    #[test]
    async fn test_adapter_creation_fusion_only() {
        let (config, _temp_dir) = create_test_config().await;
        
        let adapter = FusionAdapter::new_fusion_only(config)
            .await
            .expect("Failed to create adapter");
        
        assert_eq!(adapter.config.migration_mode, MigrationMode::FusionOnly);
        assert!(adapter.legacy_manager().is_none());
    }

    #[test]
    async fn test_session_creation() {
        let (config, _temp_dir) = create_test_config().await;
        let adapter = FusionAdapter::new_fusion_only(config)
            .await
            .expect("Failed to create adapter");
        
        let session_id = "test-session-001";
        let session = adapter
            .get_or_create_session(session_id)
            .await
            .expect("Failed to create session");
        
        assert_eq!(session.session_id(), session_id);
        
        // Verify session is stored in map
        let stats = adapter.stats().await;
        assert_eq!(stats.mapped_sessions, 1);
    }

    #[test]
    async fn test_session_reuse() {
        let (config, _temp_dir) = create_test_config().await;
        let adapter = FusionAdapter::new_fusion_only(config)
            .await
            .expect("Failed to create adapter");
        
        let session_id = "test-session-002";
        
        // Create session first time
        let session1 = adapter
            .get_or_create_session(session_id)
            .await
            .expect("Failed to create session");
        
        // Get same session second time
        let session2 = adapter
            .get_or_create_session(session_id)
            .await
            .expect("Failed to get session");
        
        // Should be the same session
        assert_eq!(session1.session_id(), session2.session_id());
        
        // Should still only have one mapped session
        let stats = adapter.stats().await;
        assert_eq!(stats.mapped_sessions, 1);
    }

    #[test]
    async fn test_message_handling() {
        let (config, _temp_dir) = create_test_config().await;
        let adapter = FusionAdapter::new_fusion_only(config)
            .await
            .expect("Failed to create adapter");
        
        let session_id = "test-session-003";
        
        // Add messages
        adapter
            .add_user_message(session_id, "Hello, test!")
            .await
            .expect("Failed to add user message");
        
        adapter
            .add_assistant_message(session_id, "Hello! How can I help?")
            .await
            .expect("Failed to add assistant message");
        
        // Get context
        let context = adapter
            .get_context(session_id)
            .await
            .expect("Failed to get context");
        
        // Should have system message + 2 messages
        assert!(context.len() >= 2);
        
        // Verify message count
        let session = adapter
            .get_or_create_session(session_id)
            .await
            .expect("Failed to get session");
        
        let message_count = session.message_count().await;
        assert_eq!(message_count, 2);
    }

    #[test]
    async fn test_system_prompt_generation() {
        let (config, _temp_dir) = create_test_config().await;
        let adapter = FusionAdapter::new_fusion_only(config)
            .await
            .expect("Failed to create adapter");
        
        let session_id = "test-session-004";
        
        let prompt = adapter
            .get_enriched_system_prompt(session_id)
            .await
            .expect("Failed to generate system prompt");
        
        // Verify prompt contains expected content
        assert!(prompt.contains("TestAgent"));
        assert!(prompt.contains("Core Values"));
        assert!(prompt.contains("Test Value"));
    }

    #[test]
    async fn test_memory_recording() {
        let (config, _temp_dir) = create_test_config().await;
        let adapter = FusionAdapter::new_fusion_only(config)
            .await
            .expect("Failed to create adapter");
        
        let session_id = "test-session-005";
        
        // Record a memory
        adapter
            .record_memory(
                "User prefers dark mode".to_string(),
                "preferences".to_string(),
                session_id,
            )
            .await
            .expect("Failed to record memory");
        
        // Search for memories
        let memories = adapter
            .search_memories(10)
            .await
            .expect("Failed to search memories");
        
        assert_eq!(memories.len(), 1);
        assert!(memories[0].content.contains("dark mode"));
    }

    #[test]
    async fn test_user_fact_management() {
        let (config, _temp_dir) = create_test_config().await;
        let adapter = FusionAdapter::new_fusion_only(config)
            .await
            .expect("Failed to create adapter");
        
        // Add a fact
        adapter
            .add_user_fact(
                "User is a software engineer".to_string(),
                "profession".to_string(),
                0.9,
            )
            .await
            .expect("Failed to add fact");
        
        // Verify by checking profile
        let user_guard = adapter.fusion_system().user().await;
        let profile = user_guard.get_profile().await;
        
        assert!(!profile.facts.is_empty());
        assert!(profile.facts[0].content.contains("software engineer"));
    }

    #[test]
    async fn test_session_end() {
        let (config, _temp_dir) = create_test_config().await;
        let adapter = FusionAdapter::new_fusion_only(config)
            .await
            .expect("Failed to create adapter");
        
        let session_id = "test-session-006";
        
        // Create session
        adapter
            .get_or_create_session(session_id)
            .await
            .expect("Failed to create session");
        
        assert_eq!(adapter.stats().await.mapped_sessions, 1);
        
        // End session
        adapter
            .end_session(session_id)
            .await
            .expect("Failed to end session");
        
        // Session should be removed from map
        assert_eq!(adapter.stats().await.mapped_sessions, 0);
    }

    #[test]
    async fn test_process_message_pipeline() {
        let (config, _temp_dir) = create_test_config().await;
        let adapter = FusionAdapter::new_fusion_only(config)
            .await
            .expect("Failed to create adapter");
        
        let session_id = "test-session-007";
        let input = "I prefer dark mode and use Python for programming";
        
        let (messages, info) = adapter
            .process_message(session_id, input)
            .await
            .expect("Failed to process message");
        
        // Should have system message + user message
        assert!(!messages.is_empty());
        
        // Info should contain session details
        assert!(info.contains("Session"));
        
        // Verify memories were extracted
        let memories = adapter
            .search_memories(10)
            .await
            .expect("Failed to search memories");
        
        // Should have extracted preferences
        assert!(!memories.is_empty());
    }

    #[test]
    async fn test_concurrent_sessions() {
        let (config, _temp_dir) = create_test_config().await;
        let adapter = Arc::new(
            FusionAdapter::new_fusion_only(config)
                .await
                .expect("Failed to create adapter")
        );
        
        let mut handles = vec![];
        
        // Create 10 concurrent sessions
        for i in 0..10 {
            let adapter_clone = adapter.clone();
            let handle = tokio::spawn(async move {
                let session_id = format!("concurrent-session-{}", i);
                adapter_clone
                    .get_or_create_session(&session_id)
                    .await
                    .expect("Failed to create session");
                
                adapter_clone
                    .add_user_message(&session_id, &format!("Message {}", i))
                    .await
                    .expect("Failed to add message");
            });
            handles.push(handle);
        }
        
        // Wait for all to complete
        for handle in handles {
            handle.await.expect("Task failed");
        }
        
        // Verify all sessions were created
        assert_eq!(adapter.stats().await.mapped_sessions, 10);
    }

    #[test]
    async fn test_tool_invocation() {
        let (config, _temp_dir) = create_test_config().await;
        let adapter = FusionAdapter::new_fusion_only(config)
            .await
            .expect("Failed to create adapter");
        
        // List tools
        let tools = adapter.tools().list_tools();
        
        // Should have at least the built-in tools
        assert!(!tools.is_empty());
        
        // Try to invoke a tool (may fail if tool not available, but should not panic)
        let result = adapter
            .invoke_tool("memory_search", serde_json::json!({"query": "test"}))
            .await;
        
        // Result may be Ok or Err depending on tool availability
        // but should not panic
    }

    #[test]
    async fn test_maintenance() {
        let (config, _temp_dir) = create_test_config().await;
        let adapter = FusionAdapter::new_fusion_only(config)
            .await
            .expect("Failed to create adapter");
        
        // Run maintenance
        let report = adapter
            .maintenance()
            .await
            .expect("Failed to run maintenance");
        
        // Maintenance should complete without error
        // Specific results depend on system state
    }

    #[test]
    async fn test_export_to_markdown() {
        let (config, temp_dir) = create_test_config().await;
        let adapter = FusionAdapter::new_fusion_only(config)
            .await
            .expect("Failed to create adapter");
        
        // Add some data
        adapter
            .add_user_fact("Test fact".to_string(), "test".to_string(), 0.8)
            .await
            .expect("Failed to add fact");
        
        // Export
        let export_path = temp_dir.path().join("export");
        tokio::fs::create_dir(&export_path).await.unwrap();
        
        adapter
            .export_to_markdown(&export_path)
            .await
            .expect("Failed to export");
        
        // Verify files were created
        assert!(export_path.join("USER.md").exists());
        assert!(export_path.join("MEMORY.md").exists());
    }
}
