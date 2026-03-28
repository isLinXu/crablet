//! Real-time Metrics System for Agentic Harness
//!
//! Provides comprehensive metrics collection and export capabilities:
//! - Counter, Gauge, Histogram metrics
//! - Prometheus exporter
//! - OpenTelemetry exporter
//! - Real-time dashboard data

use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

// ============================================================================
// Metric Types
// ============================================================================

/// Counter metric - only increases
#[derive(Debug, Clone)]
pub struct Counter {
    value: Arc<AtomicU64>,
    name: String,
    description: String,
}

impl Default for Counter {
    fn default() -> Self {
        Self {
            value: Arc::new(AtomicU64::new(0)),
            name: "unnamed".to_string(),
            description: "".to_string(),
        }
    }
}

impl Counter {
    pub fn new(name: String, description: String) -> Self {
        Self {
            value: Arc::new(AtomicU64::new(0)),
            name,
            description,
        }
    }

    pub fn inc(&self, amount: u64) {
        self.value.fetch_add(amount, Ordering::SeqCst);
    }

    pub fn get(&self) -> u64 {
        self.value.load(Ordering::SeqCst)
    }

    pub fn value(&self) -> u64 {
        self.get()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> &str {
        &self.description
    }
}

/// Gauge metric - can increase or decrease
#[derive(Debug, Clone)]
pub struct Gauge {
    value: Arc<AtomicU64>,
    name: String,
    description: String,
}

impl Default for Gauge {
    fn default() -> Self {
        Self {
            value: Arc::new(AtomicU64::new(0)),
            name: "unnamed".to_string(),
            description: "".to_string(),
        }
    }
}

impl Gauge {
    pub fn new(name: String, description: String) -> Self {
        Self {
            value: Arc::new(AtomicU64::new(0)),
            name,
            description,
        }
    }

    pub fn set(&self, value: u64) {
        self.value.store(value, Ordering::SeqCst);
    }

    pub fn inc(&self, amount: u64) {
        self.value.fetch_add(amount, Ordering::SeqCst);
    }

    pub fn dec(&self, amount: u64) {
        // Use saturating_sub to prevent underflow: if value < amount, clamp to 0
        self.value.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| {
            Some(v.saturating_sub(amount))
        }).ok();
    }

    pub fn get(&self) -> u64 {
        self.value.load(Ordering::SeqCst)
    }

    pub fn value(&self) -> u64 {
        self.get()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> &str {
        &self.description
    }
}

/// Histogram metric - collects distributions
#[derive(Debug, Clone)]
pub struct Histogram {
    counts: Arc<[AtomicU64; 11]>,  // Buckets: 0, 5, 10, 25, 50, 100, 250, 500, 1000, 2500, +Inf
    sum: Arc<AtomicU64>,
    count: Arc<AtomicU64>,
    name: String,
    description: String,
}

impl Default for Histogram {
    fn default() -> Self {
        Self {
            counts: Arc::new([
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ]),
            sum: Arc::new(AtomicU64::new(0)),
            count: Arc::new(AtomicU64::new(0)),
            name: "unnamed".to_string(),
            description: "".to_string(),
        }
    }
}

impl Histogram {
    pub fn new(name: String, description: String) -> Self {
        Self {
            counts: Arc::new([
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ]),
            sum: Arc::new(AtomicU64::new(0)),
            count: Arc::new(AtomicU64::new(0)),
            name,
            description,
        }
    }

    /// Record a duration in milliseconds
    pub fn observe(&self, value_ms: u64) {
        let bucket = Self::bucket_for(value_ms);
        self.counts[bucket].fetch_add(1, Ordering::SeqCst);
        self.sum.fetch_add(value_ms, Ordering::SeqCst);
        self.count.fetch_add(1, Ordering::SeqCst);
    }

    fn bucket_for(value_ms: u64) -> usize {
        match value_ms {
            0 => 0,
            1..=5 => 1,
            6..=10 => 2,
            11..=25 => 3,
            26..=50 => 4,
            51..=100 => 5,
            101..=250 => 6,
            251..=500 => 7,
            501..=1000 => 8,
            1001..=2500 => 9,
            _ => 10,
        }
    }

