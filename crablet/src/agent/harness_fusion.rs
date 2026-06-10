//! Unified harness fusion engine.

use crate::agent::adaptive_harness::{
    AdaptiveTimeout, Breakpoint, BreakpointAction, BreakpointContext, BreakpointEvent,
    BreakpointManager,
};
use crate::agent::harness::{AgentHarnessContext, ExecutionMetadata, HarnessConfig, HarnessError};
use crate::agent::hooks::{
    HookAction, HookContext, HookPoint, HookRegistry, QualityGateHook, ResourceWarningHook,
    SecurityAuditHook,
};
use crate::agent::metrics::AgentMetricsSnapshot;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{info, warn};

/// Engine state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineState {
    Idle,
    Running,
    Paused,
    Stopped,
}

/// Context passed into fusion step execution.
#[derive(Debug, Clone)]
pub struct FusionStepContext {
    pub metadata: ExecutionMetadata,
    pub state: EngineState,
    pub attempt: usize,
    pub step_timeout: Duration,
}

/// Summary snapshot returned by the engine.
#[derive(Debug, Clone)]
pub struct FusionSummary {
    pub step_count: usize,
    pub error_count: usize,
    pub state: EngineState,
}

#[derive(Debug, Clone)]
struct EngineConfig {
    self_healing: bool,
    adaptive_timeout: bool,
    metrics_enabled: bool,
    max_repair_attempts: usize,
    circuit_sensitivity: f64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            self_healing: false,
            adaptive_timeout: true,
            metrics_enabled: true,
            max_repair_attempts: 3,
            circuit_sensitivity: 0.5,
        }
    }
}

/// Builder for the unified harness fusion engine.
#[derive(Clone)]
pub struct UnifiedHarnessFusionBuilder {
    config: EngineConfig,
    hook_registry: Option<Arc<HookRegistry>>,
    harness_config: HarnessConfig,
    shared_harness: Option<Arc<RwLock<AgentHarnessContext>>>,
}

impl UnifiedHarnessFusionBuilder {
    pub fn new() -> Self {
        Self {
            config: EngineConfig::default(),
            hook_registry: None,
            harness_config: HarnessConfig::default(),
            shared_harness: None,
        }
    }

    pub fn with_self_healing(mut self, enabled: bool) -> Self {
        self.config.self_healing = enabled;
        self
    }

    pub fn with_adaptive_timeout(mut self, enabled: bool) -> Self {
        self.config.adaptive_timeout = enabled;
        self
    }

    pub fn with_metrics(mut self, enabled: bool) -> Self {
        self.config.metrics_enabled = enabled;
        self
    }

    pub fn with_harness_config(mut self, config: HarnessConfig) -> Self {
        self.harness_config = config;
        self
    }

    pub fn with_max_repair_attempts(mut self, attempts: usize) -> Self {
        self.config.max_repair_attempts = attempts;
        self
    }

    pub fn with_circuit_sensitivity(mut self, sensitivity: f64) -> Self {
        self.config.circuit_sensitivity = sensitivity;
        self
    }

    pub fn with_hook_registry(mut self, registry: Arc<HookRegistry>) -> Self {
        self.hook_registry = Some(registry);
        self
    }

    pub fn with_shared_harness(mut self, harness: Arc<RwLock<AgentHarnessContext>>) -> Self {
        self.shared_harness = Some(harness);
        self
    }

    pub async fn build(self) -> UnifiedHarnessFusion {
        let hook_registry = match self.hook_registry {
            Some(registry) => registry,
            None => {
                let registry = Arc::new(HookRegistry::new());
                Self::register_default_hooks(&registry).await;
                registry
            }
        };

        UnifiedHarnessFusion::new(
            self.config,
            hook_registry,
            self.harness_config,
            self.shared_harness,
        )
    }

    async fn register_default_hooks(registry: &Arc<HookRegistry>) {
        registry.register(Arc::new(SecurityAuditHook::new())).await;
        registry.register(Arc::new(QualityGateHook::new())).await;
        registry
            .register(Arc::new(ResourceWarningHook::new()))
            .await;
    }
}

impl Default for UnifiedHarnessFusionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Unified harness fusion engine.
pub struct UnifiedHarnessFusion {
    config: EngineConfig,
    state: Arc<RwLock<EngineState>>,
    harness: Arc<RwLock<AgentHarnessContext>>,
    hook_registry: Arc<HookRegistry>,
    breakpoint_manager: BreakpointManager,
    metrics: Arc<RwLock<AgentMetricsSnapshot>>,
    adaptive_timeout: Arc<RwLock<AdaptiveTimeout>>,
}

