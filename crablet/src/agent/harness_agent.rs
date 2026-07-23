//! Harness-aware Agent Trait
//!
//! Provides a trait extension for agents to leverage Harness capabilities:
//! - Integrated lifecycle management
//! - Automatic error handling and retry
//! - Circuit breaker support
//! - Checkpoint and resume
//! - Resource tracking

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};

use super::harness::{
    AgentHarnessContext, ExecutionMetadata, HarnessConfig, HarnessError, RetryConfig,
};
use super::harness_fusion::UnifiedHarnessFusionBuilder;
use super::hooks::HookRegistry;
use super::tool_executor::HarnessToolExecutor;
use super::{Agent, AgentRole};
use crate::skills::SkillRegistry;
use crate::types::{FunctionCall, Message, ToolCall, TraceStep};

/// Result of harness-aware agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessAgentResult {
    pub output: String,
    pub success: bool,
    pub metadata: ExecutionMetadata,
    pub errors: Vec<HarnessError>,
    pub trace: Vec<TraceStep>,
}

impl HarnessAgentResult {
    pub fn success(output: String, metadata: ExecutionMetadata) -> Self {
        Self {
            output,
            success: true,
            metadata,
            errors: Vec::new(),
            trace: Vec::new(),
        }
    }

    pub fn failure(output: String, metadata: ExecutionMetadata, errors: Vec<HarnessError>) -> Self {
        Self {
            output,
            success: false,
            metadata,
            errors,
            trace: Vec::new(),
        }
    }
}

/// Minimal persisted execution state required to resume a harness-aware agent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HarnessExecutionState {
    pub task: String,
    #[serde(default)]
    pub context: Vec<Message>,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub trace: Vec<TraceStep>,
}

impl HarnessExecutionState {
    pub fn new(
        task: impl Into<String>,
        context: &[Message],
        system_prompt: Option<String>,
    ) -> Self {
        Self {
            task: task.into(),
            context: context.to_vec(),
            system_prompt,
            trace: Vec::new(),
        }
    }

    pub fn with_trace(mut self, trace: Vec<TraceStep>) -> Self {
        self.trace = trace;
        self
    }

    pub fn build_messages(&self) -> Vec<Message> {
        let mut messages = Vec::new();
        if let Some(prompt) = &self.system_prompt {
            messages.push(Message::system(prompt.clone()));
        }
        messages.extend(self.context.iter().cloned());
        messages.push(Message::user(self.task.clone()));
        replay_trace_messages(&mut messages, &self.trace);
        messages
    }
}

/// Optional sink that receives replayable execution state after each successful step.
#[async_trait]
pub trait HarnessExecutionProgressSink: Send + Sync {
    async fn persist(&self, state: HarnessExecutionState);
}

struct CompositeHarnessExecutionProgressSink {
    sinks: Vec<Arc<dyn HarnessExecutionProgressSink>>,
}

#[async_trait]
impl HarnessExecutionProgressSink for CompositeHarnessExecutionProgressSink {
    async fn persist(&self, state: HarnessExecutionState) {
        for sink in &self.sinks {
            sink.persist(state.clone()).await;
        }
    }
}

/// Trait for harness-aware agents
#[async_trait]
pub trait HarnessAgent: Send + Sync {
    /// Execute with harness context
    async fn execute_with_harness(
        &self,
        task: &str,
        context: &[Message],
        config: HarnessConfig,
    ) -> Result<HarnessAgentResult> {
        crate::agent::harness_agent::execute_with_harness(self, task, context, config).await
    }

    /// Get the agent's base name
    fn name(&self) -> &str;

    /// Get the agent's role
    fn role(&self) -> AgentRole;

    /// Get available tools
    fn tools(&self) -> Vec<String>;

    /// Execute a single step (to be implemented by subclasses)
    async fn execute_step(
        &self,
        task: &str,
        context: &[Message],
        step_number: usize,
    ) -> Result<(String, Option<String>, Option<serde_json::Value>)>;
}

/// Builder for creating harness-aware agent executions
pub struct HarnessAgentBuilder<A: HarnessAgent> {
    agent: Arc<A>,
    config: HarnessConfig,
    runtime: HarnessAgentRuntime,
    system_prompt: Option<String>,
}

