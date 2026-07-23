//! Core harness primitives for agent execution.
//!
//! This module provides the execution context and fault-tolerance helpers that
//! higher-level harness-aware agents, tool executors, and distributed managers
//! depend on.

use std::collections::{HashMap, VecDeque};
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

fn lock_guard<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Circuit breaker configuration for tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: usize,
    pub success_threshold: usize,
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

/// Public circuit breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug)]
struct CircuitBreakerInner {
    state: CircuitState,
    failure_count: usize,
    success_count: usize,
    opened_at: Option<Instant>,
}

/// Lightweight synchronous circuit breaker for harness primitives.
#[derive(Debug)]
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    inner: Mutex<CircuitBreakerInner>,
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            inner: Mutex::new(CircuitBreakerInner {
                state: CircuitState::Closed,
                failure_count: 0,
                success_count: 0,
                opened_at: None,
            }),
        }
    }

    fn refresh_state(&self, inner: &mut CircuitBreakerInner) {
        if inner.state == CircuitState::Open {
            if let Some(opened_at) = inner.opened_at {
                if opened_at.elapsed() >= self.config.timeout {
                    inner.state = CircuitState::HalfOpen;
                    inner.success_count = 0;
                }
            }
        }
    }

    pub fn state(&self) -> CircuitState {
        let mut inner = lock_guard(&self.inner);
        self.refresh_state(&mut inner);
        inner.state
    }

    pub fn is_allowed(&self) -> bool {
        let mut inner = lock_guard(&self.inner);
        self.refresh_state(&mut inner);
        inner.state != CircuitState::Open
    }

    pub fn record_failure(&self) {
        let mut inner = lock_guard(&self.inner);
        self.refresh_state(&mut inner);

        match inner.state {
            CircuitState::HalfOpen => {
                inner.state = CircuitState::Open;
                inner.failure_count = self.config.failure_threshold;
                inner.success_count = 0;
                inner.opened_at = Some(Instant::now());
            }
            CircuitState::Closed | CircuitState::Open => {
                inner.failure_count += 1;
                inner.success_count = 0;
                if inner.failure_count >= self.config.failure_threshold {
                    inner.state = CircuitState::Open;
                    inner.opened_at = Some(Instant::now());
                }
            }
        }
    }

    pub fn record_success(&self) {
        let mut inner = lock_guard(&self.inner);
        self.refresh_state(&mut inner);

        match inner.state {
            CircuitState::HalfOpen => {
                inner.success_count += 1;
                if inner.success_count >= self.config.success_threshold {
                    inner.state = CircuitState::Closed;
                    inner.failure_count = 0;
                    inner.success_count = 0;
                    inner.opened_at = None;
                }
            }
            CircuitState::Closed => {
                inner.failure_count = 0;
            }
            CircuitState::Open => {}
        }
    }

    pub fn reset(&self) {
        let mut inner = lock_guard(&self.inner);
        inner.state = CircuitState::Closed;
        inner.failure_count = 0;
        inner.success_count = 0;
        inner.opened_at = None;
    }
}

/// Base harness configuration shared by agent execution helpers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessConfig {
    pub max_steps: usize,
    pub tool_timeout: Duration,
    pub step_timeout: Duration,
    pub enable_self_reflection: bool,
    pub circuit_breaker: Option<CircuitBreakerConfig>,
    pub max_memory_bytes: Option<u64>,
    pub max_cpu_time_ms: Option<u64>,
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
            max_memory_bytes: Some(512 * 1024 * 1024),
            max_cpu_time_ms: Some(60_000),
            metadata: HashMap::new(),
        }
    }
}

/// Retry policy for tool execution.
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
            base_delay: Duration::from_millis(250),
            max_delay: Duration::from_secs(5),
            multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        let base_ms = self.base_delay.as_millis() as f64;
        let max_ms = self.max_delay.as_millis() as f64;
        let factor = self.multiplier.powi(attempt as i32);
        let delay_ms = (base_ms * factor).min(max_ms).max(0.0);
        Duration::from_millis(delay_ms.round() as u64)
    }
}