    pub fn get_count(&self) -> u64 {
        self.count.load(Ordering::SeqCst)
    }

    pub fn get_sum(&self) -> u64 {
        self.sum.load(Ordering::SeqCst)
    }

    pub fn get_avg(&self) -> f64 {
        let count = self.count.load(Ordering::SeqCst);
        if count == 0 {
            return 0.0;
        }
        self.sum.load(Ordering::SeqCst) as f64 / count as f64
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    /// Approximate percentile from histogram buckets.
    /// Uses linear interpolation between bucket boundaries for a rough but reasonable estimate.
    pub fn approximate_percentile(&self, percentile: f64) -> f64 {
        let count = self.count.load(Ordering::SeqCst);
        if count == 0 {
            return 0.0;
        }
        let target = (percentile * count as f64 / 100.0).ceil() as u64;
        let bucket_bounds = [0u64, 5, 10, 25, 50, 100, 250, 500, 1000, 2500];
        let mut cumulative: u64 = 0;
        for (i, &bound) in bucket_bounds.iter().enumerate() {
            cumulative += self.counts[i].load(Ordering::SeqCst);
            if cumulative >= target {
                let prev_cumulative = cumulative - self.counts[i].load(Ordering::SeqCst);
                let prev_bound: f64 = if i > 0 { bucket_bounds[i - 1] as f64 } else { 0.0 };
                let bucket_count = self.counts[i].load(Ordering::SeqCst);
                if bucket_count > 0 && cumulative > prev_cumulative {
                    let fraction = (target - prev_cumulative) as f64 / bucket_count as f64;
                    return prev_bound + fraction * (bound as f64 - prev_bound);
                }
                return bound as f64;
            }
        }
        2500.0
    }

    /// Get bucket values for Prometheus format
    pub fn get_buckets(&self) -> Vec<(u64, u64)> {
        let mut cumulative = 0u64;
        let bucket_bounds = [0, 5, 10, 25, 50, 100, 250, 500, 1000, 2500, u64::MAX];
        let mut result = Vec::with_capacity(11);

        for (i, bound) in bucket_bounds.iter().enumerate() {
            cumulative += self.counts[i].load(Ordering::SeqCst);
            result.push((*bound, cumulative));
        }
        result
    }
}

// ============================================================================
// Realtime Metrics Container
// ============================================================================

/// Container for all agent metrics
#[derive(Debug, Clone)]
pub struct RealtimeMetrics {
    // Counters
    pub harness_active: Counter,
    pub harness_completed: Counter,
    pub harness_failed: Counter,
    pub steps_total: Counter,
    pub tool_calls_total: Counter,
    pub tool_failures_total: Counter,
    pub messages_sent: Counter,
    pub messages_received: Counter,

    // Gauges
    pub current_step: Gauge,
    pub active_harnesses: Gauge,
    pub token_usage: Gauge,
    pub queue_depth: Gauge,

