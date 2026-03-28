//! Agent Tool Executor with Harness Integration
//!
//! Provides robust tool execution with:
//! - Exponential backoff retry
//! - Timeout handling
//! - Error classification
//! - Execution tracking
//! - Harness integration

use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tracing::warn;

use super::harness::{AgentHarnessContext, HarnessError, RetryConfig, ToolExecResult};
use crate::skills::SkillRegistry;

/// Tool execution wrapper with harness support
pub struct HarnessToolExecutor {
    registry: Arc<RwLock<SkillRegistry>>,
    harness: Arc<RwLock<Option<AgentHarnessContext>>>,
    retry_config: RetryConfig,
    /// Concurrency limit for parallel tool calls
    concurrency_limit: usize,
}

impl HarnessToolExecutor {
    pub fn new(registry: Arc<RwLock<SkillRegistry>>) -> Self {
        Self {
            registry,
            harness: Arc::new(RwLock::new(None)),
            retry_config: RetryConfig::default(),
            concurrency_limit: 5,
        }
    }

    pub fn with_harness(mut self, harness: AgentHarnessContext) -> Self {
        self.harness = Arc::new(RwLock::new(Some(harness)));
        self
    }

    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    pub fn with_concurrency_limit(mut self, limit: usize) -> Self {
        self.concurrency_limit = limit;
        self
    }

    /// Execute a tool with retry and timeout support
    pub async fn execute(&self, tool_name: &str, args: serde_json::Value) -> ToolExecResult {
        let start = Instant::now();
        let args_str = serde_json::to_string(&args).unwrap_or_default();
        let mut attempts = 0u32;

        loop {
            attempts += 1;

            // Check harness cancellation
            if let Some(ref harness) = *self.harness.read().await {
                if harness.should_stop() {
                    return ToolExecResult::failure(
                        tool_name.to_string(),
                        args_str.clone(),
                        HarnessError::Cancelled,
                        attempts,
                        start.elapsed().as_millis() as u64,
                    );
                }
            }

            // Execute tool
            match self.execute_once(tool_name, args.clone()).await {
                Ok(output) => {
                    if let Some(ref mut harness) = *self.harness.write().await {
                        harness.record_tool_call(true);
                        harness.record_tool_success(tool_name);
                    }

                    let duration_ms = start.elapsed().as_millis() as u64;
                    return ToolExecResult::success(
                        tool_name.to_string(),
                        args_str,
                        output,
                        attempts,
                        duration_ms,
                    );
                }
                Err(e) => {
                    let harness_error = self.classify_error(tool_name, e);
                    let can_retry =
                        harness_error.is_retryable() && attempts < self.retry_config.max_retries;

                    if let Some(ref mut harness) = *self.harness.write().await {
                        harness.record_tool_call(false);
                        harness.record_error(harness_error.clone());
                    }

                    if !can_retry {
                        let duration_ms = start.elapsed().as_millis() as u64;
                        return ToolExecResult::failure(
                            tool_name.to_string(),
                            args_str,
                            harness_error,
                            attempts,
                            duration_ms,
                        );
                    }

                    // Exponential backoff
                    let delay = self.retry_config.calculate_delay(attempts - 1);
                    warn!(
                        "Tool {} failed (attempt {}/{}), retrying in {:?}: {}",
                        tool_name, attempts, self.retry_config.max_retries, delay, harness_error
                    );

                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    /// Execute tool once without retry
    async fn execute_once(&self, tool_name: &str, args: serde_json::Value) -> Result<String> {
        let registry = self.registry.read().await;
        registry.execute(tool_name, args).await
    }

    /// Classify error into HarnessError type
    fn classify_error(&self, tool_name: &str, err: anyhow::Error) -> HarnessError {
        let err_msg = err.to_string();

        // Timeout detection
        if err_msg.contains("timeout") || err_msg.contains("timed out") {
            return HarnessError::Timeout(Duration::from_secs(30));
        }

        // Connection errors
        if err_msg.contains("connection") || err_msg.contains("network") {
            return HarnessError::ToolFailure(tool_name.to_string(), err_msg);
        }

        // Rate limiting
        if err_msg.contains("rate limit") || err_msg.contains("429") {
            return HarnessError::ToolFailure(tool_name.to_string(), err_msg);
        }

        // Resource not found
        if err_msg.contains("not found") || err_msg.contains("404") {
            return HarnessError::ToolFailure(tool_name.to_string(), err_msg);
        }

        // Default to generic failure
        HarnessError::ToolFailure(tool_name.to_string(), err_msg)
    }

    /// Execute multiple tools in parallel with concurrency limit
    pub async fn execute_parallel(
        &self,
        calls: Vec<(String, serde_json::Value)>,
    ) -> Vec<ToolExecResult> {
        let semaphore = Arc::new(Semaphore::new(self.concurrency_limit));
        let mut handles = Vec::new();

        for (tool_name, args) in calls {
            let executor = self.clone();
            let sem = semaphore.clone();

            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.ok();
                executor.execute(&tool_name, args).await
            }));
        }

        let mut results = Vec::new();
        for handle in handles {
            if let Ok(result) = handle.await {
                results.push(result);
            }
        }
        results
    }
}

impl Clone for HarnessToolExecutor {
    fn clone(&self) -> Self {
        Self {
            registry: self.registry.clone(),
            harness: self.harness.clone(),
            retry_config: self.retry_config.clone(),
            concurrency_limit: self.concurrency_limit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::SkillRegistry;

    #[tokio::test]
    async fn test_executor_creation() {
        let registry = Arc::new(RwLock::new(SkillRegistry::new()));
        let executor = HarnessToolExecutor::new(registry);

        assert_eq!(executor.concurrency_limit, 5);
        assert_eq!(executor.retry_config.max_retries, 3);
    }

    #[tokio::test]
    async fn test_executor_with_options() {
        let registry = Arc::new(RwLock::new(SkillRegistry::new()));
        let executor = HarnessToolExecutor::new(registry)
            .with_concurrency_limit(10)
            .with_retry_config(RetryConfig {
                max_retries: 5,
                ..Default::default()
            });

        assert_eq!(executor.concurrency_limit, 10);
        assert_eq!(executor.retry_config.max_retries, 5);
    }
}
