//! Performance Monitoring and Optimization System
//!
//! Provides real-time performance metrics, bottleneck detection,
//! and automatic optimization recommendations.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, info};

/// Performance metrics for a single operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationMetrics {
    pub operation_name: String,
    pub count: u64,
    pub total_duration_ms: f64,
    pub min_duration_ms: f64,
    pub max_duration_ms: f64,
    pub avg_duration_ms: f64,
    pub p50_duration_ms: f64,
    pub p95_duration_ms: f64,
    pub p99_duration_ms: f64,
    pub error_count: u64,
    pub error_rate: f64,
    pub throughput_per_minute: f64,
}

impl Default for OperationMetrics {
    fn default() -> Self {
        Self {
            operation_name: String::new(),
            count: 0,
            total_duration_ms: 0.0,
            min_duration_ms: f64::MAX,
            max_duration_ms: 0.0,
            avg_duration_ms: 0.0,
            p50_duration_ms: 0.0,
            p95_duration_ms: 0.0,
            p99_duration_ms: 0.0,
            error_count: 0,
            error_rate: 0.0,
            throughput_per_minute: 0.0,
        }
    }
}

/// System-wide performance snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSnapshot {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub operations: HashMap<String, OperationMetrics>,
    pub system_load: SystemLoad,
    pub bottlenecks: Vec<Bottleneck>,
    pub recommendations: Vec<OptimizationRecommendation>,
}

/// System load metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemLoad {
    pub cpu_percent: f64,
    pub memory_percent: f64,
    pub active_connections: usize,
    pub queue_depth: usize,
}

impl Default for SystemLoad {
    fn default() -> Self {
        Self {
            cpu_percent: 0.0,
            memory_percent: 0.0,
            active_connections: 0,
            queue_depth: 0,
        }
    }
}

/// Detected bottleneck
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bottleneck {
    pub component: String,
    pub severity: BottleneckSeverity,
    pub description: String,
    pub metric_value: f64,
    pub threshold: f64,
    pub suggested_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BottleneckSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Optimization recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationRecommendation {
    pub category: String,
    pub priority: RecommendationPriority,
    pub description: String,
    pub expected_impact: String,
    pub implementation_effort: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RecommendationPriority {
    Low,
    Medium,
    High,
    Critical,
}

/// Performance tracker for individual operations
#[derive(Debug)]
struct OperationTracker {
    durations: Vec<f64>,
    error_count: u64,
    start_time: Option<Instant>,
}

impl OperationTracker {
    fn new() -> Self {
        Self {
            durations: Vec::with_capacity(1000),
            error_count: 0,
            start_time: None,
        }
    }

    fn start(&mut self) {
        self.start_time = Some(Instant::now());
    }

    fn record(&mut self, success: bool) {
        if let Some(start) = self.start_time {
            let duration = start.elapsed().as_secs_f64() * 1000.0;
            self.durations.push(duration);
            
            // Keep only last 10000 measurements
            if self.durations.len() > 10000 {
                self.durations.remove(0);
            }

            if !success {
                self.error_count += 1;
            }
        }
        self.start_time = None;
    }

    fn metrics(&self, operation_name: &str) -> OperationMetrics {
        if self.durations.is_empty() {
            return OperationMetrics {
                operation_name: operation_name.to_string(),
                ..Default::default()
            };
        }

        let count = self.durations.len() as u64;
        let total: f64 = self.durations.iter().sum();
        let avg = total / count as f64;
        
        let mut sorted = self.durations.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let min = *sorted.first().unwrap();
        let max = *sorted.last().unwrap();
        let p50 = sorted[sorted.len() / 2];
        let p95_idx = (sorted.len() as f64 * 0.95) as usize;
        let p95 = sorted[p95_idx.min(sorted.len() - 1)];
        let p99_idx = (sorted.len() as f64 * 0.99) as usize;
        let p99 = sorted[p99_idx.min(sorted.len() - 1)];

        let error_rate = if count > 0 {
            self.error_count as f64 / count as f64
        } else {
            0.0
        };

        // Calculate throughput (last minute)
        let throughput = if count > 0 {
            let time_span_minutes = total / 60000.0;
            if time_span_minutes > 0.0 {
                count as f64 / time_span_minutes
            } else {
                count as f64
            }
        } else {
            0.0
        };

        OperationMetrics {
            operation_name: operation_name.to_string(),
            count,
            total_duration_ms: total,
            min_duration_ms: min,
            max_duration_ms: max,
            avg_duration_ms: avg,
            p50_duration_ms: p50,
            p95_duration_ms: p95,
            p99_duration_ms: p99,
            error_count: self.error_count,
            error_rate,
            throughput_per_minute: throughput,
        }
    }
}

/// Performance monitor
pub struct PerformanceMonitor {
    trackers: Arc<DashMap<String, RwLock<OperationTracker>>>,
    system_load: Arc<RwLock<SystemLoad>>,
    config: PerformanceConfig,
}

#[derive(Clone, Debug)]
pub struct PerformanceConfig {
    pub enable_auto_reporting: bool,
    pub report_interval_secs: u64,
    pub bottleneck_threshold_p95_ms: f64,
    pub bottleneck_threshold_error_rate: f64,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            enable_auto_reporting: true,
            report_interval_secs: 60,
            bottleneck_threshold_p95_ms: 1000.0,
            bottleneck_threshold_error_rate: 0.05,
        }
    }
}

