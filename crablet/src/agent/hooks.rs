//! Hook Interception System for Agentic Harness
//!
//! Provides fine-grained lifecycle hooks inspired by Oh-My-OpenAgent (25+ hooks)
//! and Trellis (PreToolUse/PostToolUse). Hooks enable:
//! - Pre-execution validation (security audit, spec injection)
//! - Post-execution quality gates (output validation, logging)
//! - Resource monitoring (budget warnings, token limits, memory pressure)
//! - Self-healing lifecycle events (strategy selection, topology evolution)
//!
//! # Architecture
//! ```text
//! ┌────────────────────────────────────────────────────┐
//! │                  HookRegistry                       │
//! │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐  │
//! │  │Session  │ │  Task   │ │  Tool   │ │  LLM    │  │
//! │  │Hooks    │ │Hooks    │ │Hooks    │ │Hooks    │  │
//! │  └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘  │
//! │       │           │           │           │        │
//! │  ┌────┴────┐ ┌────┴────┐ ┌────┴────┐ ┌────┴────┐  │
//! │  │Message  │ │ Agent   │ │Resource │ │  Self   │  │
//! │  │Hooks    │ │Hooks    │ │Hooks    │ │Healing  │  │
//! │  └─────────┘ └─────────┘ └─────────┘ └─────────┘  │
//! └────────────────────────────────────────────────────┘
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{info, warn};

use super::harness::HarnessError;

// ============================================================================
// Hook Points — 25+ lifecycle events
// ============================================================================

/// Hook interception points covering the full agent lifecycle.
/// Inspired by OmO's 25+ hooks with additions from Hive/Langroid patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookPoint {
    // === Session-level ===
    /// Triggered when a new session starts
    SessionStart,
    /// Triggered when a session ends (completion or failure)
    SessionEnd,

    // === Task-level ===
    /// Triggered when a new task is created
    TaskCreated,
    /// Triggered when task execution begins
    TaskStarted,
    /// Triggered when a task completes successfully
    TaskCompleted,
    /// Triggered when a task fails
    TaskFailed,
    /// Triggered when a task is cancelled
    TaskCancelled,

    // === Tool-level ===
    /// Triggered before a tool is called (can modify args)
    PreToolUse,
    /// Triggered after a tool returns (can inspect/modify result)
    PostToolUse,
    /// Triggered when a tool call times out
    ToolTimeout,
    /// Triggered when a tool is rate-limited
    ToolRateLimited,

    // === LLM-level ===
    /// Triggered before an LLM call (can modify prompt)
    PreLLMCall,
    /// Triggered after an LLM response (can inspect/modify response)
    PostLLMCall,
    /// Triggered when the model is degraded to a cheaper one
    LLMDegraded,

    // === Message-level ===
    /// Triggered before a message is sent
    PreMessageSend,
    /// Triggered after a message is received
    PostMessageReceive,
    /// Triggered when a message is filtered
    MessageFiltered,

    // === Agent-level ===
    /// Triggered before an agent stops (can prevent stop)
    PreAgentStop,
    /// Triggered when control is handed off between agents
    AgentHandoff,
    /// Triggered when an agent encounters an error
    AgentError,

    // === Resource-level ===
    /// Triggered when budget reaches warning threshold (~80%)
    BudgetWarning,
    /// Triggered when budget is exceeded
    BudgetExceeded,
    /// Triggered when token usage approaches limit
    TokenLimitWarning,
    /// Triggered when memory usage is high
    MemoryPressure,

    // === Self-healing-level ===
    /// Triggered when a self-healing strategy is selected
    StrategySelected,
    /// Triggered when a repair attempt begins
    RepairAttempted,
    /// Triggered when a repair succeeds
    RepairSucceeded,
    /// Triggered when a repair fails
    RepairFailed,
    /// Triggered when execution topology evolves (Hive-inspired)
    TopologyEvolved,
}