impl<A: HarnessAgent> HarnessAgentBuilder<A> {
    pub fn new(agent: Arc<A>) -> Self {
        Self {
            agent,
            config: HarnessConfig::default(),
            runtime: HarnessAgentRuntime::default(),
            system_prompt: None,
        }
    }

    pub fn with_config(mut self, config: HarnessConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_retry_config(mut self, retry_config: RetryConfig) -> Self {
        self.runtime = self.runtime.with_retry_config(retry_config);
        self
    }

    pub fn with_skill_registry(mut self, skill_registry: Arc<RwLock<SkillRegistry>>) -> Self {
        self.runtime = self.runtime.with_skill_registry(skill_registry);
        self
    }

    pub fn with_hook_registry(mut self, hook_registry: Arc<HookRegistry>) -> Self {
        self.runtime = self.runtime.with_hook_registry(hook_registry);
        self
    }

    pub fn with_shared_harness(mut self, harness: Arc<RwLock<AgentHarnessContext>>) -> Self {
        self.runtime = self.runtime.with_shared_harness(harness);
        self
    }

    pub fn with_adaptive_timeout(mut self, enabled: bool) -> Self {
        self.runtime = self.runtime.with_adaptive_timeout(enabled);
        self
    }

    pub fn with_metrics(mut self, enabled: bool) -> Self {
        self.runtime = self.runtime.with_metrics(enabled);
        self
    }

    pub fn with_system_prompt(mut self, prompt: String) -> Self {
        self.system_prompt = Some(prompt);
        self
    }

    pub fn with_progress_sink(mut self, sink: Arc<dyn HarnessExecutionProgressSink>) -> Self {
        self.runtime = self.runtime.with_progress_sink(sink);
        self
    }

    pub fn with_allowed_tools(mut self, tools: Vec<String>) -> Self {
        self.runtime = self.runtime.with_allowed_tools(tools);
        self
    }

    pub fn max_steps(mut self, steps: usize) -> Self {
        self.config.max_steps = steps;
        self
    }

    pub fn enable_self_reflection(mut self, enabled: bool) -> Self {
        self.config.enable_self_reflection = enabled;
        self
    }

    pub fn config(&self) -> &HarnessConfig {
        &self.config
    }

    pub fn system_prompt(&self) -> Option<&str> {
        self.system_prompt.as_deref()
    }

    pub fn execution_state(&self, task: &str, context: &[Message]) -> HarnessExecutionState {
        HarnessExecutionState::new(task, context, self.system_prompt.clone())
    }

    pub async fn execute(&self, task: &str, context: &[Message]) -> Result<HarnessAgentResult> {
        self.runtime
            .execute(
                self.agent.as_ref(),
                task,
                context,
                self.config.clone(),
                self.system_prompt.as_deref(),
            )
            .await
    }

    pub async fn execute_from_state(
        &self,
        state: HarnessExecutionState,
    ) -> Result<HarnessAgentResult> {
        self.runtime
            .execute_from_state(self.agent.as_ref(), self.config.clone(), state)
            .await
    }
}

impl<A: HarnessAgent> Clone for HarnessAgentBuilder<A> {
    fn clone(&self) -> Self {
        Self {
            agent: self.agent.clone(),
            config: self.config.clone(),
            runtime: self.runtime.clone(),
            system_prompt: self.system_prompt.clone(),
        }
    }
}

#[derive(Clone)]
pub struct HarnessAgentRuntime {
    retry_config: RetryConfig,
    skill_registry: Arc<RwLock<SkillRegistry>>,
    hook_registry: Option<Arc<HookRegistry>>,
    shared_harness: Option<Arc<RwLock<AgentHarnessContext>>>,
    progress_sink: Option<Arc<dyn HarnessExecutionProgressSink>>,
    allowed_tools: Option<Arc<HashSet<String>>>,
    adaptive_timeout: bool,
    metrics_enabled: bool,
}

impl Default for HarnessAgentRuntime {
    fn default() -> Self {
        Self {
            retry_config: RetryConfig::default(),
            skill_registry: Arc::new(RwLock::new(SkillRegistry::new())),
            hook_registry: None,
            shared_harness: None,
            progress_sink: None,
            allowed_tools: None,
            adaptive_timeout: true,
            metrics_enabled: true,
        }
    }
}

impl HarnessAgentRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_retry_config(mut self, retry_config: RetryConfig) -> Self {
        self.retry_config = retry_config;
        self
    }

