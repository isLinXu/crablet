//! Adaptive Harness - Self-adjusting execution parameters
//!
//! Provides intelligent timeout and breakpoint management:
//! - Adaptive timeout based on execution history
//! - Multi-level breakpoints with conditional triggers
//! - Step duration tracking and prediction

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ============================================================================
// Adaptive Timeout
// ============================================================================

/// Configuration for adaptive timeout behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveTimeoutConfig {
    /// Base timeout for all operations
    pub base_timeout: Duration,
    /// Minimum timeout bound
    pub min_timeout: Duration,
    /// Maximum timeout bound
    pub max_timeout: Duration,
    /// Multiplier for complexity-based adjustment
    pub complexity_multiplier: f64,
    /// Window size for calculating average step duration
    pub history_window: usize,
}

impl Default for AdaptiveTimeoutConfig {
    fn default() -> Self {
        Self {
            base_timeout: Duration::from_secs(60),
            min_timeout: Duration::from_secs(10),
            max_timeout: Duration::from_secs(300),
            complexity_multiplier: 0.15,
            history_window: 10,
        }
    }
}

/// Tracks step execution history for adaptive decisions
#[derive(Debug, Clone)]
pub struct StepHistory {
    /// Duration of each step in milliseconds
    step_durations: VecDeque<u64>,
    /// Maximum window size
    window_size: usize,
}

impl StepHistory {
    pub fn new(window_size: usize) -> Self {
        Self {
            step_durations: VecDeque::with_capacity(window_size),
            window_size,
        }
    }

    /// Record a step completion
    pub fn record_step(&mut self, duration_ms: u64) {
        if self.step_durations.len() >= self.window_size {
            self.step_durations.pop_front();
        }
        self.step_durations.push_back(duration_ms);
    }

    /// Calculate average step duration
    pub fn avg_duration(&self) -> Duration {
        if self.step_durations.is_empty() {
            return Duration::from_secs(60); // Default
        }
        let sum: u64 = self.step_durations.iter().sum();
        Duration::from_millis(sum / self.step_durations.len() as u64)
    }

    /// Calculate standard deviation of step durations
    pub fn std_deviation(&self) -> Duration {
        if self.step_durations.len() < 2 {
            return Duration::from_secs(0);
        }
        let avg = self.avg_duration().as_millis() as f64;
        let variance = self.step_durations.iter()
            .map(|d| {
                let diff = *d as f64 - avg;
                diff * diff
            })
            .sum::<f64>() / self.step_durations.len() as f64;
        Duration::from_millis(variance.sqrt() as u64)
    }

    /// Check if execution is showing signs of slowdown
    pub fn is_slowing_down(&self) -> bool {
        if self.step_durations.len() < 4 {
            return false;
        }
        let split_at = self.step_durations.len() / 2;
        let recent_sum: u64 = self.step_durations.iter().skip(split_at).sum();
        let recent_len = self.step_durations.len() - split_at;
        let older_sum: u64 = self.step_durations.iter().take(split_at).sum();
        let recent_avg: u64 = recent_sum / recent_len as u64;
        let older_avg: u64 = older_sum / split_at as u64;
        recent_avg > older_avg * 2 // 2x slowdown threshold
    }
}

/// Adaptive timeout calculator
pub struct AdaptiveTimeout {
    config: AdaptiveTimeoutConfig,
    history: StepHistory,
}

impl AdaptiveTimeout {
    pub fn new(config: AdaptiveTimeoutConfig) -> Self {
        Self {
            history: StepHistory::new(config.history_window),
            config,
        }
    }

    pub fn with_default() -> Self {
        Self::new(AdaptiveTimeoutConfig::default())
    }

    /// Record step completion for history tracking
    pub fn record_step(&mut self, duration: Duration) {
        self.history.record_step(duration.as_millis() as u64);
    }

    /// Calculate adaptive timeout based on current context
    pub fn calculate_timeout(&self, step_count: usize) -> Duration {
        let avg_duration = self.history.avg_duration();
        let std_dev = self.history.std_deviation();

        // Complexity factor grows with step count
        let complexity_factor = 1.0 + (step_count as f64 * self.config.complexity_multiplier);

        // Base calculation: average duration * complexity factor + buffer for variance
        let avg_millis = avg_duration.as_millis() as f64;
        let std_dev_millis = std_dev.as_millis() as f64;
        let calculated_millis = avg_millis * complexity_factor + std_dev_millis * 2.0;
        let mut calculated = Duration::from_millis(calculated_millis as u64);

        // Clamp to bounds
        calculated = calculated.max(self.config.min_timeout);
        calculated = calculated.min(self.config.max_timeout);

        calculated
    }

    /// Get the current history for inspection
    pub fn history(&self) -> &StepHistory {
        &self.history
    }