impl HookPoint {
    /// Returns all hook points grouped by category
    pub fn all() -> &'static [HookPoint] {
        &[
            HookPoint::SessionStart,
            HookPoint::SessionEnd,
            HookPoint::TaskCreated,
            HookPoint::TaskStarted,
            HookPoint::TaskCompleted,
            HookPoint::TaskFailed,
            HookPoint::TaskCancelled,
            HookPoint::PreToolUse,
            HookPoint::PostToolUse,
            HookPoint::ToolTimeout,
            HookPoint::ToolRateLimited,
            HookPoint::PreLLMCall,
            HookPoint::PostLLMCall,
            HookPoint::LLMDegraded,
            HookPoint::PreMessageSend,
            HookPoint::PostMessageReceive,
            HookPoint::MessageFiltered,
            HookPoint::PreAgentStop,
            HookPoint::AgentHandoff,
            HookPoint::AgentError,
            HookPoint::BudgetWarning,
            HookPoint::BudgetExceeded,
            HookPoint::TokenLimitWarning,
            HookPoint::MemoryPressure,
            HookPoint::StrategySelected,
            HookPoint::RepairAttempted,
            HookPoint::RepairSucceeded,
            HookPoint::RepairFailed,
            HookPoint::TopologyEvolved,
        ]
    }

    /// Human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            HookPoint::SessionStart => "Session started",
            HookPoint::SessionEnd => "Session ended",
            HookPoint::TaskCreated => "Task created",
            HookPoint::TaskStarted => "Task started",
            HookPoint::TaskCompleted => "Task completed",
            HookPoint::TaskFailed => "Task failed",
            HookPoint::TaskCancelled => "Task cancelled",
            HookPoint::PreToolUse => "Before tool call (can modify args)",
            HookPoint::PostToolUse => "After tool call (can inspect result)",
            HookPoint::ToolTimeout => "Tool call timed out",
            HookPoint::ToolRateLimited => "Tool rate limited",
            HookPoint::PreLLMCall => "Before LLM call (can modify prompt)",
            HookPoint::PostLLMCall => "After LLM response (can inspect)",
            HookPoint::LLMDegraded => "Model degraded to cheaper option",
            HookPoint::PreMessageSend => "Before message send",
            HookPoint::PostMessageReceive => "After message received",
            HookPoint::MessageFiltered => "Message filtered",
            HookPoint::PreAgentStop => "Before agent stop (can prevent)",
            HookPoint::AgentHandoff => "Agent handoff between agents",
            HookPoint::AgentError => "Agent encountered error",
            HookPoint::BudgetWarning => "Budget ~80% used",
            HookPoint::BudgetExceeded => "Budget exceeded",
            HookPoint::TokenLimitWarning => "Token limit approaching",
            HookPoint::MemoryPressure => "High memory usage",
            HookPoint::StrategySelected => "Self-healing strategy selected",
            HookPoint::RepairAttempted => "Repair attempt started",
            HookPoint::RepairSucceeded => "Repair succeeded",
            HookPoint::RepairFailed => "Repair failed",
            HookPoint::TopologyEvolved => "Execution topology evolved",
        }
    }
}

impl std::fmt::Display for HookPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

// ============================================================================
// Hook Result & Action
// ============================================================================

/// Result returned by a hook execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResult {
    /// Whether to allow execution to continue
    pub action: HookAction,
    /// Modified tool args (for PreToolUse hooks)
    pub modified_args: Option<serde_json::Value>,
    /// Additional message to inject into context
    pub message: Option<String>,
    /// Whether the current step should be retried
    pub retry: bool,
    /// Metadata for observability
    pub metadata: HashMap<String, String>,
}

impl Default for HookResult {
    fn default() -> Self {
        Self {
            action: HookAction::Allow,
            modified_args: None,
            message: None,
            retry: false,
            metadata: HashMap::new(),
        }
    }
}

impl HookResult {
    /// Create an allow result
    pub fn allow() -> Self {
        Self::default()
    }

    /// Create an allow result with an injected message
    pub fn allow_with_message(msg: impl Into<String>) -> Self {
        Self {
            action: HookAction::Allow,
            message: Some(msg.into()),
            ..Default::default()
        }
    }

    /// Create a block result with reason
    pub fn block(reason: impl Into<String>) -> Self {
        Self {
            action: HookAction::Block {
                reason: reason.into(),
            },
            ..Default::default()
        }
    }

    /// Create a modify result with new args
    pub fn modify(new_args: serde_json::Value) -> Self {
        Self {
            action: HookAction::Modify,
            modified_args: Some(new_args),
            ..Default::default()
        }
    }

