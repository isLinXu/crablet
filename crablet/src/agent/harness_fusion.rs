//! Unified Harness Fusion Engine
//!
//! 融合自适应超时 + 熔断器 + 自愈系统 + 指标收集的统一入口
//!
//! # 核心特性
//! - 自适应超时基于熔断状态的动态调整
//! - 断点触发时自动调用自愈策略
//! - 资源使用实时反馈到超时计算
//! - 统一的指标收集和导出
//!
//! # 架构
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │              UnifiedHarnessFusionEngine                     │
//│ ├─────────────────────────────────────────────────────────────┤
//│ │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐   │
//│ │  │   Harness   │  │   Circuit   │  │  Self-Healing   │   │
//│ │  │   Context   │  │   Breaker   │  │     Engine      │   │
//│ │  └──────┬──────┘  └──────┬──────┘  └────────┬────────┘   │
//│ │         │                │                   │             │
//│ │  ┌──────▼──────────────────▼──────────────────▼────────┐  │
//│ │  │              Adaptive Timeout Engine                 │  │
//│ │  │   (融合熔断状态 + 历史执行 + 资源使用)               │  │
//│ │  └──────────────────────┬───────────────────────────────┘  │
//│ │                         │                                   │
//│ │  ┌──────────────────────▼───────────────────────────────┐  │
//│ │  │              Metrics Collector                      │  │
//│ │  │   (Prometheus + OpenTelemetry + JSON)              │  │
//│ │  └─────────────────────────────────────────────────────┘  │
//│ └─────────────────────────────────────────────────────────────┘
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, broadcast};
use tokio::sync::broadcast::Sender;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::harness::{AgentHarnessContext, HarnessConfig, HarnessError};
use super::adaptive_harness::{AdaptiveTimeout, AdaptiveTimeoutConfig, BreakpointManager, BreakpointAction, BreakpointContext};
use super::self_healing_agent::{DiagnosticEngine, ErrorType, RepairOutcome, RepairStrategy};
use super::metrics::{Counter, Gauge};

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum FusionError {
    #[error("Harness error: {0}")]
    HarnessError(String),

    #[error("Self-healing failed: {0}")]
    SelfHealingFailed(String),

    #[error("Metric collection failed: {0}")]
    MetricError(String),

    #[error("Fusion engine is closed")]
    EngineClosed,

    #[error("All repair strategies exhausted")]
    RepairExhausted,
}

impl From<HarnessError> for FusionError {
    fn from(e: HarnessError) -> Self {
        FusionError::HarnessError(e.to_string())
    }
}

// ============================================================================
// Fusion Configuration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionConfig {
    /// Enable self-healing on errors
    pub enable_self_healing: bool,
    /// Enable adaptive timeout
    pub enable_adaptive_timeout: bool,
    /// Enable metrics collection
    pub enable_metrics: bool,
    /// Max repair attempts before giving up
    pub max_repair_attempts: u32,
    /// Adaptive timeout configuration
    pub adaptive_timeout_config: AdaptiveTimeoutConfig,
    /// Circuit breaker sensitivity (0.0 - 1.0, higher = more sensitive)
    pub circuit_sensitivity: f64,
    /// Resource pressure threshold for timeout adjustment
    pub resource_pressure_threshold: f64,
}

impl Default for FusionConfig {
    fn default() -> Self {
        Self {
            enable_self_healing: true,
            enable_adaptive_timeout: true,
            enable_metrics: true,
            max_repair_attempts: 3,
            adaptive_timeout_config: AdaptiveTimeoutConfig::default(),
            circuit_sensitivity: 0.7,
            resource_pressure_threshold: 0.8,
        }
    }
}

// ============================================================================
// State Types
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineState {
    Idle,
    Running,
    Paused,
    SelfHealing,
    Stopped,
}

impl Default for EngineState {
    fn default() -> Self {
        EngineState::Idle
    }
}

// ============================================================================
// Metrics Types
// ============================================================================

#[derive(Debug, Clone, Default)]
pub struct FusionMetrics {
    pub steps_completed: Counter,
    pub steps_failed: Counter,
    pub self_healing_attempts: Counter,
    pub self_healing_successes: Counter,
    pub circuit_breaker_trips: Counter,
    pub adaptive_timeout_adjustments: Counter,
    pub resource_pressure_events: Counter,
    pub current_step_duration_ms: Gauge,
    pub avg_step_duration_ms: Gauge,
    pub memory_usage_bytes: Gauge,
    pub cpu_time_ms: Gauge,
}

