use async_trait::async_trait;
use crablet::agent::harness::RetryConfig;
use crablet::agent::harness::{AgentHarnessContext, HarnessConfig, HarnessError};
use crablet::agent::hooks::{Hook, HookContext, HookError, HookPoint, HookRegistry, HookResult};
use crablet::agent::tool_executor::HarnessToolExecutor;
use crablet::plugins::Plugin;
use crablet::skills::SkillRegistry;
use serde_json::Value;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::RwLock;

struct CountingHook {
    point: HookPoint,
    count: Arc<AtomicU32>,
    action: HookResult,
}

impl CountingHook {
    fn new(point: HookPoint) -> Self {
        Self {
            point,
            count: Arc::new(AtomicU32::new(0)),
            action: HookResult::allow(),
        }
    }

    fn with_action(mut self, action: HookResult) -> Self {
        self.action = action;
        self
    }

    fn count(&self) -> u32 {
        self.count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Hook for CountingHook {
    fn name(&self) -> &str {
        "counting-hook"
    }

    fn point(&self) -> HookPoint {
        self.point
    }

    async fn execute(&self, _ctx: &HookContext) -> Result<HookResult, HookError> {
        self.count.fetch_add(1, Ordering::SeqCst);
        Ok(self.action.clone())
    }
}

struct SlowPlugin {
    name: String,
    delay: Duration,
    response: String,
    calls: Arc<AtomicU32>,
}

struct CapturingPlugin {
    name: String,
    response: String,
    calls: Arc<AtomicU32>,
    last_args: Arc<Mutex<Option<Value>>>,
}

struct RetryingModifyHook {
    count: Arc<AtomicU32>,
}

#[async_trait]
impl Plugin for SlowPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "slow test plugin"
    }

    async fn initialize(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn execute(&self, _command: &str, _args: Value) -> anyhow::Result<String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(self.delay).await;
        Ok(self.response.clone())
    }

    async fn shutdown(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}

#[async_trait]
impl Plugin for CapturingPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "capturing test plugin"
    }

    async fn initialize(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn execute(&self, _command: &str, args: Value) -> anyhow::Result<String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        *self.last_args.lock().unwrap() = Some(args);
        Ok(self.response.clone())
    }

    async fn shutdown(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}

#[async_trait]
impl Hook for RetryingModifyHook {
    fn name(&self) -> &str {
        "retrying-modify-hook"
    }

    fn point(&self) -> HookPoint {
        HookPoint::PreToolUse
    }

    async fn execute(&self, _ctx: &HookContext) -> Result<HookResult, HookError> {
        let attempt = self.count.fetch_add(1, Ordering::SeqCst);
        if attempt == 0 {
            let mut result = HookResult::modify(serde_json::json!({"value": "modified"}));
            result.retry = true;
            result.message = Some("retry after modification".to_string());
            Ok(result)
        } else {
            Ok(HookResult::allow())
        }
    }
}

#[tokio::test]
async fn executor_runs_hook_lifecycle() {
    let registry = Arc::new(RwLock::new(SkillRegistry::new()));
    let hook_registry = Arc::new(HookRegistry::new());
    let pre_hook = Arc::new(CountingHook::new(HookPoint::PreToolUse));
    let post_hook = Arc::new(CountingHook::new(HookPoint::PostToolUse));
    hook_registry.register(pre_hook.clone()).await;
    hook_registry.register(post_hook.clone()).await;

    let executor = HarnessToolExecutor::new(registry)
        .with_hook_registry(hook_registry)
        .with_harness(AgentHarnessContext::new(HarnessConfig::default()));

    let result = executor
        .execute("list_resources", serde_json::json!({}))
        .await;

    assert!(result.success);
    assert_eq!(result.output.as_deref(), Some("[]"));
    assert_eq!(pre_hook.count(), 1);
    assert_eq!(post_hook.count(), 1);
}

