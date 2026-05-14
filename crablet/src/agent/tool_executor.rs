//! Agent Tool Executor with Harness Integration
//!
//! Provides robust tool execution with:
//! - Exponential backoff retry
//! - Timeout handling
//! - Error classification
//! - Execution tracking
//! - Harness integration

use anyhow::Result;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tokio::time::timeout;
use tracing::warn;

use super::harness::{AgentHarnessContext, HarnessError, RetryConfig, ToolExecResult};
use super::hooks::{HookAction, HookContext, HookPoint, HookRegistry, HookResult};
use crate::skills::SkillRegistry;

/// Tool execution wrapper with harness support
pub struct HarnessToolExecutor {
    registry: Arc<RwLock<SkillRegistry>>,
    harness: Arc<RwLock<Option<Arc<RwLock<AgentHarnessContext>>>>>,
    hook_registry: Option<Arc<HookRegistry>>,
    retry_config: RetryConfig,
    allowed_tools: Option<Arc<HashSet<String>>>,
    /// Concurrency limit for parallel tool calls
    concurrency_limit: usize,
}

impl HarnessToolExecutor {
    pub fn new(registry: Arc<RwLock<SkillRegistry>>) -> Self {
        Self {
            registry,
            harness: Arc::new(RwLock::new(None)),
            hook_registry: None,
            retry_config: RetryConfig::default(),
            allowed_tools: None,
            concurrency_limit: 5,
        }
    }

    pub fn with_harness(mut self, harness: AgentHarnessContext) -> Self {
        self.harness = Arc::new(RwLock::new(Some(Arc::new(RwLock::new(harness)))));
        self
    }

    pub fn with_shared_harness(mut self, harness: Arc<RwLock<AgentHarnessContext>>) -> Self {
        self.harness = Arc::new(RwLock::new(Some(harness)));
        self
    }

    pub fn with_hook_registry(mut self, registry: Arc<HookRegistry>) -> Self {
        self.hook_registry = Some(registry);
        self
    }

    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    pub fn with_allowed_tools(mut self, tools: HashSet<String>) -> Self {
        self.allowed_tools = Some(Arc::new(tools));
        self
    }

    pub fn with_concurrency_limit(mut self, limit: usize) -> Self {
        self.concurrency_limit = limit;
        self
    }