    /// Create a replace result
    pub fn replace(tool_name: impl Into<String>, args: serde_json::Value) -> Self {
        Self {
            action: HookAction::Replace {
                tool_name: tool_name.into(),
                args,
            },
            ..Default::default()
        }
    }

    /// Create a retry result
    pub fn retry_with_message(msg: impl Into<String>) -> Self {
        Self {
            action: HookAction::Allow,
            retry: true,
            message: Some(msg.into()),
            ..Default::default()
        }
    }
}

/// Action taken by a hook
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HookAction {
    /// Allow execution to continue
    Allow,
    /// Modify parameters/result and continue
    Modify,
    /// Block execution with reason
    Block { reason: String },
    /// Replace with alternative tool call
    Replace {
        tool_name: String,
        args: serde_json::Value,
    },
}

// ============================================================================
// Hook Context
// ============================================================================

/// Context passed to hook execution functions
#[derive(Debug, Clone)]
pub struct HookContext {
    /// Harness instance ID
    pub harness_id: String,
    /// Current step number
    pub step_number: usize,
    /// Tool name (for tool-level hooks)
    pub tool_name: String,
    /// Tool arguments (for PreToolUse hooks)
    pub tool_args: serde_json::Value,
    /// Tool result (for PostToolUse hooks)
    pub tool_result: Option<Result<String, HarnessError>>,
    /// Agent type/role
    pub agent_type: Option<String>,
    /// Task ID
    pub task_id: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
    /// Which hook point triggered this
    pub hook_point: HookPoint,
}

impl HookContext {
    /// Create a minimal context for a hook point
    pub fn for_point(point: HookPoint) -> Self {
        Self {
            harness_id: String::new(),
            step_number: 0,
            tool_name: String::new(),
            tool_args: serde_json::Value::Null,
            tool_result: None,
            agent_type: None,
            task_id: None,
            metadata: HashMap::new(),
            hook_point: point,
        }
    }

    /// Create a tool-use context
    pub fn for_tool_use(
        point: HookPoint,
        harness_id: impl Into<String>,
        step_number: usize,
        tool_name: impl Into<String>,
        args: serde_json::Value,
    ) -> Self {
        Self {
            harness_id: harness_id.into(),
            step_number,
            tool_name: tool_name.into(),
            tool_args: args,
            tool_result: None,
            agent_type: None,
            task_id: None,
            metadata: HashMap::new(),
            hook_point: point,
        }
    }

    /// Create a post-tool-use context with result
    pub fn for_tool_result(
        point: HookPoint,
        harness_id: impl Into<String>,
        step_number: usize,
        tool_name: impl Into<String>,
        result: Result<String, HarnessError>,
    ) -> Self {
        Self {
            harness_id: harness_id.into(),
            step_number,
            tool_name: tool_name.into(),
            tool_args: serde_json::Value::Null,
            tool_result: Some(result),
            agent_type: None,
            task_id: None,
            metadata: HashMap::new(),
            hook_point: point,
        }
    }
}

// ============================================================================
// Hook Trait
// ============================================================================

/// Error type for hook operations
#[derive(Debug, Error)]
pub enum HookError {
    #[error("Hook execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Hook timeout after {0:?}: {1}")]
    Timeout(Duration, String),
    #[error("Hook not found: {0}")]
    NotFound(String),
}

/// Trait that all hooks must implement
#[async_trait::async_trait]
pub trait Hook: Send + Sync {
    /// Unique name of this hook
    fn name(&self) -> &str;

    /// Which hook point(s) this hook listens to
    fn point(&self) -> HookPoint;

    /// Priority (lower = executed first). Default 0.
    fn priority(&self) -> i32 {
        0
    }

    /// Whether this hook is enabled
    fn is_enabled(&self) -> bool {
        true
    }

    /// Timeout for this hook's execution. Default 5s.
    fn timeout(&self) -> Duration {
        Duration::from_secs(5)
    }

    /// Execute the hook with given context
    async fn execute(&self, ctx: &HookContext) -> Result<HookResult, HookError>;
}

// ============================================================================
// Hook Registry
// ============================================================================