#[tokio::test]
async fn executor_respects_tool_timeout() {
    let mut registry = SkillRegistry::new();
    let calls = Arc::new(AtomicU32::new(0));
    registry.register_plugin(Box::new(SlowPlugin {
        name: "slow_tool".to_string(),
        delay: Duration::from_millis(30),
        response: "done".to_string(),
        calls: calls.clone(),
    }));

    let executor = HarnessToolExecutor::new(Arc::new(RwLock::new(registry)))
        .with_retry_config(RetryConfig {
            max_retries: 1,
            ..Default::default()
        })
        .with_harness(AgentHarnessContext::new(HarnessConfig {
            tool_timeout: Duration::from_millis(5),
            ..Default::default()
        }));

    let result = executor.execute("slow_tool", serde_json::json!({})).await;

    assert!(!result.success);
    assert!(matches!(result.error_kind, Some(HarnessError::Timeout(_))));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn executor_allows_hook_block() {
    let mut registry = SkillRegistry::new();
    let calls = Arc::new(AtomicU32::new(0));
    registry.register_plugin(Box::new(SlowPlugin {
        name: "blocked_tool".to_string(),
        delay: Duration::from_millis(1),
        response: "never".to_string(),
        calls: calls.clone(),
    }));

    let hook_registry = Arc::new(HookRegistry::new());
    hook_registry
        .register(Arc::new(
            CountingHook::new(HookPoint::PreToolUse)
                .with_action(HookResult::block("blocked by policy")),
        ))
        .await;

    let executor =
        HarnessToolExecutor::new(Arc::new(RwLock::new(registry))).with_hook_registry(hook_registry);

    let result = executor
        .execute("blocked_tool", serde_json::json!({}))
        .await;

    assert!(!result.success);
    assert!(result
        .error
        .as_deref()
        .unwrap_or("")
        .contains("blocked by policy"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn executor_retries_when_modify_hook_requests_retry() {
    let mut registry = SkillRegistry::new();
    let calls = Arc::new(AtomicU32::new(0));
    let last_args = Arc::new(Mutex::new(None));
    registry.register_plugin(Box::new(CapturingPlugin {
        name: "capture_tool".to_string(),
        response: "ok".to_string(),
        calls: calls.clone(),
        last_args: last_args.clone(),
    }));

    let hook_registry = Arc::new(HookRegistry::new());
    let retry_hook = Arc::new(RetryingModifyHook {
        count: Arc::new(AtomicU32::new(0)),
    });
    hook_registry.register(retry_hook.clone()).await;

    let executor = HarnessToolExecutor::new(Arc::new(RwLock::new(registry)))
        .with_hook_registry(hook_registry)
        .with_retry_config(RetryConfig {
            max_retries: 2,
            ..Default::default()
        });

    let result = executor
        .execute("capture_tool", serde_json::json!({"value": "original"}))
        .await;

    assert!(result.success);
    assert_eq!(result.attempts, 2);
    assert_eq!(result.output.as_deref(), Some("ok"));
    assert_eq!(
        result.args,
        serde_json::json!({"value": "modified"}).to_string()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(retry_hook.count.load(Ordering::SeqCst), 2);
    assert_eq!(
        *last_args.lock().unwrap(),
        Some(serde_json::json!({"value": "modified"}))
    );
}

#[tokio::test]
async fn executor_supports_tool_replacement() {
    let mut registry = SkillRegistry::new();
    let original_calls = Arc::new(AtomicU32::new(0));
    let replacement_calls = Arc::new(AtomicU32::new(0));
    registry.register_plugin(Box::new(CapturingPlugin {
        name: "original_tool".to_string(),
        response: "original".to_string(),
        calls: original_calls.clone(),
        last_args: Arc::new(Mutex::new(None)),
    }));
    registry.register_plugin(Box::new(CapturingPlugin {
        name: "replacement_tool".to_string(),
        response: "replacement".to_string(),
        calls: replacement_calls.clone(),
        last_args: Arc::new(Mutex::new(None)),
    }));

    let hook_registry = Arc::new(HookRegistry::new());
    hook_registry
        .register(Arc::new(
            CountingHook::new(HookPoint::PreToolUse).with_action(HookResult::replace(
                "replacement_tool",
                serde_json::json!({"value": "replacement"}),
            )),
        ))
        .await;

    let executor =
        HarnessToolExecutor::new(Arc::new(RwLock::new(registry))).with_hook_registry(hook_registry);

    let result = executor
        .execute("original_tool", serde_json::json!({"value": "original"}))
        .await;

    assert!(result.success);
    assert_eq!(result.tool_name, "replacement_tool");
    assert_eq!(result.attempts, 1);
    assert_eq!(result.output.as_deref(), Some("replacement"));
    assert_eq!(
        result.args,
        serde_json::json!({"value": "replacement"}).to_string()
    );
    assert_eq!(original_calls.load(Ordering::SeqCst), 0);
    assert_eq!(replacement_calls.load(Ordering::SeqCst), 1);
}
