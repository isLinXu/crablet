//! Metrics and Observability - 可观测性建设
//!
//! 提供全面的指标收集、监控和告警功能

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
// tracing imported when needed
use tokio::time::interval;

/// 指标类型
#[derive(Debug, Clone)]
pub enum MetricType {
    /// 计数器
    Counter(u64),
    /// 仪表盘（瞬时值）
    Gauge(f64),
    /// 直方图
    Histogram(Vec<f64>),
    /// 计时器
    Timer(Duration),
}

/// 指标标签
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct MetricLabels {
    labels: HashMap<String, String>,
}

impl std::hash::Hash for MetricLabels {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // 将 HashMap 转换为有序的 Vec 再哈希
        let mut pairs: Vec<_> = self.labels.iter().collect();
        pairs.sort_by(|a, b| a.0.cmp(b.0));
        for (k, v) in pairs {
            k.hash(state);
            v.hash(state);
        }
    }
}

impl MetricLabels {
    pub fn new() -> Self {
        Self {
            labels: HashMap::new(),
        }
    }

    pub fn with(mut self, key: &str, value: &str) -> Self {
        self.labels.insert(key.to_string(), value.to_string());
        self
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.labels.get(key)
    }
}

impl Default for MetricLabels {
    fn default() -> Self {
        Self::new()
    }
}

/// 指标记录
#[derive(Debug, Clone)]
struct MetricRecord {
    name: String,
    metric_type: MetricType,
    labels: MetricLabels,
    timestamp: Instant,
}

/// 指标聚合器
#[derive(Debug, Default)]
pub struct MetricsAggregator {
    counters: RwLock<HashMap<String, HashMap<MetricLabels, u64>>>,
    gauges: RwLock<HashMap<String, HashMap<MetricLabels, f64>>>,
    histograms: RwLock<HashMap<String, HashMap<MetricLabels, Vec<f64>>>>,
    timers: RwLock<HashMap<String, HashMap<MetricLabels, Vec<Duration>>>>,
}

impl MetricsAggregator {
    pub fn new() -> Self {
        Self::default()
    }

    /// 记录计数器
    pub fn record_counter(&self, name: &str, labels: MetricLabels, value: u64) {
        let mut counters = self.counters.write();
        counters
            .entry(name.to_string())
            .or_insert_with(HashMap::new)
            .entry(labels)
            .and_modify(|v| *v += value)
            .or_insert(value);
    }

    /// 记录仪表盘
    pub fn record_gauge(&self, name: &str, labels: MetricLabels, value: f64) {
        let mut gauges = self.gauges.write();
        gauges
            .entry(name.to_string())
            .or_insert_with(HashMap::new)
            .insert(labels, value);
    }

    /// 记录直方图
    pub fn record_histogram(&self, name: &str, labels: MetricLabels, value: f64) {
        let mut histograms = self.histograms.write();
        histograms
            .entry(name.to_string())
            .or_insert_with(HashMap::new)
            .entry(labels)
            .or_insert_with(Vec::new)
            .push(value);
    }

    /// 记录计时器
    pub fn record_timer(&self, name: &str, labels: MetricLabels, duration: Duration) {
        let mut timers = self.timers.write();
        timers
            .entry(name.to_string())
            .or_insert_with(HashMap::new)
            .entry(labels)
            .or_insert_with(Vec::new)
            .push(duration);
    }

    /// 获取计数器值
    pub fn get_counter(&self, name: &str, labels: &MetricLabels) -> Option<u64> {
        self.counters
            .read()
            .get(name)
            .and_then(|m| m.get(labels).copied())
    }

    /// 获取仪表盘值
    pub fn get_gauge(&self, name: &str, labels: &MetricLabels) -> Option<f64> {
        self.gauges
            .read()
            .get(name)
            .and_then(|m| m.get(labels).copied())
    }