    /// Get config
    pub fn config(&self) -> &AdaptiveTimeoutConfig {
        &self.config
    }
}

// ============================================================================
// Multi-level Breakpoints
// ============================================================================

/// Error types for breakpoint operations
#[derive(Error, Debug, Clone)]
pub enum BreakpointError {
    #[error("Breakpoint not found: {0}")]
    NotFound(String),

    #[error("Invalid breakpoint condition: {0}")]
    InvalidCondition(String),

    #[error("Breakpoint triggered: {0}")]
    Triggered(String),
}

/// Condition that can trigger a breakpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum BreakpointCondition {
    /// Trigger after N steps
    StepCount { count: usize },

    /// Trigger when token usage exceeds threshold
    TokenThreshold { threshold: u64 },

    /// Trigger when error rate exceeds percentage (0.0 - 1.0)
    ErrorRate { threshold: f64 },

    /// Trigger when specific tool is called
    ToolCall { tool_name: String },

    /// Trigger after elapsed time
    TimeElapsed { duration: Duration },

    /// Trigger when memory usage exceeds bytes
    MemoryThreshold { bytes: u64 },

    /// Trigger when CPU time exceeds ms
    CpuTimeThreshold { ms: u64 },
}

impl BreakpointCondition {
    /// Check if condition is met based on current state
    pub fn evaluate(&self, ctx: &BreakpointContext) -> bool {
        match self {
            BreakpointCondition::StepCount { count } => {
                ctx.step_count >= *count
            }
            BreakpointCondition::TokenThreshold { threshold } => {
                ctx.tokens_used.unwrap_or(0) >= *threshold
            }
            BreakpointCondition::ErrorRate { threshold } => {
                if ctx.total_calls == 0 {
                    return false;
                }
                ctx.failed_calls as f64 / ctx.total_calls as f64 >= *threshold
            }
            BreakpointCondition::ToolCall { tool_name } => {
                ctx.last_tool.as_ref() == Some(tool_name)
            }
            BreakpointCondition::TimeElapsed { duration } => {
                ctx.elapsed >= *duration
            }
            BreakpointCondition::MemoryThreshold { bytes } => {
                ctx.memory_usage >= *bytes
            }
            BreakpointCondition::CpuTimeThreshold { ms } => {
                ctx.cpu_time_ms >= *ms
            }
        }
    }

    /// Get a human-readable description
    pub fn description(&self) -> String {
        match self {
            BreakpointCondition::StepCount { count } => format!("After {} steps", count),
            BreakpointCondition::TokenThreshold { threshold } => format!("Token usage >= {}", threshold),
            BreakpointCondition::ErrorRate { threshold } => format!("Error rate >= {}%", (*threshold * 100.0) as i32),
            BreakpointCondition::ToolCall { tool_name } => format!("Tool call: {}", tool_name),
            BreakpointCondition::TimeElapsed { duration } => format!("After {:?}", duration),
            BreakpointCondition::MemoryThreshold { bytes } => format!("Memory >= {} bytes", bytes),
            BreakpointCondition::CpuTimeThreshold { ms } => format!("CPU time >= {} ms", ms),
        }
    }
}

/// Context information for breakpoint evaluation
#[derive(Debug, Clone)]
pub struct BreakpointContext {
    pub step_count: usize,
    pub tokens_used: Option<u64>,
    pub total_calls: usize,
    pub failed_calls: usize,
    pub last_tool: Option<String>,
    pub elapsed: Duration,
    pub memory_usage: u64,
    pub cpu_time_ms: u64,
}

impl Default for BreakpointContext {
    fn default() -> Self {
        Self {
            step_count: 0,
            tokens_used: None,
            total_calls: 0,
            failed_calls: 0,
            last_tool: None,
            elapsed: Duration::from_secs(0),
            memory_usage: 0,
            cpu_time_ms: 0,
        }
    }
}

/// Action to take when breakpoint is triggered
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", content = "value")]
pub enum BreakpointAction {
    /// Pause execution and wait for human confirmation
    Pause,

    /// Log the event and continue execution
    LogAndContinue { message: String },

    /// Cancel execution immediately
    Cancel,

    /// Switch to alternative strategy
    SwitchStrategy { strategy: String },

    /// Trigger reflection/rethinking
    Reflect,
}

/// A configured breakpoint
#[derive(Debug, Clone)]
pub struct Breakpoint {
    /// Unique identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Condition that triggers this breakpoint
    pub condition: BreakpointCondition,
    /// Action to take when triggered
    pub action: BreakpointAction,
    /// Whether this breakpoint is enabled
    pub enabled: bool,
    /// Metadata for custom data
    pub metadata: HashMap<String, String>,
    /// Number of times this breakpoint has fired
    pub fire_count: u64,
}

