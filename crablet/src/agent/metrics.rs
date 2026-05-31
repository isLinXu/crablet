//! Lightweight agent metrics primitives.

use serde::{Deserialize, Serialize};

/// Snapshot exposed by the harness fusion engine.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentMetricsSnapshot {
    pub steps_completed: usize,
    pub steps_failed: usize,
    pub self_healing_attempts: usize,
    pub circuit_breaker_trips: usize,
    pub current_step_duration_ms: f64,
    pub total_steps: usize,
    pub total_errors: usize,
    pub total_tool_calls: usize,
    pub total_duration_ms: u64,
}