impl UnifiedHarnessFusion {
    fn new(
        config: EngineConfig,
        hook_registry: Arc<HookRegistry>,
        harness_config: HarnessConfig,
        shared_harness: Option<Arc<RwLock<AgentHarnessContext>>>,
    ) -> Self {
        let harness = shared_harness
            .unwrap_or_else(|| Arc::new(RwLock::new(AgentHarnessContext::new(harness_config))));
        Self {
            config,
            state: Arc::new(RwLock::new(EngineState::Idle)),
            harness,
            hook_registry,
            breakpoint_manager: BreakpointManager::new(),
            metrics: Arc::new(RwLock::new(AgentMetricsSnapshot::default())),
            adaptive_timeout: Arc::new(RwLock::new(AdaptiveTimeout::with_default())),
        }
    }

    pub fn with_default() -> Self {
        Self::new(
            EngineConfig::default(),
            Arc::new(HookRegistry::new()),
            HarnessConfig::default(),
            None,
        )
    }

    pub async fn state(&self) -> EngineState {
        *self.state.read().await
    }

    pub async fn start(&self) {
        *self.state.write().await = EngineState::Running;
    }

    pub async fn pause(&self) {
        *self.state.write().await = EngineState::Paused;
        self.harness.read().await.pause();
    }

    pub async fn resume(&self) {
        *self.state.write().await = EngineState::Running;
        self.harness.read().await.resume();
    }

    pub async fn stop(&self) {
        *self.state.write().await = EngineState::Stopped;
        self.harness.read().await.cancel();
    }

    pub async fn harness(&self) -> Arc<RwLock<AgentHarnessContext>> {
        self.harness.clone()
    }

    pub fn hook_registry(&self) -> Arc<HookRegistry> {
        self.hook_registry.clone()
    }

    pub async fn metrics(&self) -> AgentMetricsSnapshot {
        self.metrics.read().await.clone()
    }

    pub async fn add_breakpoint(&self, breakpoint: Breakpoint) {
        self.breakpoint_manager.add_breakpoint(breakpoint).await;
    }

    pub async fn list_breakpoints(&self) -> Vec<Breakpoint> {
        self.breakpoint_manager.list_breakpoints().await
    }

