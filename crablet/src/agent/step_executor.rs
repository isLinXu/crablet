//! Step Executor - Harness-based step execution for agents
//!
//! Provides:
//! - Step-by-step execution with harness tracking
//! - Thought/Action/Observation pattern
//! - Self-reflection on failures
//! - Timeout and cancellation handling

use std::sync::Arc;
use tokio::sync::RwLock;
use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::harness::{AgentHarnessContext, HarnessConfig};

/// A single step in agent execution
#[derive(Debug, Clone)]
pub struct ExecutionStep {
    /// Step number (1-indexed)
    pub step_number: usize,
    /// Thought/plan for this step
    pub thought: String,
    /// Action to take (tool call)
    pub action: Option<String>,
    /// Action arguments
    pub action_args: Option<serde_json::Value>,
    /// Observation from action
    pub observation: Option<String>,
    /// Whether this step succeeded
    pub success: bool,
    /// Error if failed
    pub error: Option<String>,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

impl ExecutionStep {
    pub fn new(step_number: usize, thought: String) -> Self {
        Self {
            step_number,
            thought,
            action: None,
            action_args: None,
            observation: None,
            success: true,
            error: None,
            duration_ms: 0,
        }
    }

    pub fn with_action(mut self, action: String, args: serde_json::Value) -> Self {
        self.action = Some(action);
        self.action_args = Some(args);
        self
    }

    pub fn with_observation(mut self, observation: String) -> Self {
        self.observation = Some(observation);
        self
    }

    pub fn with_failure(mut self, error: String) -> Self {
        self.success = false;
        self.error = Some(error);
        self
    }
}

/// Result of executing a full agent task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepExecutionResult {
    /// Final output
    pub output: String,
    /// Whether the execution succeeded
    pub success: bool,
    /// Total steps executed
    pub steps_executed: usize,
    /// Steps that failed
    pub steps_failed: usize,
    /// All execution steps
    pub execution_steps: Vec<StepExecutionStep>,
    /// Total duration in milliseconds
    pub total_duration_ms: u64,
    /// Token usage if available
    pub tokens_used: Option<u64>,
    /// Final error if any
    pub error: Option<String>,
}

/// Serializable version of execution step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepExecutionStep {
    pub step_number: usize,
    pub thought: String,
    pub action: Option<String>,
    pub action_args: Option<serde_json::Value>,
    pub observation: Option<String>,
    pub success: bool,
    pub error: Option<String>,
    pub duration_ms: u64,
}

impl From<&ExecutionStep> for StepExecutionStep {
    fn from(step: &ExecutionStep) -> Self {
        Self {
            step_number: step.step_number,
            thought: step.thought.clone(),
            action: step.action.clone(),
            action_args: step.action_args.clone(),
            observation: step.observation.clone(),
            success: step.success,
            error: step.error.clone(),
            duration_ms: step.duration_ms,
        }
    }
}

/// Configuration for step executor
#[derive(Debug, Clone)]
pub struct StepExecutorConfig {
    /// Harness configuration
    pub harness_config: HarnessConfig,
    /// Enable self-reflection on failure
    pub enable_self_reflection: bool,
    /// Max reflection attempts per step
    pub max_reflection_attempts: u32,
}

impl Default for StepExecutorConfig {
    fn default() -> Self {
        Self {
            harness_config: HarnessConfig::default(),
            enable_self_reflection: true,
            max_reflection_attempts: 2,
        }
    }
}

/// Step Executor - Executes agent tasks step by step with harness tracking
pub struct StepExecutor {
    config: StepExecutorConfig,
}

impl StepExecutor {
    pub fn new(config: StepExecutorConfig) -> Self {
        Self { config }
    }

    pub fn with_default_config() -> Self {
        Self::new(StepExecutorConfig::default())
    }

    /// Execute a task step by step using async generators and executors
    pub async fn execute<F, G>(
        &self,
        task: &str,
        harness: Arc<RwLock<AgentHarnessContext>>,
        mut generate_fn: F,
        mut execute_fn: G,
    ) -> Result<StepExecutionResult>
    where
        F: FnMut(usize, &str, &[ExecutionStep]) -> Pin<Box<dyn std::future::Future<Output = Result<(String, Option<String>, Option<serde_json::Value>)>> + Send>>,
        G: FnMut(String, serde_json::Value) -> Pin<Box<dyn std::future::Future<Output = Result<String>> + Send>>,
    {
        let start_time = std::time::Instant::now();
        let mut execution_steps = Vec::new();
        let mut steps_failed = 0;
        let total_steps = {
            let h = harness.read().await;
            h.config().max_steps
        };

        // Main execution loop
        for step_num in 1..=total_steps {
            // Check cancellation
            {
                let h = harness.read().await;
                if h.should_stop() {
                    break;
                }
            }

            // Generate thought/action
            let history: Vec<ExecutionStep> = execution_steps.clone();
            let (thought, action_opt, action_args) = generate_fn(
                step_num,
                task,
                &history,
            ).await?;

            let mut step = ExecutionStep::new(step_num, thought);

            // Execute action if present
            if let (Some(action), Some(args)) = (action_opt, action_args) {
                step = step.with_action(action.clone(), args.clone());

                match execute_fn(action, args).await {
                    Ok(observation) => {
                        step = step.with_observation(observation);
                    }
                    Err(e) => {
                        step = step.with_failure(e.to_string());
                        steps_failed += 1;
                    }
                }
            }

            execution_steps.push(step);

            // Check if we should continue
            {
                let h = harness.read().await;
                if !h.can_continue() {
                    break;
                }
            }
        }

        let total_duration_ms = start_time.elapsed().as_millis() as u64;

        // Determine final output
        let (output, success, error) = if steps_failed == 0 {
            let final_obs = execution_steps.last()
                .and_then(|s| s.observation.clone())
                .unwrap_or_else(|| "Task completed".to_string());
            (final_obs, true, None)
        } else if execution_steps.is_empty() {
            ("No steps executed".to_string(), false, Some("No steps were executed".to_string()))
        } else {
            let last_step = execution_steps.last().unwrap();
            (
                last_step.observation.clone().unwrap_or_default(),
                false,
                last_step.error.clone(),
            )
        };

        Ok(StepExecutionResult {
            output,
            success,
            steps_executed: execution_steps.len(),
            steps_failed,
            execution_steps: execution_steps.iter().map(|s| s.into()).collect(),
            total_duration_ms,
            tokens_used: None,
            error,
        })
    }
}

use std::pin::Pin;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_step_executor_config() {
        let config = StepExecutorConfig::default();
        assert_eq!(config.harness_config.max_steps, 10);
        assert!(config.enable_self_reflection);
    }

    #[tokio::test]
    async fn test_execution_step_builder() {
        let step = ExecutionStep::new(1, "I need to search for info".to_string())
            .with_action("search".to_string(), serde_json::json!({"query": "rust"}))
            .with_observation("Found 5 results".to_string());

        assert_eq!(step.step_number, 1);
        assert!(step.action.is_some());
        assert!(step.observation.is_some());
        assert!(step.success);
    }

    #[tokio::test]
    async fn test_failed_step() {
        let step = ExecutionStep::new(1, "I will try to read".to_string())
            .with_failure("File not found".to_string());

        assert!(!step.success);
        assert!(step.error.is_some());
    }
}