/// Registry that manages hook registration, lookup, and execution
pub struct HookRegistry {
    hooks: Arc<RwLock<Vec<Arc<dyn Hook>>>>,
    /// Index by hook point for fast lookup
    by_point: Arc<RwLock<HashMap<HookPoint, Vec<usize>>>>,
    /// Whether hooks are globally enabled
    enabled: Arc<std::sync::atomic::AtomicBool>,
    /// Names of disabled hooks
    disabled_hooks: Arc<RwLock<Vec<String>>>,
}

impl HookRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            hooks: Arc::new(RwLock::new(Vec::new())),
            by_point: Arc::new(RwLock::new(HashMap::new())),
            enabled: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            disabled_hooks: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create a registry with default built-in hooks
    pub fn with_defaults() -> Self {
        let registry = Self::new();
        registry
    }

    /// Register a hook
    pub async fn register(&self, hook: Arc<dyn Hook>) {
        let point = hook.point();
        let mut hooks = self.hooks.write().await;
        let idx = hooks.len();
        hooks.push(hook);

        let mut by_point = self.by_point.write().await;
        by_point.entry(point).or_default().push(idx);
    }

    /// Unregister a hook by name
    pub async fn unregister(&self, name: &str) {
        let disabled = self.disabled_hooks.read().await;
        if !disabled.contains(&name.to_string()) {
            drop(disabled);
            let mut disabled = self.disabled_hooks.write().await;
            disabled.push(name.to_string());
        }
    }

    /// Check if a hook name is disabled
    pub async fn is_hook_disabled(&self, name: &str) -> bool {
        let disabled = self.disabled_hooks.read().await;
        disabled.iter().any(|n| n == name)
    }

    /// Enable or disable all hooks globally
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled
            .store(enabled, std::sync::atomic::Ordering::SeqCst);
    }

    /// Check if hooks are globally enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Get hooks for a specific point, sorted by priority
    async fn get_hooks_for_point(&self, point: &HookPoint) -> Vec<Arc<dyn Hook>> {
        if !self.is_enabled() {
            return Vec::new();
        }

        let indices = {
            let by_point = self.by_point.read().await;
            by_point.get(point).cloned().unwrap_or_default()
        };

        if indices.is_empty() {
            return Vec::new();
        }

        let hooks = self.hooks.read().await;
        let disabled = self.disabled_hooks.read().await;

        let mut result: Vec<Arc<dyn Hook>> = Vec::new();
        for &idx in &indices {
            if let Some(hook) = hooks.get(idx) {
                if hook.is_enabled() && !disabled.iter().any(|n| n == hook.name()) {
                    result.push(hook.clone());
                }
            }
        }

        // Sort by priority (lower = first)
        result.sort_by_key(|h| h.priority());
        result
    }

    /// Execute all hooks for a given point. Returns the first non-Allow result.
    /// If all hooks return Allow, returns Ok(HookResult::default()).
    pub async fn run_hooks(
        &self,
        point: HookPoint,
        ctx: &HookContext,
    ) -> Result<HookResult, HookError> {
        let hooks = self.get_hooks_for_point(&point).await;

        let mut last_allow = HookResult::default();

        for hook in hooks {
            let hook_name = hook.name().to_string();
            let timeout = hook.timeout();

            let result = tokio::time::timeout(timeout, hook.execute(ctx)).await;

            match result {
                Ok(Ok(hook_result)) => {
                    // If hook returns non-Allow, propagate immediately
                    if !matches!(hook_result.action, HookAction::Allow) {
                        info!(
                            hook = %hook_name,
                            point = %point,
                            action = ?hook_result.action,
                            "Hook intercepted execution"
                        );
                        return Ok(hook_result);
                    }
                    // Accumulate the last Allow result (preserves messages from all hooks)
                    if hook_result.message.is_some() {
                        last_allow = hook_result;
                    } else if !hook_result.metadata.is_empty() {
                        last_allow.metadata.extend(hook_result.metadata);
                    }
                }
                Ok(Err(e)) => {
                    warn!(
                        hook = %hook_name,
                        point = %point,
                        error = %e,
                        "Hook execution failed, continuing"
                    );
                    // Don't fail the whole pipeline for a single hook error
                }
                Err(_) => {
                    warn!(
                        hook = %hook_name,
                        point = %point,
                        timeout_ms = timeout.as_millis(),
                        "Hook timed out, continuing"
                    );
                    // Don't fail the whole pipeline for a hook timeout
                }
            }
        }

        Ok(last_allow)
    }

    /// Convenience: run PreToolUse hooks
    pub async fn run_pre_tool_use(
        &self,
        ctx: &HookContext,
    ) -> Result<HookResult, HookError> {
        self.run_hooks(HookPoint::PreToolUse, ctx).await
    }

    /// Convenience: run PostToolUse hooks
    pub async fn run_post_tool_use(
        &self,
        ctx: &HookContext,
    ) -> Result<HookResult, HookError> {
        self.run_hooks(HookPoint::PostToolUse, ctx).await
    }

    /// List all registered hook names for a point
    pub async fn list_hooks(&self, point: HookPoint) -> Vec<String> {
        let hooks = self.get_hooks_for_point(&point).await;
        hooks.iter().map(|h| h.name().to_string()).collect()
    }

    /// List all registered hooks with their points
    pub async fn list_all_hooks(&self) -> Vec<(String, HookPoint, i32, bool)> {
        let hooks = self.hooks.read().await;
        let disabled = self.disabled_hooks.read().await;
        hooks
            .iter()
            .map(|h| {
                (
                    h.name().to_string(),
                    h.point(),
                    h.priority(),
                    !disabled.iter().any(|n| n == h.name()) && h.is_enabled(),
                )
            })
            .collect()
    }
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for HookRegistry {
    fn clone(&self) -> Self {
        Self {
            hooks: self.hooks.clone(),
            by_point: self.by_point.clone(),
            enabled: self.enabled.clone(),
            disabled_hooks: self.disabled_hooks.clone(),
        }
    }
}