    /// 获取直方图统计
    pub fn get_histogram_stats(&self, name: &str, labels: &MetricLabels) -> Option<HistogramStats> {
        let histograms = self.histograms.read();
        let values = histograms.get(name)?.get(labels)?;
        
        if values.is_empty() {
            return None;
        }

        let mut sorted = values.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let sum: f64 = sorted.iter().sum();
        let count = sorted.len() as f64;
        let mean = sum / count;

        let min = sorted[0];
        let max = sorted[sorted.len() - 1];

        let p50_idx = (count * 0.5) as usize;
        let p95_idx = (count * 0.95) as usize;
        let p99_idx = (count * 0.99) as usize;

        Some(HistogramStats {
            count: sorted.len(),
            min,
            max,
            mean,
            p50: sorted[p50_idx.min(sorted.len() - 1)],
            p95: sorted[p95_idx.min(sorted.len() - 1)],
            p99: sorted[p99_idx.min(sorted.len() - 1)],
        })
    }

    /// 获取计时器统计
    pub fn get_timer_stats(&self, name: &str, labels: &MetricLabels) -> Option<TimerStats> {
        let timers = self.timers.read();
        let durations = timers.get(name)?.get(labels)?;
        
        if durations.is_empty() {
            return None;
        }

        let mut sorted: Vec<u64> = durations.iter().map(|d| d.as_millis() as u64).collect();
        sorted.sort();

        let sum: u64 = sorted.iter().sum();
        let count = sorted.len() as u64;
        let mean_ms = sum as f64 / count as f64;

        Some(TimerStats {
            count: sorted.len(),
            mean_ms,
            min_ms: sorted[0],
            max_ms: sorted[sorted.len() - 1],
            p50_ms: sorted[(sorted.len() as f64 * 0.5) as usize],
            p95_ms: sorted[(sorted.len() as f64 * 0.95) as usize],
            p99_ms: sorted[((sorted.len() as f64 * 0.99) as usize).min(sorted.len() - 1)],
        })
    }

    /// 清空所有指标
    pub fn clear(&self) {
        self.counters.write().clear();
        self.gauges.write().clear();
        self.histograms.write().clear();
        self.timers.write().clear();
    }

    /// 导出为 Prometheus 格式
    pub fn export_prometheus(&self) -> String {
        let mut output = String::new();

        // 计数器
        for (name, metrics) in self.counters.read().iter() {
            output.push_str(&format!("# TYPE {} counter\n", name));
            for (labels, value) in metrics {
                let label_str = self.format_labels(labels);
                output.push_str(&format!("{}{} {}\n", name, label_str, value));
            }
        }

        // 仪表盘
        for (name, metrics) in self.gauges.read().iter() {
            output.push_str(&format!("# TYPE {} gauge\n", name));
            for (labels, value) in metrics {
                let label_str = self.format_labels(labels);
                output.push_str(&format!("{}{} {}\n", name, label_str, value));
            }
        }

        output
    }

    fn format_labels(&self, labels: &MetricLabels) -> String {
        if labels.labels.is_empty() {
            return String::new();
        }
        
        let pairs: Vec<String> = labels
            .labels
            .iter()
            .map(|(k, v)| format!("{}=\"{}\"", k, v))
            .collect();
        
        format!("{{{}}}", pairs.join(","))
    }
}

/// 直方图统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramStats {
    pub count: usize,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
}

/// 计时器统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerStats {
    pub count: usize,
    pub mean_ms: f64,
    pub min_ms: u64,
    pub max_ms: u64,
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub p99_ms: u64,
}

/// 系统指标收集器
pub struct SystemMetricsCollector {
    aggregator: Arc<MetricsAggregator>,
    collection_interval: Duration,
}

impl SystemMetricsCollector {
    pub fn new(aggregator: Arc<MetricsAggregator>, collection_interval: Duration) -> Self {
        Self {
            aggregator,
            collection_interval,
        }
    }

    /// 启动收集循环
    pub async fn start_collection(&self) {
        let mut interval = interval(self.collection_interval);
        
        loop {
            interval.tick().await;
            self.collect_system_metrics().await;
        }
    }

