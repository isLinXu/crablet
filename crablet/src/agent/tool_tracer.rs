//! Tool Execution Tracer - Complete call tracking and visualization
//!
//! Provides comprehensive tool execution tracing:
//! - Full call chain tracking
//! - Execution timeline
//! - Performance metrics
//! - Error classification

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;
use chrono::{DateTime, Utc};

// ============================================================================
// Tool Call Trace
// ============================================================================

/// Status of a tool call
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ToolCallStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    Timeout,
}

impl std::fmt::Display for ToolCallStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolCallStatus::Pending => write!(f, "pending"),
            ToolCallStatus::Running => write!(f, "running"),
            ToolCallStatus::Completed => write!(f, "completed"),
            ToolCallStatus::Failed => write!(f, "failed"),
            ToolCallStatus::Cancelled => write!(f, "cancelled"),
            ToolCallStatus::Timeout => write!(f, "timeout"),
        }
    }
}

/// A complete tool call trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallTrace {
    /// Unique identifier for this call
    pub call_id: String,
    /// Name of the tool being called
    pub tool_name: String,
    /// Arguments passed to the tool
    pub arguments: serde_json::Value,
    /// When the call started
    pub start_time: DateTime<Utc>,
    /// When the call ended (if completed)
    pub end_time: Option<DateTime<Utc>>,
    /// Current status
    pub status: ToolCallStatus,
    /// Number of retry attempts
    pub retry_count: u8,
    /// Maximum retries allowed
    pub max_retries: u8,
    /// Output preview (first N characters)
    pub output_preview: String,
    /// Error message if failed
    pub error_message: Option<String>,
    /// Node/worker that executed this call
    pub execution_node: String,
    /// Parent call ID (for nested calls)
    pub parent_call_id: Option<String>,
    /// Child call IDs (for parallel calls)
    pub child_call_ids: Vec<String>,
    /// Execution metadata
    pub metadata: HashMap<String, String>,
}