    // Histograms
    pub step_duration_ms: Histogram,
    pub tool_duration_ms: Histogram,
    pub llm_latency_ms: Histogram,
    pub queue_wait_ms: Histogram,
}

impl Default for RealtimeMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl RealtimeMetrics {
    pub fn new() -> Self {
        Self {
            harness_active: Counter::new(
                "harness_active".to_string(),
                "Number of currently active harnesses".to_string(),
            ),
            harness_completed: Counter::new(
                "harness_completed_total".to_string(),
                "Total number of completed harnesses".to_string(),
            ),
            harness_failed: Counter::new(
                "harness_failed_total".to_string(),
                "Total number of failed harnesses".to_string(),
            ),
            steps_total: Counter::new(
                "steps_total".to_string(),
                "Total number of executed steps".to_string(),
            ),
            tool_calls_total: Counter::new(
                "tool_calls_total".to_string(),
                "Total number of tool calls".to_string(),
            ),
            tool_failures_total: Counter::new(
                "tool_failures_total".to_string(),
                "Total number of tool call failures".to_string(),
            ),
            messages_sent: Counter::new(
                "messages_sent_total".to_string(),
                "Total number of messages sent".to_string(),
            ),
            messages_received: Counter::new(
                "messages_received_total".to_string(),
                "Total number of messages received".to_string(),
            ),
            current_step: Gauge::new(
                "current_step".to_string(),
                "Current step number".to_string(),
            ),
            active_harnesses: Gauge::new(
                "active_harnesses".to_string(),
                "Number of active harnesses".to_string(),
            ),
            token_usage: Gauge::new(
                "token_usage_current".to_string(),
                "Current token usage".to_string(),
            ),
            queue_depth: Gauge::new(
                "queue_depth".to_string(),
                "Current message queue depth".to_string(),
            ),
            step_duration_ms: Histogram::new(
                "step_duration_ms".to_string(),
                "Step execution duration in milliseconds".to_string(),
            ),
            tool_duration_ms: Histogram::new(
                "tool_duration_ms".to_string(),
                "Tool execution duration in milliseconds".to_string(),
            ),
            llm_latency_ms: Histogram::new(
                "llm_latency_ms".to_string(),
                "LLM response latency in milliseconds".to_string(),
            ),
            queue_wait_ms: Histogram::new(
                "queue_wait_ms".to_string(),
                "Time spent waiting in queue".to_string(),
            ),
        }
    }

