//! Agentic Harness - Core Execution Context
//!
//! Provides unified execution context for agent operations, including:
//! - Lifecycle management (start, pause, resume, cancel)
//! - Execution metadata (steps, timing, cost tracking)
//! - Error recovery with exponential backoff
//! - Tool call retry logic
//! - Signal handling for graceful shutdown
//! - Circuit breaker for fault tolerance
//! - Resource usage tracking
//! - Checkpoint and resume capability

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use tokio::sync::{RwLock, broadcast};
use tokio::sync::broadcast::Sender;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use thiserror::Error;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum HarnessError {
    #[error("Execution timeout after {0:?}")]
    Timeout(Duration),

    #[error("Execution cancelled")]
    Cancelled,

    #[error("Max retries exceeded for tool: {0}")]
    MaxRetriesExceeded(String),

    #[error("LLM failure: {0}")]
    LlmFailure(String),

    #[error("Tool execution failed: {0} - {1}")]
    ToolFailure(String, String),

    #[error("Context closed")]
    ContextClosed,

    #[error("Circuit breaker open for: {0}")]
    CircuitBreakerOpen(String),

    #[error("Resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),
}

impl HarnessError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            HarnessError::Timeout(_) |
            HarnessError::ToolFailure(_, _) |
            HarnessError::LlmFailure(_)
        )
    }
}

// ============================================================================
// Execution Metadata
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetadata {
    pub step_count: usize,
    pub total_duration_ms: u64,
    pub tool_call_count: usize,
    pub tool_failure_count: usize,
    pub llm_tokens_used: Option<u64>,
    pub started_at: DateTime<Utc>,
    pub last_activity_at: DateTime<Utc>,
}

impl ExecutionMetadata {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            step_count: 0,
            total_duration_ms: 0,
            tool_call_count: 0,
            tool_failure_count: 0,
            llm_tokens_used: None,
            started_at: now,
            last_activity_at: now,
        }
    }

    pub fn record_step(&mut self) {
        self.step_count += 1;
        self.last_activity_at = Utc::now();
    }

    pub fn record_tool_call(&mut self, success: bool) {
        self.tool_call_count += 1;
        if !success {
            self.tool_failure_count += 1;
        }
        self.last_activity_at = Utc::now();
    }

    pub fn update_duration(&mut self) {
        self.total_duration_ms = (Utc::now()
            .signed_duration_since(self.started_at)
            .num_milliseconds()) as u64;
    }

    pub fn set_tokens(&mut self, tokens: u64) {
        self.llm_tokens_used = Some(tokens);
    }

    pub fn to_summary(&self) -> ExecutionSummary {
        ExecutionSummary {
            step_count: self.step_count,
            total_duration_ms: self.total_duration_ms,
            tool_call_count: self.tool_call_count,
            tool_failure_count: self.tool_failure_count,
            llm_tokens_used: self.llm_tokens_used,
        }
    }
}

impl Default for ExecutionMetadata {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of execution for logging/reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSummary {
    pub step_count: usize,
    pub total_duration_ms: u64,
    pub tool_call_count: usize,
    pub tool_failure_count: usize,
    pub llm_tokens_used: Option<u64>,
}

// ============================================================================
// Retry Configuration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        let delay_ms = self.base_delay.as_millis() as f64 * self.multiplier.powi(attempt as i32);
        let delay_ms = delay_ms.min(self.max_delay.as_millis() as f64);
        Duration::from_millis(delay_ms as u64)
    }
}

// ============================================================================
// Circuit Breaker - Fault tolerance for tool calls
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub timeout: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 3,
            success_threshold: 2,
            timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    Closed,    // Normal operation
    Open,      // Failing, reject calls
    HalfOpen,  // Testing if recovery is possible
}