    pub async fn execute_step<F, Fut>(&self, step: F) -> Result<String>
    where
        F: FnMut(&FusionStepContext, &HookRegistry) -> Fut,
        Fut: Future<Output = Result<String>>,
    {
        if !matches!(self.state().await, EngineState::Running) {
            return Err(anyhow!("engine is not running"));
        }

        let mut step = step;
        let step_started = Instant::now();
        let mut repair_attempt = 0usize;

        loop {
            let engine_state = self.state().await;
            if !matches!(engine_state, EngineState::Running) {
                return Err(anyhow!("engine is not running"));
            }

            let harness_snapshot = self.harness.read().await.metadata();
            let step_number = harness_snapshot.step_count + 1;
            let step_timeout = self.current_step_timeout(step_number).await;
            let step_context = FusionStepContext {
                metadata: harness_snapshot.clone(),
                state: engine_state,
                attempt: repair_attempt + 1,
                step_timeout,
            };

            let pre_step_ctx = self.build_hook_context(
                HookPoint::PreStep,
                &harness_snapshot,
                step_number,
                repair_attempt + 1,
                step_timeout,
                None,
                None,
                engine_state,
            );
            match self.run_hook_phase(HookPoint::PreStep, pre_step_ctx).await {
                Ok(pre_result) => match self.classify_hook_result(&pre_result) {
                    HookPhaseDirective::Allow => {
                        if let Some(message) = pre_result.message.as_deref() {
                            info!("pre-step hook message: {}", message);
                        }
                    }
                    HookPhaseDirective::Retry(reason) => {
                        let error = anyhow!(reason);
                        self.record_attempt_error(
                            HarnessError::LlmFailure(error.to_string()),
                            step_started.elapsed(),
                            true,
                        )
                        .await;
                        if self.should_retry(repair_attempt, true) {
                            self.run_recovery_hooks(
                                &harness_snapshot,
                                step_number,
                                repair_attempt + 1,
                                step_timeout,
                                error.to_string(),
                            )
                            .await;
                            repair_attempt += 1;
                            continue;
                        }
                        self.finalize_failure(step_started.elapsed()).await;
                        return Err(error);
                    }
                    HookPhaseDirective::Block(reason) => {
                        let error = anyhow!(reason);
                        self.record_attempt_error(
                            HarnessError::ResourceLimitExceeded(error.to_string()),
                            step_started.elapsed(),
                            false,
                        )
                        .await;
                        self.finalize_failure(step_started.elapsed()).await;
                        return Err(error);
                    }
                },
                Err(error) => {
                    self.record_attempt_error(
                        HarnessError::LlmFailure(error.to_string()),
                        step_started.elapsed(),
                        true,
                    )
                    .await;
                    if self.should_retry(repair_attempt, true) {
                        self.run_recovery_hooks(
                            &harness_snapshot,
                            step_number,
                            repair_attempt + 1,
                            step_timeout,
                            error.to_string(),
                        )
                        .await;
                        repair_attempt += 1;
                        continue;
                    }
                    self.finalize_failure(step_started.elapsed()).await;
                    return Err(error);
                }
            }

            let output = timeout(
                step_timeout,
                step(&step_context, self.hook_registry.as_ref()),
            )
            .await;

            match output {
                Ok(Ok(mut result)) => {
                    let post_step_ctx = self.build_hook_context(
                        HookPoint::PostStep,
                        &harness_snapshot,
                        step_number,
                        repair_attempt + 1,
                        step_timeout,
                        Some(result.clone()),
                        None,
                        engine_state,
                    );

                    match self
                        .run_hook_phase(HookPoint::PostStep, post_step_ctx)
                        .await
                    {
                        Ok(post_result) => match self.classify_hook_result(&post_result) {
                            HookPhaseDirective::Allow => {
                                self.apply_hook_payload(&mut result, &post_result);
                            }
                            HookPhaseDirective::Retry(reason) => {
                                let error = anyhow!(reason);
                                self.record_attempt_error(
                                    HarnessError::LlmFailure(error.to_string()),
                                    step_started.elapsed(),
                                    true,
                                )
                                .await;
                                if self.should_retry(repair_attempt, true) {
                                    self.run_recovery_hooks(
                                        &harness_snapshot,
                                        step_number,
                                        repair_attempt + 1,
                                        step_timeout,
                                        error.to_string(),
                                    )
                                    .await;
                                    repair_attempt += 1;
                                    continue;
                                }
                                self.finalize_failure(step_started.elapsed()).await;
                                return Err(error);
                            }
                            HookPhaseDirective::Block(reason) => {
                                let error = anyhow!(reason);
                                self.record_attempt_error(
                                    HarnessError::ResourceLimitExceeded(error.to_string()),
                                    step_started.elapsed(),
                                    false,
                                )
                                .await;
                                self.finalize_failure(step_started.elapsed()).await;
                                return Err(error);
                            }
                        },
                        Err(error) => {
                            self.record_attempt_error(
                                HarnessError::LlmFailure(error.to_string()),
                                step_started.elapsed(),
                                true,
                            )
                            .await;
                            if self.should_retry(repair_attempt, true) {
                                self.run_recovery_hooks(
                                    &harness_snapshot,
                                    step_number,
                                    repair_attempt + 1,
                                    step_timeout,
                                    error.to_string(),
                                )
                                .await;
                                repair_attempt += 1;
                                continue;
                            }
                            self.finalize_failure(step_started.elapsed()).await;
                            return Err(error);
                        }
                    }

                    self.harness.read().await.record_step();
                    let total_elapsed = step_started.elapsed();
                    self.update_success_metrics(total_elapsed).await;
                    self.handle_breakpoints(total_elapsed).await;
                    return Ok(result);
                }
                Ok(Err(err)) => {
                    let error = anyhow!(err.to_string());
                    self.record_attempt_error(
                        HarnessError::LlmFailure(error.to_string()),
                        step_started.elapsed(),
                        true,
                    )
                    .await;
                    if self.should_retry(repair_attempt, true) {
                        self.run_recovery_hooks(
                            &harness_snapshot,
                            step_number,
                            repair_attempt + 1,
                            step_timeout,
                            error.to_string(),
                        )
                        .await;
                        repair_attempt += 1;
                        continue;
                    }
                    self.finalize_failure(step_started.elapsed()).await;
                    return Err(error);
                }
                Err(_) => {
                    let error = anyhow!("step timed out after {:?}", step_timeout);
                    self.record_attempt_error(
                        HarnessError::Timeout(step_timeout),
                        step_started.elapsed(),
                        true,
                    )
                    .await;
                    if self.should_retry(repair_attempt, true) {
                        self.run_recovery_hooks(
                            &harness_snapshot,
                            step_number,
                            repair_attempt + 1,
                            step_timeout,
                            error.to_string(),
                        )
                        .await;
                        repair_attempt += 1;
                        continue;
                    }
                    self.finalize_failure(step_started.elapsed()).await;
                    return Err(error);
                }
            }
        }
    }