// ============================================================================
// Built-in Hooks
// ============================================================================

/// Security audit hook — blocks dangerous tool calls
pub struct SecurityAuditHook {
    /// Tool names that are completely blocked
    blocked_tools: Vec<String>,
    /// Patterns that must NOT appear in args
    blocked_patterns: Vec<String>,
}

impl SecurityAuditHook {
    pub fn new() -> Self {
        Self {
            blocked_tools: vec![],
            blocked_patterns: vec![
                "rm -rf /".to_string(),
                "DROP TABLE".to_string(),
                "DELETE FROM".to_string(),
            ],
        }
    }

    pub fn with_blocked_tools(tools: Vec<String>) -> Self {
        Self {
            blocked_tools: tools,
            ..Self::new()
        }
    }
}

impl Default for SecurityAuditHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Hook for SecurityAuditHook {
    fn name(&self) -> &str {
        "security-audit"
    }

    fn point(&self) -> HookPoint {
        HookPoint::PreToolUse
    }

    fn priority(&self) -> i32 {
        -100 // Run first (highest priority)
    }

    async fn execute(&self, ctx: &HookContext) -> Result<HookResult, HookError> {
        // Check blocked tools
        for blocked in &self.blocked_tools {
            if ctx.tool_name == *blocked {
                return Ok(HookResult::block(format!(
                    "Tool '{}' is blocked by security policy",
                    blocked
                )));
            }
        }

        // Check blocked patterns in args
        let args_str = serde_json::to_string(&ctx.tool_args).unwrap_or_default();
        for pattern in &self.blocked_patterns {
            if args_str.to_lowercase().contains(&pattern.to_lowercase()) {
                return Ok(HookResult::block(format!(
                    "Arguments contain blocked pattern: '{}'",
                    pattern
                )));
            }
        }

        Ok(HookResult::allow())
    }
}

/// Spec injection hook — injects spec/rules before tool calls
pub struct SpecInjectionHook {
    /// Rules to inject (key = tool name or "*", value = spec text)
    specs: HashMap<String, String>,
}

impl SpecInjectionHook {
    pub fn new() -> Self {
        Self {
            specs: HashMap::new(),
        }
    }

    /// Add a spec for a specific tool (use "*" for all tools)
    pub fn with_spec(mut self, tool_pattern: impl Into<String>, spec: impl Into<String>) -> Self {
        self.specs.insert(tool_pattern.into(), spec.into());
        self
    }
}

impl Default for SpecInjectionHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Hook for SpecInjectionHook {
    fn name(&self) -> &str {
        "spec-injection"
    }

    fn point(&self) -> HookPoint {
        HookPoint::PreToolUse
    }

