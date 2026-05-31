//! Agent lifecycle hooks.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Hook execution points across the agent lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookPoint {
    PreToolUse,
    PostToolUse,
    BeforeAgentStart,
    AfterAgentStart,
    BeforeAgentEnd,
    AfterAgentEnd,
    PreModelCall,
    PostModelCall,
    PrePlan,
    PostPlan,
    PreReflect,
    PostReflect,
    PreStep,
    PostStep,
    BeforeMemoryWrite,
    AfterMemoryWrite,
    BeforeMemoryRead,
    AfterMemoryRead,
    BeforeTaskStart,
    AfterTaskEnd,
    PreCheckpoint,
    PostCheckpoint,
    PreDispatch,
    PostDispatch,
    PreError,
    PostError,
    PreRecovery,
    PostRecovery,
    PreFinalize,
    PostFinalize,
}

impl HookPoint {
    pub fn all() -> Vec<Self> {
        vec![
            Self::PreToolUse,
            Self::PostToolUse,
            Self::BeforeAgentStart,
            Self::AfterAgentStart,
            Self::BeforeAgentEnd,
            Self::AfterAgentEnd,
            Self::PreModelCall,
            Self::PostModelCall,
            Self::PrePlan,
            Self::PostPlan,
            Self::PreReflect,
            Self::PostReflect,
            Self::PreStep,
            Self::PostStep,
            Self::BeforeMemoryWrite,
            Self::AfterMemoryWrite,
            Self::BeforeMemoryRead,
            Self::AfterMemoryRead,
            Self::BeforeTaskStart,
            Self::AfterTaskEnd,
            Self::PreCheckpoint,
            Self::PostCheckpoint,
            Self::PreDispatch,
            Self::PostDispatch,
            Self::PreError,
            Self::PostError,
            Self::PreRecovery,
            Self::PostRecovery,
            Self::PreFinalize,
            Self::PostFinalize,
        ]
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::PreToolUse => "Before a tool is used",
            Self::PostToolUse => "After a tool is used",
            Self::BeforeAgentStart => "Before agent start",
            Self::AfterAgentStart => "After agent start",
            Self::BeforeAgentEnd => "Before agent end",
            Self::AfterAgentEnd => "After agent end",
            Self::PreModelCall => "Before model call",
            Self::PostModelCall => "After model call",
            Self::PrePlan => "Before planning",
            Self::PostPlan => "After planning",
            Self::PreReflect => "Before reflection",
            Self::PostReflect => "After reflection",
            Self::PreStep => "Before step execution",
            Self::PostStep => "After step execution",
            Self::BeforeMemoryWrite => "Before memory write",
            Self::AfterMemoryWrite => "After memory write",
            Self::BeforeMemoryRead => "Before memory read",
            Self::AfterMemoryRead => "After memory read",
            Self::BeforeTaskStart => "Before task start",
            Self::AfterTaskEnd => "After task end",
            Self::PreCheckpoint => "Before checkpoint",
            Self::PostCheckpoint => "After checkpoint",
            Self::PreDispatch => "Before dispatch",
            Self::PostDispatch => "After dispatch",
            Self::PreError => "Before error handling",
            Self::PostError => "After error handling",
            Self::PreRecovery => "Before recovery",
            Self::PostRecovery => "After recovery",
            Self::PreFinalize => "Before finalize",
            Self::PostFinalize => "After finalize",
        }
    }
}

/// Context passed to hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookContext {
    pub point: HookPoint,
    pub session_id: String,
    pub step_number: usize,
    pub tool_name: String,
    pub tool_args: serde_json::Value,
    pub input: Option<String>,
    pub output: Option<String>,
    pub metadata: HashMap<String, String>,
}

impl HookContext {
    pub fn for_point(point: HookPoint) -> Self {
        Self {
            point,
            session_id: String::new(),
            step_number: 0,
            tool_name: String::new(),
            tool_args: serde_json::json!({}),
            input: None,
            output: None,
            metadata: HashMap::new(),
        }
    }

    pub fn for_tool_use(
        point: HookPoint,
        session_id: impl Into<String>,
        step_number: usize,
        tool_name: impl Into<String>,
        tool_args: serde_json::Value,
    ) -> Self {
        Self {
            point,
            session_id: session_id.into(),
            step_number,
            tool_name: tool_name.into(),
            tool_args,
            input: None,
            output: None,
            metadata: HashMap::new(),
        }
    }
}