    pub fn with_skill_registry(mut self, skill_registry: Arc<RwLock<SkillRegistry>>) -> Self {
        self.skill_registry = skill_registry;
        self
    }

    pub fn with_hook_registry(mut self, hook_registry: Arc<HookRegistry>) -> Self {
        self.hook_registry = Some(hook_registry);
        self
    }

    pub fn with_shared_harness(mut self, harness: Arc<RwLock<AgentHarnessContext>>) -> Self {
        self.shared_harness = Some(harness);
        self
    }

    pub fn with_progress_sink(mut self, sink: Arc<dyn HarnessExecutionProgressSink>) -> Self {
        self.progress_sink = Some(match self.progress_sink.take() {
            Some(existing) => Arc::new(CompositeHarnessExecutionProgressSink {
                sinks: vec![existing, sink],
            }),
            None => sink,
        });
        self
    }

    pub fn with_allowed_tools(mut self, tools: Vec<String>) -> Self {
        self.allowed_tools = Some(Arc::new(tools.into_iter().collect()));
        self
    }

    pub fn with_adaptive_timeout(mut self, enabled: bool) -> Self {
        self.adaptive_timeout = enabled;
        self
    }

    pub fn with_metrics(mut self, enabled: bool) -> Self {
        self.metrics_enabled = enabled;
        self
    }

    pub async fn execute<A: HarnessAgent + ?Sized>(
        &self,
        agent: &A,
        task: &str,
        context: &[Message],
        config: HarnessConfig,
        system_prompt: Option<&str>,
    ) -> Result<HarnessAgentResult> {
        let state = HarnessExecutionState::new(task, context, system_prompt.map(ToOwned::to_owned));
        self.execute_from_state(agent, config, state).await
    }