pub struct CircuitBreaker {
    state: AtomicState,
    failure_count: AtomicU64,
    success_count: AtomicU64,
    last_failure_time: std::sync::atomic::AtomicU64,
    config: CircuitBreakerConfig,
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: AtomicState::new(CircuitState::Closed),
            failure_count: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            last_failure_time: AtomicU64::new(0),
            config,
        }
    }

    pub fn is_allowed(&self) -> bool {
        match self.state.load(Ordering::SeqCst) {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if timeout has passed
                let last_failure = self.last_failure_time.load(Ordering::SeqCst);
                if last_failure == 0 {
                    return true;
                }
                let elapsed = Duration::from_secs(last_failure);
                if elapsed > self.config.timeout {
                    self.state.store(CircuitState::HalfOpen, Ordering::SeqCst);
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    pub fn record_failure(&self) {
        let failures = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
        self.last_failure_time.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            Ordering::SeqCst,
        );

        if failures >= self.config.failure_threshold as u64 {
            self.state.store(CircuitState::Open, Ordering::SeqCst);
        }
    }

    pub fn record_success(&self) {
        self.failure_count.store(0, Ordering::SeqCst);

        if self.state.load(Ordering::SeqCst) == CircuitState::HalfOpen {
            let successes = self.success_count.fetch_add(1, Ordering::SeqCst) + 1;
            if successes >= self.config.success_threshold as u64 {
                self.state.store(CircuitState::Closed, Ordering::SeqCst);
                self.success_count.store(0, Ordering::SeqCst);
            }
        }
    }

    pub fn state(&self) -> CircuitState {
        self.state.load(Ordering::SeqCst)
    }

    pub fn reset(&self) {
        self.state.store(CircuitState::Closed, Ordering::SeqCst);
        self.failure_count.store(0, Ordering::SeqCst);
        self.success_count.store(0, Ordering::SeqCst);
    }
}

use std::sync::atomic::AtomicU8;

struct AtomicState {
    state: AtomicU8,
}

impl AtomicState {
    fn new(initial: CircuitState) -> Self {
        Self {
            state: AtomicU8::new(Self::to_u8(initial)),
        }
    }

    fn load(&self, order: Ordering) -> CircuitState {
        Self::from_u8(self.state.load(order))
    }

    fn store(&self, state: CircuitState, order: Ordering) {
        self.state.store(Self::to_u8(state), order);
    }

    fn to_u8(state: CircuitState) -> u8 {
        match state {
            CircuitState::Closed => 0,
            CircuitState::Open => 1,
            CircuitState::HalfOpen => 2,
        }
    }

    fn from_u8(val: u8) -> CircuitState {
        match val {
            1 => CircuitState::Open,
            2 => CircuitState::HalfOpen,
            _ => CircuitState::Closed,
        }
    }
}

// ============================================================================
// Resource Tracking
// ============================================================================

#[derive(Debug, Clone, Default)]
pub struct ResourceUsage {
    pub memory_bytes: u64,
    pub cpu_time_ms: u64,
}

pub struct ResourceTracker {
    max_memory_bytes: u64,
    max_cpu_time_ms: u64,
    current_memory: AtomicU64,
    current_cpu_time: AtomicU64,
}

impl ResourceTracker {
    pub fn new(max_memory_bytes: u64, max_cpu_time_ms: u64) -> Self {
        Self {
            max_memory_bytes,
            max_cpu_time_ms,
            current_memory: AtomicU64::new(0),
            current_cpu_time: AtomicU64::new(0),
        }
    }

    pub fn update_memory(&self, bytes: u64) {
        self.current_memory.store(bytes, Ordering::SeqCst);
    }

    pub fn add_cpu_time(&self, ms: u64) {
        self.current_cpu_time.fetch_add(ms, Ordering::SeqCst);
    }

    pub fn check(&self) -> Result<(), HarnessError> {
        if self.current_memory.load(Ordering::SeqCst) > self.max_memory_bytes {
            return Err(HarnessError::ResourceLimitExceeded("memory".to_string()));
        }
        if self.current_cpu_time.load(Ordering::SeqCst) > self.max_cpu_time_ms {
            return Err(HarnessError::ResourceLimitExceeded("CPU time".to_string()));
        }
        Ok(())
    }

    pub fn get_usage(&self) -> ResourceUsage {
        ResourceUsage {
            memory_bytes: self.current_memory.load(Ordering::SeqCst),
            cpu_time_ms: self.current_cpu_time.load(Ordering::SeqCst),
        }
    }

    pub fn reset(&self) {
        self.current_memory.store(0, Ordering::SeqCst);
        self.current_cpu_time.store(0, Ordering::SeqCst);
    }
}

// ============================================================================
// Checkpoint - Save and resume execution state
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub metadata: ExecutionMetadata,
    pub error_history: Vec<HarnessError>,
    pub step_number: usize,
    pub created_at: DateTime<Utc>,
}

impl Checkpoint {
    pub fn from_context(ctx: &AgentHarnessContext, step_number: usize) -> Self {
        Self {
            metadata: ctx.metadata().clone(),
            error_history: ctx.error_history().to_vec(),
            step_number,
            created_at: Utc::now(),
        }
    }
}

// ============================================================================
// Harness Signal - For cancellation and control
// ============================================================================