    /// Get a snapshot of all metrics
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            harness_active: self.harness_active.get(),
            harness_completed: self.harness_completed.get(),
            harness_failed: self.harness_failed.get(),
            steps_total: self.steps_total.get(),
            tool_calls_total: self.tool_calls_total.get(),
            tool_failures_total: self.tool_failures_total.get(),
            messages_sent: self.messages_sent.get(),
            messages_received: self.messages_received.get(),
            current_step: self.current_step.get(),
            active_harnesses: self.active_harnesses.get(),
            token_usage: self.token_usage.get(),
            queue_depth: self.queue_depth.get(),
            avg_step_duration_ms: self.step_duration_ms.get_avg(),
            avg_tool_duration_ms: self.tool_duration_ms.get_avg(),
            avg_llm_latency_ms: self.llm_latency_ms.get_avg(),
            avg_queue_wait_ms: self.queue_wait_ms.get_avg(),
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub harness_active: u64,
    pub harness_completed: u64,
    pub harness_failed: u64,
    pub steps_total: u64,
    pub tool_calls_total: u64,
    pub tool_failures_total: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub current_step: u64,
    pub active_harnesses: u64,
    pub token_usage: u64,
    pub queue_depth: u64,
    pub avg_step_duration_ms: f64,
    pub avg_tool_duration_ms: f64,
    pub avg_llm_latency_ms: f64,
    pub avg_queue_wait_ms: f64,
    pub timestamp: DateTime<Utc>,
}

// ============================================================================
// Metrics Exporters
// ============================================================================

/// Trait for metrics exporters
pub trait MetricsExporter: Send + Sync {
    fn export(&self, metrics: &RealtimeMetrics) -> Result<String, ExportError>;
    fn name(&self) -> &str;
}

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Export failed: {0}")]
    ExportFailed(String),
}

/// Prometheus format exporter
pub struct PrometheusExporter {
    prefix: String,
}

impl PrometheusExporter {
    pub fn new(prefix: String) -> Self {
        Self { prefix }
    }
}

impl MetricsExporter for PrometheusExporter {
    fn export(&self, metrics: &RealtimeMetrics) -> Result<String, ExportError> {
        let mut output = String::new();

        // Header
        output.push_str(&format!("# {} metrics\n\n", self.prefix));

        // Counters (with _total suffix for Prometheus convention)
        output.push_str(&format!(
            "# HELP {}_harness_completed_total Total completed harnesses\n",
            self.prefix
        ));
        output.push_str(&format!(
            "# TYPE {}_harness_completed_total counter\n",
            self.prefix
        ));
        output.push_str(&format!(
            "{}_harness_completed_total {}\n",
            self.prefix,
            metrics.harness_completed.get()
        ));

        output.push_str(&format!(
            "# HELP {}_harness_failed_total Total failed harnesses\n",
            self.prefix
        ));
        output.push_str(&format!(
            "# TYPE {}_harness_failed_total counter\n",
            self.prefix
        ));
        output.push_str(&format!(
            "{}_harness_failed_total {}\n",
            self.prefix,
            metrics.harness_failed.get()
        ));

        output.push_str(&format!(
            "# HELP {}_steps_total Total executed steps\n",
            self.prefix
        ));
        output.push_str(&format!(
            "# TYPE {}_steps_total counter\n",
            self.prefix
        ));
        output.push_str(&format!(
            "{}_steps_total {}\n",
            self.prefix,
            metrics.steps_total.get()
        ));

        output.push_str(&format!(
            "# HELP {}_tool_calls_total Total tool calls\n",
            self.prefix
        ));
        output.push_str(&format!(
            "# TYPE {}_tool_calls_total counter\n",
            self.prefix
        ));
        output.push_str(&format!(
            "{}_tool_calls_total {}\n",
            self.prefix,
            metrics.tool_calls_total.get()
        ));

        output.push_str(&format!(
            "# HELP {}_tool_failures_total Total tool call failures\n",
            self.prefix
        ));
        output.push_str(&format!(
            "# TYPE {}_tool_failures_total counter\n",
            self.prefix
        ));
        output.push_str(&format!(
            "{}_tool_failures_total {}\n",
            self.prefix,
            metrics.tool_failures_total.get()
        ));

        // Gauges
        output.push_str(&format!(
            "# HELP {}_active_harnesses Number of active harnesses\n",
            self.prefix
        ));
        output.push_str(&format!(
            "# TYPE {}_active_harnesses gauge\n",
            self.prefix
        ));
        output.push_str(&format!(
            "{}_active_harnesses {}\n",
            self.prefix,
            metrics.active_harnesses.get()
        ));

        output.push_str(&format!(
            "# HELP {}_current_step Current step number\n",
            self.prefix
        ));
        output.push_str(&format!(
            "# TYPE {}_current_step gauge\n",
            self.prefix
        ));
        output.push_str(&format!(
            "{}_current_step {}\n",
            self.prefix,
            metrics.current_step.get()
        ));

        output.push_str(&format!(
            "# HELP {}_token_usage_current Current token usage\n",
            self.prefix
        ));
        output.push_str(&format!(
            "# TYPE {}_token_usage_current gauge\n",
            self.prefix
        ));
        output.push_str(&format!(
            "{}_token_usage_current {}\n",
            self.prefix,
            metrics.token_usage.get()
        ));

        output.push_str(&format!(
            "# HELP {}_queue_depth Current queue depth\n",
            self.prefix
        ));
        output.push_str(&format!(
            "# TYPE {}_queue_depth gauge\n",
            self.prefix
        ));
        output.push_str(&format!(
            "{}_queue_depth {}\n",
            self.prefix,
            metrics.queue_depth.get()
        ));

        // Histograms
        let step_buckets = metrics.step_duration_ms.get_buckets();
        output.push_str(&format!(
            "# HELP {}_step_duration_ms Step execution duration\n",
            self.prefix
        ));
        output.push_str(&format!(
            "# TYPE {}_step_duration_ms histogram\n",
            self.prefix
        ));
        let bucket_labels = ["0", "5", "10", "25", "50", "100", "250", "500", "1000", "2500", "+Inf"];
        for (i, (_bound, cumulative)) in step_buckets.iter().enumerate() {
            output.push_str(&format!(
                "{}_step_duration_ms_bucket{{le=\"{}\"}} {}\n",
                self.prefix, bucket_labels[i], cumulative
            ));
        }
        output.push_str(&format!(
            "{}_step_duration_ms_sum {}\n",
            self.prefix,
            metrics.step_duration_ms.get_sum()
        ));
        output.push_str(&format!(
            "{}_step_duration_ms_count {}\n",
            self.prefix,
            metrics.step_duration_ms.get_count()
        ));

        let tool_buckets = metrics.tool_duration_ms.get_buckets();
        output.push_str(&format!(
            "# HELP {}_tool_duration_ms Tool execution duration\n",
            self.prefix
        ));
        output.push_str(&format!(
            "# TYPE {}_tool_duration_ms histogram\n",
            self.prefix
        ));
        for (i, (_bound, cumulative)) in tool_buckets.iter().enumerate() {
            output.push_str(&format!(
                "{}_tool_duration_ms_bucket{{le=\"{}\"}} {}\n",
                self.prefix, bucket_labels[i], cumulative
            ));
        }
        output.push_str(&format!(
            "{}_tool_duration_ms_sum {}\n",
            self.prefix,
            metrics.tool_duration_ms.get_sum()
        ));
        output.push_str(&format!(
            "{}_tool_duration_ms_count {}\n",
            self.prefix,
            metrics.tool_duration_ms.get_count()
        ));

        Ok(output)
    }

