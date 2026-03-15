//! Observability Events
//!
//! Event system for real-time monitoring and WebSocket broadcasting.

use super::{AgentSpan, ExecutionContext, BreakpointAction};
use serde::{Serialize, Deserialize};
use tokio::sync::broadcast;

/// Event publisher for observability events
pub struct EventPublisher {
    sender: broadcast::Sender<ObservabilityEvent>,
}

impl EventPublisher {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1000);
        Self { sender }
    }

    pub fn publish(&self, event: ObservabilityEvent) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ObservabilityEvent> {
        self.sender.subscribe()
    }
}

impl Default for EventPublisher {
    fn default() -> Self {
        Self::new()
    }
}

/// All observability events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum ObservabilityEvent {
    /// Session started
    SessionStarted {
        execution_id: String,
        workflow_id: String,
        timestamp: u64,
    },

    /// Session completed
    SessionCompleted {
        execution_id: String,
        success: bool,
        final_output: Option<String>,
        timestamp: u64,
    },

    /// New span recorded
    SpanRecorded {
        execution_id: String,
        span: AgentSpan,
        timestamp: u64,
    },

    /// Breakpoint hit
    BreakpointHit {
        execution_id: String,
        breakpoint_id: String,
        context: ExecutionContext,
        timestamp: u64,
    },

    /// Execution paused
    ExecutionPaused {
        execution_id: String,
        reason: super::PauseReason,
        context: ExecutionContext,
        timeout_seconds: u64,
        timestamp: u64,
    },

    /// Execution resumed
    ExecutionResumed {
        execution_id: String,
        action: BreakpointAction,
        timestamp: u64,
    },

    /// Trace event (detailed)
    TraceEvent(TraceEvent),

    /// Metrics update
    MetricsUpdate {
        execution_id: String,
        metrics: serde_json::Value,
        timestamp: u64,
    },

    /// Error occurred
    Error {
        execution_id: String,
        error: String,
        recoverable: bool,
        timestamp: u64,
    },
}

/// Detailed trace event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    pub execution_id: String,
    pub step_number: usize,
    pub event_type: TraceEventType,
    pub timestamp: u64,
    pub details: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceEventType {
    ThoughtStarted,
    ThoughtCompleted,
    ActionStarted,
    ActionCompleted,
    ObservationReceived,
    ReflectionStarted,
    ReflectionCompleted,
    DecisionMade,
    LoopDetected,
    Error,
}

impl ObservabilityEvent {
    /// Get the execution ID associated with this event
    pub fn execution_id(&self) -> Option<&str> {
        match self {
            ObservabilityEvent::SessionStarted { execution_id, .. } => Some(execution_id),
            ObservabilityEvent::SessionCompleted { execution_id, .. } => Some(execution_id),
            ObservabilityEvent::SpanRecorded { execution_id, .. } => Some(execution_id),
            ObservabilityEvent::BreakpointHit { execution_id, .. } => Some(execution_id),
            ObservabilityEvent::ExecutionPaused { execution_id, .. } => Some(execution_id),
            ObservabilityEvent::ExecutionResumed { execution_id, .. } => Some(execution_id),
            ObservabilityEvent::TraceEvent(event) => Some(&event.execution_id),
            ObservabilityEvent::MetricsUpdate { execution_id, .. } => Some(execution_id),
            ObservabilityEvent::Error { execution_id, .. } => Some(execution_id),
        }
    }

    /// Get the timestamp
    pub fn timestamp(&self) -> u64 {
        match self {
            ObservabilityEvent::SessionStarted { timestamp, .. } => *timestamp,
            ObservabilityEvent::SessionCompleted { timestamp, .. } => *timestamp,
            ObservabilityEvent::SpanRecorded { timestamp, .. } => *timestamp,
            ObservabilityEvent::BreakpointHit { timestamp, .. } => *timestamp,
            ObservabilityEvent::ExecutionPaused { timestamp, .. } => *timestamp,
            ObservabilityEvent::ExecutionResumed { timestamp, .. } => *timestamp,
            ObservabilityEvent::TraceEvent(event) => event.timestamp,
            ObservabilityEvent::MetricsUpdate { timestamp, .. } => *timestamp,
            ObservabilityEvent::Error { timestamp, .. } => *timestamp,
        }
    }