#[derive(Debug, Clone)]
pub enum HarnessSignal {
    Cancel,
    Pause,
    Resume,
    Checkpoint,
}

pub struct HarnessSignalChannel {
    sender: Sender<HarnessSignal>,
}

impl HarnessSignalChannel {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(16);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<HarnessSignal> {
        self.sender.subscribe()
    }

    pub fn send(&self, signal: HarnessSignal) -> bool {
        self.sender.send(signal).is_ok()
    }

    pub fn cancel(&self) -> bool {
        self.send(HarnessSignal::Cancel)
    }

    pub fn pause(&self) -> bool {
        self.send(HarnessSignal::Pause)
    }

    pub fn resume(&self) -> bool {
        self.send(HarnessSignal::Resume)
    }

    pub fn checkpoint(&self) -> bool {
        self.send(HarnessSignal::Checkpoint)
    }
}

impl Default for HarnessSignalChannel {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Agent Harness Context - Core execution state
// ============================================================================

pub struct AgentHarnessContext {
    /// Execution metadata
    metadata: ExecutionMetadata,
    /// Configuration
    config: HarnessConfig,
    /// Retry configuration for tool calls
    retry_config: RetryConfig,
    /// Circuit breaker for fault tolerance
    circuit_breaker: CircuitBreaker,
    /// Resource tracker
    resource_tracker: ResourceTracker,
    /// Cancellation and control signals
    signals: HarnessSignalChannel,
    /// Error history for analysis
    error_history: Vec<HarnessError>,
    /// Flag indicating if execution should stop
    should_stop: AtomicBool,
    /// Flag indicating if execution is paused
    is_paused: AtomicBool,
    /// Pause waiter for resume
    pause_waiter: Arc<RwLock<()>>,
    /// Execution start time
    start_time: Instant,
    /// Checkpoint for resume
    checkpoint: Arc<RwLock<Option<Checkpoint>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessConfig {
    pub max_steps: usize,
    pub tool_timeout: Duration,
    pub step_timeout: Duration,
    pub enable_self_reflection: bool,
    #[serde(default)]
    pub circuit_breaker: Option<CircuitBreakerConfig>,
    #[serde(default)]
    pub max_memory_bytes: Option<u64>,
    #[serde(default)]
    pub max_cpu_time_ms: Option<u64>,
    /// Additional metadata for distributed coordination
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl Default for HarnessConfig {
    fn default() -> Self {
        Self {
            max_steps: 10,
            tool_timeout: Duration::from_secs(30),
            step_timeout: Duration::from_secs(60),
            enable_self_reflection: true,
            circuit_breaker: None,
            max_memory_bytes: None,
            max_cpu_time_ms: None,
            metadata: HashMap::new(),
        }
    }
}

impl AgentHarnessContext {
    pub fn new(config: HarnessConfig) -> Self {
        let circuit_breaker = config.circuit_breaker
            .as_ref()
            .map(|cb| CircuitBreaker::new(cb.clone()))
            .unwrap_or_else(|| CircuitBreaker::new(CircuitBreakerConfig::default()));

        let max_memory = config.max_memory_bytes.unwrap_or(u64::MAX);
        let max_cpu = config.max_cpu_time_ms.unwrap_or(u64::MAX);
        let resource_tracker = ResourceTracker::new(max_memory, max_cpu);

        Self {
            metadata: ExecutionMetadata::new(),
            config,
            retry_config: RetryConfig::default(),
            circuit_breaker,
            resource_tracker,
            signals: HarnessSignalChannel::new(),
            error_history: Vec::new(),
            should_stop: AtomicBool::new(false),
            is_paused: AtomicBool::new(false),
            pause_waiter: Arc::new(RwLock::new(())),
            start_time: Instant::now(),
            checkpoint: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    // --- Signal handling ---

    pub fn signals(&self) -> &HarnessSignalChannel {
        &self.signals
    }

    pub fn cancel(&self) {
        self.should_stop.store(true, Ordering::SeqCst);
        let _ = self.signals.cancel();
    }

    pub fn should_stop(&self) -> bool {
        self.should_stop.load(Ordering::SeqCst)
    }

    pub fn pause(&self) {
        self.is_paused.store(true, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.is_paused.store(false, Ordering::SeqCst);
    }

    pub fn is_paused(&self) -> bool {
        self.is_paused.load(Ordering::SeqCst)
    }

    // --- Error handling ---

    pub fn record_error(&mut self, error: HarnessError) {
        // Update circuit breaker if tool failure
        if matches!(error, HarnessError::ToolFailure(_, _)) {
            self.circuit_breaker.record_failure();
        }
        self.error_history.push(error.clone());
    }

    pub fn get_last_error(&self) -> Option<&HarnessError> {
        self.error_history.last()
    }

    pub fn has_recent_errors(&self, count: usize) -> bool {
        self.error_history.len() >= count
    }

    pub fn error_history(&self) -> &[HarnessError] {
        &self.error_history
    }

    // --- Circuit Breaker ---

    pub fn circuit_breaker(&self) -> &CircuitBreaker {
        &self.circuit_breaker
    }

    pub fn is_circuit_open(&self, _tool_name: &str) -> bool {
        !self.circuit_breaker.is_allowed()
    }

    // --- Resource tracking ---

    pub fn resource_tracker(&self) -> &ResourceTracker {
        &self.resource_tracker
    }

    // --- Metadata access ---

    pub fn metadata(&self) -> &ExecutionMetadata {
        &self.metadata
    }

    pub fn metadata_mut(&mut self) -> &mut ExecutionMetadata {
        &mut self.metadata
    }

    pub fn config(&self) -> &HarnessConfig {
        &self.config
    }

    pub fn retry_config(&self) -> &RetryConfig {
        &self.retry_config
    }

    // --- Execution control ---

    pub fn record_step(&mut self) {
        self.metadata.record_step();
    }

    pub fn can_continue(&self) -> bool {
        !self.should_stop() && self.metadata.step_count < self.config.max_steps
    }

    pub fn remaining_steps(&self) -> usize {
        self.config.max_steps.saturating_sub(self.metadata.step_count)
    }

    // --- Wait for pause/resume ---

    pub async fn wait_if_paused(&self) {
        while self.is_paused() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    // --- Checkpoint ---

    pub async fn save_checkpoint(&self) -> Checkpoint {
        let checkpoint = Checkpoint::from_context(self, self.metadata.step_count);
        let mut cp = self.checkpoint.write().await;
        *cp = Some(checkpoint.clone());
        checkpoint
    }

    pub async fn load_checkpoint(&self) -> Option<Checkpoint> {
        let cp = self.checkpoint.read().await;
        cp.clone()
    }

    pub async fn clear_checkpoint(&self) {
        let mut cp = self.checkpoint.write().await;
        *cp = None;
    }

    // --- Execution info ---

    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub fn summary(&self) -> ExecutionSummary {
        self.metadata.to_summary()
    }

    /// Reset the context to a clean state (for context refresh strategy)
    pub fn reset(&mut self) {
        // Clear error history
        self.error_history.clear();
        // Reset circuit breaker
        self.circuit_breaker.reset();
        // Reset resource tracker
        self.resource_tracker.reset();
        // Reset step count but keep timing
        self.metadata = ExecutionMetadata::new();
        // Ensure not stopped
        self.should_stop.store(false, Ordering::SeqCst);
    }
}

// ============================================================================
// Tool Execution Result with Retry Support
// ============================================================================

#[derive(Debug)]
pub struct ToolExecResult {
    pub tool_name: String,
    pub args: String,
    pub output: Result<String, HarnessError>,
    pub attempts: u32,
    pub duration_ms: u64,
}

impl ToolExecResult {
    pub fn success(tool_name: String, args: String, output: String, attempts: u32, duration_ms: u64) -> Self {
        Self {
            tool_name,
            args,
            output: Ok(output),
            attempts,
            duration_ms,
        }
    }

    pub fn failure(tool_name: String, args: String, error: HarnessError, attempts: u32, duration_ms: u64) -> Self {
        Self {
            tool_name,
            args,
            output: Err(error),
            attempts,
            duration_ms,
        }
    }

    pub fn is_success(&self) -> bool {
        self.output.is_ok()
    }

    pub fn can_retry(&self, config: &RetryConfig) -> bool {
        self.output.is_err() &&
        self.output.as_ref().unwrap_err().is_retryable() &&
        self.attempts < config.max_retries
    }
}

// ============================================================================
// Execution Guard - RAII guard for step execution
// ============================================================================

pub struct ExecutionGuard<'a> {
    ctx: &'a mut AgentHarnessContext,
    step_start: Instant,
}

impl<'a> ExecutionGuard<'a> {
    pub fn new(ctx: &'a mut AgentHarnessContext) -> Self {
        ctx.record_step();
        ctx.metadata.update_duration();
        let step_start = Instant::now();
        Self { ctx, step_start }
    }

    pub fn check_cancellation(&self) -> Result<(), HarnessError> {
        if self.ctx.should_stop() {
            return Err(HarnessError::Cancelled);
        }
        Ok(())
    }

    pub fn check_circuit_breaker(&self, tool_name: &str) -> Result<(), HarnessError> {
        if self.ctx.is_circuit_open(tool_name) {
            return Err(HarnessError::CircuitBreakerOpen(tool_name.to_string()));
        }
        Ok(())
    }

    pub fn check_resources(&self) -> Result<(), HarnessError> {
        self.ctx.resource_tracker.check()
    }
}

impl Drop for ExecutionGuard<'_> {
    fn drop(&mut self) {
        self.ctx.metadata.update_duration();
        // Track CPU time for this step
        self.ctx.resource_tracker.add_cpu_time(
            self.step_start.elapsed().as_millis() as u64
        );
    }
}

// ============================================================================
// Utility functions
// ============================================================================

/// Parse tool call from LLM response with fallback
pub fn parse_tool_calls(response: &str) -> Vec<(String, String)> {
    let mut calls = Vec::new();

    // Try JSON parsing first
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(response) {
        if let Some(actions) = json.get("actions").and_then(|a| a.as_array()) {
            for action in actions {
                if let (Some(name), Some(args)) = (
                    action.get("name").and_then(|n| n.as_str()),
                    action.get("args").or(action.get("arguments"))
                ) {
                    let args_str = match args {
                        serde_json::Value::String(s) => s.clone(),
                        _ => args.to_string(),
                    };
                    calls.push((name.to_string(), args_str));
                }
            }
        }
    }

    // Try regex parsing as fallback
    if calls.is_empty() {
        use regex::Regex;
        use lazy_static::lazy_static;

        lazy_static! {
            static ref RE: Regex = Regex::new(
                r"(?is)Action:\s*(?:use\s+)?([\w\-]+)\s*(?:with\s+)?(?:\{\s*([\s\S]*?)\s*\}|\(([\s\S]*?)\))"
            ).expect("Invalid regex");
        }

        for cap in RE.captures_iter(response) {
            let name = cap.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
            let args = cap.get(2).or(cap.get(3))
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            if !name.is_empty() {
                calls.push((name, args));
            }
        }
    }

    calls
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_delay_calculation() {
        let config = RetryConfig::default();

        assert_eq!(config.calculate_delay(0), Duration::from_millis(100));
        assert_eq!(config.calculate_delay(1), Duration::from_millis(200));
        assert_eq!(config.calculate_delay(2), Duration::from_millis(400));
        assert_eq!(config.calculate_delay(3), Duration::from_millis(800));
    }

    #[test]
    fn test_circuit_breaker() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 3,
            success_threshold: 2,
            timeout: Duration::from_secs(1),
        });

        // Should be closed initially
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.is_allowed());

        // Record failures
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        cb.record_failure();

        // Now should be open
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.is_allowed());
    }