/// Result of a hook execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResult {
    pub action: HookAction,
    pub message: Option<String>,
    pub retry: bool,
    pub metadata: HashMap<String, String>,
    pub payload: Option<serde_json::Value>,
}

impl HookResult {
    pub fn allow() -> Self {
        Self {
            action: HookAction::Allow,
            message: None,
            retry: false,
            metadata: HashMap::new(),
            payload: None,
        }
    }

    pub fn allow_with_message(message: impl Into<String>) -> Self {
        let mut result = Self::allow();
        result.message = Some(message.into());
        result
    }

    pub fn block(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        Self {
            action: HookAction::Block {
                reason: reason.clone(),
            },
            message: Some(reason),
            retry: false,
            metadata: HashMap::new(),
            payload: None,
        }
    }

    pub fn modify(payload: serde_json::Value) -> Self {
        Self {
            action: HookAction::Modify,
            message: None,
            retry: false,
            metadata: HashMap::new(),
            payload: Some(payload),
        }
    }

    pub fn replace(tool_name: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            action: HookAction::Replace {
                tool_name: tool_name.into(),
                args: payload.clone(),
            },
            message: None,
            retry: false,
            metadata: HashMap::new(),
            payload: Some(payload),
        }
    }

    pub fn retry_with_message(message: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            action: HookAction::Retry {
                message: message.clone(),
            },
            message: Some(message),
            retry: true,
            metadata: HashMap::new(),
            payload: None,
        }
    }
}

/// Hook action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HookAction {
    Allow,
    Block {
        reason: String,
    },
    Modify,
    Replace {
        tool_name: String,
        args: serde_json::Value,
    },
    Retry {
        message: String,
    },
}

/// Hook execution error.
#[derive(Debug, Clone, thiserror::Error)]
pub enum HookError {
    #[error("hook execution failed: {0}")]
    Execution(String),
}

/// Hook trait.
#[async_trait]
pub trait Hook: Send + Sync {
    fn name(&self) -> &str;
    fn point(&self) -> HookPoint;
    fn priority(&self) -> i32 {
        0
    }
    async fn execute(&self, ctx: &HookContext) -> Result<HookResult, HookError>;
}

#[derive(Clone)]
struct HookEntry {
    priority: i32,
    name: String,
    hook: Arc<dyn Hook>,
}

/// Registry of hooks.
#[derive(Clone, Default)]
pub struct HookRegistry {
    hooks: Arc<RwLock<HashMap<HookPoint, Vec<HookEntry>>>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register(&self, hook: Arc<dyn Hook>) {
        let entry = HookEntry {
            priority: hook.priority(),
            name: hook.name().to_string(),
            hook: hook.clone(),
        };

        let mut hooks = self.hooks.write().await;
        let list = hooks.entry(hook.point()).or_default();
        list.push(entry);
        list.sort_by_key(|entry| entry.priority);
    }

    pub async fn list_hooks(&self, point: HookPoint) -> Vec<String> {
        let hooks = self.hooks.read().await;
        hooks
            .get(&point)
            .map(|entries| entries.iter().map(|entry| entry.name.clone()).collect())
            .unwrap_or_default()
    }

    pub async fn run_hooks(
        &self,
        point: HookPoint,
        ctx: &HookContext,
    ) -> Result<HookResult, HookError> {
        let entries = {
            let hooks = self.hooks.read().await;
            hooks.get(&point).cloned().unwrap_or_default()
        };

        let mut final_result = HookResult::allow();
        for entry in entries {
            let result = entry.hook.execute(ctx).await?;
            Self::merge_result(&mut final_result, &result);

            if matches!(
                result.action,
                HookAction::Block { .. } | HookAction::Retry { .. }
            ) {
                return Ok(result);
            }
        }

        Ok(final_result)
    }

    pub async fn run_pre_tool_use(&self, ctx: &HookContext) -> Result<HookResult, HookError> {
        self.run_hooks(HookPoint::PreToolUse, ctx).await
    }

