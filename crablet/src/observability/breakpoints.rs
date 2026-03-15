//! Smart Breakpoint System
//!
//! Allows pausing Agent execution at specific points for inspection or intervention.

use super::{ExecutionContext, ObservabilityEvent, EventPublisher};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, oneshot};
use tokio::time::{timeout, Duration};

/// Manages breakpoints for Agent execution
pub struct BreakpointManager {
    breakpoints: RwLock<HashMap<String, Breakpoint>>,
    active_holds: RwLock<HashMap<String, HoldState>>,
    event_publisher: Arc<EventPublisher>,
}

struct HoldState {
    context: ExecutionContext,
    response_tx: oneshot::Sender<BreakpointAction>,
}

impl BreakpointManager {
    pub fn new() -> Self {
        Self {
            breakpoints: RwLock::new(HashMap::new()),
            active_holds: RwLock::new(HashMap::new()),
            event_publisher: Arc::new(EventPublisher::new()),
        }
    }

    /// Set a new breakpoint
    pub async fn set_breakpoint(&self, breakpoint: Breakpoint) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.breakpoints.write().await.insert(id.clone(), breakpoint);
        id
    }

    /// Add a new breakpoint with components
    pub fn add_breakpoint(
        &mut self,
        workflow_id: Option<String>,
        condition: BreakpointCondition,
        action: BreakpointAction,
    ) -> Breakpoint {
        let mut bp = Breakpoint::new(condition).with_action(action);
        bp.workflow_id = workflow_id;
        bp
    }

    /// Remove a breakpoint
    pub async fn remove_breakpoint(&self, id: &str) -> bool {
        self.breakpoints.write().await.remove(id).is_some()
    }

    /// List all breakpoints
    pub async fn list_breakpoints(&self) -> Vec<(String, Breakpoint)> {
        self.breakpoints
            .read()
            .await
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Check if execution should pause at current context
    pub async fn check_breakpoint(&self, context: &ExecutionContext) -> Option<BreakpointAction> {
        let breakpoints = self.breakpoints.read().await;
        
        for (_, breakpoint) in breakpoints.iter() {
            if breakpoint.condition.matches(context) {
                // Publish breakpoint hit event
                self.event_publisher.publish(ObservabilityEvent::BreakpointHit {
                    execution_id: context.execution_id.clone(),
                    breakpoint_id: breakpoint.id.clone(),
                    context: context.clone(),
                    timestamp: current_timestamp(),
                });
                
                // If auto-continue, return immediately
                if let BreakpointAction::Continue = breakpoint.action {
                    return Some(BreakpointAction::Continue);
                }
                
                // Otherwise, hold and wait for human response
                return self.hold_execution(context, breakpoint).await;
            }
        }
        
        None
    }

    /// Hold execution and wait for human intervention
    async fn hold_execution(
        &self,
        context: &ExecutionContext,
        breakpoint: &Breakpoint,
    ) -> Option<BreakpointAction> {
        let (tx, rx) = oneshot::channel();
        
        // Store hold state
        self.active_holds.write().await.insert(
            context.execution_id.clone(),
            HoldState {
                context: context.clone(),
                response_tx: tx,
            },
        );
        
        // Publish hold event
        self.event_publisher.publish(ObservabilityEvent::ExecutionPaused {
            execution_id: context.execution_id.clone(),
            reason: PauseReason::Breakpoint(breakpoint.id.clone()),
            context: context.clone(),
            timeout_seconds: breakpoint.timeout_secs,
            timestamp: current_timestamp(),
        });
        
        // Wait for response with timeout
        let timeout_duration = Duration::from_secs(breakpoint.timeout_secs);
        
        match timeout(timeout_duration, rx).await {
            Ok(Ok(action)) => {
                self.active_holds.write().await.remove(&context.execution_id);
                
                // Publish resume event
                self.event_publisher.publish(ObservabilityEvent::ExecutionResumed {
                    execution_id: context.execution_id.clone(),
                    action: action.clone(),
                    timestamp: current_timestamp(),
                });
                
                Some(action)
            }
            _ => {
                // Timeout or channel closed
                self.active_holds.write().await.remove(&context.execution_id);
                
                // Use fallback action
                let fallback = breakpoint.fallback_action.clone();
                
                self.event_publisher.publish(ObservabilityEvent::ExecutionResumed {
                    execution_id: context.execution_id.clone(),
                    action: fallback.clone(),
                    timestamp: current_timestamp(),
                });
                
                Some(fallback)
            }
        }
    }

    /// Resume a paused execution with an action
    pub async fn resume_execution(
        &self,
        execution_id: &str,
        action: BreakpointAction,
    ) -> Result<(), BreakpointError> {
        let mut holds = self.active_holds.write().await;
        
        if let Some(hold) = holds.remove(execution_id) {
            hold.response_tx
                .send(action)
                .map_err(|_| BreakpointError::SendFailed)?;
            Ok(())
        } else {
            Err(BreakpointError::ExecutionNotPaused)
        }
    }

    /// Get all paused executions
    pub async fn get_paused_executions(&self) -> Vec<(String, ExecutionContext)> {
        self.active_holds
            .read()
            .await
            .iter()
            .map(|(k, v)| (k.clone(), v.context.clone()))
            .collect()
    }

    /// Check if an execution is currently paused
    pub async fn is_paused(&self, execution_id: &str) -> bool {
        self.active_holds.read().await.contains_key(execution_id)
    }
}

