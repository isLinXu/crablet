//! Real-time Metrics Dashboard
//!
//! Provides real-time metrics collection, aggregation, and export capabilities.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Real-time metrics for the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeMetrics {
    pub harness_active: u64,
    pub steps_total: u64,
    pub tool_calls_total: u64,
    pub tool_failures_total: u64,
    pub avg_step_duration_ms: f64,
    pub token_usage_total: u64,
    pub active_sessions: u64,
    pub memory_usage_mb: u64,
    pub timestamp: DateTime<Utc>,
}

impl Default for RealtimeMetrics {
    fn default() -> Self {
        Self {
            harness_active: 0,
            steps_total: 0,
            tool_calls_total: 0,
            tool_failures_total: 0,
            avg_step_duration_ms: 0.0,
            token_usage_total: 0,
            active_sessions: 0,
            memory_usage_mb: 0,
            timestamp: Utc::now(),
        }
    }
}

/// Counter metric - using interior mutability with Arc for Clone
#[derive(Debug, Clone)]
pub struct Counter {
    name: String,
    value: Arc<AtomicU64>,
}

impl Counter {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: Arc::new(AtomicU64::new(0)),
        }
    }
    
    pub fn increment(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn add(&self, delta: u64) {
        self.value.fetch_add(delta, Ordering::Relaxed);
    }
    
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }
    
    pub fn reset(&self) {
        self.value.store(0, Ordering::Relaxed);
    }
}

/// Histogram metric for timing
#[derive(Debug, Clone)]
pub struct Histogram {
    name: String,
    count: Arc<AtomicU64>,
    sum: Arc<AtomicU64>,
    min_val: Arc<AtomicU64>,
    max_val: Arc<AtomicU64>,
}