    fn priority(&self) -> i32 {
        -50
    }

    async fn execute(&self, ctx: &HookContext) -> Result<HookResult, HookError> {
        // Check tool-specific spec
        if let Some(spec) = self.specs.get(&ctx.tool_name) {
            return Ok(HookResult::allow_with_message(spec.clone()));
        }

        // Check wildcard spec
        if let Some(spec) = self.specs.get("*") {
            return Ok(HookResult::allow_with_message(spec.clone()));
        }

        Ok(HookResult::allow())
    }
}

/// Quality gate hook — validates tool output after execution
pub struct QualityGateHook {
    /// Minimum output length (0 = no minimum)
    pub min_output_length: usize,
    /// Maximum output length (0 = no maximum)
    pub max_output_length: usize,
    /// Whether empty results are acceptable
    pub allow_empty: bool,
}

impl QualityGateHook {
    pub fn new() -> Self {
        Self {
            min_output_length: 0,
            max_output_length: 0,
            allow_empty: true,
        }
    }

    pub fn with_min_length(mut self, len: usize) -> Self {
        self.min_output_length = len;
        self
    }

    pub fn with_max_length(mut self, len: usize) -> Self {
        self.max_output_length = len;
        self
    }
}

impl Default for QualityGateHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Hook for QualityGateHook {
    fn name(&self) -> &str {
        "quality-gate"
    }

    fn point(&self) -> HookPoint {
        HookPoint::PostToolUse
    }

    fn priority(&self) -> i32 {
        50
    }

    async fn execute(&self, ctx: &HookContext) -> Result<HookResult, HookError> {
        if let Some(ref result) = ctx.tool_result {
            match result {
                Ok(output) => {
                    if !self.allow_empty && output.trim().is_empty() {
                        return Ok(HookResult::retry_with_message(
                            "Tool returned empty result, retrying",
                        ));
                    }
                    if self.min_output_length > 0 && output.len() < self.min_output_length {
                        return Ok(HookResult::allow_with_message(format!(
                            "Warning: output length {} is below minimum {}",
                            output.len(),
                            self.min_output_length
                        )));
                    }
                    if self.max_output_length > 0 && output.len() > self.max_output_length {
                        return Ok(HookResult::allow_with_message(format!(
                            "Warning: output length {} exceeds maximum {}",
                            output.len(),
                            self.max_output_length
                        )));
                    }
                }
                Err(_) => {
                    // Errors are handled by the self-healing system, not quality gates
                }
            }
        }

        Ok(HookResult::allow())
    }
}

/// Resource warning hook — monitors resource usage and emits warnings
pub struct ResourceWarningHook {
    /// Budget warning threshold (0.0 - 1.0, default 0.8)
    pub budget_warning_threshold: f64,
    /// Token limit warning threshold (0.0 - 1.0, default 0.8)
    pub token_warning_threshold: f64,
}

impl ResourceWarningHook {
    pub fn new() -> Self {
        Self {
            budget_warning_threshold: 0.8,
            token_warning_threshold: 0.8,
        }
    }
}

impl Default for ResourceWarningHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Hook for ResourceWarningHook {
    fn name(&self) -> &str {
        "resource-warning"
    }

    fn point(&self) -> HookPoint {
        HookPoint::PostToolUse
    }

    fn priority(&self) -> i32 {
        10
    }