    pub async fn execute_from_state<A: HarnessAgent + ?Sized>(
        &self,
        agent: &A,
        config: HarnessConfig,
        state: HarnessExecutionState,
    ) -> Result<HarnessAgentResult> {
        let mut fusion_builder = UnifiedHarnessFusionBuilder::new()
            .with_harness_config(config.clone())
            .with_self_healing(self.retry_config.max_retries > 1)
            .with_max_repair_attempts(self.retry_config.max_retries.saturating_sub(1) as usize)
            .with_adaptive_timeout(self.adaptive_timeout)
            .with_metrics(self.metrics_enabled);

        if let Some(hook_registry) = &self.hook_registry {
            fusion_builder = fusion_builder.with_hook_registry(hook_registry.clone());
        }

        if let Some(shared_harness) = &self.shared_harness {
            fusion_builder = fusion_builder.with_shared_harness(shared_harness.clone());
        }

        let engine = fusion_builder.build().await;
        engine.start().await;

        let shared_harness = engine.harness().await;
        let tool_executor = HarnessToolExecutor::new(self.skill_registry.clone())
            .with_shared_harness(shared_harness.clone())
            .with_hook_registry(engine.hook_registry())
            .with_retry_config(self.retry_config.clone());
        let tool_executor = if let Some(allowed_tools) = &self.allowed_tools {
            tool_executor.with_allowed_tools((**allowed_tools).clone())
        } else {
            tool_executor
        };

        let mut persisted_state = state;
        let mut messages = persisted_state.build_messages();
        let mut trace = persisted_state.trace.clone();
        let mut final_output = String::new();
        let mut terminated_normally = false;
        let task_owned = persisted_state.task.clone();
        let resource_tracker = super::harness::ResourceTracker::new(
            config.max_memory_bytes.unwrap_or(u64::MAX),
            config.max_cpu_time_ms.unwrap_or(u64::MAX),
        );
        resource_tracker.update_memory(estimate_state_memory_bytes(&messages, &trace));
        if let Err(error) = resource_tracker.check() {
            let harness = shared_harness.read().await;
            harness.record_error(error.clone());
            final_output = error.to_string();
            terminated_normally = false;
        }

        if final_output.is_empty() {
            loop {
                let (can_continue, should_stop, paused) = {
                    let harness = shared_harness.read().await;
                    (
                        harness.can_continue(),
                        harness.should_stop(),
                        harness.is_paused(),
                    )
                };

                if should_stop || !can_continue {
                    break;
                }

                if paused {
                    tokio::time::sleep(Duration::from_millis(25)).await;
                    continue;
                }

                let step_messages = messages.clone();
                let outcome_slot = Arc::new(Mutex::new(None));
                let outcome_slot_clone = outcome_slot.clone();
                let tool_executor_clone = tool_executor.clone();
                let task_for_step = task_owned.clone();
                let step_started = Instant::now();
                let step_result = engine
                    .execute_step(move |ctx, _| {
                        let step_messages = step_messages.clone();
                        let outcome_slot = outcome_slot_clone.clone();
                        let task_owned = task_for_step.clone();
                        let tool_executor = tool_executor_clone.clone();
                        let step_number = ctx.metadata.step_count + 1;
                        async move {
                            let (thought, action_opt, args_opt) = agent
                                .execute_step(&task_owned, &step_messages, step_number)
                                .await?;

                            let mut outcome = StepOutcome {
                                step_number,
                                thought: thought.clone(),
                                action: action_opt.clone(),
                                action_input: args_opt.as_ref().map(|value| value.to_string()),
                                observation: None,
                                final_output: thought.clone(),
                            };

                            if let Some(action) = action_opt {
                                let args = args_opt.unwrap_or_else(|| serde_json::json!({}));
                                let tool_result = tool_executor.execute(&action, args).await;
                                if !tool_result.success {
                                    return Err(anyhow!(tool_result
                                        .error
                                        .unwrap_or_else(|| format!("tool {} failed", action))));
                                }

                                let observation = tool_result.output.unwrap_or_default();
                                outcome.action = Some(tool_result.tool_name);
                                outcome.action_input = Some(tool_result.args);
                                outcome.observation = Some(observation.clone());
                                outcome.final_output = observation;
                            }

                            *outcome_slot.lock().await = Some(outcome.clone());
                            Ok(outcome.final_output)
                        }
                    })
                    .await;
                resource_tracker.add_cpu_time(step_started.elapsed().as_millis() as u64);

                match step_result {
                    Ok(output) => {
                        final_output = output;
                        let step_outcome = outcome_slot.lock().await.take().ok_or_else(|| {
                            anyhow!("missing step outcome after successful execution")
                        })?;

                        trace.push(TraceStep {
                            step: step_outcome.step_number,
                            thought: step_outcome.thought.clone(),
                            action: step_outcome.action.clone(),
                            action_input: step_outcome.action_input.clone(),
                            observation: step_outcome.observation.clone(),
                        });

                        append_step_messages(&mut messages, &step_outcome);
                        persisted_state.trace = trace.clone();
                        resource_tracker
                            .update_memory(estimate_state_memory_bytes(&messages, &trace));
                        if let Err(error) = resource_tracker.check() {
                            let harness = shared_harness.read().await;
                            harness.record_error(error.clone());
                            final_output = error.to_string();
                            break;
                        }
                        if let Some(progress_sink) = &self.progress_sink {
                            progress_sink.persist(persisted_state.clone()).await;
                        }

                        if step_outcome.action.is_none() {
                            terminated_normally = true;
                            break;
                        }
                    }
                    Err(err) => {
                        final_output = err.to_string();
                        break;
                    }
                }
            }
        }

        let harness = shared_harness.read().await;
        let metadata = harness.metadata();
        let errors = harness.error_history();
        let success = terminated_normally && !metadata.cancelled;
        let output = if final_output.is_empty() {
            if metadata.cancelled {
                "Execution cancelled".to_string()
            } else if !terminated_normally {
                format!("Max steps exceeded after {} steps", metadata.step_count)
            } else {
                "Execution completed".to_string()
            }
        } else {
            final_output
        };

        Ok(HarnessAgentResult {
            output,
            success,
            metadata,
            errors,
            trace,
        })
    }
}

