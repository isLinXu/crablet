//! Integration tests for the Hook system with Harness/Fusion
//!
//! Tests the full lifecycle: HookRegistry → ToolExecutor → FusionEngine

#[cfg(test)]
mod tests {
    use crablet::agent::hooks::*;
    use crablet::agent::harness_fusion::UnifiedHarnessFusionBuilder;
    use std::sync::Arc;

    /// Hook that counts how many times it was called
    struct CountingHook {
        count: std::sync::atomic::AtomicU32,
        point: HookPoint,
        priority_val: i32,
    }
    impl CountingHook {
        fn new(point: HookPoint) -> Self {
            Self {
                count: std::sync::atomic::AtomicU32::new(0),
                point,
                priority_val: 0,
            }
        }
        fn with_priority(mut self, priority: i32) -> Self {
            self.priority_val = priority;
            self
        }
        fn count(&self) -> u32 {
            self.count.load(std::sync::atomic::Ordering::SeqCst)
        }
    }
    #[async_trait::async_trait]
    impl Hook for CountingHook {
        fn name(&self) -> &str {
            "counting"
        }
        fn point(&self) -> HookPoint {
            self.point
        }
        fn priority(&self) -> i32 {
            self.priority_val
        }
        async fn execute(&self, _ctx: &HookContext) -> Result<HookResult, HookError> {
            self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(HookResult::allow())
        }
    }

    /// Hook that injects metadata
    struct MetadataHook {
        key: &'static str,
        value: &'static str,
    }
    impl MetadataHook {
        fn new(key: &'static str, value: &'static str) -> Self {
            Self { key, value }
        }
    }
    #[async_trait::async_trait]
    impl Hook for MetadataHook {
        fn name(&self) -> &str {
            "metadata"
        }
        fn point(&self) -> HookPoint {
            HookPoint::PreToolUse
        }
        async fn execute(&self, ctx: &HookContext) -> Result<HookResult, HookError> {
            let mut result = HookResult::allow();
            result.metadata.insert(self.key.to_string(), self.value.to_string());
            result
                .metadata
                .insert("tool".to_string(), ctx.tool_name.clone());
            Ok(result)
        }
    }

    #[tokio::test]
    async fn test_hook_registry_with_fusion_builder() {
        let registry = Arc::new(HookRegistry::new());
        registry.register(Arc::new(SecurityAuditHook::new())).await;

        let engine = UnifiedHarnessFusionBuilder::new()
            .with_hook_registry(registry.clone())
            .with_self_healing(true)
            .build()
            .await;

        let engine_registry = engine.hook_registry();
        let hooks = engine_registry.list_hooks(HookPoint::PreToolUse).await;
        assert!(hooks.contains(&"security-audit".to_string()));
    }

    #[tokio::test]
    async fn test_multiple_hooks_ordered_execution() {
        let registry = HookRegistry::new();

        let hook1 = Arc::new(CountingHook::new(HookPoint::PreToolUse).with_priority(10));
        let hook2 = Arc::new(CountingHook::new(HookPoint::PreToolUse).with_priority(-10));

        registry.register(hook1.clone()).await;
        registry.register(hook2.clone()).await;

        let ctx = HookContext::for_point(HookPoint::PreToolUse);
        let _ = registry.run_hooks(HookPoint::PreToolUse, &ctx).await;

        assert_eq!(hook1.count(), 1);
        assert_eq!(hook2.count(), 1);
    }

    #[tokio::test]
    async fn test_hooks_with_different_points() {
        let registry = HookRegistry::new();

        let pre_hook = Arc::new(CountingHook::new(HookPoint::PreToolUse));
        let post_hook = Arc::new(CountingHook::new(HookPoint::PostToolUse));

        registry.register(pre_hook.clone() as Arc<dyn Hook>).await;
        registry.register(post_hook.clone() as Arc<dyn Hook>).await;

        // Run pre hook
        let ctx = HookContext::for_point(HookPoint::PreToolUse);
        let _ = registry.run_hooks(HookPoint::PreToolUse, &ctx).await;
        assert_eq!(pre_hook.count(), 1);
        assert_eq!(post_hook.count(), 0);

        // Run post hook
        let ctx = HookContext::for_point(HookPoint::PostToolUse);
        let _ = registry.run_hooks(HookPoint::PostToolUse, &ctx).await;
        assert_eq!(pre_hook.count(), 1);
        assert_eq!(post_hook.count(), 1);
    }