    fn name(&self) -> &str {
        "prometheus"
    }
}

/// JSON format exporter for dashboards
pub struct JsonExporter {
    pretty: bool,
}

impl JsonExporter {
    pub fn new(pretty: bool) -> Self {
        Self { pretty }
    }
}

impl MetricsExporter for JsonExporter {
    fn export(&self, metrics: &RealtimeMetrics) -> Result<String, ExportError> {
        let snapshot = metrics.snapshot();
        let json = serde_json::to_string(&snapshot)
            .map_err(|e| ExportError::Serialization(e.to_string()))?;

        if self.pretty {
            serde_json::to_string_pretty(&snapshot)
                .map_err(|e| ExportError::Serialization(e.to_string()))
        } else {
            Ok(json)
        }
    }

    fn name(&self) -> &str {
        "json"
    }
}

/// OpenTelemetry format exporter
pub struct OpenTelemetryExporter {
    service_name: String,
}

impl OpenTelemetryExporter {
    pub fn new(service_name: String) -> Self {
        Self { service_name }
    }
}

impl MetricsExporter for OpenTelemetryExporter {
    fn export(&self, metrics: &RealtimeMetrics) -> Result<String, ExportError> {
        let snapshot = metrics.snapshot();

        // Simplified OTLP metrics format
        let otel_metrics = OtelMetricsOutput {
            resource_metrics: vec![OtelResourceMetrics {
                resource: OtelResource {
                    attributes: vec![
                        OtelAttribute { key: "service.name".to_string(), value: self.service_name.clone() },
                    ],
                },
                scope_metrics: vec![OtelScopeMetrics {
                    scope: OtelScope { name: "crablet".to_string(), version: "1.0.0".to_string() },
                    metrics: vec![
                        OtelMetric {
                            name: "harness.active".to_string(),
                            description: "Number of active harnesses".to_string(),
                            gauge: Some(OtelGauge {
                                data_points: vec![OtelDataPoint {
                                    as_int: snapshot.harness_active as i64,
                                    as_double: 0.0,
                                    time_unix_nano: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or(std::time::Duration::ZERO)
                                        .as_nanos() as u64,
                                }],
                            }),
                            ..Default::default()
                        },
                        OtelMetric {
                            name: "harness.completed".to_string(),
                            description: "Total completed harnesses".to_string(),
                            counter: Some(OtelCounter {
                                data_points: vec![OtelDataPoint {
                                    as_int: snapshot.harness_completed as i64,
                                    as_double: 0.0,
                                    time_unix_nano: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or(std::time::Duration::ZERO)
                                        .as_nanos() as u64,
                                }],
                            }),
                            ..Default::default()
                        },
                        OtelMetric {
                            name: "steps.total".to_string(),
                            description: "Total executed steps".to_string(),
                            counter: Some(OtelCounter {
                                data_points: vec![OtelDataPoint {
                                    as_int: snapshot.steps_total as i64,
                                    as_double: 0.0,
                                    time_unix_nano: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or(std::time::Duration::ZERO)
                                        .as_nanos() as u64,
                                }],
                            }),
                            ..Default::default()
                        },
                        OtelMetric {
                            name: "tool.calls.total".to_string(),
                            description: "Total tool calls".to_string(),
                            counter: Some(OtelCounter {
                                data_points: vec![OtelDataPoint {
                                    as_int: snapshot.tool_calls_total as i64,
                                    as_double: 0.0,
                                    time_unix_nano: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or(std::time::Duration::ZERO)
                                        .as_nanos() as u64,
                                }],
                            }),
                            ..Default::default()
                        },
                        OtelMetric {
                            name: "tool.duration.avg".to_string(),
                            description: "Average tool duration".to_string(),
                            gauge: Some(OtelGauge {
                                data_points: vec![OtelDataPoint {
                                    as_int: 0,
                                    as_double: snapshot.avg_tool_duration_ms,
                                    time_unix_nano: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or(std::time::Duration::ZERO)
                                        .as_nanos() as u64,
                                }],
                            }),
                            ..Default::default()
                        },
                    ],
                }],
            }],
        };

        serde_json::to_string(&otel_metrics)
            .map_err(|e| ExportError::Serialization(e.to_string()))
    }