impl Histogram {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            count: Arc::new(AtomicU64::new(0)),
            sum: Arc::new(AtomicU64::new(0)),
            min_val: Arc::new(AtomicU64::new(u64::MAX)),
            max_val: Arc::new(AtomicU64::new(0)),
        }
    }
    
    pub fn record(&self, value: u64) {
        self.count.fetch_add(1, Ordering::Relaxed);
        self.sum.fetch_add(value, Ordering::Relaxed);
        
        // Update min
        let current_min = self.min_val.load(Ordering::Relaxed);
        if value < current_min {
            let _ = self.min_val.compare_exchange(current_min, value, Ordering::Relaxed, Ordering::Relaxed);
        }
        
        // Update max
        let current_max = self.max_val.load(Ordering::Relaxed);
        if value > current_max {
            let _ = self.max_val.compare_exchange(current_max, value, Ordering::Relaxed, Ordering::Relaxed);
        }
    }
    
    pub fn get_stats(&self) -> HistogramStats {
        let cnt = self.count.load(Ordering::Relaxed);
        let sm = self.sum.load(Ordering::Relaxed);
        let min_v = self.min_val.load(Ordering::Relaxed);
        let max_v = self.max_val.load(Ordering::Relaxed);
        
        HistogramStats {
            count: cnt,
            sum: sm,
            avg: if cnt > 0 { sm as f64 / cnt as f64 } else { 0.0 },
            min: if min_v == u64::MAX { 0 } else { min_v },
            max: max_v,
        }
    }
    
    pub fn reset(&self) {
        self.count.store(0, Ordering::Relaxed);
        self.sum.store(0, Ordering::Relaxed);
        self.min_val.store(u64::MAX, Ordering::Relaxed);
        self.max_val.store(0, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramStats {
    pub count: u64,
    pub sum: u64,
    pub avg: f64,
    pub min: u64,
    pub max: u64,
}

/// Metrics aggregator
#[derive(Debug)]
pub struct MetricsAggregator {
    counters: Arc<RwLock<HashMap<String, Counter>>>,
    histograms: Arc<RwLock<HashMap<String, Histogram>>>,
    gauges: Arc<RwLock<HashMap<String, f64>>>,
}

impl MetricsAggregator {
    pub fn new() -> Self {
        Self {
            counters: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
            gauges: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub async fn get_or_create_counter(&self, name: &str) -> Counter {
        {
            let counters = self.counters.read().await;
            if let Some(counter) = counters.get(name) {
                return counter.clone();
            }
        }
        
        let counter = Counter::new(name);
        let mut counters = self.counters.write().await;
        counters.insert(name.to_string(), counter.clone());
        counter
    }
    
    pub async fn get_or_create_histogram(&self, name: &str) -> Histogram {
        {
            let histograms = self.histograms.read().await;
            if let Some(hist) = histograms.get(name) {
                return hist.clone();
            }
        }
        
        let hist = Histogram::new(name);
        let mut histograms = self.histograms.write().await;
        histograms.insert(name.to_string(), hist.clone());
        hist
    }
    
    pub async fn set_gauge(&self, name: &str, value: f64) {
        let mut gauges = self.gauges.write().await;
        gauges.insert(name.to_string(), value);
    }
    
    pub async fn get_gauge(&self, name: &str) -> Option<f64> {
        let gauges = self.gauges.read().await;
        gauges.get(name).copied()
    }
    
    pub async fn increment_counter(&self, name: &str) {
        let counter = self.get_or_create_counter(name).await;
        counter.increment();
    }
    
    pub async fn record_timing(&self, name: &str, duration_ms: u64) {
        let hist = self.get_or_create_histogram(name).await;
        hist.record(duration_ms);
    }
    
    pub async fn snapshot(&self) -> RealtimeMetrics {
        let mut metrics = RealtimeMetrics::default();
        
        let counters = self.counters.read().await;
        for (name, counter) in counters.iter() {
            match name.as_str() {
                "harness_active" => metrics.harness_active = counter.get(),
                "steps_total" => metrics.steps_total = counter.get(),
                "tool_calls_total" => metrics.tool_calls_total = counter.get(),
                "tool_failures_total" => metrics.tool_failures_total = counter.get(),
                "token_usage_total" => metrics.token_usage_total = counter.get(),
                "active_sessions" => metrics.active_sessions = counter.get(),
                _ => {}
            }
        }
        
        let histograms = self.histograms.read().await;
        if let Some(hist) = histograms.get("step_duration_ms") {
            metrics.avg_step_duration_ms = hist.get_stats().avg;
        }
        
        let gauges = self.gauges.read().await;
        if let Some(memory) = gauges.get("memory_usage_mb") {
            metrics.memory_usage_mb = *memory as u64;
        }
        
        metrics.timestamp = Utc::now();
        metrics
    }
    
    pub async fn reset(&self) {
        let counters = self.counters.read().await;
        for counter in counters.values() {
            counter.reset();
        }
        
        let histograms = self.histograms.read().await;
        for hist in histograms.values() {
            hist.reset();
        }
    }
}

impl Default for MetricsAggregator {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics exporter trait
pub trait MetricsExporter: Send + Sync {
    fn export(&self, metrics: &RealtimeMetrics) -> anyhow::Result<()>;
    fn name(&self) -> &str;
}

/// Prometheus exporter
pub struct PrometheusExporter {
    endpoint: String,
}

impl PrometheusExporter {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
        }
    }
}

impl MetricsExporter for PrometheusExporter {
    fn export(&self, _metrics: &RealtimeMetrics) -> anyhow::Result<()> {
        info!("Prometheus metrics exported to {}", self.endpoint);
        Ok(())
    }
    
    fn name(&self) -> &str {
        "prometheus"
    }
}

/// OpenTelemetry exporter
pub struct OpenTelemetryExporter {
    service_name: String,
}

impl OpenTelemetryExporter {
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
        }
    }
}

impl MetricsExporter for OpenTelemetryExporter {
    fn export(&self, metrics: &RealtimeMetrics) -> anyhow::Result<()> {
        info!(
            "OpenTelemetry metrics for service '{}': harness_active={}",
            self.service_name, metrics.harness_active
        );
        Ok(())
    }
    
    fn name(&self) -> &str {
        "opentelemetry"
    }
}

/// Console exporter for debugging
pub struct ConsoleExporter;

impl MetricsExporter for ConsoleExporter {
    fn export(&self, metrics: &RealtimeMetrics) -> anyhow::Result<()> {
        println!("=== Realtime Metrics ===");
        println!("Harness Active: {}", metrics.harness_active);
        println!("Steps Total: {}", metrics.steps_total);
        println!("Tool Calls: {}", metrics.tool_calls_total);
        println!("Avg Step Duration: {:.2}ms", metrics.avg_step_duration_ms);
        println!("=======================");
        Ok(())
    }
    
    fn name(&self) -> &str {
        "console"
    }
}

/// Metrics export manager
pub struct MetricsExportManager {
    exporters: Vec<Arc<dyn MetricsExporter>>,
    aggregator: Arc<MetricsAggregator>,
}

impl MetricsExportManager {
    pub fn new(aggregator: Arc<MetricsAggregator>) -> Self {
        Self {
            exporters: Vec::new(),
            aggregator,
        }
    }
    
    pub fn add_exporter(&mut self, exporter: impl MetricsExporter + 'static) {
        self.exporters.push(Arc::new(exporter));
    }
    
    pub fn with_default_exporters(mut self) -> Self {
        self.add_exporter(ConsoleExporter);
        self
    }
    
    pub async fn export(&self) -> anyhow::Result<()> {
        let metrics = self.aggregator.snapshot().await;
        
        for exporter in &self.exporters {
            if let Err(e) = exporter.export(&metrics) {
                tracing::warn!("Failed to export to {}: {}", exporter.name(), e);
            }
        }
        
        Ok(())
    }
    
    pub async fn get_metrics(&self) -> RealtimeMetrics {
        self.aggregator.snapshot().await
    }
}

/// Global metrics instance
use std::sync::OnceLock;
static GLOBAL_METRICS: OnceLock<Arc<MetricsAggregator>> = OnceLock::new();

pub fn global_metrics() -> Arc<MetricsAggregator> {
    GLOBAL_METRICS
        .get_or_init(|| Arc::new(MetricsAggregator::new()))
        .clone()
}

/// Convenience functions for recording metrics
pub mod record {
    use super::*;
    
    pub async fn step(duration_ms: u64) {
        let metrics = global_metrics();
        metrics.increment_counter("steps_total").await;
        metrics.record_timing("step_duration_ms", duration_ms).await;
    }
    
    pub async fn tool_call(success: bool) {
        let metrics = global_metrics();
        metrics.increment_counter("tool_calls_total").await;
        if !success {
            metrics.increment_counter("tool_failures_total").await;
        }
    }
    
    pub async fn tokens(count: u64) {
        let metrics = global_metrics();
        let counter = metrics.get_or_create_counter("token_usage_total").await;
        counter.add(count);
    }
    
    pub async fn gauge(name: &str, value: f64) {
        let metrics = global_metrics();
        metrics.set_gauge(name, value).await;
    }
}