    #[test]
    fn test_harness_context_lifecycle() {
        let config = HarnessConfig {
            max_steps: 5,
            ..Default::default()
        };

        let mut ctx = AgentHarnessContext::new(config);

        assert!(ctx.can_continue());
        assert_eq!(ctx.remaining_steps(), 5);

        ctx.record_step();
        assert_eq!(ctx.remaining_steps(), 4);

        ctx.record_error(HarnessError::Timeout(Duration::from_secs(1)));
        assert!(ctx.has_recent_errors(1));
        assert!(ctx.get_last_error().is_some());

        ctx.cancel();
        assert!(ctx.should_stop());
        assert!(!ctx.can_continue());
    }

    #[test]
    fn test_parse_tool_calls_json() {
        let response = r#"{"actions": [{"name": "search", "args": {"query": "rust"}}, {"name": "read", "args": {"path": "/file.txt"}}]}"#;
        let calls = parse_tool_calls(response);

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "search");
        assert_eq!(calls[1].0, "read");
    }

    #[test]
    fn test_parse_tool_calls_regex() {
        let response = r#"Action: search {"query": "rust"}
Action: read {path: "/file.txt"}"#;
        let calls = parse_tool_calls(response);

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "search");
        assert_eq!(calls[1].0, "read");
    }

    #[test]
    fn test_tool_exec_result() {
        let result = ToolExecResult::success(
            "search".to_string(),
            r#"{"query": "test"}"#.to_string(),
            "results".to_string(),
            1,
            100
        );

        assert!(result.is_success());
        assert!(!result.can_retry(&RetryConfig::default()));
    }
}