    pub async fn summary(&self) -> FusionSummary {
        let harness = self.harness.read().await.metadata();
        FusionSummary {
            step_count: harness.step_count,
            error_count: harness.error_count,
            state: self.state().await,
        }
    }

    async fn current_step_timeout(&self, step_count: usize) -> Duration {
        if self.config.adaptive_timeout {
            self.adaptive_timeout
                .read()
                .await
                .calculate_timeout(step_count)
        } else {
            self.harness.read().await.config().step_timeout
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build_hook_context(
        &self,
        point: HookPoint,
        metadata: &ExecutionMetadata,
        step_number: usize,
        attempt: usize,
        step_timeout: Duration,
        output: Option<String>,
        error: Option<String>,
        state: EngineState,
    ) -> HookContext {
        let mut hook_metadata = HashMap::new();
        hook_metadata.insert("step_number".to_string(), step_number.to_string());
        hook_metadata.insert("attempt".to_string(), attempt.to_string());
        hook_metadata.insert(
            "step_timeout_ms".to_string(),
            step_timeout.as_millis().to_string(),
        );
        hook_metadata.insert("state".to_string(), format!("{:?}", state));
        hook_metadata.insert("step_count".to_string(), metadata.step_count.to_string());
        hook_metadata.insert("error_count".to_string(), metadata.error_count.to_string());
        hook_metadata.insert(
            "tool_call_count".to_string(),
            metadata.tool_call_count.to_string(),
        );

        if let Some(ref error) = error {
            hook_metadata.insert("error".to_string(), error.clone());
        }

        HookContext {
            point,
            session_id: format!("harness-{}", metadata.started_at.timestamp_millis()),
            step_number,
            tool_name: "fusion-step".to_string(),
            tool_args: serde_json::json!({
                "point": format!("{:?}", point),
                "step_number": step_number,
                "attempt": attempt,
                "step_timeout_ms": step_timeout.as_millis(),
                "state": format!("{:?}", state),
                "step_count": metadata.step_count,
                "error_count": metadata.error_count,
                "tool_call_count": metadata.tool_call_count,
                "output": output,
                "error": error,
            }),
            input: Some("fusion-step".to_string()),
            output,
            metadata: hook_metadata,
        }
    }

    async fn run_hook_phase(
        &self,
        point: HookPoint,
        ctx: HookContext,
    ) -> Result<crate::agent::hooks::HookResult> {
        self.hook_registry
            .run_hooks(point, &ctx)
            .await
            .map_err(|err| anyhow!("hook phase {:?} failed: {}", point, err))
    }

    fn classify_hook_result(&self, result: &crate::agent::hooks::HookResult) -> HookPhaseDirective {
        match &result.action {
            HookAction::Allow => {
                if result.retry {
                    HookPhaseDirective::Retry(
                        result
                            .message
                            .clone()
                            .unwrap_or_else(|| "hook requested retry".to_string()),
                    )
                } else {
                    HookPhaseDirective::Allow
                }
            }
            HookAction::Block { reason } => HookPhaseDirective::Block(reason.clone()),
            HookAction::Retry { message } => HookPhaseDirective::Retry(message.clone()),
            HookAction::Modify | HookAction::Replace { .. } => {
                if result.retry {
                    HookPhaseDirective::Retry(
                        result
                            .message
                            .clone()
                            .unwrap_or_else(|| "hook requested retry".to_string()),
                    )
                } else {
                    HookPhaseDirective::Allow
                }
            }
        }
    }

    fn apply_hook_payload(
        &self,
        result: &mut String,
        hook_result: &crate::agent::hooks::HookResult,
    ) {
        if let Some(payload) = &hook_result.payload {
            if let Ok(output) = serde_json::from_value::<String>(payload.clone()) {
                *result = output;
                return;
            }

            if let Some(output) = payload.get("output").and_then(|value| value.as_str()) {
                *result = output.to_string();
                return;
            }

            if let Ok(rendered) = serde_json::to_string(payload) {
                *result = rendered;
            }
        }
    }

    fn should_retry(&self, repair_attempt: usize, retryable: bool) -> bool {
        retryable && self.config.self_healing && repair_attempt < self.config.max_repair_attempts
    }

    async fn run_recovery_hooks(
        &self,
        metadata: &ExecutionMetadata,
        step_number: usize,
        attempt: usize,
        step_timeout: Duration,
        error: String,
    ) {
        let engine_state = self.state().await;
        let pre_recovery = self.build_hook_context(
            HookPoint::PreRecovery,
            metadata,
            step_number,
            attempt,
            step_timeout,
            None,
            Some(error.clone()),
            engine_state,
        );
        let _ = self
            .run_hook_phase(HookPoint::PreRecovery, pre_recovery)
            .await;

        let backoff_ms = ((50.0 + 150.0 * self.config.circuit_sensitivity.clamp(0.0, 1.0))
            * attempt.max(1) as f64)
            .round() as u64;
        tokio::time::sleep(Duration::from_millis(backoff_ms.max(25))).await;

        let post_recovery = self.build_hook_context(
            HookPoint::PostRecovery,
            metadata,
            step_number,
            attempt,
            step_timeout,
            None,
            Some(error),
            engine_state,
        );
        let _ = self
            .run_hook_phase(HookPoint::PostRecovery, post_recovery)
            .await;
    }

    async fn record_attempt_error(&self, error: HarnessError, elapsed: Duration, retryable: bool) {
        self.harness.read().await.record_error(error);
        if !self.config.metrics_enabled {
            return;
        }

        let mut metrics = self.metrics.write().await;
        metrics.total_errors = self.harness.read().await.error_count();
        if retryable && self.config.self_healing {
            metrics.self_healing_attempts += 1;
        }
        metrics.current_step_duration_ms = elapsed.as_secs_f64() * 1000.0;
    }

    async fn finalize_failure(&self, elapsed: Duration) {
        if !self.config.metrics_enabled {
            return;
        }

        let harness = self.harness.read().await.metadata();
        let mut metrics = self.metrics.write().await;
        metrics.steps_failed += 1;
        metrics.total_steps += 1;
        metrics.current_step_duration_ms = elapsed.as_secs_f64() * 1000.0;
        metrics.total_duration_ms = metrics
            .total_duration_ms
            .saturating_add(elapsed.as_millis() as u64);
        metrics.total_tool_calls = harness.tool_call_count;
        metrics.total_errors = harness.error_count;
    }

    async fn update_success_metrics(&self, elapsed: Duration) {
        if !self.config.metrics_enabled {
            return;
        }

        let harness = self.harness.read().await.metadata();
        let mut metrics = self.metrics.write().await;
        metrics.steps_completed += 1;
        metrics.total_steps += 1;
        metrics.current_step_duration_ms = elapsed.as_secs_f64() * 1000.0;
        metrics.total_duration_ms = metrics
            .total_duration_ms
            .saturating_add(elapsed.as_millis() as u64);
        metrics.total_tool_calls = harness.tool_call_count;
        metrics.total_errors = harness.error_count;
    }

    async fn handle_breakpoints(&self, elapsed: Duration) {
        let harness = self.harness.read().await.metadata();
        let context = BreakpointContext {
            step_count: harness.step_count,
            elapsed,
            total_calls: harness.tool_call_count,
            failed_calls: harness.error_count,
            ..Default::default()
        };

        let events: Vec<BreakpointEvent> = self.breakpoint_manager.check_all(&context).await;
        if events.is_empty() {
            return;
        }

        for event in events {
            match event.action {
                BreakpointAction::Pause => {
                    info!("breakpoint '{}' paused the engine", event.breakpoint_name);
                    self.pause().await;
                }
                BreakpointAction::Cancel => {
                    warn!("breakpoint '{}' stopped the engine", event.breakpoint_name);
                    self.stop().await;
                }
                BreakpointAction::Reflect => {
                    info!(
                        "breakpoint '{}' requested reflection",
                        event.breakpoint_name
                    );
                    if self.config.metrics_enabled && self.config.self_healing {
                        self.metrics.write().await.self_healing_attempts += 1;
                    }
                }
                BreakpointAction::SwitchStrategy { strategy } => {
                    info!(
                        "breakpoint '{}' switched strategy to {}",
                        event.breakpoint_name, strategy
                    );
                    if self.config.metrics_enabled && self.config.self_healing {
                        self.metrics.write().await.self_healing_attempts += 1;
                    }
                }
                BreakpointAction::LogAndContinue { message } => {
                    info!(
                        "breakpoint '{}' logged message: {}",
                        event.breakpoint_name, message
                    );
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
enum HookPhaseDirective {
    Allow,
    Block(String),
    Retry(String),
}