    fn name(&self) -> &str {
        "opentelemetry"
    }
}

// OpenTelemetry structures
#[derive(Serialize)]
struct OtelMetricsOutput {
    resource_metrics: Vec<OtelResourceMetrics>,
}

#[derive(Serialize)]
struct OtelResourceMetrics {
    resource: OtelResource,
    scope_metrics: Vec<OtelScopeMetrics>,
}

#[derive(Serialize)]
struct OtelResource {
    attributes: Vec<OtelAttribute>,
}

#[derive(Serialize)]
struct OtelAttribute {
    key: String,
    value: String,
}

#[derive(Serialize)]
struct OtelScopeMetrics {
    scope: OtelScope,
    metrics: Vec<OtelMetric>,
}

#[derive(Serialize)]
struct OtelScope {
    name: String,
    version: String,
}

#[derive(Serialize, Default)]
struct OtelMetric {
    name: String,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    gauge: Option<OtelGauge>,
    #[serde(skip_serializing_if = "Option::is_none")]
    counter: Option<OtelCounter>,
}

#[derive(Serialize)]
struct OtelGauge {
    data_points: Vec<OtelDataPoint>,
}

#[derive(Serialize)]
struct OtelCounter {
    data_points: Vec<OtelDataPoint>,
}

#[derive(Serialize)]
struct OtelDataPoint {
    #[serde(rename = "as_int", skip_serializing_if = "is_zero")]
    #[serde(default)]
    as_int: i64,
    #[serde(rename = "as_double", skip_serializing_if = "is_zero_float")]
    #[serde(default)]
    as_double: f64,
    time_unix_nano: u64,
}

fn is_zero(v: &i64) -> bool {
    *v == 0
}

fn is_zero_float(v: &f64) -> bool {
    *v == 0.0
}

// ============================================================================
// Metrics Manager
// ============================================================================

/// Central metrics manager
pub struct MetricsManager {
    metrics: Arc<RealtimeMetrics>,
    exporters: Arc<RwLock<Vec<Box<dyn MetricsExporter>>>>,
    enabled: Arc<AtomicBool>,
}

impl Default for MetricsManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsManager {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RealtimeMetrics::new()),
            exporters: Arc::new(RwLock::new(Vec::new())),
            enabled: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Get reference to metrics
    pub fn metrics(&self) -> &RealtimeMetrics {
        &self.metrics
    }

    /// Add an exporter
    pub async fn add_exporter(&self, exporter: Box<dyn MetricsExporter>) {
        let mut exporters = self.exporters.write().await;
        exporters.push(exporter);
    }