    async fn execute(&self, ctx: &HookContext) -> Result<HookResult, HookError> {
        // Check for budget/token metadata from the context
        if let Some(budget_ratio) = ctx
            .metadata
            .get("budget_ratio")
            .and_then(|v| v.parse::<f64>().ok())
        {
            if budget_ratio >= self.budget_warning_threshold {
                return Ok(HookResult::allow_with_message(format!(
                    "⚠️ Budget {}% used (threshold: {}%)",
                    (budget_ratio * 100.0) as u32,
                    (self.budget_warning_threshold * 100.0) as u32
                )));
            }
        }

        if let Some(token_ratio) = ctx
            .metadata
            .get("token_ratio")
            .and_then(|v| v.parse::<f64>().ok())
        {
            if token_ratio >= self.token_warning_threshold {
                return Ok(HookResult::allow_with_message(format!(
                    "⚠️ Token limit {}% used (threshold: {}%)",
                    (token_ratio * 100.0) as u32,
                    (self.token_warning_threshold * 100.0) as u32
                )));
            }
        }

        Ok(HookResult::allow())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Test hook that always allows
    struct AllowHook;
    #[async_trait::async_trait]
    impl Hook for AllowHook {
        fn name(&self) -> &str { "allow" }
        fn point(&self) -> HookPoint { HookPoint::PreToolUse }
        async fn execute(&self, _ctx: &HookContext) -> Result<HookResult, HookError> {
            Ok(HookResult::allow())
        }
    }

    /// Test hook that blocks everything
    struct BlockHook;
    #[async_trait::async_trait]
    impl Hook for BlockHook {
        fn name(&self) -> &str { "block" }
        fn point(&self) -> HookPoint { HookPoint::PreToolUse }
        fn priority(&self) -> i32 { -10 } // Higher priority
        async fn execute(&self, _ctx: &HookContext) -> Result<HookResult, HookError> {
            Ok(HookResult::block("blocked by test"))
        }
    }

    /// Test hook that modifies args
    struct ModifyHook;
    #[async_trait::async_trait]
    impl Hook for ModifyHook {
        fn name(&self) -> &str { "modify" }
        fn point(&self) -> HookPoint { HookPoint::PreToolUse }
        async fn execute(&self, _ctx: &HookContext) -> Result<HookResult, HookError> {
            Ok(HookResult::modify(serde_json::json!({"modified": true})))
        }
    }

    /// Test hook that times out
    struct TimeoutHook;
    #[async_trait::async_trait]
    impl Hook for TimeoutHook {
        fn name(&self) -> &str { "timeout" }
        fn point(&self) -> HookPoint { HookPoint::PreToolUse }
        fn timeout(&self) -> Duration { Duration::from_millis(10) }
        async fn execute(&self, _ctx: &HookContext) -> Result<HookResult, HookError> {
            tokio::time::sleep(Duration::from_secs(10)).await;
            Ok(HookResult::allow())
        }
    }

    #[tokio::test]
    async fn test_hook_registry_registration() {
        let registry = HookRegistry::new();
        registry.register(Arc::new(AllowHook)).await;
        registry.register(Arc::new(BlockHook)).await;

        let hooks = registry.list_hooks(HookPoint::PreToolUse).await;
        assert_eq!(hooks.len(), 2);
        // BlockHook has priority -10, should be first
        assert_eq!(hooks[0], "block");
        assert_eq!(hooks[1], "allow");
    }

    #[tokio::test]
    async fn test_hook_block_action() {
        let registry = HookRegistry::new();
        registry.register(Arc::new(BlockHook)).await;
        registry.register(Arc::new(AllowHook)).await;

        let ctx = HookContext::for_point(HookPoint::PreToolUse);
        let result = registry.run_hooks(HookPoint::PreToolUse, &ctx).await.unwrap();
        assert!(matches!(result.action, HookAction::Block { .. }));
    }

    #[tokio::test]
    async fn test_hook_allow_all() {
        let registry = HookRegistry::new();
        registry.register(Arc::new(AllowHook)).await;

        let ctx = HookContext::for_point(HookPoint::PreToolUse);
        let result = registry.run_hooks(HookPoint::PreToolUse, &ctx).await.unwrap();
        assert!(matches!(result.action, HookAction::Allow));
    }

    #[tokio::test]
    async fn test_hook_modify_action() {
        let registry = HookRegistry::new();
        registry.register(Arc::new(ModifyHook)).await;

        let ctx = HookContext::for_point(HookPoint::PreToolUse);
        let result = registry.run_hooks(HookPoint::PreToolUse, &ctx).await.unwrap();
        assert!(matches!(result.action, HookAction::Modify));
        assert!(result.modified_args.is_some());
    }

    #[tokio::test]
    async fn test_hook_timeout_does_not_block() {
        let registry = HookRegistry::new();
        registry.register(Arc::new(TimeoutHook)).await;
        registry.register(Arc::new(AllowHook)).await;

        let ctx = HookContext::for_point(HookPoint::PreToolUse);
        let result = registry.run_hooks(HookPoint::PreToolUse, &ctx).await.unwrap();
        // TimeoutHook should be skipped, AllowHook should allow
        assert!(matches!(result.action, HookAction::Allow));
    }

    #[tokio::test]
    async fn test_hook_disable_global() {
        let registry = HookRegistry::new();
        registry.register(Arc::new(BlockHook)).await;

        registry.set_enabled(false);
        let ctx = HookContext::for_point(HookPoint::PreToolUse);
        let result = registry.run_hooks(HookPoint::PreToolUse, &ctx).await.unwrap();
        assert!(matches!(result.action, HookAction::Allow));
    }

    #[tokio::test]
    async fn test_hook_disable_by_name() {
        let registry = HookRegistry::new();
        registry.register(Arc::new(BlockHook)).await;
        registry.register(Arc::new(AllowHook)).await;

        registry.unregister("block").await;
        let ctx = HookContext::for_point(HookPoint::PreToolUse);
        let result = registry.run_hooks(HookPoint::PreToolUse, &ctx).await.unwrap();
        // BlockHook disabled, AllowHook should allow
        assert!(matches!(result.action, HookAction::Allow));
    }

    #[tokio::test]
    async fn test_security_audit_blocks_dangerous_patterns() {
        let hook = SecurityAuditHook::new();
        let ctx = HookContext::for_tool_use(
            HookPoint::PreToolUse,
            "h1",
            1,
            "db_query",
            serde_json::json!({"sql": "DROP TABLE users"}),
        );

        let result = hook.execute(&ctx).await.unwrap();
        assert!(matches!(result.action, HookAction::Block { .. }));
    }

    #[tokio::test]
    async fn test_security_audit_allows_safe_calls() {
        let hook = SecurityAuditHook::new();
        let ctx = HookContext::for_tool_use(
            HookPoint::PreToolUse,
            "h1",
            1,
            "search",
            serde_json::json!({"query": "hello world"}),
        );

        let result = hook.execute(&ctx).await.unwrap();
        assert!(matches!(result.action, HookAction::Allow));
    }

    #[tokio::test]
    async fn test_quality_gate_rejects_empty() {
        let hook = QualityGateHook {
            allow_empty: false,
            ..QualityGateHook::new()
        };
        let ctx = HookContext::for_tool_result(
            HookPoint::PostToolUse,
            "h1",
            1,
            "search",
            Ok(String::new()),
        );

        let result = hook.execute(&ctx).await.unwrap();
        assert!(result.retry);
    }

    #[tokio::test]
    async fn test_quality_gate_allows_valid() {
        let hook = QualityGateHook::new().with_min_length(5);
        let ctx = HookContext::for_tool_result(
            HookPoint::PostToolUse,
            "h1",
            1,
            "search",
            Ok("hello world".to_string()),
        );

        let result = hook.execute(&ctx).await.unwrap();
        assert!(matches!(result.action, HookAction::Allow));
        assert!(!result.retry);
    }

    #[tokio::test]
    async fn test_spec_injection() {
        let hook = SpecInjectionHook::new()
            .with_spec("*", "Always respond in JSON format")
            .with_spec("search", "Search only in the docs directory");

        // Test wildcard spec
        let ctx = HookContext::for_tool_use(
            HookPoint::PreToolUse,
            "h1",
            1,
            "read_file",
            serde_json::json!({"path": "/tmp/test"}),
        );
        let result = hook.execute(&ctx).await.unwrap();
        assert_eq!(result.message.as_deref(), Some("Always respond in JSON format"));

        // Test tool-specific spec
        let ctx = HookContext::for_tool_use(
            HookPoint::PreToolUse,
            "h1",
            1,
            "search",
            serde_json::json!({"query": "test"}),
        );
        let result = hook.execute(&ctx).await.unwrap();
        assert_eq!(result.message.as_deref(), Some("Search only in the docs directory"));
    }

    #[tokio::test]
    async fn test_all_hook_points_defined() {
        let all = HookPoint::all();
        assert!(all.len() >= 28, "Expected at least 28 hook points, got {}", all.len());
    }

    #[tokio::test]
    async fn test_list_all_hooks() {
        let registry = HookRegistry::new();
        registry.register(Arc::new(SecurityAuditHook::new())).await;
        registry.register(Arc::new(QualityGateHook::new())).await;

        let all = registry.list_all_hooks().await;
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].0, "security-audit");
        assert_eq!(all[1].0, "quality-gate");
    }
}