/// Execution metadata captured by a harness context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetadata {
    pub started_at: DateTime<Utc>,
    pub last_activity_at: DateTime<Utc>,
    pub total_duration_ms: u64,
    pub step_count: usize,
    pub error_count: usize,
    pub tool_call_count: usize,
    pub successful_tool_calls: usize,
    pub paused: bool,
    pub cancelled: bool,
}

impl ExecutionMetadata {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            started_at: now,
            last_activity_at: now,
            total_duration_ms: 0,
            step_count: 0,
            error_count: 0,
            tool_call_count: 0,
            successful_tool_calls: 0,
            paused: false,
            cancelled: false,
        }
    }

    pub fn update_duration(&mut self) {
        let now = Utc::now();
        self.last_activity_at = now;
        self.total_duration_ms = now
            .signed_duration_since(self.started_at)
            .num_milliseconds()
            .max(0) as u64;
    }
}

impl Default for ExecutionMetadata {
    fn default() -> Self {
        Self::new()
    }
}

/// Mutable metadata access backed by the harness mutex.
pub struct ExecutionMetadataGuard<'a> {
    guard: MutexGuard<'a, ExecutionMetadata>,
}

impl<'a> Deref for ExecutionMetadataGuard<'a> {
    type Target = ExecutionMetadata;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl<'a> DerefMut for ExecutionMetadataGuard<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}

/// Errors surfaced by the harness execution layer.
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
pub enum HarnessError {
    #[error("LLM failure: {0}")]
    LlmFailure(String),

    #[error("Tool failure for {0}: {1}")]
    ToolFailure(String, String),

    #[error("Operation timed out after {0:?}")]
    Timeout(Duration),

    #[error("Circuit breaker open for tool: {0}")]
    CircuitBreakerOpen(String),

    #[error("Resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),

    #[error("Max retries exceeded for: {0}")]
    MaxRetriesExceeded(String),

    #[error("Execution cancelled")]
    Cancelled,

    #[error("Harness context closed")]
    ContextClosed,

    #[error("Resume state unavailable: {0}")]
    ResumeStateUnavailable(String),
}

impl HarnessError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            HarnessError::LlmFailure(_)
                | HarnessError::ToolFailure(_, _)
                | HarnessError::Timeout(_)
        )
    }
}

/// Control signals used by local and distributed harness managers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HarnessSignal {
    Cancel,
    Pause,
    Resume,
    Checkpoint,
}

/// Broadcast wrapper for harness lifecycle signals.
#[derive(Debug, Clone)]
pub struct HarnessSignalChannel {
    sender: broadcast::Sender<HarnessSignal>,
}

impl HarnessSignalChannel {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(32);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<HarnessSignal> {
        self.sender.subscribe()
    }

    pub fn cancel(&self) -> bool {
        self.sender.send(HarnessSignal::Cancel).is_ok()
    }

    pub fn pause(&self) -> bool {
        self.sender.send(HarnessSignal::Pause).is_ok()
    }

    pub fn resume(&self) -> bool {
        self.sender.send(HarnessSignal::Resume).is_ok()
    }

    pub fn checkpoint(&self) -> bool {
        self.sender.send(HarnessSignal::Checkpoint).is_ok()
    }
}

impl Default for HarnessSignalChannel {
    fn default() -> Self {
        Self::new()
    }
}

/// Serializable result for tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecResult {
    pub tool_name: String,
    pub args: String,
    pub output: Option<String>,
    pub error: Option<String>,
    pub error_kind: Option<HarnessError>,
    pub attempts: u32,
    pub duration_ms: u64,
    pub success: bool,
}

impl ToolExecResult {
    pub fn success(
        tool_name: String,
        args: String,
        output: String,
        attempts: u32,
        duration_ms: u64,
    ) -> Self {
        Self {
            tool_name,
            args,
            output: Some(output),
            error: None,
            error_kind: None,
            attempts,
            duration_ms,
            success: true,
        }
    }

