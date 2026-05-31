use anyhow::{bail, Result};
use async_trait::async_trait;
use crablet::agent::harness::{AgentHarnessContext, HarnessConfig, RetryConfig};
use crablet::agent::harness_agent::{
    HarnessAgent, HarnessAgentBuilder, HarnessExecutionProgressSink, HarnessExecutionState,
};
use crablet::agent::harness_manager::{HarnessManager, HarnessStatus};
use crablet::agent::hooks::{Hook, HookContext, HookError, HookPoint, HookRegistry, HookResult};
use crablet::agent::AgentRole;
use crablet::plugins::Plugin;
use crablet::skills::SkillRegistry;
use crablet::types::Message;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

struct EchoPlugin {
    name: String,
    prefix: String,
}

#[async_trait]
impl Plugin for EchoPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "echo test plugin"
    }

    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    async fn execute(&self, _command: &str, args: Value) -> Result<String> {
        let payload = args
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or("missing");
        Ok(format!("{}{}", self.prefix, payload))
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

struct ToolUsingAgent {
    calls: Arc<AtomicU32>,
}

#[async_trait]
impl HarnessAgent for ToolUsingAgent {
    fn name(&self) -> &str {
        "tool-using-agent"
    }

    fn role(&self) -> AgentRole {
        AgentRole::Executor
    }

    fn tools(&self) -> Vec<String> {
        vec!["echo_tool".to_string(), "replacement_tool".to_string()]
    }

    async fn execute_step(
        &self,
        _task: &str,
        context: &[Message],
        step_number: usize,
    ) -> Result<(String, Option<String>, Option<Value>)> {
        self.calls.fetch_add(1, Ordering::SeqCst);

        if step_number == 1 {
            Ok((
                "Need to call a tool".to_string(),
                Some("echo_tool".to_string()),
                Some(json!({"query": "rust"})),
            ))
        } else {
            let observation = context.last().and_then(Message::text).unwrap_or_default();
            Ok((format!("Final answer: {}", observation), None, None))
        }
    }
}

struct FlakyAgent {
    calls: Arc<AtomicU32>,
}

#[async_trait]
impl HarnessAgent for FlakyAgent {
    fn name(&self) -> &str {
        "flaky-agent"
    }

    fn role(&self) -> AgentRole {
        AgentRole::Executor
    }

    fn tools(&self) -> Vec<String> {
        Vec::new()
    }

    async fn execute_step(
        &self,
        _task: &str,
        _context: &[Message],
        _step_number: usize,
    ) -> Result<(String, Option<String>, Option<Value>)> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if call == 0 {
            bail!("transient failure")
        }

        Ok(("Recovered response".to_string(), None, None))
    }
}

struct ResumeCapableAgent {
    calls: Arc<AtomicU32>,
    second_step_attempts: Arc<AtomicU32>,
}

#[async_trait]
impl HarnessAgent for ResumeCapableAgent {
    fn name(&self) -> &str {
        "resume-capable-agent"
    }

    fn role(&self) -> AgentRole {
        AgentRole::Executor
    }

    fn tools(&self) -> Vec<String> {
        vec!["echo_tool".to_string()]
    }

    async fn execute_step(
        &self,
        _task: &str,
        context: &[Message],
        step_number: usize,
    ) -> Result<(String, Option<String>, Option<Value>)> {
        self.calls.fetch_add(1, Ordering::SeqCst);

        match step_number {
            1 => Ok((
                "Need a tool before we can finish".to_string(),
                Some("echo_tool".to_string()),
                Some(json!({"query": "resume"})),
            )),
            2 => {
                if self.second_step_attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                    bail!("resume required")
                }

                let observation = context.last().and_then(Message::text).unwrap_or_default();
                Ok((format!("Resumed answer: {}", observation), None, None))
            }
            _ => bail!("unexpected step {}", step_number),
        }
    }
}

struct ReplaceToolHook;

