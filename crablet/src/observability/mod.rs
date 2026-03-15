//! Agent Observability Module
//!
//! Provides comprehensive tracing, debugging, and monitoring capabilities for Agent execution.
//!
//! # Features
//! - Real-time execution tracing
//! - Smart breakpoints with conditions
//! - Execution replay and forking
//! - Performance metrics and visualization

pub mod tracer;
pub mod breakpoints;
pub mod replay;
pub mod metrics;
pub mod events;
pub mod storage;

pub use tracer::{AgentTracer, AgentSpan, TraceSession, TraceFilter, ThoughtMetadata, LoopType, LoopResolution};
pub use breakpoints::{BreakpointManager, Breakpoint, BreakpointCondition, BreakpointAction, PauseReason};
pub use replay::{ExecutionReplay, ExecutionRecording, ReplayPoint, RecordingMetadata};
pub use metrics::{ExecutionMetrics, PerformanceStats, CostTracker, StepMetrics, TokenUsage};
pub use events::{ObservabilityEvent, TraceEvent, EventPublisher};
pub use storage::{TraceStorage, InMemoryStorage, PersistentStorage};
pub use tracer::SessionStatus;

use std::sync::Arc;
use tokio::sync::RwLock;

/// Main observability coordinator
pub struct ObservabilityManager {
    tracer: Arc<RwLock<AgentTracer>>,
    breakpoint_manager: Arc<RwLock<BreakpointManager>>,
    storage: Arc<dyn TraceStorage>,
    event_publisher: Arc<EventPublisher>,
}

impl ObservabilityManager {
    pub fn new(storage: Arc<dyn TraceStorage>) -> Self {
        let event_publisher = Arc::new(EventPublisher::new());
        
        Self {
            tracer: Arc::new(RwLock::new(AgentTracer::new(event_publisher.clone()))),
            breakpoint_manager: Arc::new(RwLock::new(BreakpointManager::new())),
            storage,
            event_publisher,
        }
    }

    /// Start a new trace session
    pub async fn start_session(&self, execution_id: String, workflow_id: String) -> TraceSession {
        let session = TraceSession::new(execution_id.clone(), workflow_id.clone());
        
        // Store session
        self.storage.store_session(&session).await.ok();
        
        // Initialize tracer
        let mut tracer = self.tracer.write().await;
        tracer.start_session(session.clone());
        
        // Publish event
        self.event_publisher.publish(ObservabilityEvent::SessionStarted {
            execution_id,
            workflow_id,
            timestamp: current_timestamp(),
        });
        
        session
    }

    /// Get the tracer instance
    pub fn tracer(&self) -> Arc<RwLock<AgentTracer>> {
        self.tracer.clone()
    }

    /// Get the breakpoint manager
    pub fn breakpoint_manager(&self) -> Arc<RwLock<BreakpointManager>> {
        self.breakpoint_manager.clone()
    }

    /// Subscribe to observability events
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<ObservabilityEvent> {
        self.event_publisher.subscribe()
    }
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionContext {
    pub execution_id: String,
    pub step_number: usize,
    pub current_thought: Option<String>,
    pub current_action: Option<String>,
    pub variables: std::collections::HashMap<String, serde_json::Value>,
}