    pub fn failure(
        tool_name: String,
        args: String,
        error: HarnessError,
        attempts: u32,
        duration_ms: u64,
    ) -> Self {
        Self {
            tool_name,
            args,
            output: None,
            error: Some(error.to_string()),
            error_kind: Some(error),
            attempts,
            duration_ms,
            success: false,
        }
    }

    pub fn is_success(&self) -> bool {
        self.success
    }

    pub fn can_retry(&self, config: &RetryConfig) -> bool {
        !self.success
            && self.attempts < config.max_retries
            && self
                .error_kind
                .as_ref()
                .map(HarnessError::is_retryable)
                .unwrap_or(false)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HarnessState {
    Running,
    Paused,
    Cancelled,
}

/// Persisted harness checkpoint.
pub const HARNESS_CHECKPOINT_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessCheckpoint {
    #[serde(default = "harness_checkpoint_schema_version")]
    pub schema_version: u16,
    pub step_number: usize,
    pub error_history: Vec<HarnessError>,
    pub metadata: ExecutionMetadata,
    pub config: HarnessConfig,
}

fn harness_checkpoint_schema_version() -> u16 {
    HARNESS_CHECKPOINT_SCHEMA_VERSION
}

/// Tracks simple memory/CPU budgets for long-running harnesses.
#[derive(Debug)]
pub struct ResourceTracker {
    max_memory_bytes: u64,
    max_cpu_time_ms: u64,
    current_memory_bytes: AtomicU64,
    current_cpu_time_ms: AtomicU64,
}

impl ResourceTracker {
    pub fn new(max_memory_bytes: u64, max_cpu_time_ms: u64) -> Self {
        Self {
            max_memory_bytes,
            max_cpu_time_ms,
            current_memory_bytes: AtomicU64::new(0),
            current_cpu_time_ms: AtomicU64::new(0),
        }
    }

    pub fn update_memory(&self, bytes: u64) {
        self.current_memory_bytes.store(bytes, Ordering::SeqCst);
    }

    pub fn add_cpu_time(&self, ms: u64) {
        self.current_cpu_time_ms.fetch_add(ms, Ordering::SeqCst);
    }

    pub fn check(&self) -> Result<(), HarnessError> {
        let memory = self.current_memory_bytes.load(Ordering::SeqCst);
        if memory >= self.max_memory_bytes {
            return Err(HarnessError::ResourceLimitExceeded(format!(
                "memory usage {} >= {}",
                memory, self.max_memory_bytes
            )));
        }

        let cpu = self.current_cpu_time_ms.load(Ordering::SeqCst);
        if cpu >= self.max_cpu_time_ms {
            return Err(HarnessError::ResourceLimitExceeded(format!(
                "cpu time {} >= {}",
                cpu, self.max_cpu_time_ms
            )));
        }

        Ok(())
    }

    pub fn reset(&self) {
        self.current_memory_bytes.store(0, Ordering::SeqCst);
        self.current_cpu_time_ms.store(0, Ordering::SeqCst);
    }
}

/// In-memory execution context tracked across agent steps.
#[derive(Debug)]
pub struct AgentHarnessContext {
    config: HarnessConfig,
    metadata: Mutex<ExecutionMetadata>,
    errors: Mutex<VecDeque<HarnessError>>,
    state: Mutex<HarnessState>,
    tool_failures: Mutex<HashMap<String, usize>>,
    tool_successes: Mutex<HashMap<String, usize>>,
    tool_circuit_opened_at: Mutex<HashMap<String, DateTime<Utc>>>,
}

impl AgentHarnessContext {
    fn effective_circuit_breaker(&self) -> CircuitBreakerConfig {
        self.config.circuit_breaker.clone().unwrap_or_default()
    }

    pub fn new(config: HarnessConfig) -> Self {
        Self {
            config,
            metadata: Mutex::new(ExecutionMetadata::new()),
            errors: Mutex::new(VecDeque::new()),
            state: Mutex::new(HarnessState::Running),
            tool_failures: Mutex::new(HashMap::new()),
            tool_successes: Mutex::new(HashMap::new()),
            tool_circuit_opened_at: Mutex::new(HashMap::new()),
        }
    }

    pub fn config(&self) -> &HarnessConfig {
        &self.config
    }

    pub fn replace_config(&mut self, config: HarnessConfig) {
        self.config = config;
    }

    pub fn metadata(&self) -> ExecutionMetadata {
        lock_guard(&self.metadata).clone()
    }

    pub fn metadata_mut(&self) -> ExecutionMetadataGuard<'_> {
        ExecutionMetadataGuard {
            guard: lock_guard(&self.metadata),
        }
    }

    pub fn remaining_steps(&self) -> usize {
        let metadata = self.metadata();
        self.config.max_steps.saturating_sub(metadata.step_count)
    }

    pub fn record_step(&self) {
        let mut metadata = lock_guard(&self.metadata);
        metadata.step_count += 1;
        metadata.update_duration();
    }

    pub fn record_error(&self, error: HarnessError) {
        if let HarnessError::ToolFailure(tool_name, _) = &error {
            {
                let mut failures = lock_guard(&self.tool_failures);
                let count = failures.entry(tool_name.clone()).or_default();
                *count += 1;
            }
            {
                let mut successes = lock_guard(&self.tool_successes);
                successes.insert(tool_name.clone(), 0);
            }

            let cb = self.effective_circuit_breaker();
            let failures = lock_guard(&self.tool_failures);
            if failures.get(tool_name).copied().unwrap_or(0) >= cb.failure_threshold {
                let mut opened_at = lock_guard(&self.tool_circuit_opened_at);
                opened_at.entry(tool_name.clone()).or_insert_with(Utc::now);
            }
        }

        {
            let mut metadata = lock_guard(&self.metadata);
            metadata.error_count += 1;
            metadata.update_duration();
        }

        let mut errors = lock_guard(&self.errors);
        if errors.len() >= 64 {
            errors.pop_front();
        }
        errors.push_back(error);
    }

    pub fn record_tool_call(&self, success: bool) {
        let mut metadata = lock_guard(&self.metadata);
        metadata.tool_call_count += 1;
        if success {
            metadata.successful_tool_calls += 1;
        }
        metadata.update_duration();
    }

    pub fn record_tool_success(&self, tool_name: &str) {
        let cb = self.effective_circuit_breaker();
        let mut successes = lock_guard(&self.tool_successes);
        let count = successes.entry(tool_name.to_string()).or_default();
        *count += 1;

        if *count >= cb.success_threshold {
            lock_guard(&self.tool_failures).remove(tool_name);
            successes.remove(tool_name);
            lock_guard(&self.tool_circuit_opened_at).remove(tool_name);
        }

        lock_guard(&self.metadata).update_duration();
    }

    pub fn error_count(&self) -> usize {
        self.metadata().error_count
    }

    pub fn error_history(&self) -> Vec<HarnessError> {
        lock_guard(&self.errors).iter().cloned().collect()
    }

    /// Restore metadata and recent errors from a persisted snapshot.
    pub fn restore_snapshot(&self, metadata: ExecutionMetadata, error_history: Vec<HarnessError>) {
        *lock_guard(&self.metadata) = metadata.clone();

        let mut errors = lock_guard(&self.errors);
        errors.clear();
        let start = error_history.len().saturating_sub(64);
        for error in error_history.into_iter().skip(start) {
            errors.push_back(error);
        }
        drop(errors);

        *lock_guard(&self.state) = if metadata.cancelled {
            HarnessState::Cancelled
        } else if metadata.paused {
            HarnessState::Paused
        } else {
            HarnessState::Running
        };

        lock_guard(&self.tool_failures).clear();
        lock_guard(&self.tool_successes).clear();
        lock_guard(&self.tool_circuit_opened_at).clear();
    }

    pub fn has_recent_errors(&self, count: usize) -> bool {
        lock_guard(&self.errors).len() >= count
    }

    pub fn circuit_breaker_for(&self, tool_name: &str) -> CircuitState {
        let config = self.effective_circuit_breaker();

        let failures = lock_guard(&self.tool_failures);
        let failure_count = failures.get(tool_name).copied().unwrap_or(0);
        drop(failures);

        if failure_count < config.failure_threshold {
            return CircuitState::Closed;
        }

        let opened_at = lock_guard(&self.tool_circuit_opened_at)
            .get(tool_name)
            .copied();

        match opened_at {
            Some(started) => {
                let elapsed = Utc::now()
                    .signed_duration_since(started)
                    .to_std()
                    .unwrap_or_default();
                if elapsed >= config.timeout {
                    CircuitState::HalfOpen
                } else {
                    CircuitState::Open
                }
            }
            None => CircuitState::Open,
        }
    }

    pub fn is_circuit_open(&self, tool_name: &str) -> bool {
        self.circuit_breaker_for(tool_name) == CircuitState::Open
    }

    pub fn can_continue(&self) -> bool {
        !matches!(*lock_guard(&self.state), HarnessState::Cancelled)
            && self.metadata().step_count < self.config.max_steps
    }

    pub fn should_stop(&self) -> bool {
        matches!(*lock_guard(&self.state), HarnessState::Cancelled)
    }

    pub fn is_paused(&self) -> bool {
        matches!(*lock_guard(&self.state), HarnessState::Paused)
    }

    pub async fn wait_if_paused(&self) {
        while self.is_paused() {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    pub fn pause(&self) {
        *lock_guard(&self.state) = HarnessState::Paused;
        let mut metadata = lock_guard(&self.metadata);
        metadata.paused = true;
        metadata.update_duration();
    }

    pub fn resume(&self) {
        *lock_guard(&self.state) = HarnessState::Running;
        let mut metadata = lock_guard(&self.metadata);
        metadata.paused = false;
        metadata.update_duration();
    }

    pub fn cancel(&self) {
        *lock_guard(&self.state) = HarnessState::Cancelled;
        let mut metadata = lock_guard(&self.metadata);
        metadata.cancelled = true;
        metadata.update_duration();
    }

    fn checkpoint_dir(&self) -> PathBuf {
        self.config
            .metadata
            .get("checkpoint_dir")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("crablet-harness-checkpoints"))
    }

    fn checkpoint_prefix(&self) -> String {
        let metadata = self.metadata();
        format!("harness-{}-", metadata.started_at.timestamp_millis())
    }

    pub fn checkpoint_artifact(&self) -> (HarnessCheckpoint, PathBuf) {
        let checkpoint = HarnessCheckpoint {
            schema_version: HARNESS_CHECKPOINT_SCHEMA_VERSION,
            step_number: self.metadata().step_count,
            error_history: lock_guard(&self.errors).iter().cloned().collect(),
            metadata: self.metadata(),
            config: self.config.clone(),
        };

        let checkpoint_dir = self.checkpoint_dir();
        let checkpoint_name = format!(
            "{}{}.json",
            self.checkpoint_prefix(),
            checkpoint.step_number
        );

        (checkpoint, checkpoint_dir.join(checkpoint_name))
    }

    pub async fn persist_checkpoint_artifact(checkpoint: &HarnessCheckpoint, path: PathBuf) {
        if let Some(checkpoint_dir) = path.parent() {
            if tokio::fs::create_dir_all(checkpoint_dir).await.is_ok() {
                if let Ok(data) = serde_json::to_vec_pretty(checkpoint) {
                    let _ = tokio::fs::write(path, data).await;
                }
            }
        }
    }

    pub async fn save_checkpoint(&self) -> HarnessCheckpoint {
        let (checkpoint, path) = self.checkpoint_artifact();
        Self::persist_checkpoint_artifact(&checkpoint, path).await;

        checkpoint
    }

    pub async fn load_checkpoint(&self) -> Option<HarnessCheckpoint> {
        let checkpoint_dir = self.checkpoint_dir();
        let prefix = self.checkpoint_prefix();
        let mut entries = tokio::fs::read_dir(checkpoint_dir).await.ok()?;
        let mut candidate_paths = Vec::new();

        while let Ok(Some(entry)) = entries.next_entry().await {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();
            if file_name.starts_with(&prefix) && file_name.ends_with(".json") {
                candidate_paths.push(entry.path());
            }
        }

        candidate_paths.sort();
        let path = candidate_paths.pop()?;
        let data = tokio::fs::read(path).await.ok()?;
        serde_json::from_slice(&data).ok()
    }

    /// Restore the existing harness state from its latest persisted checkpoint.
    pub async fn resume_from_checkpoint(&mut self) -> Result<HarnessCheckpoint, HarnessError> {
        let checkpoint = self
            .load_checkpoint()
            .await
            .ok_or_else(|| HarnessError::ResumeStateUnavailable("checkpoint not found".into()))?;
        if checkpoint.schema_version != HARNESS_CHECKPOINT_SCHEMA_VERSION {
            return Err(HarnessError::ResumeStateUnavailable(format!(
                "unsupported checkpoint schema version {}",
                checkpoint.schema_version
            )));
        }

        self.replace_config(checkpoint.config.clone());
        self.restore_snapshot(
            checkpoint.metadata.clone(),
            checkpoint.error_history.clone(),
        );
        Ok(checkpoint)
    }

    pub fn reset(&self) {
        *lock_guard(&self.metadata) = ExecutionMetadata::new();
        lock_guard(&self.errors).clear();
        *lock_guard(&self.state) = HarnessState::Running;
        lock_guard(&self.tool_failures).clear();
        lock_guard(&self.tool_successes).clear();
        lock_guard(&self.tool_circuit_opened_at).clear();
    }
}

/// Parse tool calls from either JSON action arrays or line-based action text.
pub fn parse_tool_calls(response: &str) -> Vec<(String, serde_json::Value)> {
    if response.trim().is_empty() {
        return Vec::new();
    }

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(response) {
        if let Some(actions) = value.get("actions").and_then(|actions| actions.as_array()) {
            let mut parsed = Vec::new();
            for action in actions {
                if let Some(name) = action.get("name").and_then(|name| name.as_str()) {
                    let args = action
                        .get("args")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!({}));
                    parsed.push((name.to_string(), args));
                }
            }
            if !parsed.is_empty() {
                return parsed;
            }
        }
    }

    let mut parsed = Vec::new();
    for line in response.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("Action:") else {
            continue;
        };

        let rest = rest.trim();
        if rest.is_empty() {
            continue;
        }

        let mut parts = rest.splitn(2, char::is_whitespace);
        let Some(name) = parts.next() else {
            continue;
        };
        let raw_args = parts.next().unwrap_or("").trim();
        let args = serde_json::from_str::<serde_json::Value>(raw_args)
            .unwrap_or_else(|_| serde_json::json!({ "raw": raw_args }));
        parsed.push((name.to_string(), args));
    }