    async fn collect_system_metrics(&self) {
        // 内存使用
        if let Some(memory) = Self::get_memory_usage() {
            self.aggregator.record_gauge(
                "system_memory_usage_bytes",
                MetricLabels::new(),
                memory as f64,
            );
        }

        // CPU 使用
        if let Some(cpu) = Self::get_cpu_usage() {
            self.aggregator.record_gauge(
                "system_cpu_usage_percent",
                MetricLabels::new(),
                cpu,
            );
        }

        // 任务数
        self.aggregator.record_gauge(
            "system_active_tasks",
            MetricLabels::new(),
            Self::get_active_tasks() as f64,
        );
    }

    fn get_memory_usage() -> Option<usize> {
        // 简化实现，实际可使用 sysinfo crate
        None
    }

    fn get_cpu_usage() -> Option<f64> {
        // 简化实现
        None
    }

    fn get_active_tasks() -> usize {
        // 简化实现
        0
    }
}

/// 业务指标收集器
pub struct BusinessMetricsCollector {
    aggregator: Arc<MetricsAggregator>,
}

impl BusinessMetricsCollector {
    pub fn new(aggregator: Arc<MetricsAggregator>) -> Self {
        Self { aggregator }
    }

    /// 记录请求
    pub fn record_request(&self, endpoint: &str, status: &str, latency: Duration) {
        let labels = MetricLabels::new()
            .with("endpoint", endpoint)
            .with("status", status);

        self.aggregator.record_counter("http_requests_total", labels.clone(), 1);
        self.aggregator.record_timer("http_request_duration_seconds", labels, latency);
    }

    /// 记录路由决策
    pub fn record_routing_decision(&self, target: &str, confidence: f32) {
        let labels = MetricLabels::new().with("target", target);
        
        self.aggregator.record_counter("routing_decisions_total", labels.clone(), 1);
        self.aggregator.record_histogram("routing_confidence", labels, confidence as f64);
    }

    /// 记录 LLM 调用
    pub fn record_llm_call(&self, model: &str, tokens_in: u64, tokens_out: u64, latency: Duration) {
        let labels = MetricLabels::new().with("model", model);
        
        self.aggregator.record_counter("llm_tokens_input", labels.clone(), tokens_in);
        self.aggregator.record_counter("llm_tokens_output", labels.clone(), tokens_out);
        self.aggregator.record_timer("llm_request_duration", labels, latency);
    }

    /// 记录缓存命中
    pub fn record_cache_hit(&self, level: &str) {
        let labels = MetricLabels::new().with("level", level);
        self.aggregator.record_counter("cache_hits_total", labels, 1);
    }

    /// 记录缓存未命中
    pub fn record_cache_miss(&self) {
        self.aggregator.record_counter("cache_misses_total", MetricLabels::new(), 1);
    }

    /// 记录技能执行
    pub fn record_skill_execution(&self, skill_name: &str, success: bool, latency: Duration) {
        let status = if success { "success" } else { "failure" };
        let labels = MetricLabels::new()
            .with("skill", skill_name)
            .with("status", status);
        
        self.aggregator.record_counter("skill_executions_total", labels.clone(), 1);
        self.aggregator.record_timer("skill_execution_duration", labels, latency);
    }

    /// 记录用户反馈
    pub fn record_user_feedback(&self, rating: u8) {
        self.aggregator.record_histogram(
            "user_feedback_rating",
            MetricLabels::new(),
            rating as f64,
        );
    }
}

/// 告警规则
#[derive(Debug, Clone)]
pub struct AlertRule {
    pub name: String,
    pub metric_name: String,
    pub condition: AlertCondition,
    pub threshold: f64,
    pub duration: Duration,
}

#[derive(Debug, Clone)]
pub enum AlertCondition {
    GreaterThan,
    LessThan,
    EqualTo,
}

/// 告警管理器
pub struct AlertManager {
    rules: RwLock<Vec<AlertRule>>,
    aggregator: Arc<MetricsAggregator>,
}

impl AlertManager {
    pub fn new(aggregator: Arc<MetricsAggregator>) -> Self {
        Self {
            rules: RwLock::new(Vec::new()),
            aggregator,
        }
    }

    /// 添加告警规则
    pub fn add_rule(&self, rule: AlertRule) {
        self.rules.write().push(rule);
    }