impl FusionMetrics {
    pub fn snapshot(&self) -> FusionMetricsSnapshot {
        FusionMetricsSnapshot {
            steps_completed: self.steps_completed.value(),
            steps_failed: self.steps_failed.value(),
            self_healing_attempts: self.self_healing_attempts.value(),
            self_healing_successes: self.self_healing_successes.value(),
            circuit_breaker_trips: self.circuit_breaker_trips.value(),
            adaptive_timeout_adjustments: self.adaptive_timeout_adjustments.value(),
            resource_pressure_events: self.resource_pressure_events.value(),
            current_step_duration_ms: self.current_step_duration_ms.value() as f64,
            avg_step_duration_ms: self.avg_step_duration_ms.value() as f64,
            memory_usage_bytes: self.memory_usage_bytes.value() as f64,
            cpu_time_ms: self.cpu_time_ms.value() as f64,
            timestamp: chrono::Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionMetricsSnapshot {
    pub steps_completed: u64,
    pub steps_failed: u64,
    pub self_healing_attempts: u64,
    pub self_healing_successes: u64,
    pub circuit_breaker_trips: u64,
    pub adaptive_timeout_adjustments: u64,
    pub resource_pressure_events: u64,
    pub current_step_duration_ms: f64,
    pub avg_step_duration_ms: f64,
    pub memory_usage_bytes: f64,
    pub cpu_time_ms: f64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

// ============================================================================
// Unified Harness Fusion Engine
// ============================================================================

pub struct UnifiedHarnessFusion {
    /// Core harness context
    harness: Arc<RwLock<AgentHarnessContext>>,
    /// Adaptive timeout engine
    adaptive_timeout: Arc<RwLock<AdaptiveTimeout>>,
    /// Breakpoint manager
    breakpoint_manager: Arc<BreakpointManager>,
    /// Self-healing diagnostic engine
    diagnostic_engine: Arc<DiagnosticEngine>,
    /// Repair strategies
    repair_strategies: Arc<RwLock<Vec<Box<dyn RepairStrategy + Send + Sync>>>>,
    /// Metrics collector
    metrics: Arc<RwLock<FusionMetrics>>,
    /// Configuration
    config: FusionConfig,
    /// Current engine state
    state: Arc<RwLock<EngineState>>,
    /// Event notification channel
    event_tx: Sender<FusionEvent>,
    /// Step timing
    step_start: Arc<RwLock<Option<Instant>>>,
    /// Step duration history (for avg calculation)
    step_durations: Arc<RwLock<Vec<u64>>>,
}

#[derive(Debug, Clone)]
pub enum FusionEvent {
    StepStarted { step: usize },
    StepCompleted { step: usize, duration_ms: u64, success: bool },
    SelfHealingStarted { error_type: ErrorType, strategy: String },
    SelfHealingCompleted { success: bool, message: String },
    CircuitBreakerTripped { tool_name: String },
    CircuitBreakerReset,
    AdaptiveTimeoutAdjusted { old_ms: u64, new_ms: u64 },
    ResourcePressureDetected { pressure: f64 },
    EngineStateChanged { from: EngineState, to: EngineState },
    BreakpointTriggered { breakpoint_id: String, action: BreakpointAction },
}

impl UnifiedHarnessFusion {
    /// Create a new fusion engine
    pub fn new(config: FusionConfig) -> Self {
        let (event_tx, _) = broadcast::channel(1000);

        // Build harness config from fusion config
        let harness_config = HarnessConfig {
            max_steps: 100,
            tool_timeout: config.adaptive_timeout_config.base_timeout,
            step_timeout: config.adaptive_timeout_config.base_timeout * 2,
            enable_self_reflection: true,
            ..Default::default()
        };

        Self {
            harness: Arc::new(RwLock::new(AgentHarnessContext::new(harness_config))),
            adaptive_timeout: Arc::new(RwLock::new(AdaptiveTimeout::new(config.adaptive_timeout_config.clone()))),
            breakpoint_manager: Arc::new(BreakpointManager::new()),
            diagnostic_engine: Arc::new(DiagnosticEngine::new(1000)),
            repair_strategies: Arc::new(RwLock::new(Vec::new())),
            metrics: Arc::new(RwLock::new(FusionMetrics::default())),
            config,
            state: Arc::new(RwLock::new(EngineState::Idle)),
            event_tx,
            step_start: Arc::new(RwLock::new(None)),
            step_durations: Arc::new(RwLock::new(Vec::with_capacity(100))),
        }
    }

    /// Create with default configuration
    pub fn with_default() -> Self {
        Self::new(FusionConfig::default())
    }

    /// Register a repair strategy
    pub async fn register_repair_strategy(&mut self, strategy: Box<dyn RepairStrategy + Send + Sync>) {
        let mut strategies = self.repair_strategies.write().await;
        strategies.push(strategy);
    }

    /// Subscribe to fusion events
    pub fn subscribe(&self) -> broadcast::Receiver<FusionEvent> {
        self.event_tx.subscribe()
    }

    // --- State Management ---

    /// Get current engine state
    pub async fn state(&self) -> EngineState {
        *self.state.read().await
    }

    async fn set_state(&self, new_state: EngineState) {
        let old_state = *self.state.read().await;
        if old_state != new_state {
            *self.state.write().await = new_state;
            let _ = self.event_tx.send(FusionEvent::EngineStateChanged {
                from: old_state,
                to: new_state,
            });
        }
    }

    /// Start the engine
    pub async fn start(&self) {
        self.set_state(EngineState::Running).await;
    }

    /// Pause execution
    pub async fn pause(&self) {
        self.set_state(EngineState::Paused).await;
        self.harness.write().await.pause();
    }

    /// Resume execution
    pub async fn resume(&self) {
        self.set_state(EngineState::Running).await;
        self.harness.write().await.resume();
    }

    /// Stop the engine
    pub async fn stop(&self) {
        self.set_state(EngineState::Stopped).await;
        self.harness.write().await.cancel();
    }

    // --- Core Execution Loop ---

    /// Execute a step with full fusion capabilities
    pub async fn execute_step<F, Fut>(&self, step_fn: F) -> Result<String, FusionError>
    where
        F: FnOnce(Arc<RwLock<AgentHarnessContext>>, Duration) -> Fut,
        Fut: std::future::Future<Output = Result<String, HarnessError>>,
    {
        // Check state
        let state = self.state().await;
        if state != EngineState::Running {
            return Err(FusionError::EngineClosed);
        }

        let harness = self.harness.read().await;

        // Check if we should stop
        if harness.should_stop() {
            return Err(FusionError::EngineClosed);
        }

        // Calculate adaptive timeout
        let timeout = self.calculate_adaptive_timeout(&harness).await;

        // Record step start
        let step_number = harness.metadata().step_count;
        *self.step_start.write().await = Some(Instant::now());

        let _ = self.event_tx.send(FusionEvent::StepStarted { step: step_number });

        // Execute with timeout
        let result = tokio::time::timeout(timeout, step_fn(self.harness.clone(), timeout)).await;

        // Record step completion
        let duration = if let Some(start) = *self.step_start.read().await {
            start.elapsed().as_millis() as u64
        } else {
            0
        };

        self.record_step_completion(step_number, duration).await;

        // Handle result
        match result {
            Ok(Ok(output)) => {
                self.metrics.write().await.steps_completed.inc(1);
                let _ = self.event_tx.send(FusionEvent::StepCompleted {
                    step: step_number,
                    duration_ms: duration,
                    success: true,
                });
                Ok(output)
            }
            Ok(Err(error)) => {
                self.metrics.write().await.steps_failed.inc(1);
                let _ = self.event_tx.send(FusionEvent::StepCompleted {
                    step: step_number,
                    duration_ms: duration,
                    success: false,
                });

                // Try self-healing if enabled
                if self.config.enable_self_healing {
                    self.handle_error(error).await
                } else {
                    Err(FusionError::HarnessError(error.to_string()))
                }
            }
            Err(_) => {
                self.metrics.write().await.steps_failed.inc(1);
                let _ = self.event_tx.send(FusionEvent::StepCompleted {
                    step: step_number,
                    duration_ms: duration,
                    success: false,
                });
                Err(FusionError::HarnessError("Step timeout".to_string()))
            }
        }
    }

    /// Handle error with self-healing
    async fn handle_error(&self, error: HarnessError) -> Result<String, FusionError> {
        self.set_state(EngineState::SelfHealing).await;
        self.metrics.write().await.self_healing_attempts.inc(1);

        // Diagnose the error
        let diagnostic = self.diagnostic_engine.diagnose(&error, "fusion-engine").await;

        let _ = self.event_tx.send(FusionEvent::SelfHealingStarted {
            error_type: diagnostic.error_type.clone(),
            strategy: diagnostic.suggested_strategies.first().cloned().unwrap_or_default(),
        });

        // Get repair strategies - iterate directly without clone
        let strategies = self.repair_strategies.read().await;

        let mut repair_attempts = 0u32;
        let last_error = error.clone();

        for strategy in strategies.iter() {
            if !strategy.can_handle(&diagnostic.error_type, diagnostic.severity) {
                continue;
            }

            repair_attempts += 1;

            // Attempt repair
            let mut harness = self.harness.write().await;
            let outcome = strategy.repair(&last_error, &mut harness).await;

            match outcome {
                RepairOutcome::Success { repaired: true, message, .. } => {
                    self.metrics.write().await.self_healing_successes.inc(1);
                    let _ = self.event_tx.send(FusionEvent::SelfHealingCompleted {
                        success: true,
                        message: message.clone(),
                    });

                    // Update circuit breaker if it was a tool failure
                    if matches!(last_error, HarnessError::ToolFailure(_, _)) {
                        harness.circuit_breaker().record_success();
                    }

                    self.set_state(EngineState::Running).await;
                    return Err(FusionError::SelfHealingFailed(message));
                }
                RepairOutcome::Failed { can_retry: true, .. } => {
                    // Try next strategy
                    continue;
                }
                _ => {
                    // This strategy failed, try next
                    continue;
                }
            }
        }

        // Record circuit breaker trip if applicable
        if matches!(error, HarnessError::ToolFailure(_, _)) {
            self.metrics.write().await.circuit_breaker_trips.inc(1);
            self.harness.read().await.circuit_breaker().record_failure();

            let _ = self.event_tx.send(FusionEvent::CircuitBreakerTripped {
                tool_name: match &error {
                    HarnessError::ToolFailure(name, _) => name.clone(),
                    _ => "unknown".to_string(),
                },
            });
        }

        let _ = self.event_tx.send(FusionEvent::SelfHealingCompleted {
            success: false,
            message: "All repair strategies exhausted".to_string(),
        });

        self.set_state(EngineState::Stopped).await;

        if repair_attempts >= self.config.max_repair_attempts {
            Err(FusionError::RepairExhausted)
        } else {
            Err(FusionError::HarnessError(error.to_string()))
        }
    }

    // --- Adaptive Timeout Calculation ---

    /// Calculate adaptive timeout based on multiple factors
    async fn calculate_adaptive_timeout(&self, harness: &AgentHarnessContext) -> Duration {
        if !self.config.enable_adaptive_timeout {
            return harness.config().step_timeout;
        }

        #[allow(unused_assignments)]
        let mut timeout_adjusted = false;
        #[allow(unused_assignments)]
        let mut new_timeout_ms = 0u64;

        // Base timeout from adaptive engine
        let step_count = harness.metadata().step_count;
        let adaptive_timeout = {
            let timeout = self.adaptive_timeout.read().await;
            timeout.calculate_timeout(step_count)
        };

        // Factor 1: Circuit breaker state
        let cb_state = harness.circuit_breaker().state();
        let cb_multiplier = match cb_state {
            super::harness::CircuitState::Closed => 1.0,
            super::harness::CircuitState::HalfOpen => 1.5,
            super::harness::CircuitState::Open => 2.5, // Significantly increase timeout when circuit is open
        };

        // Factor 2: Resource pressure
        let resource_usage = harness.resource_tracker().get_usage();
        let max_memory = harness.config().max_memory_bytes.unwrap_or(u64::MAX);
        let max_cpu = harness.config().max_cpu_time_ms.unwrap_or(u64::MAX);

        let memory_pressure = if max_memory > 0 {
            resource_usage.memory_bytes as f64 / max_memory as f64
        } else {
            0.0
        };

        let cpu_pressure = if max_cpu > 0 {
            resource_usage.cpu_time_ms as f64 / max_cpu as f64
        } else {
            0.0
        };

        let avg_pressure = (memory_pressure + cpu_pressure) / 2.0;

        let resource_multiplier = if avg_pressure > self.config.resource_pressure_threshold {
            self.metrics.write().await.resource_pressure_events.inc(1);
            let _ = self.event_tx.send(FusionEvent::ResourcePressureDetected {
                pressure: avg_pressure,
            });
            1.5 + (avg_pressure - self.config.resource_pressure_threshold)
        } else {
            1.0
        };

        // Factor 3: Recent error rate
        let error_rate = if harness.metadata().tool_call_count > 0 {
            harness.metadata().tool_failure_count as f64 / harness.metadata().tool_call_count as f64
        } else {
            0.0
        };

        let error_multiplier = if error_rate > 0.3 {
            1.5
        } else if error_rate > 0.1 {
            1.2
        } else {
            1.0
        };

        // Calculate final timeout
        let base_ms = adaptive_timeout.as_millis() as f64;
        let final_timeout_ms = base_ms * cb_multiplier * resource_multiplier * error_multiplier;

        // Clamp to configured bounds
        let min_timeout = self.config.adaptive_timeout_config.min_timeout;
        let max_timeout = self.config.adaptive_timeout_config.max_timeout;

        new_timeout_ms = final_timeout_ms as u64;
        new_timeout_ms = new_timeout_ms.max(min_timeout.as_millis() as u64);
        new_timeout_ms = new_timeout_ms.min(max_timeout.as_millis() as u64);

        // Track if we made significant adjustments
        let old_timeout = harness.config().step_timeout.as_millis() as u64;
        if (new_timeout_ms as i64 - old_timeout as i64).abs() > 100 {
            timeout_adjusted = true;
            let _ = self.event_tx.send(FusionEvent::AdaptiveTimeoutAdjusted {
                old_ms: old_timeout,
                new_ms: new_timeout_ms,
            });
        }

        if timeout_adjusted {
            self.metrics.write().await.adaptive_timeout_adjustments.inc(1);
        }

        Duration::from_millis(new_timeout_ms)
    }

    /// Record step completion for metrics and history
    async fn record_step_completion(&self, step: usize, duration_ms: u64) {
        // Update metrics
        {
            let metrics = self.metrics.write().await;
            metrics.current_step_duration_ms.set(duration_ms);

            // Update avg
            let mut durations = self.step_durations.write().await;
            durations.push(duration_ms);
            if durations.len() > 100 {
                durations.remove(0);
            }
            let sum: u64 = durations.iter().sum();
            let avg = sum / durations.len() as u64;
            metrics.avg_step_duration_ms.set(avg);
        }

        // Update adaptive timeout
        {
            let mut timeout = self.adaptive_timeout.write().await;
            timeout.record_step(Duration::from_millis(duration_ms));
        }

        // Update resource tracking
        {
            let harness = self.harness.read().await;
            let usage = harness.resource_tracker().get_usage();
            let metrics = self.metrics.write().await;
            metrics.memory_usage_bytes.set(usage.memory_bytes);
            metrics.cpu_time_ms.set(usage.cpu_time_ms);
        }

        // Check breakpoints
        self.check_breakpoints(step).await;
    }

    /// Check and handle breakpoints
    async fn check_breakpoints(&self, step: usize) {
        let harness = self.harness.read().await;

        let ctx = BreakpointContext {
            step_count: step,
            tokens_used: harness.metadata().llm_tokens_used,
            total_calls: harness.metadata().tool_call_count,
            failed_calls: harness.metadata().tool_failure_count,
            last_tool: None, // Would need to track this
            elapsed: harness.elapsed(),
            memory_usage: harness.resource_tracker().get_usage().memory_bytes,
            cpu_time_ms: harness.resource_tracker().get_usage().cpu_time_ms,
        };

        drop(harness);

        let events = self.breakpoint_manager.check_all(&ctx).await;

        for event in events {
            let _ = self.event_tx.send(FusionEvent::BreakpointTriggered {
                breakpoint_id: event.breakpoint_id,
                action: event.action,
            });
        }
    }

    // --- Breakpoint Management ---

    /// Add a breakpoint
    pub async fn add_breakpoint(&self, breakpoint: super::adaptive_harness::Breakpoint) {
        self.breakpoint_manager.add_breakpoint(breakpoint).await;
    }

    /// List all breakpoints
    pub async fn list_breakpoints(&self) -> Vec<super::adaptive_harness::Breakpoint> {
        self.breakpoint_manager.list_breakpoints().await
    }

    // --- Metrics ---

    /// Get current metrics snapshot
    pub async fn metrics(&self) -> FusionMetricsSnapshot {
        self.metrics.read().await.snapshot()
    }

    /// Get harness context (read-only)
    pub async fn harness(&self) -> Arc<RwLock<AgentHarnessContext>> {
        self.harness.clone()
    }

    /// Get adaptive timeout engine
    pub async fn adaptive_timeout(&self) -> Arc<RwLock<AdaptiveTimeout>> {
        self.adaptive_timeout.clone()
    }

    /// Get diagnostic engine
    pub fn diagnostic_engine(&self) -> Arc<DiagnosticEngine> {
        self.diagnostic_engine.clone()
    }

    /// Check if engine can continue
    pub async fn can_continue(&self) -> bool {
        self.harness.read().await.can_continue()
    }

    /// Get remaining steps
    pub async fn remaining_steps(&self) -> usize {
        self.harness.read().await.remaining_steps()
    }

    /// Get execution summary
    pub async fn summary(&self) -> ExecutionSummary {
        let metrics = self.metrics.read().await;
        ExecutionSummary {
            step_count: metrics.steps_completed.value() as usize,
            total_duration_ms: 0, // Not directly tracked
            tool_call_count: 0,   // Not directly tracked
            tool_failure_count: metrics.steps_failed.value() as usize,
            llm_tokens_used: None,
            self_healing_attempts: metrics.self_healing_attempts.value(),
            self_healing_successes: metrics.self_healing_successes.value(),
            circuit_breaker_state: "Closed".to_string(),
            avg_step_duration_ms: metrics.avg_step_duration_ms.value() as f64,
            final_timeout_ms: 0,
        }
    }
}

/// Execution summary for the fusion engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSummary {
    pub step_count: usize,
    pub total_duration_ms: u64,
    pub tool_call_count: usize,
    pub tool_failure_count: usize,
    pub llm_tokens_used: Option<u64>,
    pub self_healing_attempts: u64,
    pub self_healing_successes: u64,
    pub circuit_breaker_state: String,
    pub avg_step_duration_ms: f64,
    pub final_timeout_ms: u64,
}

impl From<FusionMetricsSnapshot> for ExecutionSummary {
    fn from(m: FusionMetricsSnapshot) -> Self {
        Self {
            step_count: m.steps_completed as usize + m.steps_failed as usize,
            total_duration_ms: 0, // Would need to track separately
            tool_call_count: 0,
            tool_failure_count: m.steps_failed as usize,
            llm_tokens_used: None,
            self_healing_attempts: m.self_healing_attempts,
            self_healing_successes: m.self_healing_successes,
            circuit_breaker_state: "unknown".to_string(),
            avg_step_duration_ms: m.avg_step_duration_ms,
            final_timeout_ms: 0,
        }
    }
}

// ============================================================================
// Builder Pattern for Easy Construction
// ============================================================================

pub struct UnifiedHarnessFusionBuilder {
    config: FusionConfig,
    repair_strategies: Vec<Box<dyn RepairStrategy + Send + Sync>>,
}

impl UnifiedHarnessFusionBuilder {
    pub fn new() -> Self {
        Self {
            config: FusionConfig::default(),
            repair_strategies: Vec::new(),
        }
    }

    /// Enable or disable self-healing
    pub fn with_self_healing(mut self, enabled: bool) -> Self {
        self.config.enable_self_healing = enabled;
        self
    }

    /// Enable or disable adaptive timeout
    pub fn with_adaptive_timeout(mut self, enabled: bool) -> Self {
        self.config.enable_adaptive_timeout = enabled;
        self
    }

    /// Enable or disable metrics
    pub fn with_metrics(mut self, enabled: bool) -> Self {
        self.config.enable_metrics = enabled;
        self
    }

    /// Set max repair attempts
    pub fn with_max_repair_attempts(mut self, max: u32) -> Self {
        self.config.max_repair_attempts = max;
        self
    }

    /// Set adaptive timeout config
    pub fn with_adaptive_timeout_config(mut self, config: AdaptiveTimeoutConfig) -> Self {
        self.config.adaptive_timeout_config = config;
        self
    }

    /// Set circuit sensitivity (0.0 - 1.0)
    pub fn with_circuit_sensitivity(mut self, sensitivity: f64) -> Self {
        self.config.circuit_sensitivity = sensitivity;
        self
    }

    /// Add a repair strategy
    pub fn with_repair_strategy(mut self, strategy: Box<dyn RepairStrategy + Send + Sync>) -> Self {
        self.repair_strategies.push(strategy);
        self
    }

    /// Build the fusion engine
    pub async fn build(self) -> UnifiedHarnessFusion {
        let enable_self_healing = self.config.enable_self_healing;
        let mut engine = UnifiedHarnessFusion::new(self.config);

        // Register default repair strategies if self-healing is enabled
        if enable_self_healing && self.repair_strategies.is_empty() {
            // Use default strategies from self_healing_agent module
            // This would typically import and add them here
        }

        // Register custom strategies
        for strategy in self.repair_strategies {
            engine.register_repair_strategy(strategy).await;
        }

        engine
    }
}

impl Default for UnifiedHarnessFusionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Convenience Traits
// ============================================================================

/// Extension trait to add fusion capabilities to any harness
#[async_trait::async_trait]
pub trait FuseHarness {
    /// Wrap a harness context with fusion capabilities
    async fn into_fusion(self, config: FusionConfig) -> UnifiedHarnessFusion
    where
        Self: Sized;

    /// Quick fusion with default config
    async fn into_fusion_default(self) -> UnifiedHarnessFusion
    where
        Self: Sized,
    {
        self.into_fusion(FusionConfig::default()).await
    }
}

#[async_trait::async_trait]
impl FuseHarness for AgentHarnessContext {
    async fn into_fusion(self, config: FusionConfig) -> UnifiedHarnessFusion {
        let engine = UnifiedHarnessFusion::new(config);
        *engine.harness.write().await = self;
        engine
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fusion_engine_creation() {
        let engine = UnifiedHarnessFusion::with_default();
        assert_eq!(engine.state().await, EngineState::Idle);
    }

    #[tokio::test]
    async fn test_fusion_engine_start_stop() {
        let engine = UnifiedHarnessFusion::with_default();
        engine.start().await;
        assert_eq!(engine.state().await, EngineState::Running);

        engine.stop().await;
        assert_eq!(engine.state().await, EngineState::Stopped);
    }

    #[tokio::test]
    async fn test_fusion_engine_pause_resume() {
        let engine = UnifiedHarnessFusion::with_default();
        engine.start().await;
        engine.pause().await;
        assert_eq!(engine.state().await, EngineState::Paused);

        engine.resume().await;
        assert_eq!(engine.state().await, EngineState::Running);
    }

    #[tokio::test]
    async fn test_metrics_collection() {
        let engine = UnifiedHarnessFusion::with_default();
        let snapshot = engine.metrics().await;

        assert_eq!(snapshot.steps_completed, 0);
        assert_eq!(snapshot.steps_failed, 0);
    }

    #[tokio::test]
    async fn test_builder_pattern() {
        let builder = UnifiedHarnessFusionBuilder::new()
            .with_self_healing(true)
            .with_adaptive_timeout(false)
            .with_max_repair_attempts(5);

        let config = builder.config;
        assert!(config.enable_self_healing);
        assert!(!config.enable_adaptive_timeout);
        assert_eq!(config.max_repair_attempts, 5);
    }

    #[tokio::test]
    async fn test_event_subscription() {
        let engine = UnifiedHarnessFusion::with_default();
        let mut rx = engine.subscribe();

        engine.start().await;

        // Should receive state change event
        let event = tokio::time::timeout(Duration::from_secs(1), rx.recv()).await;
        assert!(event.is_ok());
    }
}