    /// Convert to WebSocket message format
    pub fn to_websocket_message(&self) -> WebSocketMessage {
        WebSocketMessage {
            message_type: self.message_type(),
            payload: serde_json::to_value(self).unwrap_or_default(),
            timestamp: self.timestamp(),
        }
    }

    fn message_type(&self) -> String {
        match self {
            ObservabilityEvent::SessionStarted { .. } => "session_started",
            ObservabilityEvent::SessionCompleted { .. } => "session_completed",
            ObservabilityEvent::SpanRecorded { .. } => "span_recorded",
            ObservabilityEvent::BreakpointHit { .. } => "breakpoint_hit",
            ObservabilityEvent::ExecutionPaused { .. } => "execution_paused",
            ObservabilityEvent::ExecutionResumed { .. } => "execution_resumed",
            ObservabilityEvent::TraceEvent { .. } => "trace_event",
            ObservabilityEvent::MetricsUpdate { .. } => "metrics_update",
            ObservabilityEvent::Error { .. } => "error",
        }
        .to_string()
    }
}

/// WebSocket message format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketMessage {
    pub message_type: String,
    pub payload: serde_json::Value,
    pub timestamp: u64,
}

/// Event filter for subscriptions
pub struct EventFilter {
    pub execution_ids: Option<Vec<String>>,
    pub event_types: Option<Vec<String>>,
    pub start_time: Option<u64>,
    pub end_time: Option<u64>,
}

impl EventFilter {
    pub fn matches(&self, event: &ObservabilityEvent) -> bool {
        // Filter by execution ID
        if let Some(ref ids) = self.execution_ids {
            if let Some(event_id) = event.execution_id() {
                if !ids.contains(&event_id.to_string()) {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Filter by event type
        if let Some(ref types) = self.event_types {
            let event_type = match event {
                ObservabilityEvent::SessionStarted { .. } => "session_started",
                ObservabilityEvent::SessionCompleted { .. } => "session_completed",
                ObservabilityEvent::SpanRecorded { .. } => "span_recorded",
                ObservabilityEvent::BreakpointHit { .. } => "breakpoint_hit",
                ObservabilityEvent::ExecutionPaused { .. } => "execution_paused",
                ObservabilityEvent::ExecutionResumed { .. } => "execution_resumed",
                ObservabilityEvent::TraceEvent { .. } => "trace_event",
                ObservabilityEvent::MetricsUpdate { .. } => "metrics_update",
                ObservabilityEvent::Error { .. } => "error",
            };
            if !types.contains(&event_type.to_string()) {
                return false;
            }
        }

        // Filter by time range
        let timestamp = event.timestamp();
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

        true
    }
}

/// Event stream with filtering
pub struct FilteredEventStream {
    receiver: broadcast::Receiver<ObservabilityEvent>,
    filter: EventFilter,
}

impl FilteredEventStream {
    pub fn new(receiver: broadcast::Receiver<ObservabilityEvent>, filter: EventFilter) -> Self {
        Self { receiver, filter }
    }

    pub async fn recv(&mut self) -> Result<ObservabilityEvent, broadcast::error::RecvError> {
        loop {
            let event = self.receiver.recv().await?;
            if self.filter.matches(&event) {
                return Ok(event);
            }
        }
    }

    pub fn try_recv(&mut self) -> Result<ObservabilityEvent, broadcast::error::TryRecvError> {
        loop {
            let event = self.receiver.try_recv()?;
            if self.filter.matches(&event) {
                return Ok(event);
            }
        }
    }
}