/// A breakpoint definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Breakpoint {
    pub id: String,
    pub name: Option<String>,
    pub workflow_id: Option<String>,
    pub condition: BreakpointCondition,
    pub action: BreakpointAction,
    pub fallback_action: BreakpointAction,
    pub timeout_secs: u64,
    pub enabled: bool,
    pub created_at: u64,
}

impl Breakpoint {
    pub fn new(condition: BreakpointCondition) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: None,
            workflow_id: None,
            condition,
            action: BreakpointAction::Pause,
            fallback_action: BreakpointAction::Continue,
            timeout_secs: 300, // 5 minutes default
            enabled: true,
            created_at: current_timestamp(),
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_action(mut self, action: BreakpointAction) -> Self {
        self.action = action;
        self
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }
}

/// Condition for triggering a breakpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BreakpointCondition {
    /// Break before a specific tool is called
    BeforeToolCall {
        tool_pattern: Option<String>, // Regex pattern
    },

    /// Break after a specific number of iterations (alias for OnStep)
    AfterIteration {
        count: usize,
    },

    /// Break on a specific step number
    OnStep {
        count: usize,
    },

    /// Break when confidence is below threshold
    LowConfidence {
        threshold: f32,
    },

    /// Break when a loop is detected
    LoopDetected,

    /// Break when specific text appears in thought
    ThoughtContains {
        text: String,
    },

    /// Break when execution time exceeds limit
    ExecutionTimeExceeded {
        max_duration_ms: u64,
    },

    /// Break when token usage exceeds limit
    TokenBudgetExceeded {
        max_tokens: usize,
    },

    /// Break on any error
    OnError {
        recoverable_only: bool,
    },

    /// Compound condition: ALL must match
    All(Vec<BreakpointCondition>),

    /// Compound condition: ANY must match
    Any(Vec<BreakpointCondition>),
}

impl BreakpointCondition {
    pub fn matches(&self, context: &ExecutionContext) -> bool {
        match self {
            BreakpointCondition::BeforeToolCall { tool_pattern } => {
                // Check if current action is a tool call matching pattern
                if let Some(ref action) = context.current_action {
                    if let Some(ref pattern) = tool_pattern {
                        regex::Regex::new(pattern)
                            .map(|re| re.is_match(action))
                            .unwrap_or(false)
                    } else {
                        true // Match any tool call
                    }
                } else {
                    false
                }
            }
            
            BreakpointCondition::AfterIteration { count } => {
                context.step_number >= *count
            }

            BreakpointCondition::OnStep { count } => {
                context.step_number == *count
            }

            BreakpointCondition::LowConfidence { threshold: _ } => {
                // Would need to extract confidence from context
                false // Placeholder
            }
            
            BreakpointCondition::LoopDetected => {
                // Would need loop detection state in context
                false // Placeholder
            }
            
            BreakpointCondition::ThoughtContains { text } => {
                if let Some(ref thought) = context.current_thought {
                    thought.to_lowercase().contains(&text.to_lowercase())
                } else {
                    false
                }
            }
            
            BreakpointCondition::ExecutionTimeExceeded { max_duration_ms: _ } => {
                // Would need start time in context
                false // Placeholder
            }
            
            BreakpointCondition::TokenBudgetExceeded { max_tokens: _ } => {
                // Would need token count in context
                false // Placeholder
            }
            
            BreakpointCondition::OnError { recoverable_only: _ } => {
                // Would need error state in context
                false // Placeholder
            }
            
            BreakpointCondition::All(conditions) => {
                conditions.iter().all(|c| c.matches(context))
            }
            
            BreakpointCondition::Any(conditions) => {
                conditions.iter().any(|c| c.matches(context))
            }
        }
    }
}

/// Action to take when breakpoint is hit
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BreakpointAction {
    /// Pause and wait for human intervention
    Pause,
    
    /// Continue execution
    Continue,
    
    /// Continue with modified context
    ModifyContext {
        variable_updates: HashMap<String, serde_json::Value>,
    },
    
    /// Inject a hint into the context
    InjectHint {
        hint: String,
    },
    
    /// Skip the current step
    Skip,
    
    /// Abort execution
    Abort {
        reason: String,
    },
    
    /// Retry with modified parameters
    RetryWithParams {
        params: serde_json::Value,
    },
}

#[derive(Debug)]
pub enum BreakpointError {
    ExecutionNotPaused,
    SendFailed,
    Timeout,
}

impl std::fmt::Display for BreakpointError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BreakpointError::ExecutionNotPaused => write!(f, "Execution is not paused"),
            BreakpointError::SendFailed => write!(f, "Failed to send resume signal"),
            BreakpointError::Timeout => write!(f, "Breakpoint response timed out"),
        }
    }
}

impl std::error::Error for BreakpointError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PauseReason {
    Breakpoint(String),
    ClarificationNeeded,
    LowConfidence,
    Error,
    Manual,
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