impl Breakpoint {
    pub fn new(id: &str, name: &str, condition: BreakpointCondition, action: BreakpointAction) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            condition,
            action,
            enabled: true,
            metadata: HashMap::new(),
            fire_count: 0,
        }
    }

    /// Check and potentially trigger this breakpoint
    pub fn check(&mut self, ctx: &BreakpointContext) -> Option<BreakpointAction> {
        if !self.enabled {
            return None;
        }

        if self.condition.evaluate(ctx) {
            self.fire_count += 1;
            Some(self.action.clone())
        } else {
            None
        }
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

/// Breakpoint manager for handling multiple breakpoints
pub struct BreakpointManager {
    breakpoints: Arc<RwLock<Vec<Breakpoint>>>,
    /// Channel for breakpoint notifications
    notification_tx: tokio::sync::broadcast::Sender<BreakpointEvent>,
}

/// Event emitted when a breakpoint is triggered
#[derive(Debug, Clone)]
pub struct BreakpointEvent {
    pub breakpoint_id: String,
    pub breakpoint_name: String,
    pub action: BreakpointAction,
    pub context: BreakpointContext,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl BreakpointManager {
    pub fn new() -> Self {
        let (tx, _) = tokio::sync::broadcast::channel(100);
        Self {
            breakpoints: Arc::new(RwLock::new(Vec::new())),
            notification_tx: tx,
        }
    }

    /// Add a new breakpoint
    pub async fn add_breakpoint(&self, breakpoint: Breakpoint) {
        let mut breakpoints = self.breakpoints.write().await;
        breakpoints.push(breakpoint);
    }

    /// Remove a breakpoint by ID
    pub async fn remove_breakpoint(&self, id: &str) -> bool {
        let mut breakpoints = self.breakpoints.write().await;
        let len_before = breakpoints.len();
        breakpoints.retain(|bp| bp.id != id);
        breakpoints.len() < len_before
    }

    /// Get a breakpoint by ID
    pub async fn get_breakpoint(&self, id: &str) -> Option<Breakpoint> {
        let breakpoints = self.breakpoints.read().await;
        breakpoints.iter().find(|bp| bp.id == id).cloned()
    }

    /// List all breakpoints
    pub async fn list_breakpoints(&self) -> Vec<Breakpoint> {
        let breakpoints = self.breakpoints.read().await;
        breakpoints.clone()
    }

    /// Enable/disable a breakpoint
    pub async fn set_enabled(&self, id: &str, enabled: bool) -> bool {
        let mut breakpoints = self.breakpoints.write().await;
        if let Some(bp) = breakpoints.iter_mut().find(|bp| bp.id == id) {
            bp.enabled = enabled;
            true
        } else {
            false
        }
    }

    /// Check all breakpoints and return triggered actions
    pub async fn check_all(&self, ctx: &BreakpointContext) -> Vec<BreakpointEvent> {
        let mut breakpoints = self.breakpoints.write().await;
        let mut events = Vec::new();

        for bp in breakpoints.iter_mut() {
            if let Some(action) = bp.check(ctx) {
                let event = BreakpointEvent {
                    breakpoint_id: bp.id.clone(),
                    breakpoint_name: bp.name.clone(),
                    action: action.clone(),
                    context: ctx.clone(),
                    timestamp: chrono::Utc::now(),
                };
                events.push(event.clone());

                // Send notification
                let _ = self.notification_tx.send(event);
            }
        }

        events
    }

    /// Subscribe to breakpoint events
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<BreakpointEvent> {
        self.notification_tx.subscribe()
    }

    /// Create common predefined breakpoints
    pub async fn add_common_breakpoints(&self) {
        // Step limit breakpoint
        self.add_breakpoint(Breakpoint::new(
            "step_limit",
            "Step Limit",
            BreakpointCondition::StepCount { count: 100 },
            BreakpointAction::Cancel,
        )).await;

        // Token budget breakpoint
        self.add_breakpoint(Breakpoint::new(
            "token_budget",
            "Token Budget",
            BreakpointCondition::TokenThreshold { threshold: 100_000 },
            BreakpointAction::LogAndContinue { message: "Token budget reached".to_string() },
        )).await;

        // High error rate breakpoint
        self.add_breakpoint(Breakpoint::new(
            "high_error_rate",
            "High Error Rate",
            BreakpointCondition::ErrorRate { threshold: 0.5 },
            BreakpointAction::Reflect,
        )).await;

        // Timeout breakpoint
        self.add_breakpoint(Breakpoint::new(
            "timeout",
            "Execution Timeout",
            BreakpointCondition::TimeElapsed { duration: Duration::from_secs(600) },
            BreakpointAction::Cancel,
        )).await;
    }
}

impl Default for BreakpointManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Integration with Harness
// ============================================================================

/// Extended harness context with adaptive features
pub struct AdaptiveHarnessExt {
    pub adaptive_timeout: AdaptiveTimeout,
    pub breakpoint_manager: BreakpointManager,
    step_start_times: Vec<Instant>,
}

impl AdaptiveHarnessExt {
    pub fn new(adaptive_config: Option<AdaptiveTimeoutConfig>) -> Self {
        Self {
            adaptive_timeout: AdaptiveTimeout::new(
                adaptive_config.unwrap_or_default()
            ),
            breakpoint_manager: BreakpointManager::new(),
            step_start_times: Vec::new(),
        }
    }

    /// Start timing a step
    pub fn start_step(&mut self) {
        self.step_start_times.push(Instant::now());
    }

    /// End timing current step and record duration
    pub fn end_step(&mut self) {
        if let Some(start) = self.step_start_times.pop() {
            let duration = start.elapsed();
            self.adaptive_timeout.record_step(duration);
        }
    }

    /// Get current step timeout
    pub fn current_timeout(&self, step_count: usize) -> Duration {
        self.adaptive_timeout.calculate_timeout(step_count)
    }

    /// Check breakpoints with current context
    pub async fn check_breakpoints(&self, ctx: &BreakpointContext) -> Vec<BreakpointEvent> {
        self.breakpoint_manager.check_all(ctx).await
    }

    /// Add custom breakpoint
    pub async fn add_breakpoint(&self, breakpoint: Breakpoint) {
        self.breakpoint_manager.add_breakpoint(breakpoint).await;
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adaptive_timeout_calculation() {
        let mut timeout = AdaptiveTimeout::with_default();

        // Record some steps
        timeout.record_step(Duration::from_millis(100));
        timeout.record_step(Duration::from_millis(150));
        timeout.record_step(Duration::from_millis(120));

        // Calculate timeout for step 5
        let calc_timeout = timeout.calculate_timeout(5);
        assert!(calc_timeout >= Duration::from_secs(10));
        assert!(calc_timeout <= Duration::from_secs(300));
    }

    #[test]
    fn test_step_history() {
        let mut history = StepHistory::new(5);
        history.record_step(100);
        history.record_step(200);
        history.record_step(150);

        assert_eq!(history.avg_duration(), Duration::from_millis(150));
    }

    #[test]
    fn test_breakpoint_step_count() {
        let bp = Breakpoint::new(
            "test",
            "Test",
            BreakpointCondition::StepCount { count: 5 },
            BreakpointAction::Cancel,
        );

        let ctx = BreakpointContext {
            step_count: 4,
            ..Default::default()
        };
        assert!(!bp.condition.evaluate(&ctx));

        let ctx = BreakpointContext {
            step_count: 5,
            ..Default::default()
        };
        assert!(bp.condition.evaluate(&ctx));
    }

    #[test]
    fn test_breakpoint_error_rate() {
        let bp = Breakpoint::new(
            "test",
            "Test",
            BreakpointCondition::ErrorRate { threshold: 0.5 },
            BreakpointAction::Cancel,
        );

        let ctx = BreakpointContext {
            total_calls: 10,
            failed_calls: 4,
            ..Default::default()
        };
        assert!(!bp.condition.evaluate(&ctx));

        let ctx = BreakpointContext {
            total_calls: 10,
            failed_calls: 6,
            ..Default::default()
        };
        assert!(bp.condition.evaluate(&ctx));
    }

    #[tokio::test]
    async fn test_breakpoint_manager() {
        let manager = BreakpointManager::new();

        manager.add_breakpoint(Breakpoint::new(
            "test",
            "Test Breakpoint",
            BreakpointCondition::StepCount { count: 10 },
            BreakpointAction::LogAndContinue { message: "Test".to_string() },
        )).await;

        let breakpoints = manager.list_breakpoints().await;
        assert_eq!(breakpoints.len(), 1);
        assert_eq!(breakpoints[0].id, "test");

        let removed = manager.remove_breakpoint("test").await;
        assert!(removed);

        let breakpoints = manager.list_breakpoints().await;
        assert!(breakpoints.is_empty());
    }

    #[tokio::test]
    async fn test_breakpoint_trigger() {
        let manager = BreakpointManager::new();

        manager.add_breakpoint(Breakpoint::new(
            "test",
            "Test",
            BreakpointCondition::StepCount { count: 5 },
            BreakpointAction::Cancel,
        )).await;

        let ctx = BreakpointContext {
            step_count: 5,
            ..Default::default()
        };

        let events = manager.check_all(&ctx).await;
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].action, BreakpointAction::Cancel));
    }
}