    /// Execute a tool with retry and timeout support
    pub async fn execute(&self, tool_name: &str, args: serde_json::Value) -> ToolExecResult {
        let start = Instant::now();
        let mut attempts = 0u32;
        let mut current_tool_name = tool_name.to_string();
        let mut current_args = args;

        loop {
            attempts += 1;
            let tool_timeout = self.current_tool_timeout().await;

            // Check harness cancellation
            if let Some(harness) = self.shared_harness().await {
                if harness.read().await.should_stop() {
                    return ToolExecResult::failure(
                        current_tool_name,
                        serde_json::to_string(&current_args).unwrap_or_default(),
                        HarnessError::Cancelled,
                        attempts,
                        start.elapsed().as_millis() as u64,
                    );
                }
            }

            if let Some(hook_registry) = &self.hook_registry {
                let hook_ctx = self
                    .build_hook_context(
                        HookPoint::PreToolUse,
                        &current_tool_name,
                        current_args.clone(),
                        None,
                    )
                    .await;

                match hook_registry.run_pre_tool_use(&hook_ctx).await {
                    Ok(result) => match self.apply_pre_hook_result(
                        &mut current_tool_name,
                        &mut current_args,
                        &result,
                    ) {
                        HookDecision::Continue => {}
                        HookDecision::Retry(reason) => {
                            let error =
                                HarnessError::ToolFailure(current_tool_name.clone(), reason);
                            self.record_failure(error.clone(), false).await;
                            if !self.should_retry(attempts, &error) {
                                return ToolExecResult::failure(
                                    current_tool_name,
                                    serde_json::to_string(&current_args).unwrap_or_default(),
                                    error,
                                    attempts,
                                    start.elapsed().as_millis() as u64,
                                );
                            }
                            let delay = self.retry_config.calculate_delay(attempts - 1);
                            warn!(
                                "Tool {} pre-hook requested retry (attempt {}/{}), retrying in {:?}",
                                current_tool_name, attempts, self.retry_config.max_retries, delay
                            );
                            tokio::time::sleep(delay).await;
                            continue;
                        }
                        HookDecision::Block(reason) => {
                            let error =
                                HarnessError::ToolFailure(current_tool_name.clone(), reason);
                            self.record_failure(error.clone(), false).await;
                            return ToolExecResult::failure(
                                current_tool_name,
                                serde_json::to_string(&current_args).unwrap_or_default(),
                                error,
                                attempts,
                                start.elapsed().as_millis() as u64,
                            );
                        }
                    },
                    Err(err) => {
                        let error =
                            HarnessError::ToolFailure(current_tool_name.clone(), err.to_string());
                        self.record_failure(error.clone(), false).await;
                        if !self.should_retry(attempts, &error) {
                            return ToolExecResult::failure(
                                current_tool_name,
                                serde_json::to_string(&current_args).unwrap_or_default(),
                                error,
                                attempts,
                                start.elapsed().as_millis() as u64,
                            );
                        }
                        let delay = self.retry_config.calculate_delay(attempts - 1);
                        warn!(
                            "Tool {} pre-hook failed (attempt {}/{}), retrying in {:?}: {}",
                            current_tool_name, attempts, self.retry_config.max_retries, delay, err
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                }
            }

            let args_str = serde_json::to_string(&current_args).unwrap_or_default();

            if let Some(allowed_tools) = &self.allowed_tools {
                if !allowed_tools.contains(&current_tool_name) {
                    let error = HarnessError::ToolFailure(
                        current_tool_name.clone(),
                        "tool not allowed by execution policy".to_string(),
                    );
                    self.record_failure(error.clone(), false).await;
                    return ToolExecResult::failure(
                        current_tool_name,
                        args_str,
                        error,
                        attempts,
                        start.elapsed().as_millis() as u64,
                    );
                }
            }

            if let Some(harness) = self.shared_harness().await {
                if harness.read().await.is_circuit_open(&current_tool_name) {
                    let error = HarnessError::CircuitBreakerOpen(current_tool_name.clone());
                    self.record_failure(error.clone(), false).await;
                    return ToolExecResult::failure(
                        current_tool_name,
                        args_str,
                        error,
                        attempts,
                        start.elapsed().as_millis() as u64,
                    );
                }
            }

            // Execute tool with timeout
            match timeout(
                tool_timeout,
                self.execute_once(&current_tool_name, current_args.clone()),
            )
            .await
            {
                Ok(output) => match output {
                    Ok(mut output) => {
                        if let Some(hook_registry) = &self.hook_registry {
                            let hook_ctx = self
                                .build_hook_context(
                                    HookPoint::PostToolUse,
                                    &current_tool_name,
                                    current_args.clone(),
                                    Some(output.clone()),
                                )
                                .await;

                            match hook_registry
                                .run_hooks(HookPoint::PostToolUse, &hook_ctx)
                                .await
                            {
                                Ok(result) => match self
                                    .apply_post_hook_result(&mut output, &result)
                                {
                                    HookDecision::Continue => {}
                                    HookDecision::Retry(reason) => {
                                        let error = HarnessError::ToolFailure(
                                            current_tool_name.clone(),
                                            reason,
                                        );
                                        self.record_failure(error.clone(), true).await;
                                        if !self.should_retry(attempts, &error) {
                                            return ToolExecResult::failure(
                                                current_tool_name,
                                                args_str,
                                                error,
                                                attempts,
                                                start.elapsed().as_millis() as u64,
                                            );
                                        }
                                        let delay = self.retry_config.calculate_delay(attempts - 1);
                                        warn!(
                                                "Tool {} post-hook requested retry (attempt {}/{}), retrying in {:?}",
                                                current_tool_name, attempts, self.retry_config.max_retries, delay
                                            );
                                        tokio::time::sleep(delay).await;
                                        continue;
                                    }
                                    HookDecision::Block(reason) => {
                                        let error = HarnessError::ToolFailure(
                                            current_tool_name.clone(),
                                            reason,
                                        );
                                        self.record_failure(error.clone(), true).await;
                                        return ToolExecResult::failure(
                                            current_tool_name,
                                            args_str,
                                            error,
                                            attempts,
                                            start.elapsed().as_millis() as u64,
                                        );
                                    }
                                },
                                Err(err) => {
                                    let error = HarnessError::ToolFailure(
                                        current_tool_name.clone(),
                                        err.to_string(),
                                    );
                                    self.record_failure(error.clone(), true).await;
                                    if !self.should_retry(attempts, &error) {
                                        return ToolExecResult::failure(
                                            current_tool_name,
                                            args_str,
                                            error,
                                            attempts,
                                            start.elapsed().as_millis() as u64,
                                        );
                                    }
                                    let delay = self.retry_config.calculate_delay(attempts - 1);
                                    warn!(
                                            "Tool {} post-hook failed (attempt {}/{}), retrying in {:?}: {}",
                                            current_tool_name, attempts, self.retry_config.max_retries, delay, err
                                        );
                                    tokio::time::sleep(delay).await;
                                    continue;
                                }
                            }
                        }

                        if let Some(harness) = self.shared_harness().await {
                            let harness = harness.read().await;
                            harness.record_tool_call(true);
                            harness.record_tool_success(&current_tool_name);
                        }

                        let duration_ms = start.elapsed().as_millis() as u64;
                        return ToolExecResult::success(
                            current_tool_name,
                            args_str,
                            output,
                            attempts,
                            duration_ms,
                        );
                    }
                    Err(e) => {
                        let harness_error =
                            self.classify_error(&current_tool_name, e, tool_timeout);
                        let can_retry = self.should_retry(attempts, &harness_error);

                        self.record_failure(harness_error.clone(), true).await;

                        if !can_retry {
                            let duration_ms = start.elapsed().as_millis() as u64;
                            return ToolExecResult::failure(
                                current_tool_name,
                                args_str,
                                harness_error,
                                attempts,
                                duration_ms,
                            );
                        }

                        let delay = self.retry_config.calculate_delay(attempts - 1);
                        warn!(
                            "Tool {} failed (attempt {}/{}), retrying in {:?}: {}",
                            current_tool_name,
                            attempts,
                            self.retry_config.max_retries,
                            delay,
                            harness_error
                        );

                        tokio::time::sleep(delay).await;
                    }
                },
                Err(_) => {
                    let harness_error = HarnessError::Timeout(tool_timeout);
                    let can_retry = self.should_retry(attempts, &harness_error);

                    self.record_failure(harness_error.clone(), true).await;

                    if !can_retry {
                        let duration_ms = start.elapsed().as_millis() as u64;
                        return ToolExecResult::failure(
                            current_tool_name,
                            args_str,
                            harness_error,
                            attempts,
                            duration_ms,
                        );
                    }

                    let delay = self.retry_config.calculate_delay(attempts - 1);
                    warn!(
                        "Tool {} timed out (attempt {}/{}), retrying in {:?}",
                        current_tool_name, attempts, self.retry_config.max_retries, delay
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

    async fn build_hook_context(
        &self,
        point: HookPoint,
        tool_name: &str,
        tool_args: serde_json::Value,
        output: Option<String>,
    ) -> HookContext {
        let mut ctx = HookContext::for_tool_use(
            point,
            self.session_id().await,
            self.step_number().await,
            tool_name.to_string(),
            tool_args,
        );
        ctx.output = output;
        ctx.input = Some(tool_name.to_string());
        ctx
    }

    async fn session_id(&self) -> String {
        if let Some(harness) = self.shared_harness().await {
            format!(
                "harness-{}",
                harness
                    .read()
                    .await
                    .metadata()
                    .started_at
                    .timestamp_millis()
            )
        } else {
            "tool-executor".to_string()
        }
    }

    async fn step_number(&self) -> usize {
        match self.shared_harness().await {
            Some(harness) => harness.read().await.metadata().step_count + 1,
            None => 0,
        }
    }

    async fn current_tool_timeout(&self) -> Duration {
        match self.shared_harness().await {
            Some(harness) => harness.read().await.config().tool_timeout,
            None => Duration::from_secs(30),
        }
    }

    fn should_retry(&self, attempts: u32, error: &HarnessError) -> bool {
        error.is_retryable() && attempts < self.retry_config.max_retries
    }

    async fn record_failure(&self, error: HarnessError, count_tool_call: bool) {
        if let Some(harness) = self.shared_harness().await {
            let harness = harness.read().await;
            if count_tool_call {
                harness.record_tool_call(false);
            }
            harness.record_error(error);
        }
    }

    async fn shared_harness(&self) -> Option<Arc<RwLock<AgentHarnessContext>>> {
        self.harness.read().await.clone()
    }

    fn apply_pre_hook_result(
        &self,
        tool_name: &mut String,
        args: &mut serde_json::Value,
        result: &HookResult,
    ) -> HookDecision {
        if let Some(message) = &result.message {
            warn!("pre-tool hook message for {}: {}", tool_name, message);
        }

        if let Some(payload) = &result.payload {
            match &result.action {
                HookAction::Modify => {
                    *args = payload.clone();
                }
                HookAction::Replace {
                    tool_name: replacement,
                    args: replacement_args,
                } => {
                    *tool_name = replacement.clone();
                    *args = replacement_args.clone();
                }
                _ => {}
            }
        }

        match &result.action {
            HookAction::Allow => {
                if result.retry {
                    HookDecision::Retry(
                        result
                            .message
                            .clone()
                            .unwrap_or_else(|| "hook requested retry".to_string()),
                    )
                } else {
                    HookDecision::Continue
                }
            }
            HookAction::Block { reason } => HookDecision::Block(reason.clone()),
            HookAction::Retry { message } => HookDecision::Retry(message.clone()),
            HookAction::Modify | HookAction::Replace { .. } => {
                if result.retry {
                    HookDecision::Retry(
                        result
                            .message
                            .clone()
                            .unwrap_or_else(|| "hook requested retry".to_string()),
                    )
                } else {
                    HookDecision::Continue
                }
            }
        }
    }

    fn apply_post_hook_result(&self, output: &mut String, result: &HookResult) -> HookDecision {
        if let Some(message) = &result.message {
            warn!("post-tool hook message: {}", message);
        }

        if let Some(payload) = &result.payload {
            if let Ok(rendered) = serde_json::from_value::<String>(payload.clone()) {
                *output = rendered;
            } else if let Some(rendered) = payload.get("output").and_then(|value| value.as_str()) {
                *output = rendered.to_string();
            } else if let Ok(rendered) = serde_json::to_string(payload) {
                *output = rendered;
            }
        }

        match &result.action {
            HookAction::Allow => {
                if result.retry {
                    HookDecision::Retry(
                        result
                            .message
                            .clone()
                            .unwrap_or_else(|| "hook requested retry".to_string()),
                    )
                } else {
                    HookDecision::Continue
                }
            }
            HookAction::Block { reason } => HookDecision::Block(reason.clone()),
            HookAction::Retry { message } => HookDecision::Retry(message.clone()),
            HookAction::Modify | HookAction::Replace { .. } => {
                if result.retry {
                    HookDecision::Retry(
                        result
                            .message
                            .clone()
                            .unwrap_or_else(|| "hook requested retry".to_string()),
                    )
                } else {
                    HookDecision::Continue
                }
            }
        }
    }

    /// Classify error into HarnessError type
    fn classify_error(
        &self,
        tool_name: &str,
        err: anyhow::Error,
        tool_timeout: Duration,
    ) -> HarnessError {
        let err_msg = err.to_string();

        // Timeout detection
        if err_msg.contains("timeout") || err_msg.contains("timed out") {
            return HarnessError::Timeout(tool_timeout);
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
            hook_registry: self.hook_registry.clone(),
            retry_config: self.retry_config.clone(),
            allowed_tools: self.allowed_tools.clone(),
            concurrency_limit: self.concurrency_limit,
        }
    }
}

#[derive(Debug, Clone)]
enum HookDecision {
    Continue,
    Retry(String),
    Block(String),
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