impl ToolCallTrace {
    pub fn new(tool_name: &str, arguments: serde_json::Value, node: &str) -> Self {
        Self {
            call_id: Uuid::new_v4().to_string(),
            tool_name: tool_name.to_string(),
            arguments,
            start_time: Utc::now(),
            end_time: None,
            status: ToolCallStatus::Pending,
            retry_count: 0,
            max_retries: 3,
            output_preview: String::new(),
            error_message: None,
            execution_node: node.to_string(),
            parent_call_id: None,
            child_call_ids: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Start the tool execution
    pub fn start(&mut self) {
        self.status = ToolCallStatus::Running;
    }

    /// Complete the tool execution
    pub fn complete(&mut self, output: &str, max_preview: usize) {
        self.status = ToolCallStatus::Completed;
        self.end_time = Some(Utc::now());
        self.output_preview = output.chars().take(max_preview).collect();
    }

    /// Fail the tool execution
    pub fn fail(&mut self, error: &str) {
        self.status = ToolCallStatus::Failed;
        self.end_time = Some(Utc::now());
        self.error_message = Some(error.to_string());
    }

    /// Timeout the tool execution
    pub fn timeout(&mut self) {
        self.status = ToolCallStatus::Timeout;
        self.end_time = Some(Utc::now());
        self.error_message = Some("Execution timed out".to_string());
    }

    /// Cancel the tool execution
    pub fn cancel(&mut self) {
        self.status = ToolCallStatus::Cancelled;
        self.end_time = Some(Utc::now());
    }

    /// Record a retry attempt
    pub fn retry(&mut self) {
        self.retry_count += 1;
        self.status = ToolCallStatus::Pending;
    }

    /// Get duration of the call
    pub fn duration(&self) -> Option<Duration> {
        self.end_time.map(|end| end.signed_duration_since(self.start_time).to_std().unwrap_or_default())
    }

    /// Set parent call
    pub fn set_parent(&mut self, parent_id: &str) {
        self.parent_call_id = Some(parent_id.to_string());
    }

    /// Add child call
    pub fn add_child(&mut self, child_id: &str) {
        self.child_call_ids.push(child_id.to_string());
    }
}

/// Tool call statistics
#[derive(Debug, Clone, Default)]
pub struct ToolCallStats {
    pub total_calls: u64,
    pub successful_calls: u64,
    pub failed_calls: u64,
    pub timeout_calls: u64,
    pub cancelled_calls: u64,
    pub total_duration_ms: u64,
    pub avg_duration_ms: f64,
}

impl ToolCallStats {
    pub fn record_call(&mut self, trace: &ToolCallTrace) {
        self.total_calls += 1;

        if let Some(duration) = trace.duration() {
            self.total_duration_ms += duration.as_millis() as u64;
        }

        match trace.status {
            ToolCallStatus::Completed => {
                self.successful_calls += 1;
            }
            ToolCallStatus::Failed => {
                self.failed_calls += 1;
            }
            ToolCallStatus::Timeout => {
                self.timeout_calls += 1;
            }
            ToolCallStatus::Cancelled => {
                self.cancelled_calls += 1;
            }
            _ => {}
        }

        if self.total_calls > 0 {
            self.avg_duration_ms = self.total_duration_ms as f64 / self.total_calls as f64;
        }
    }
}

// ============================================================================
// Tool Execution Tracer
// ============================================================================

/// Complete tool execution tracer
pub struct ToolExecutionTracer {
    /// All traces indexed by call ID
    traces: Arc<DashMap<String, ToolCallTrace>>,
    /// Statistics by tool name
    stats_by_tool: Arc<DashMap<String, ToolCallStats>>,
    /// Global statistics
    global_stats: Arc<RwLock<ToolCallStats>>,
    /// Maximum output preview length
    max_preview_len: usize,
    /// Maximum traces to keep in memory
    max_traces: usize,
}

impl ToolExecutionTracer {
    pub fn new(max_preview_len: usize, max_traces: usize) -> Self {
        Self {
            traces: Arc::new(DashMap::new()),
            stats_by_tool: Arc::new(DashMap::new()),
            global_stats: Arc::new(RwLock::new(ToolCallStats::default())),
            max_preview_len,
            max_traces,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(200, 10000)
    }

    /// Start a new tool call trace
    pub fn start_call(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
        node: &str,
    ) -> String {
        let trace = ToolCallTrace::new(tool_name, arguments, node);
        let call_id = trace.call_id.clone();
        self.traces.insert(call_id.clone(), trace);
        call_id
    }

    /// Get a trace by ID
    pub fn get_trace(&self, call_id: &str) -> Option<ToolCallTrace> {
        self.traces.get(call_id).map(|r| r.clone())
    }

    /// Update trace status
    pub fn update_trace<F>(&self, call_id: &str, updater: F) -> bool
    where
        F: FnOnce(&mut ToolCallTrace),
    {
        if let Some(mut trace) = self.traces.get_mut(call_id) {
            updater(&mut trace);
            true
        } else {
            false
        }
    }

    /// Record call completion
    pub async fn complete_call(&self, call_id: &str, output: &str) {
        if let Some(mut trace) = self.traces.get_mut(call_id) {
            trace.complete(output, self.max_preview_len);

            // Update stats
            self.record_in_stats(&trace).await;
        }
    }

    /// Record call failure
    pub async fn fail_call(&self, call_id: &str, error: &str) {
        if let Some(mut trace) = self.traces.get_mut(call_id) {
            trace.fail(error);

            // Update stats
            self.record_in_stats(&trace).await;
        }
    }

    /// Record call timeout
    pub async fn timeout_call(&self, call_id: &str) {
        if let Some(mut trace) = self.traces.get_mut(call_id) {
            trace.timeout();

            // Update stats
            self.record_in_stats(&trace).await;
        }
    }

    /// Record retry
    pub fn retry_call(&self, call_id: &str) -> bool {
        if let Some(mut trace) = self.traces.get_mut(call_id) {
            trace.retry();
            true
        } else {
            false
        }
    }

    /// Get all traces for a specific tool
    pub fn get_traces_for_tool(&self, tool_name: &str) -> Vec<ToolCallTrace> {
        self.traces
            .iter()
            .filter(|r| r.tool_name == tool_name)
            .map(|r| r.clone())
            .collect()
    }

    /// Get traces within a time range
    pub fn get_traces_in_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<ToolCallTrace> {
        self.traces
            .iter()
            .filter(|r| r.start_time >= start && r.start_time <= end)
            .map(|r| r.clone())
            .collect()
    }

    /// Get statistics for a specific tool
    pub fn get_stats_for_tool(&self, tool_name: &str) -> Option<ToolCallStats> {
        self.stats_by_tool.get(tool_name).map(|r| r.clone())
    }

    /// Get global statistics
    pub async fn get_global_stats(&self) -> ToolCallStats {
        self.global_stats.read().await.clone()
    }

    /// Get all tool names that have been called
    pub fn get_called_tools(&self) -> Vec<String> {
        self.stats_by_tool.iter().map(|r| r.key().clone()).collect()
    }

    /// Get trace tree for visualization
    pub fn get_trace_tree(&self, root_call_id: &str) -> Option<TraceTree> {
        self.build_trace_tree(root_call_id)
    }

    fn build_trace_tree(&self, root_id: &str) -> Option<TraceTree> {
        let root = self.traces.get(root_id)?;

        let children: Vec<TraceTree> = root
            .child_call_ids
            .iter()
            .filter_map(|child_id| self.build_trace_tree(child_id))
            .collect();

        Some(TraceTree {
            trace: root.clone(),
            children,
        })
    }

    async fn record_in_stats(&self, trace: &ToolCallTrace) {
        // Update tool-specific stats
        let mut tool_stats = self
            .stats_by_tool
            .entry(trace.tool_name.clone())
            .or_insert_with(ToolCallStats::default);
        tool_stats.record_call(trace);

        // Update global stats
        let mut global = self.global_stats.write().await;
        global.record_call(trace);
    }

    /// Cleanup old traces to save memory
    pub fn cleanup_old_traces(&self, max_age: Duration) {
        let cutoff = Utc::now() - chrono::Duration::from_std(max_age).unwrap_or_default();
        self.traces.retain(|_, trace| trace.start_time > cutoff);
    }
}

/// Tree structure for trace visualization
#[derive(Debug, Clone)]
pub struct TraceTree {
    pub trace: ToolCallTrace,
    pub children: Vec<TraceTree>,
}

/// Summary of tool execution for reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionSummary {
    pub total_calls: u64,
    pub successful_calls: u64,
    pub failed_calls: u64,
    pub timeout_calls: u64,
    pub success_rate: f64,
    pub avg_duration_ms: f64,
    pub tool_stats: HashMap<String, ToolStats>,
    pub slowest_tools: Vec<(String, f64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStats {
    pub calls: u64,
    pub success_rate: f64,
    pub avg_duration_ms: f64,
}

impl ToolExecutionTracer {
    /// Generate a summary report
    pub async fn generate_summary(&self) -> ToolExecutionSummary {
        let global = self.get_global_stats().await;

        let success_rate = if global.total_calls > 0 {
            global.successful_calls as f64 / global.total_calls as f64
        } else {
            0.0
        };

        let mut tool_stats = HashMap::new();
        let mut slowest_tools = Vec::new();

        for entry in self.stats_by_tool.iter() {
            let tool_name = entry.key().clone();
            let stats = entry.value();
            let avg = stats.avg_duration_ms;

            tool_stats.insert(
                tool_name.clone(),
                ToolStats {
                    calls: stats.total_calls,
                    success_rate: if stats.total_calls > 0 {
                        stats.successful_calls as f64 / stats.total_calls as f64
                    } else {
                        0.0
                    },
                    avg_duration_ms: avg,
                },
            );

            slowest_tools.push((tool_name, avg));
        }

        slowest_tools.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        slowest_tools.truncate(10);

        ToolExecutionSummary {
            total_calls: global.total_calls,
            successful_calls: global.successful_calls,
            failed_calls: global.failed_calls,
            timeout_calls: global.timeout_calls,
            success_rate,
            avg_duration_ms: global.avg_duration_ms,
            tool_stats,
            slowest_tools,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tracer_basic() {
        let tracer = ToolExecutionTracer::with_defaults();

        // Start a call
        let call_id = tracer.start_call(
            "test_tool",
            serde_json::json!({"arg": "value"}),
            "node1",
        );

        // Update to running
        tracer.update_trace(&call_id, |t| t.start());

        // Complete the call
        tracer.complete_call(&call_id, "result").await;

        // Get and verify trace
        let trace = tracer.get_trace(&call_id);
        assert!(trace.is_some());
        let trace = trace.unwrap();
        assert_eq!(trace.status, ToolCallStatus::Completed);
        assert_eq!(trace.tool_name, "test_tool");
        assert_eq!(trace.output_preview, "result");
    }

    #[tokio::test]
    async fn test_tracer_stats() {
        let tracer = ToolExecutionTracer::with_defaults();

        // Make some calls
        let id1 = tracer.start_call("tool1", serde_json::json!({}), "node1");
        tracer.complete_call(&id1, "result1").await;

        let id2 = tracer.start_call("tool2", serde_json::json!({}), "node1");
        tracer.complete_call(&id2, "result2").await;

        let id3 = tracer.start_call("tool1", serde_json::json!({}), "node1");
        tracer.fail_call(&id3, "error").await;

        // Check tool stats
        let stats1 = tracer.get_stats_for_tool("tool1");
        assert!(stats1.is_some());
        let stats1 = stats1.unwrap();
        assert_eq!(stats1.total_calls, 2);
        assert_eq!(stats1.successful_calls, 1);
        assert_eq!(stats1.failed_calls, 1);

        // Check global stats
        let global = tracer.get_global_stats().await;
        assert_eq!(global.total_calls, 3);
        assert_eq!(global.successful_calls, 2);
        assert_eq!(global.failed_calls, 1);
    }

    #[tokio::test]
    async fn test_trace_tree() {
        let tracer = ToolExecutionTracer::with_defaults();

        // Create parent call
        let parent_id = tracer.start_call("parent", serde_json::json!({}), "node1");

        // Create child call
        let child_id = tracer.start_call("child", serde_json::json!({}), "node1");

        // Link them
        tracer.update_trace(&parent_id, |t| {
            t.add_child(&child_id);
        });
        tracer.update_trace(&child_id, |t| {
            t.set_parent(&parent_id);
        });

        // Complete both
        tracer.complete_call(&parent_id, "parent_result").await;
        tracer.complete_call(&child_id, "child_result").await;

        // Build tree
        let tree = tracer.get_trace_tree(&parent_id);
        assert!(tree.is_some());
        let tree = tree.unwrap();
        assert_eq!(tree.trace.tool_name, "parent");
        assert_eq!(tree.children.len(), 1);
        assert_eq!(tree.children[0].trace.tool_name, "child");
    }

    #[tokio::test]
    async fn test_summary() {
        let tracer = ToolExecutionTracer::with_defaults();

        // Make some calls
        for i in 0..5 {
            let id = tracer.start_call("tool1", serde_json::json!({}), "node1");
            tracer.complete_call(&id, &format!("result{}", i)).await;
        }

        let summary = tracer.generate_summary().await;
        assert_eq!(summary.total_calls, 5);
        assert_eq!(summary.successful_calls, 5);
        assert_eq!(summary.success_rate, 1.0);
    }
}