#[async_trait]
impl Hook for ReplaceToolHook {
    fn name(&self) -> &str {
        "replace-tool-hook"
    }

    fn point(&self) -> HookPoint {
        HookPoint::PreToolUse
    }

    async fn execute(&self, _ctx: &HookContext) -> Result<HookResult, HookError> {
        Ok(HookResult::replace(
            "replacement_tool",
            json!({"query": "hooked"}),
        ))
    }
}

struct RecordingProgressSink {
    states: Arc<Mutex<Vec<HarnessExecutionState>>>,
}

#[async_trait]
impl HarnessExecutionProgressSink for RecordingProgressSink {
    async fn persist(&self, state: HarnessExecutionState) {
        self.states.lock().await.push(state);
    }
}

fn test_config() -> HarnessConfig {
    HarnessConfig {
        max_steps: 4,
        step_timeout: Duration::from_millis(100),
        tool_timeout: Duration::from_millis(100),
        ..Default::default()
    }
}

fn registry_with_echo_tools() -> Arc<RwLock<SkillRegistry>> {
    let mut registry = SkillRegistry::new();
    registry.register_plugin(Box::new(EchoPlugin {
        name: "echo_tool".to_string(),
        prefix: "echo:".to_string(),
    }));
    registry.register_plugin(Box::new(EchoPlugin {
        name: "replacement_tool".to_string(),
        prefix: "replacement:".to_string(),
    }));
    Arc::new(RwLock::new(registry))
}

#[tokio::test]
async fn harness_agent_executes_tool_steps_and_records_trace() {
    let result = HarnessAgentBuilder::new(Arc::new(ToolUsingAgent {
        calls: Arc::new(AtomicU32::new(0)),
    }))
    .with_config(test_config())
    .with_skill_registry(registry_with_echo_tools())
    .execute("look up rust", &[])
    .await
    .unwrap();

    assert!(result.success);
    assert_eq!(result.output, "Final answer: echo:rust");
    assert_eq!(result.metadata.step_count, 2);
    assert_eq!(result.trace.len(), 2);
    assert_eq!(result.trace[0].action.as_deref(), Some("echo_tool"));
    assert_eq!(result.trace[0].observation.as_deref(), Some("echo:rust"));
    assert!(result.trace[1].action.is_none());
}

#[tokio::test]
async fn harness_agent_applies_hook_registry_to_tool_calls() {
    let hook_registry = Arc::new(HookRegistry::new());
    hook_registry.register(Arc::new(ReplaceToolHook)).await;

    let result = HarnessAgentBuilder::new(Arc::new(ToolUsingAgent {
        calls: Arc::new(AtomicU32::new(0)),
    }))
    .with_config(test_config())
    .with_skill_registry(registry_with_echo_tools())
    .with_hook_registry(hook_registry)
    .execute("look up rust", &[])
    .await
    .unwrap();

    assert!(result.success);
    assert_eq!(result.output, "Final answer: replacement:hooked");
    assert_eq!(result.trace[0].action.as_deref(), Some("replacement_tool"));
    assert_eq!(
        result.trace[0].observation.as_deref(),
        Some("replacement:hooked")
    );
}