    fn merge_result(target: &mut HookResult, source: &HookResult) {
        target.retry |= source.retry;
        if source.message.is_some() {
            target.message = source.message.clone();
        }
        target.metadata.extend(source.metadata.clone());
        if source.payload.is_some() {
            target.payload = source.payload.clone();
        }
        if !matches!(source.action, HookAction::Allow) {
            target.action = source.action.clone();
        }
    }
}

/// Security audit hook.
#[derive(Debug, Clone)]
pub struct SecurityAuditHook {
    blocked_tools: Vec<String>,
    priority: i32,
}

impl SecurityAuditHook {
    pub fn new() -> Self {
        Self {
            blocked_tools: Vec::new(),
            priority: -100,
        }
    }

    pub fn with_blocked_tools(blocked_tools: Vec<String>) -> Self {
        Self {
            blocked_tools,
            priority: -100,
        }
    }
}

impl Default for SecurityAuditHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Hook for SecurityAuditHook {
    fn name(&self) -> &str {
        "security-audit"
    }

    fn point(&self) -> HookPoint {
        HookPoint::PreToolUse
    }

    fn priority(&self) -> i32 {
        self.priority
    }

    async fn execute(&self, ctx: &HookContext) -> Result<HookResult, HookError> {
        if self.blocked_tools.iter().any(|tool| tool == &ctx.tool_name) {
            Ok(HookResult::block(format!(
                "Blocked tool: {}",
                ctx.tool_name
            )))
        } else {
            Ok(HookResult::allow())
        }
    }
}

/// Spec injection hook.
#[derive(Debug, Clone)]
pub struct SpecInjectionHook {
    specs: Vec<(String, String)>,
    priority: i32,
}

impl SpecInjectionHook {
    pub fn new() -> Self {
        Self {
            specs: Vec::new(),
            priority: -50,
        }
    }

    pub fn with_spec(mut self, pattern: impl Into<String>, message: impl Into<String>) -> Self {
        self.specs.push((pattern.into(), message.into()));
        self
    }
}

impl Default for SpecInjectionHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Hook for SpecInjectionHook {
    fn name(&self) -> &str {
        "spec-injection"
    }

    fn point(&self) -> HookPoint {
        HookPoint::PreToolUse
    }

    fn priority(&self) -> i32 {
        self.priority
    }

    async fn execute(&self, ctx: &HookContext) -> Result<HookResult, HookError> {
        for (pattern, message) in &self.specs {
            if pattern == "*" || pattern == &ctx.tool_name {
                return Ok(HookResult::allow_with_message(message.clone()));
            }
        }

        Ok(HookResult::allow())
    }
}

/// Quality gate hook.
#[derive(Debug, Clone)]
pub struct QualityGateHook {
    pub allow_empty: bool,
    pub min_output_length: usize,
    pub max_output_length: usize,
    priority: i32,
}

impl QualityGateHook {
    pub fn new() -> Self {
        Self {
            allow_empty: true,
            min_output_length: 0,
            max_output_length: 0,
            priority: 40,
        }
    }
}

impl Default for QualityGateHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Hook for QualityGateHook {
    fn name(&self) -> &str {
        "quality-gate"
    }

    fn point(&self) -> HookPoint {
        HookPoint::PostToolUse
    }

    fn priority(&self) -> i32 {
        self.priority
    }

    async fn execute(&self, _ctx: &HookContext) -> Result<HookResult, HookError> {
        Ok(HookResult::allow())
    }
}

/// Resource warning hook.
#[derive(Debug, Clone)]
pub struct ResourceWarningHook {
    pub budget_warning_threshold: f64,
    pub token_warning_threshold: f64,
    pub memory_warning_threshold: usize,
    pub cpu_warning_threshold_ms: u64,
    priority: i32,
}

impl ResourceWarningHook {
    pub fn new() -> Self {
        Self {
            budget_warning_threshold: 0.8,
            token_warning_threshold: 0.8,
            memory_warning_threshold: 0,
            cpu_warning_threshold_ms: 0,
            priority: 30,
        }
    }
}

impl Default for ResourceWarningHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Hook for ResourceWarningHook {
    fn name(&self) -> &str {
        "resource-warning"
    }

    fn point(&self) -> HookPoint {
        HookPoint::PostToolUse
    }

    fn priority(&self) -> i32 {
        self.priority
    }

    async fn execute(&self, _ctx: &HookContext) -> Result<HookResult, HookError> {
        Ok(HookResult::allow())
    }
}