    /// 检查告警
    pub fn check_alerts(&self) -> Vec<Alert> {
        let mut alerts = Vec::new();
        let rules = self.rules.read();

        for rule in rules.iter() {
            if let Some(value) = self.get_metric_value(&rule.metric_name) {
                let triggered = match rule.condition {
                    AlertCondition::GreaterThan => value > rule.threshold,
                    AlertCondition::LessThan => value < rule.threshold,
                    AlertCondition::EqualTo => (value - rule.threshold).abs() < 0.001,
                };

                if triggered {
                    alerts.push(Alert {
                        rule_name: rule.name.clone(),
                        metric_name: rule.metric_name.clone(),
                        current_value: value,
                        threshold: rule.threshold,
                        timestamp: Instant::now(),
                    });
                }
            }
        }

        alerts
    }

    fn get_metric_value(&self, metric_name: &str) -> Option<f64> {
        // 简化实现，实际应该根据指标类型获取
        self.aggregator
            .get_gauge(metric_name, &MetricLabels::new())
    }
}

/// 告警
#[derive(Debug, Clone)]
pub struct Alert {
    pub rule_name: String,
    pub metric_name: String,
    pub current_value: f64,
    pub threshold: f64,
    pub timestamp: Instant,
}

/// 全局指标注册表
pub struct MetricsRegistry {
    aggregator: Arc<MetricsAggregator>,
    business_collector: BusinessMetricsCollector,
    alert_manager: AlertManager,
}

impl MetricsRegistry {
    pub fn new() -> Self {
        let aggregator = Arc::new(MetricsAggregator::new());
        
        Self {
            aggregator: aggregator.clone(),
            business_collector: BusinessMetricsCollector::new(aggregator.clone()),
            alert_manager: AlertManager::new(aggregator),
        }
    }

    pub fn aggregator(&self) -> Arc<MetricsAggregator> {
        self.aggregator.clone()
    }

    pub fn business(&self) -> &BusinessMetricsCollector {
        &self.business_collector
    }

    pub fn alerts(&self) -> &AlertManager {
        &self.alert_manager
    }

    /// 导出 Prometheus 格式
    pub fn export_prometheus(&self) -> String {
        self.aggregator.export_prometheus()
    }

    /// 获取健康报告
    pub fn health_report(&self) -> HealthReport {
        let alerts = self.alert_manager.check_alerts();
        
        HealthReport {
            healthy: alerts.is_empty(),
            alerts,
            timestamp: Instant::now(),
        }
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 健康报告
#[derive(Debug, Clone)]
pub struct HealthReport {
    pub healthy: bool,
    pub alerts: Vec<Alert>,
    pub timestamp: Instant,
}

/// 计时器辅助宏
#[macro_export]
macro_rules! timed {
    ($registry:expr, $name:expr, $labels:expr, $block:expr) => {{
        let start = std::time::Instant::now();
        let result = $block;
        let duration = start.elapsed();
        $registry.aggregator().record_timer($name, $labels, duration);
        result
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_aggregator() {
        let aggregator = MetricsAggregator::new();
        
        let labels = MetricLabels::new().with("endpoint", "/api/test");
        aggregator.record_counter("requests", labels.clone(), 1);
        
        assert_eq!(aggregator.get_counter("requests", &labels), Some(1));
    }

    #[test]
    fn test_histogram_stats() {
        let aggregator = MetricsAggregator::new();
        let labels = MetricLabels::new();
        
        for i in 1..=100 {
            aggregator.record_histogram("latency", labels.clone(), i as f64);
        }
        
        let stats = aggregator.get_histogram_stats("latency", &labels).unwrap();
        assert_eq!(stats.count, 100);
        assert!(stats.p50 >= 49.0 && stats.p50 <= 51.0);
    }

    #[test]
    fn test_prometheus_export() {
        let aggregator = MetricsAggregator::new();
        let labels = MetricLabels::new().with("status", "200");
        
        aggregator.record_counter("requests", labels, 42);
        
        let output = aggregator.export_prometheus();
        assert!(output.contains("requests"));
        assert!(output.contains("42"));
    }
}