fn estimate_state_memory_bytes(messages: &[Message], trace: &[TraceStep]) -> u64 {
    let message_bytes = serde_json::to_vec(messages)
        .map(|bytes| bytes.len())
        .unwrap_or(0) as u64;
    let trace_bytes = serde_json::to_vec(trace)
        .map(|bytes| bytes.len())
        .unwrap_or(0) as u64;
    message_bytes + trace_bytes
}

#[derive(Debug, Clone)]
struct StepOutcome {
    step_number: usize,
    thought: String,
    action: Option<String>,
    action_input: Option<String>,
    observation: Option<String>,
    final_output: String,
}

fn append_step_messages(messages: &mut Vec<Message>, outcome: &StepOutcome) {
    if let Some(action) = &outcome.action {
        let tool_call_id = format!(
            "harness-step-{}-{}",
            outcome.step_number,
            uuid::Uuid::new_v4()
        );
        messages.push(Message::assistant_with_tool_calls(
            outcome.thought.clone(),
            vec![ToolCall {
                id: tool_call_id.clone(),
                r#type: "function".to_string(),
                function: FunctionCall {
                    name: action.clone(),
                    arguments: outcome
                        .action_input
                        .clone()
                        .unwrap_or_else(|| "{}".to_string()),
                },
            }],
        ));
        messages.push(Message::tool_result(
            tool_call_id,
            outcome.observation.clone().unwrap_or_default(),
        ));
    } else {
        messages.push(Message::assistant(outcome.thought.clone()));
    }
}

fn replay_trace_messages(messages: &mut Vec<Message>, trace: &[TraceStep]) {
    for step in trace {
        let outcome = StepOutcome {
            step_number: step.step,
            thought: step.thought.clone(),
            action: step.action.clone(),
            action_input: step.action_input.clone(),
            observation: step.observation.clone(),
            final_output: step
                .observation
                .clone()
                .unwrap_or_else(|| step.thought.clone()),
        };
        append_step_messages(messages, &outcome);
    }
}

/// Default implementation of harness-aware agent execution
pub async fn execute_with_harness<A: HarnessAgent + ?Sized>(
    agent: &A,
    task: &str,
    context: &[Message],
    config: HarnessConfig,
) -> Result<HarnessAgentResult> {
    HarnessAgentRuntime::default()
        .execute(agent, task, context, config, None)
        .await
}

// ============================================================================
// Adapter for existing Agent trait to HarnessAgent
// ============================================================================

/// Adapter to wrap an existing Agent in a HarnessAgent
pub struct AgentAdapter<A: Agent> {
    agent: Arc<A>,
    #[allow(dead_code)]
    llm: Arc<dyn crate::cognitive::llm::LlmClient>,
}

impl<A: Agent + 'static> AgentAdapter<A> {
    pub fn new(agent: Arc<A>, llm: Arc<dyn crate::cognitive::llm::LlmClient>) -> Self {
        Self { agent, llm }
    }
}

#[async_trait]
impl<A: Agent + 'static> HarnessAgent for AgentAdapter<A> {
    async fn execute_with_harness(
        &self,
        task: &str,
        context: &[Message],
        config: HarnessConfig,
    ) -> Result<HarnessAgentResult> {
        execute_with_harness(self, task, context, config).await
    }

    fn name(&self) -> &str {
        self.agent.name()
    }

    fn role(&self) -> AgentRole {
        self.agent.role()
    }

    fn tools(&self) -> Vec<String> {
        Vec::new() // Adapter doesn't expose tools directly
    }

    async fn execute_step(
        &self,
        task: &str,
        context: &[Message],
        step_number: usize,
    ) -> Result<(String, Option<String>, Option<serde_json::Value>)> {
        // Simple single-step execution for adapter
        let result = self.agent.execute(task, context).await?;
        Ok((format!("Step {}: {}", step_number, result), None, None))
    }
}

/// Adapter for shared agent trait objects.
#[derive(Clone)]
pub struct SharedAgentAdapter {
    agent: crate::agent::SharedAgent,
}

impl SharedAgentAdapter {
    pub fn new(agent: crate::agent::SharedAgent) -> Self {
        Self { agent }
    }
}