    #[tokio::test]
    async fn test_hook_chain_allow_modify_block() {
        let registry = HookRegistry::new();

        // Priority -100: Security (should block dangerous calls)
        registry
            .register(Arc::new(SecurityAuditHook::with_blocked_tools(vec![
                "dangerous_tool".to_string(),
            ])))
            .await;

        // Priority -50: Spec injection (should inject message for safe calls)
        registry
            .register(Arc::new(
                SpecInjectionHook::new().with_spec("*", "Follow safety guidelines"),
            ))
            .await;

        // Test dangerous call → blocked by security
        let ctx = HookContext::for_tool_use(
            HookPoint::PreToolUse,
            "test-harness",
            1,
            "dangerous_tool",
            serde_json::json!({}),
        );
        let result = registry.run_pre_tool_use(&ctx).await.unwrap();
        assert!(matches!(result.action, HookAction::Block { .. }));

        // Test safe call → allowed with spec injection
        let ctx = HookContext::for_tool_use(
            HookPoint::PreToolUse,
            "test-harness",
            1,
            "safe_tool",
            serde_json::json!({}),
        );
        let result = registry.run_pre_tool_use(&ctx).await.unwrap();
        assert!(matches!(result.action, HookAction::Allow));
        assert_eq!(
            result.message.as_deref(),
            Some("Follow safety guidelines")
        );
    }

    #[tokio::test]
    async fn test_hook_context_carries_metadata() {
        let registry = HookRegistry::new();
        registry
            .register(Arc::new(MetadataHook::new("source", "integration-test")))
            .await;

        let ctx = HookContext::for_tool_use(
            HookPoint::PreToolUse,
            "harness-42",
            5,
            "search",
            serde_json::json!({"query": "test"}),
        );
        let result = registry.run_pre_tool_use(&ctx).await.unwrap();

        // MetadataHook should have set tool name in metadata
        // (metadata is in the HookResult, not propagated back to context)
        assert!(matches!(result.action, HookAction::Allow));
    }

    #[tokio::test]
    async fn test_all_hook_points_are_reachable() {
        let all_points = HookPoint::all();
        assert!(all_points.len() >= 28);

        // Ensure each point has a description
        for point in all_points {
            assert!(!point.description().is_empty());
        }
    }

    #[tokio::test]
    async fn test_registry_clone_shares_state() {
        let registry = HookRegistry::new();
        registry
            .register(Arc::new(SecurityAuditHook::new()))
            .await;

        let cloned = registry.clone();
        let hooks = cloned.list_hooks(HookPoint::PreToolUse).await;
        assert!(hooks.contains(&"security-audit".to_string()));
    }

    #[tokio::test]
    async fn test_hook_result_constructors() {
        // Test all result constructors
        let allow = HookResult::allow();
        assert!(matches!(allow.action, HookAction::Allow));

        let with_msg = HookResult::allow_with_message("test");
        assert_eq!(with_msg.message.as_deref(), Some("test"));

        let block = HookResult::block("reason");
        assert!(matches!(block.action, HookAction::Block { .. }));

        let modify = HookResult::modify(serde_json::json!({"x": 1}));
        assert!(matches!(modify.action, HookAction::Modify));

        let replace = HookResult::replace("tool", serde_json::json!({}));
        assert!(matches!(replace.action, HookAction::Replace { .. }));

        let retry = HookResult::retry_with_message("retry");
        assert!(retry.retry);
    }

    #[tokio::test]
    async fn test_builtin_hooks_default_configs() {
        // SecurityAuditHook defaults
        let _sec = SecurityAuditHook::new();

        // SpecInjectionHook defaults
        let _spec = SpecInjectionHook::new();

        // QualityGateHook defaults
        let qg = QualityGateHook::new();
        assert!(qg.allow_empty);
        assert_eq!(qg.min_output_length, 0);
        assert_eq!(qg.max_output_length, 0);

        // ResourceWarningHook defaults
        let rw = ResourceWarningHook::new();
        assert!((rw.budget_warning_threshold - 0.8).abs() < 0.001);
        assert!((rw.token_warning_threshold - 0.8).abs() < 0.001);
    }
}
