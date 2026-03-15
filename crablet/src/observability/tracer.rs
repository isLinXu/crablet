//! Agent Execution Tracer
//!
//! Traces and records every step of Agent execution for debugging and visualization.

use super::{ObservabilityEvent, EventPublisher, ExecutionMetrics};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Main tracer for Agent execution
pub struct AgentTracer {
    sessions: RwLock<HashMap<String, TraceSession>>,
    event_publisher: Arc<EventPublisher>,
    active_spans: RwLock<HashMap<String, Vec<AgentSpan>>>,
}

impl AgentTracer {
    pub fn new(event_publisher: Arc<EventPublisher>) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            event_publisher,
            active_spans: RwLock::new(HashMap::new()),
        }
    }

    /// Start a new trace session
    pub fn start_session(&mut self, session: TraceSession) {
        let execution_id = session.execution_id.clone();
        
        // Store session
        self.sessions.try_write().unwrap().insert(execution_id.clone(), session);
        
        // Initialize span storage
        self.active_spans.try_write().unwrap().insert(execution_id, Vec::new());
    }

    /// Record a thought step
    pub async fn trace_thought(&self, execution_id: &str, thought: &str, metadata: Option<ThoughtMetadata>) {
        let span = AgentSpan::Thought {
            content: thought.to_string(),
            timestamp: current_timestamp(),
            metadata,
        };
        
        self.record_span(execution_id, span).await;
    }

    /// Record an action step
    pub async fn trace_action(
        &self,
        execution_id: &str,
        tool: &str,
        params: serde_json::Value,
        reasoning: Option<String>,
    ) {
        let span = AgentSpan::Action {
            tool: tool.to_string(),
            params,
            reasoning,
            timestamp: current_timestamp(),
        };
        
        self.record_span(execution_id, span).await;
    }

    /// Record an observation step
    pub async fn trace_observation(
        &self,
        execution_id: &str,
        result: serde_json::Value,
        duration_ms: u64,
        success: bool,
    ) {
        let span = AgentSpan::Observation {
            result: result.to_string(),
            duration_ms,
            success,
            timestamp: current_timestamp(),
        };
        
        self.record_span(execution_id, span).await;
    }

    /// Record a reflection step
    pub async fn trace_reflection(
        &self,
        execution_id: &str,
        critique: &str,
        confidence: f64,
        revised_response: Option<String>,
    ) {
        let span = AgentSpan::Reflection {
            critique: critique.to_string(),
            confidence,
            revised_response,
            timestamp: current_timestamp(),
        };
        
        self.record_span(execution_id, span).await;
    }

    /// Record a decision point
    pub async fn trace_decision(
        &self,
        execution_id: &str,
        choices: Vec<String>,
        selected: String,
        reasoning: String,
        confidence: f64,
    ) {
        let span = AgentSpan::Decision {
            choices,
            selected,
            reasoning,
            confidence,
            timestamp: current_timestamp(),
        };
        
        self.record_span(execution_id, span).await;
    }

    /// Record a loop detection
    pub async fn trace_loop_detected(
        &self,
        execution_id: &str,
        loop_type: LoopType,
        description: String,
        resolution: LoopResolution,
    ) {
        let span = AgentSpan::LoopDetected {
            loop_type,
            description,
            resolution,
            timestamp: current_timestamp(),
        };
        
        self.record_span(execution_id, span).await;
    }

    /// Record an error
    pub async fn trace_error(&self, execution_id: &str, error: &str, recoverable: bool) {
        let span = AgentSpan::Error {
            error: error.to_string(),
            recoverable,
            timestamp: current_timestamp(),
        };
        
        self.record_span(execution_id, span).await;
    }

    /// Get all spans for an execution
    pub async fn get_spans(&self, execution_id: &str) -> Option<Vec<AgentSpan>> {
        self.active_spans.read().await.get(execution_id).cloned()
    }

    /// Get the last N spans
    pub async fn get_recent_spans(&self, execution_id: &str, n: usize) -> Option<Vec<AgentSpan>> {
        self.active_spans
            .read()
            .await
            .get(execution_id)
            .map(|spans| {
                spans.iter().rev().take(n).cloned().collect::<Vec<_>>().into_iter().rev().collect()
            })
    }

    /// Filter spans by type
    pub async fn filter_spans(&self, execution_id: &str, filter: TraceFilter) -> Option<Vec<AgentSpan>> {
        self.active_spans
            .read()
            .await
            .get(execution_id)
            .map(|spans| {
                spans
                    .iter()
                    .filter(|span| filter.matches(span))
                    .cloned()
                    .collect()
            })
    }

    /// End a trace session
    pub async fn end_session(&self, execution_id: &str, success: bool, final_output: Option<String>) {
        // Publish completion event
        self.event_publisher.publish(ObservabilityEvent::SessionCompleted {
            execution_id: execution_id.to_string(),
            success,
            final_output,
            timestamp: current_timestamp(),
        });
        
        // Clean up (or archive) spans
        if let Some(spans) = self.active_spans.write().await.remove(execution_id) {
            // Could persist to storage here
            tracing::info!("Session {} ended with {} spans", execution_id, spans.len());
        }
    }

    /// List all sessions
    pub fn list_sessions(&self) -> Vec<TraceSession> {
        self.sessions.try_read()
            .map(|sessions| sessions.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Get a specific session
    pub fn get_session(&self, execution_id: &str) -> Option<TraceSession> {
        self.sessions.try_read()
            .ok()
            .and_then(|sessions| sessions.get(execution_id).cloned())
    }

    /// Delete a session
    pub fn delete_session(&mut self, execution_id: &str) {
        if let Ok(mut sessions) = self.sessions.try_write() {
            sessions.remove(execution_id);
        }
        if let Ok(mut spans) = self.active_spans.try_write() {
            spans.remove(execution_id);
        }
    }

    /// Resume a paused session
    pub fn resume_session(&mut self, execution_id: &str) {
        if let Ok(mut sessions) = self.sessions.try_write() {
            if let Some(session) = sessions.get_mut(execution_id) {
                session.status = SessionStatus::Running;
            }
        }
    }

    /// Abort a session
    pub fn abort_session(&mut self, execution_id: &str) {
        if let Ok(mut sessions) = self.sessions.try_write() {
            if let Some(session) = sessions.get_mut(execution_id) {
                session.status = SessionStatus::Cancelled;
                session.ended_at = Some(current_timestamp());
            }
        }
    }

    /// Modify session context
    pub fn modify_session_context(&mut self, _execution_id: &str, _modifications: serde_json::Value) {
        // Placeholder for context modification
        // Would need to store context in session
    }

    /// Internal method to record a span
    async fn record_span(&self, execution_id: &str, span: AgentSpan) {
        // Add to active spans
        if let Some(spans) = self.active_spans.write().await.get_mut(execution_id) {
            spans.push(span.clone());
        }
        
        // Publish event
        self.event_publisher.publish(ObservabilityEvent::SpanRecorded {
            execution_id: execution_id.to_string(),
            span,
            timestamp: current_timestamp(),
        });
    }
}

/// A single step in the execution trace
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentSpan {
    /// A thought/reasoning step
    Thought {
        content: String,
        timestamp: u64,
        metadata: Option<ThoughtMetadata>,
    },
    
    /// An action/tool call
    Action {
        tool: String,
        params: serde_json::Value,
        reasoning: Option<String>,
        timestamp: u64,
    },
    
    /// An observation/result
    Observation {
        result: String,
        duration_ms: u64,
        success: bool,
        timestamp: u64,
    },
    
    /// A self-reflection
    Reflection {
        critique: String,
        confidence: f64,
        revised_response: Option<String>,
        timestamp: u64,
    },
    
    /// A decision point
    Decision {
        choices: Vec<String>,
        selected: String,
        reasoning: String,
        confidence: f64,
        timestamp: u64,
    },
    
    /// Loop detection
    LoopDetected {
        loop_type: LoopType,
        description: String,
        resolution: LoopResolution,
        timestamp: u64,
    },
    
    /// An error
    Error {
        error: String,
        recoverable: bool,
        timestamp: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThoughtMetadata {
    pub step_number: usize,
    pub iteration: usize,
    pub confidence: Option<f64>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopType {
    ExactRepetition,
    SemanticSimilarity,
    ResourceAccess,
    ToolChain,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopResolution {
    Continue,
    Skip,
    ModifyParams(serde_json::Value),
    RequestHumanIntervention,
    Abort,
}

/// A complete trace session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSession {
    pub execution_id: String,
    pub workflow_id: String,
    pub started_at: u64,
    pub ended_at: Option<u64>,
    pub status: SessionStatus,
    pub metadata: SessionMetadata,
    pub spans: Vec<AgentSpan>,
    pub metrics: ExecutionMetrics,
}

impl TraceSession {
    pub fn new(execution_id: String, workflow_id: String) -> Self {
        Self {
            execution_id,
            workflow_id,
            started_at: current_timestamp(),
            ended_at: None,
            status: SessionStatus::Running,
            metadata: SessionMetadata::default(),
            spans: Vec::new(),
            metrics: ExecutionMetrics::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub agent_role: Option<String>,
    pub paradigm: Option<String>,
    pub max_iterations: Option<usize>,
    pub tags: Vec<String>,
}

/// Filter for querying spans
pub struct TraceFilter {
    pub span_types: Option<Vec<String>>,
    pub start_time: Option<u64>,
    pub end_time: Option<u64>,
    pub contains_text: Option<String>,
}

impl TraceFilter {
    pub fn matches(&self, span: &AgentSpan) -> bool {
        // Check span type
        if let Some(ref types) = self.span_types {
            let span_type = match span {
                AgentSpan::Thought { .. } => "thought",
                AgentSpan::Action { .. } => "action",
                AgentSpan::Observation { .. } => "observation",
                AgentSpan::Reflection { .. } => "reflection",
                AgentSpan::Decision { .. } => "decision",
                AgentSpan::LoopDetected { .. } => "loop_detected",
                AgentSpan::Error { .. } => "error",
            };
            if !types.contains(&span_type.to_string()) {
                return false;
            }
        }
        
        // Check time range
        let timestamp = match span {
            AgentSpan::Thought { timestamp, .. } => *timestamp,
            AgentSpan::Action { timestamp, .. } => *timestamp,
            AgentSpan::Observation { timestamp, .. } => *timestamp,
            AgentSpan::Reflection { timestamp, .. } => *timestamp,
            AgentSpan::Decision { timestamp, .. } => *timestamp,
            AgentSpan::LoopDetected { timestamp, .. } => *timestamp,
            AgentSpan::Error { timestamp, .. } => *timestamp,
        };
        
        if let Some(start) = self.start_time {
            if timestamp < start {
                return false;
            }
        }
        
        if let Some(end) = self.end_time {
            if timestamp > end {
                return false;
            }
        }
        
        // Check text content
        if let Some(ref text) = self.contains_text {
            let content = match span {
                AgentSpan::Thought { content, .. } => content.clone(),
                AgentSpan::Action { tool, reasoning, .. } => {
                    format!("{} {:?}", tool, reasoning)
                }
                AgentSpan::Observation { result, .. } => result.clone(),
                AgentSpan::Reflection { critique, .. } => critique.clone(),
                AgentSpan::Decision { reasoning, .. } => reasoning.clone(),
                AgentSpan::LoopDetected { description, .. } => description.clone(),
                AgentSpan::Error { error, .. } => error.clone(),
            };
            if !content.to_lowercase().contains(&text.to_lowercase()) {
                return false;
            }
        }
        
        true
    }
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