    parsed
}

#[cfg(test)]
mod runtime_contract_tests {
    use super::*;

    #[test]
    fn checkpoint_round_trip_preserves_version_and_resumable_state() {
        let source = AgentHarnessContext::new(HarnessConfig::default());
        source.record_step();
        source.pause();
        let (checkpoint, _) = source.checkpoint_artifact();

        let json = serde_json::to_string(&checkpoint).unwrap();
        let decoded: HarnessCheckpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.schema_version, HARNESS_CHECKPOINT_SCHEMA_VERSION);
        assert_eq!(decoded.step_number, 1);

        let restored = AgentHarnessContext::new(HarnessConfig::default());
        restored.restore_snapshot(decoded.metadata, decoded.error_history);
        assert_eq!(restored.metadata().step_count, 1);
        assert!(restored.is_paused());
    }

    #[test]
    fn legacy_checkpoint_defaults_to_current_schema() {
        let context = AgentHarnessContext::new(HarnessConfig::default());
        let (checkpoint, _) = context.checkpoint_artifact();
        let mut value = serde_json::to_value(checkpoint).unwrap();
        value.as_object_mut().unwrap().remove("schema_version");

        let decoded: HarnessCheckpoint = serde_json::from_value(value).unwrap();
        assert_eq!(decoded.schema_version, HARNESS_CHECKPOINT_SCHEMA_VERSION);
    }
}