impl PerformanceMonitor {
    /// Create a new performance monitor
    pub fn new() -> Self {
        Self::with_config(PerformanceConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(config: PerformanceConfig) -> Self {
        let monitor = Self {
            trackers: Arc::new(DashMap::new()),
            system_load: Arc::new(RwLock::new(SystemLoad::default())),
            config,
        };

        // Start auto-reporting if enabled
        if monitor.config.enable_auto_reporting {
            let trackers = monitor.trackers.clone();
            let system_load = monitor.system_load.clone();
            let config = monitor.config.clone();
            
            tokio::spawn(async move {
                let mut ticker = interval(Duration::from_secs(config.report_interval_secs));
                loop {
                    ticker.tick().await;
                    
                    let snapshot = Self::generate_snapshot(
                        &trackers,
                        &system_load,
                        &config,
                    ).await;
                    
                    info!(
                        "Performance Report:\n{}",
                        serde_json::to_string_pretty(&snapshot).unwrap_or_default()
                    );
                }
            });
        }

        monitor
    }

    /// Start tracking an operation
    pub async fn start_operation(&self, operation_name: &str) {
        let tracker = self.trackers
            .entry(operation_name.to_string())
            .or_insert_with(|| RwLock::new(OperationTracker::new()));
        
        tracker.write().await.start();
    }

    /// Finish tracking an operation
    pub async fn finish_operation(&self, operation_name: &str, success: bool) {
        if let Some(tracker) = self.trackers.get(operation_name) {
            tracker.write().await.record(success);
        }
    }

    /// Time a function execution
    pub async fn time<F, Fut, T>(&self, operation_name: &str, f: F) -> T
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        self.start_operation(operation_name).await;
        let start = Instant::now();
        
        let result = f().await;
        
        let success = true; // Could be determined by result type
        self.finish_operation(operation_name, success).await;
        
        let duration = start.elapsed();
        debug!("Operation {} took {:?}", operation_name, duration);
        
        result
    }

    /// Update system load
    pub async fn update_system_load(&self, load: SystemLoad) {
        *self.system_load.write().await = load;
    }

    /// Get current performance snapshot
    pub async fn snapshot(&self) -> PerformanceSnapshot {
        Self::generate_snapshot(
            &self.trackers,
            &self.system_load,
            &self.config,
        ).await
    }

    /// Get metrics for a specific operation
    pub async fn get_metrics(&self, operation_name: &str) -> Option<OperationMetrics> {
        if let Some(tracker) = self.trackers.get(operation_name) {
            let metrics = tracker.read().await.metrics(operation_name);
            return Some(metrics);
        }
        None
    }

    /// Get all metrics
    pub async fn get_all_metrics(&self) -> HashMap<String, OperationMetrics> {
        let mut metrics = HashMap::new();
        for entry in self.trackers.iter() {
            let m = entry.value().read().await.metrics(entry.key());
            metrics.insert(
                entry.key().clone(),
                m,
            );
        }
        metrics
    }

    /// Detect bottlenecks
    pub async fn detect_bottlenecks(&self) -> Vec<Bottleneck> {
        let mut bottlenecks = Vec::new();
        
        for entry in self.trackers.iter() {
            let metrics = entry.value().read().await.metrics(entry.key());
            
            // Check P95 latency
            if metrics.p95_duration_ms > self.config.bottleneck_threshold_p95_ms {
                bottlenecks.push(Bottleneck {
                    component: entry.key().clone(),
                    severity: if metrics.p95_duration_ms > 5000.0 {
                        BottleneckSeverity::Critical
                    } else if metrics.p95_duration_ms > 2000.0 {
                        BottleneckSeverity::High
                    } else {
                        BottleneckSeverity::Medium
                    },
                    description: format!(
                        "High P95 latency: {:.2}ms",
                        metrics.p95_duration_ms
                    ),
                    metric_value: metrics.p95_duration_ms,
                    threshold: self.config.bottleneck_threshold_p95_ms,
                    suggested_action: "Consider caching, optimization, or scaling".to_string(),
                });
            }
            
            // Check error rate
            if metrics.error_rate > self.config.bottleneck_threshold_error_rate {
                bottlenecks.push(Bottleneck {
                    component: entry.key().clone(),
                    severity: if metrics.error_rate > 0.2 {
                        BottleneckSeverity::Critical
                    } else if metrics.error_rate > 0.1 {
                        BottleneckSeverity::High
                    } else {
                        BottleneckSeverity::Medium
                    },
                    description: format!(
                        "High error rate: {:.1}%",
                        metrics.error_rate * 100.0
                    ),
                    metric_value: metrics.error_rate,
                    threshold: self.config.bottleneck_threshold_error_rate,
                    suggested_action: "Review error logs and add error handling".to_string(),
                });
            }
        }
        
        // Sort by severity
        bottlenecks.sort_by(|a, b| {
            let severity_order = |s: &BottleneckSeverity| match s {
                BottleneckSeverity::Critical => 0,
                BottleneckSeverity::High => 1,
                BottleneckSeverity::Medium => 2,
                BottleneckSeverity::Low => 3,
            };
            severity_order(&a.severity).cmp(&severity_order(&b.severity))
        });
        
        bottlenecks
    }

    /// Generate optimization recommendations
    pub async fn generate_recommendations(&self) -> Vec<OptimizationRecommendation> {
        let mut recommendations = Vec::new();
        let bottlenecks = self.detect_bottlenecks().await;
        
        for bottleneck in bottlenecks {
            let recommendation = match bottleneck.severity {
                BottleneckSeverity::Critical => OptimizationRecommendation {
                    category: "Performance".to_string(),
                    priority: RecommendationPriority::Critical,
                    description: format!(
                        "Critical bottleneck in {}: {}",
                        bottleneck.component,
                        bottleneck.description
                    ),
                    expected_impact: "50-80% latency reduction".to_string(),
                    implementation_effort: "High".to_string(),
                },
                BottleneckSeverity::High => OptimizationRecommendation {
                    category: "Performance".to_string(),
                    priority: RecommendationPriority::High,
                    description: format!(
                        "High severity issue in {}: {}",
                        bottleneck.component,
                        bottleneck.description
                    ),
                    expected_impact: "30-50% improvement".to_string(),
                    implementation_effort: "Medium".to_string(),
                },
                _ => OptimizationRecommendation {
                    category: "Optimization".to_string(),
                    priority: RecommendationPriority::Medium,
                    description: format!(
                        "Optimization opportunity in {}: {}",
                        bottleneck.component,
                        bottleneck.description
                    ),
                    expected_impact: "10-20% improvement".to_string(),
                    implementation_effort: "Low".to_string(),
                },
            };
            
            recommendations.push(recommendation);
        }
        
        recommendations
    }

    // Private helper methods

    async fn generate_snapshot(
        trackers: &Arc<DashMap<String, RwLock<OperationTracker>>>,
        system_load: &Arc<RwLock<SystemLoad>>,
        config: &PerformanceConfig,
    ) -> PerformanceSnapshot {
        let mut operations = HashMap::new();
        
        for entry in trackers.iter() {
            operations.insert(
                entry.key().clone(),
                entry.value().read().await.metrics(entry.key()),
            );
        }
        
        let load = system_load.read().await.clone();
        
        // Detect bottlenecks
        let mut bottlenecks = Vec::new();
        for (name, metrics) in &operations {
            if metrics.p95_duration_ms > config.bottleneck_threshold_p95_ms {
                bottlenecks.push(Bottleneck {
                    component: name.clone(),
                    severity: BottleneckSeverity::High,
                    description: format!("High P95 latency: {:.2}ms", metrics.p95_duration_ms),
                    metric_value: metrics.p95_duration_ms,
                    threshold: config.bottleneck_threshold_p95_ms,
                    suggested_action: "Consider optimization".to_string(),
                });
            }
        }
        
        // Generate recommendations
        let recommendations = bottlenecks.iter().map(|b| OptimizationRecommendation {
            category: "Performance".to_string(),
            priority: match b.severity {
                BottleneckSeverity::Critical => RecommendationPriority::Critical,
                BottleneckSeverity::High => RecommendationPriority::High,
                _ => RecommendationPriority::Medium,
            },
            description: b.description.clone(),
            expected_impact: "Significant improvement".to_string(),
            implementation_effort: "Medium".to_string(),
        }).collect();
        
        PerformanceSnapshot {
            timestamp: chrono::Utc::now(),
            operations,
            system_load: load,
            bottlenecks,
            recommendations,
        }
    }
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Auto-scaling controller based on performance metrics
pub struct AutoScaler {
    monitor: Arc<PerformanceMonitor>,
    config: AutoScaleConfig,
}

#[derive(Clone, Debug)]
pub struct AutoScaleConfig {
    pub min_instances: usize,
    pub max_instances: usize,
    pub scale_up_threshold: f64,
    pub scale_down_threshold: f64,
    pub cooldown_secs: u64,
}

impl Default for AutoScaleConfig {
    fn default() -> Self {
        Self {
            min_instances: 2,
            max_instances: 100,
            scale_up_threshold: 0.8,
            scale_down_threshold: 0.3,
            cooldown_secs: 300,
        }
    }
}

impl AutoScaler {
    pub fn new(monitor: Arc<PerformanceMonitor>) -> Self {
        Self {
            monitor,
            config: AutoScaleConfig::default(),
        }
    }

    pub fn with_config(mut self, config: AutoScaleConfig) -> Self {
        self.config = config;
        self
    }

    /// Evaluate scaling decision
    pub async fn evaluate(&self) -> ScaleDecision {
        let snapshot = self.monitor.snapshot().await;
        
        // Calculate average CPU/memory utilization
        let avg_load = (snapshot.system_load.cpu_percent + snapshot.system_load.memory_percent) / 2.0;
        
        if avg_load > self.config.scale_up_threshold * 100.0 {
            ScaleDecision::ScaleUp
        } else if avg_load < self.config.scale_down_threshold * 100.0 {
            ScaleDecision::ScaleDown
        } else {
            ScaleDecision::NoChange
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScaleDecision {
    ScaleUp,
    ScaleDown,
    NoChange,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_performance_monitor() {
        let monitor = PerformanceMonitor::new();
        
        // Simulate some operations
        for _i in 0..10 {
            monitor.start_operation("test_op").await;
            tokio::time::sleep(Duration::from_millis(10)).await;
            monitor.finish_operation("test_op", true).await;
        }
        
        let metrics = monitor.get_metrics("test_op").await;
        assert!(metrics.is_some());
        
        let metrics = metrics.unwrap();
        assert_eq!(metrics.count, 10);
        assert!(metrics.avg_duration_ms > 0.0);
    }

    #[tokio::test]
    async fn test_bottleneck_detection() {
        let monitor = PerformanceMonitor::with_config(PerformanceConfig {
            enable_auto_reporting: false,
            report_interval_secs: 60,
            bottleneck_threshold_p95_ms: 50.0,
            bottleneck_threshold_error_rate: 0.05,
        });
        
        // Simulate slow operation
        for _ in 0..5 {
            monitor.start_operation("slow_op").await;
            tokio::time::sleep(Duration::from_millis(100)).await;
            monitor.finish_operation("slow_op", true).await;
        }
        
        let bottlenecks = monitor.detect_bottlenecks().await;
        assert!(!bottlenecks.is_empty());
    }
}