#[tokio::test]
async fn harness_agent_recovers_from_transient_step_failure() {
    let calls = Arc::new(AtomicU32::new(0));
    let result = HarnessAgentBuilder::new(Arc::new(FlakyAgent {
        calls: calls.clone(),
    }))
    .with_config(test_config())
    .with_retry_config(RetryConfig {
        max_retries: 2,
        ..Default::default()
    })
    .execute("recover", &[])
    .await
    .unwrap();

    assert!(result.success);
    assert_eq!(result.output, "Recovered response");
    assert_eq!(result.metadata.step_count, 1);
    assert_eq!(result.trace.len(), 1);
    assert!(!result.errors.is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn harness_agent_can_use_shared_harness_context() {
    let shared_harness = Arc::new(RwLock::new(AgentHarnessContext::new(test_config())));

    let result = HarnessAgentBuilder::new(Arc::new(ToolUsingAgent {
        calls: Arc::new(AtomicU32::new(0)),
    }))
    .with_config(test_config())
    .with_skill_registry(registry_with_echo_tools())
    .with_shared_harness(shared_harness.clone())
    .execute("look up rust", &[])
    .await
    .unwrap();

    let metadata = shared_harness.read().await.metadata();
    assert!(result.success);
    assert_eq!(metadata.step_count, 2);
    assert_eq!(metadata.step_count, result.metadata.step_count);
    assert_eq!(metadata.tool_call_count, result.metadata.tool_call_count);
}

#[tokio::test]
async fn harness_agent_persists_progress_after_each_step() {
    let recorder = Arc::new(RecordingProgressSink {
        states: Arc::new(Mutex::new(Vec::new())),
    });

    let result = HarnessAgentBuilder::new(Arc::new(ToolUsingAgent {
        calls: Arc::new(AtomicU32::new(0)),
    }))
    .with_config(test_config())
    .with_skill_registry(registry_with_echo_tools())
    .with_progress_sink(recorder.clone())
    .execute("look up rust", &[])
    .await
    .unwrap();

    let states = recorder.states.lock().await.clone();
    assert!(result.success);
    assert_eq!(states.len(), 2);
    assert_eq!(states[0].trace.len(), 1);
    assert_eq!(states[1].trace.len(), 2);
}

#[tokio::test]
async fn harness_manager_runs_agent_and_tracks_completion() {
    let manager = HarnessManager::new();
    let builder = HarnessAgentBuilder::new(Arc::new(ToolUsingAgent {
        calls: Arc::new(AtomicU32::new(0)),
    }))
    .with_config(test_config())
    .with_skill_registry(registry_with_echo_tools());

    let (id, result) = manager
        .run_agent(&builder, "look up rust", &[])
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(result.output, "Final answer: echo:rust");

    let info = manager.get_info(&id).await.unwrap();
    assert!(matches!(info.status, HarnessStatus::Completed));
    assert_eq!(info.step_count, 2);
    assert_eq!(info.error_count, 0);

    let stats = manager.get_stats().await;
    assert_eq!(stats.total_created, 1);
    assert_eq!(stats.total_completed, 1);
}

#[tokio::test]
async fn harness_manager_can_resume_from_saved_execution_state() {
    let manager = HarnessManager::new();
    let step2_attempts = Arc::new(AtomicU32::new(0));
    let builder = HarnessAgentBuilder::new(Arc::new(ResumeCapableAgent {
        calls: Arc::new(AtomicU32::new(0)),
        second_step_attempts: step2_attempts.clone(),
    }))
    .with_config(test_config())
    .with_retry_config(RetryConfig {
        max_retries: 1,
        ..Default::default()
    })
    .with_skill_registry(registry_with_echo_tools());

    let (id, first_result) = manager.run_agent(&builder, "resume me", &[]).await.unwrap();

    assert!(!first_result.success);
    assert_eq!(first_result.trace.len(), 1);

    let failed_info = manager.get_info(&id).await.unwrap();
    assert!(matches!(failed_info.status, HarnessStatus::Failed));
    assert_eq!(
        failed_info
            .execution_state
            .as_ref()
            .map(|state| state.trace.len()),
        Some(1)
    );

    let resumed = manager.resume_agent(&id, &builder).await.unwrap();

    assert!(resumed.success);
    assert_eq!(resumed.output, "Resumed answer: echo:resume");
    assert_eq!(resumed.trace.len(), 2);
    assert_eq!(step2_attempts.load(Ordering::SeqCst), 2);

    let completed_info = manager.get_info(&id).await.unwrap();
    assert!(matches!(completed_info.status, HarnessStatus::Completed));
    assert_eq!(
        completed_info
            .execution_state
            .as_ref()
            .map(|state| state.trace.len()),
        Some(2)
    );
}