#[async_trait]
impl HarnessAgent for SharedAgentAdapter {
    async fn execute_with_harness(
        &self,
        task: &str,
        context: &[Message],
        config: HarnessConfig,
    ) -> Result<HarnessAgentResult> {
        execute_with_harness(self, task, context, config).await
    }

    fn name(&self) -> &str {
        self.agent.name()
    }

    fn role(&self) -> AgentRole {
        self.agent.role()
    }

    fn tools(&self) -> Vec<String> {
        Vec::new()
    }

    async fn execute_step(
        &self,
        task: &str,
        context: &[Message],
        step_number: usize,
    ) -> Result<(String, Option<String>, Option<serde_json::Value>)> {
        let result = self.agent.execute(task, context).await?;
        Ok((format!("Step {}: {}", step_number, result), None, None))
    }
}

// ============================================================================
// Harness-aware wrapper for any Agent
// ============================================================================

/// Wrapper that adds harness capabilities to any agent
pub struct HarnessWrappedAgent<A: Agent> {
    inner: Arc<A>,
    config: HarnessConfig,
}

impl<A: Agent> HarnessWrappedAgent<A> {
    pub fn new(inner: Arc<A>) -> Self {
        Self {
            inner,
            config: HarnessConfig::default(),
        }
    }

    pub fn with_config(mut self, config: HarnessConfig) -> Self {
        self.config = config;
        self
    }

    pub fn max_steps(mut self, steps: usize) -> Self {
        self.config.max_steps = steps;
        self
    }
}

#[async_trait]
impl<A: Agent + 'static> Agent for HarnessWrappedAgent<A> {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn role(&self) -> AgentRole {
        self.inner.role()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    async fn execute(&self, task: &str, context: &[Message]) -> Result<String> {
        let harness = AgentHarnessContext::new(self.config.clone());
        let mut messages = context.to_vec();
        messages.push(Message::new("user", task));

        while harness.can_continue() {
            if harness.should_stop() {
                return Err(anyhow!("Execution cancelled"));
            }

            harness.wait_if_paused().await;

            // Execute through inner agent
            match self.inner.execute(task, &messages).await {
                Ok(result) => {
                    harness.record_step();
                    return Ok(result);
                }
                Err(e) => {
                    harness.record_error(HarnessError::LlmFailure(e.to_string()));
                    if harness.has_recent_errors(3) {
                        return Err(anyhow!("Too many consecutive failures"));
                    }
                }
            }
        }

        Err(anyhow!("Max steps exceeded"))
    }
}

#[cfg(test)]
mod tests {
    use crate::agent::harness::{
        AgentHarnessContext, CircuitBreakerConfig, HarnessConfig, HarnessError,
    };
    use std::time::Duration;

    #[test]
    fn test_harness_config_serialization() {
        let config = HarnessConfig {
            max_steps: 10,
            tool_timeout: Duration::from_secs(30),
            step_timeout: Duration::from_secs(60),
            enable_self_reflection: true,
            circuit_breaker: Some(CircuitBreakerConfig::default()),
            max_memory_bytes: Some(1024 * 1024 * 1024),
            max_cpu_time_ms: Some(60000),
            metadata: std::collections::HashMap::new(),
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: HarnessConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.max_steps, 10);
        assert!(parsed.circuit_breaker.is_some());
    }

    #[tokio::test]
    async fn test_execution_guard_circuit_breaker() {
        let config = HarnessConfig {
            max_steps: 5,
            circuit_breaker: Some(CircuitBreakerConfig {
                failure_threshold: 3,
                success_threshold: 2,
                timeout: Duration::from_secs(1),
            }),
            ..Default::default()
        };

        let ctx = AgentHarnessContext::new(config);

        // Open the circuit breaker by recording tool failures
        ctx.record_error(HarnessError::ToolFailure(
            "test_tool".to_string(),
            "error 1".to_string(),
        ));
        ctx.record_error(HarnessError::ToolFailure(
            "test_tool".to_string(),
            "error 2".to_string(),
        ));
        ctx.record_error(HarnessError::ToolFailure(
            "test_tool".to_string(),
            "error 3".to_string(),
        ));

        // Check if circuit is open
        assert!(ctx.is_circuit_open("test_tool"));
    }
}