    /// Enable/disable metrics collection
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    /// Export metrics using all exporters
    pub async fn export(&self) -> Result<Vec<(String, String)>, ExportError> {
        if !self.enabled.load(Ordering::SeqCst) {
            return Ok(Vec::new());
        }

        let exporters = self.exporters.read().await;
        let mut results = Vec::with_capacity(exporters.len());

        for exporter in exporters.iter() {
            let output = exporter.export(&self.metrics)?;
            results.push((exporter.name().to_string(), output));
        }

        Ok(results)
    }

    /// Export to a specific format
    pub async fn export_to(&self, format: &str) -> Result<String, ExportError> {
        let exporters = self.exporters.read().await;

        for exporter in exporters.iter() {
            if exporter.name() == format {
                return exporter.export(&self.metrics);
            }
        }

        Err(ExportError::ExportFailed(format!("Unknown format: {}", format)))
    }
}

// ============================================================================
// Dashboard Data
// ============================================================================

/// Simplified dashboard data for UI display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub summary: DashboardSummary,
    pub rates: DashboardRates,
    pub histograms: DashboardHistograms,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSummary {
    pub total_harnesses: u64,
    pub active_harnesses: u64,
    pub completed_harnesses: u64,
    pub failed_harnesses: u64,
    pub total_steps: u64,
    pub total_tool_calls: u64,
    pub tool_failure_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardRates {
    pub steps_per_minute: f64,
    pub tool_calls_per_minute: f64,
    pub harness_completion_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardHistograms {
    pub step_duration_p50: f64,
    pub step_duration_p95: f64,
    pub step_duration_p99: f64,
    pub tool_duration_p50: f64,
    pub tool_duration_p95: f64,
    pub tool_duration_p99: f64,
}

impl RealtimeMetrics {
    /// Generate dashboard data
    pub fn to_dashboard(&self) -> DashboardData {
        let completed = self.harness_completed.get();
        let failed = self.harness_failed.get();
        let total = completed + failed;
        let tool_calls = self.tool_calls_total.get();
        let tool_failures = self.tool_failures_total.get();

        DashboardData {
            summary: DashboardSummary {
                total_harnesses: total,
                active_harnesses: self.harness_active.get(),
                completed_harnesses: completed,
                failed_harnesses: failed,
                total_steps: self.steps_total.get(),
                total_tool_calls: tool_calls,
                tool_failure_rate: if tool_calls > 0 {
                    tool_failures as f64 / tool_calls as f64
                } else {
                    0.0
                },
            },
            rates: DashboardRates {
                steps_per_minute: 0.0, // Would need time tracking
                tool_calls_per_minute: 0.0,
                harness_completion_rate: if total > 0 {
                    completed as f64 / total as f64
                } else {
                    0.0
                },
            },
            histograms: DashboardHistograms {
                step_duration_p50: self.step_duration_ms.approximate_percentile(50.0),
                step_duration_p95: self.step_duration_ms.approximate_percentile(95.0),
                step_duration_p99: self.step_duration_ms.approximate_percentile(99.0),
                tool_duration_p50: self.tool_duration_ms.approximate_percentile(50.0),
                tool_duration_p95: self.tool_duration_ms.approximate_percentile(95.0),
                tool_duration_p99: self.tool_duration_ms.approximate_percentile(99.0),
            },
        }
    }
}

// ============================================================================
// Global Metrics Instance
// ============================================================================

use std::sync::OnceLock;

static GLOBAL_METRICS: OnceLock<Arc<MetricsManager>> = OnceLock::new();

/// Get the global metrics manager
pub fn global_metrics() -> Arc<MetricsManager> {
    GLOBAL_METRICS
        .get_or_init(|| Arc::new(MetricsManager::new()))
        .clone()
}

/// Initialize global metrics with exporters
pub fn init_global_metrics() -> Arc<MetricsManager> {
    let manager = global_metrics();
    // Add default Prometheus exporter
    let rt = tokio::runtime::Handle::current();
    rt.block_on(async {
        manager.add_exporter(Box::new(PrometheusExporter::new("crablet".to_string()))).await;
    });
    manager
}
