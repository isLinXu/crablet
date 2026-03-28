//! Harness-aware Agent Trait
//!
//! Provides a trait extension for agents to leverage Harness capabilities:
//! - Integrated lifecycle management
//! - Automatic error handling and retry
//! - Circuit breaker support
//! - Checkpoint and resume
//! - Resource tracking

use std::sync::Arc;
use anyhow::{Result, anyhow};
use async_trait::async_trait;

use super::harness::{
    AgentHarnessContext, HarnessConfig, HarnessError,
    ExecutionMetadata, RetryConfig,
};
use super::{Agent, AgentRole};
use crate::types::Message;

/// Result of harness-aware agent execution
#[derive(Debug)]
pub struct HarnessAgentResult {
    pub output: String,
    pub success: bool,
    pub metadata: ExecutionMetadata,
    pub errors: Vec<HarnessError>,
}

impl HarnessAgentResult {
    pub fn success(output: String, metadata: ExecutionMetadata) -> Self {
        Self {
            output,
            success: true,
            metadata,
            errors: Vec::new(),
        }
    }

    pub fn failure(output: String, metadata: ExecutionMetadata, errors: Vec<HarnessError>) -> Self {
        Self {
            output,
            success: false,
            metadata,
            errors,
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
    ) -> Result<HarnessAgentResult>;

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
    retry_config: RetryConfig,
    system_prompt: Option<String>,
}

impl<A: HarnessAgent> HarnessAgentBuilder<A> {
    pub fn new(agent: Arc<A>) -> Self {
        Self {
            agent,
            config: HarnessConfig::default(),
            retry_config: RetryConfig::default(),
            system_prompt: None,
        }
    }

    pub fn with_config(mut self, config: HarnessConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_retry_config(mut self, retry_config: RetryConfig) -> Self {
        self.retry_config = retry_config;
        self
    }

    pub fn with_system_prompt(mut self, prompt: String) -> Self {
        self.system_prompt = Some(prompt);
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

    pub async fn execute(
        &self,
        task: &str,
        context: &[Message],
    ) -> Result<HarnessAgentResult> {
        self.agent.execute_with_harness(task, context, self.config.clone()).await
    }
}

/// Default implementation of harness-aware agent execution
pub async fn execute_with_harness<A: HarnessAgent + ?Sized>(
    agent: &A,
    task: &str,
    context: &[Message],
    config: HarnessConfig,
) -> Result<HarnessAgentResult> {
    let mut harness = AgentHarnessContext::new(config);
    let start_time = std::time::Instant::now();
    let mut errors = Vec::new();

    // Build messages with optional system prompt
    let mut messages = Vec::new();
    messages.extend_from_slice(context);
    messages.push(Message::new("user", task));

    // Main execution loop
    while harness.can_continue() {
        // Check for cancellation
        if harness.should_stop() {
            break;
        }

        // Wait if paused
        harness.wait_if_paused().await;

        // Execute a step
        let step_number = harness.metadata().step_count + 1;

        match agent.execute_step(task, &messages, step_number).await {
            Ok((_thought, action_opt, args_opt)) => {
                // Record step completion
                harness.metadata_mut().update_duration();
                harness.record_step();

                // If there's an action, execute it
                if let (Some(action), Some(_args)) = (action_opt, args_opt) {
                    // Check circuit breaker
                    if harness.is_circuit_open(&action) {
                        let err = HarnessError::CircuitBreakerOpen(action.clone());
                        harness.record_error(err.clone());
                        errors.push(err);
                        break;
                    }

                    // Note: Actual tool execution would go here
                    // For now, we just record the step
                }
            }
            Err(e) => {
                let err = HarnessError::LlmFailure(e.to_string());
                harness.record_error(err.clone());
                errors.push(err.clone());

                // Check if we should continue
                if !err.is_retryable() || harness.has_recent_errors(3) {
                    break;
                }
            }
        }
    }

    harness.metadata_mut().update_duration();

    let output = format!(
        "Completed {} steps in {:?}",
        harness.metadata().step_count,
        start_time.elapsed()
    );

    Ok(HarnessAgentResult {
        output,
        success: errors.is_empty(),
        metadata: harness.metadata().clone(),
        errors,
    })
}

// ============================================================================
// Adapter for existing Agent trait to HarnessAgent
// ============================================================================

/// Adapter to wrap an existing Agent in a HarnessAgent
pub struct AgentAdapter<A: Agent> {
    agent: Arc<A>,
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
        let harness = AgentHarnessContext::new(config);
        let result = self.agent.execute(task, context).await?;

        Ok(HarnessAgentResult::success(
            result,
            harness.metadata().clone(),
        ))
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
        let mut harness = AgentHarnessContext::new(self.config.clone());
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
    use crate::agent::harness::{CircuitBreakerConfig, HarnessConfig, HarnessError, AgentHarnessContext};
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

        let mut ctx = AgentHarnessContext::new(config);

        // Open the circuit breaker by recording tool failures
        ctx.record_error(HarnessError::ToolFailure("test_tool".to_string(), "error 1".to_string()));
        ctx.record_error(HarnessError::ToolFailure("test_tool".to_string(), "error 2".to_string()));
        ctx.record_error(HarnessError::ToolFailure("test_tool".to_string(), "error 3".to_string()));

        // Check if circuit is open
        assert!(ctx.is_circuit_open("test_tool"));
    }
}