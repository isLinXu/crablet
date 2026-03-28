//! Real-time Observability and Distributed Tracing System
//!
//! 实时可观测性和分布式追踪系统:
//! - OpenTelemetry 集成
//! - 分布式追踪
//! - 性能指标实时监控
//! - 异常检测和告警

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// 追踪 span
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub trace_id: String,
    pub span_id: String,
    pub parent_id: Option<String>,
    pub operation_name: String,
    pub service_name: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_ms: Option<u64>,
    pub tags: HashMap<String, String>,
    pub logs: Vec<SpanLog>,
    pub status: SpanStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpanStatus {
    Ok,
    Error,
    Unset,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanLog {
    pub timestamp: DateTime<Utc>,
    pub fields: HashMap<String, serde_json::Value>,
}

/// 追踪上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContext {
    pub trace_id: String,
    pub span_id: String,
    pub sampled: bool,
}

impl TraceContext {
    pub fn new() -> Self {
        Self {
            trace_id: Uuid::new_v4().to_string(),
            span_id: Uuid::new_v4().to_string()[..8].to_string(),
            sampled: rand::random::<f32>() < 0.5,  // 50% 采样率
        }
    }
    
    pub fn child(&self) -> Self {
        Self {
            trace_id: self.trace_id.clone(),
            span_id: Uuid::new_v4().to_string()[..8].to_string(),
            sampled: self.sampled,
        }
    }
}

impl Default for TraceContext {
    fn default() -> Self {
        Self::new()
    }
}

/// 追踪器
pub struct Tracer {
    service_name: String,
    active_spans: RwLock<HashMap<String, Span>>,
    completed_spans: RwLock<VecDeque<Span>>,
    max_spans: usize,
}

impl Tracer {
    pub fn new(service_name: String, max_spans: usize) -> Self {
        Self {
            service_name,
            active_spans: RwLock::new(HashMap::new()),
            completed_spans: RwLock::new(VecDeque::with_capacity(max_spans)),
            max_spans,
        }
    }
    
    /// 开始一个 span
    pub async fn start_span(&self, context: &TraceContext, operation: &str) -> String {
        let span = Span {
            trace_id: context.trace_id.clone(),
            span_id: context.span_id.clone(),
            parent_id: None,  // 简化处理
            operation_name: operation.to_string(),
            service_name: self.service_name.clone(),
            start_time: Utc::now(),
            end_time: None,
            duration_ms: None,
            tags: HashMap::new(),
            logs: Vec::new(),
            status: SpanStatus::Unset,
        };
        
        let span_id = span.span_id.clone();
        
        let mut spans = self.active_spans.write().await;
        spans.insert(span_id.clone(), span);
        
        span_id
    }
    
    /// 结束一个 span
    pub async fn end_span(&self, span_id: &str, status: SpanStatus) {
        let mut spans = self.active_spans.write().await;
        
        if let Some(span) = spans.remove(span_id) {
            let mut completed = span;
            completed.end_time = Some(Utc::now());
            completed.status = status;
            
            if let (Some(start), Some(end)) = (completed.start_time, completed.end_time) {
                completed.duration_ms = Some((end - start).num_milliseconds() as u64);
            }
            
            // 添加到已完成 span
            let mut completed_spans = self.completed_spans.write().await;
            if completed_spans.len() >= self.max_spans {
                completed_spans.pop_front();
            }
            completed_spans.push_back(completed);
        }
    }
    
    /// 添加 tag
    pub async fn add_tag(&self, span_id: &str, key: &str, value: &str) {
        let spans = self.active_spans.read().await;
        if let Some(span) = spans.get(span_id) {
            drop(spans);
            let mut spans = self.active_spans.write().await;
            if let Some(span) = spans.get_mut(span_id) {
                span.tags.insert(key.to_string(), value.to_string());
            }
        }
    }
    
    /// 添加日志
    pub async fn add_log(&self, span_id: &str, fields: HashMap<String, serde_json::Value>) {
        let spans = self.active_spans.read().await;
        if let Some(span) = spans.get(span_id) {
            drop(spans);
            let mut spans = self.active_spans.write().await;
            if let Some(span) = spans.get_mut(span_id) {
                span.logs.push(SpanLog {
                    timestamp: Utc::now(),
                    fields,
                });
            }
        }
    }
    
    /// 获取追踪树
    pub async fn get_trace(&self, trace_id: &str) -> Vec<Span> {
        let completed = self.completed_spans.read().await;
        
        completed.iter()
            .filter(|s| s.trace_id == trace_id)
            .cloned()
            .collect()
    }
    
    /// 获取最近的追踪
    pub async fn get_recent_traces(&self, limit: usize) -> Vec<String> {
        let completed = self.completed_spans.read().await;
        
        let mut trace_ids: Vec<_> = completed.iter()
            .map(|s| s.trace_id.clone())
            .collect();
        
        trace_ids.dedup();
        trace_ids.truncate(limit);
        
        trace_ids
    }
}

/// 性能指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub timestamp: DateTime<Utc>,
    pub counters: HashMap<String, u64>,
    pub gauges: HashMap<String, f64>,
    pub histograms: HashMap<String, HistogramSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramSnapshot {
    pub count: u64,
    pub sum: f64,
    pub min: f64,
    pub max: f64,
    pub p50: f64,
    pub p90: f64,
    pub p99: f64,
}

/// 指标收集器
pub struct MetricsCollector {
    counters: RwLock<HashMap<String, Arc<std::sync::atomic::AtomicU64>>>,
    gauges: RwLock<HashMap<String, Arc<std::sync::atomic::AtomicU64>>>,
    histograms: RwLock<HashMap<String, RwLock<VecDeque<f64>>>>,
    max_histogram_size: usize,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            counters: RwLock::new(HashMap::new()),
            gauges: RwLock::new(HashMap::new()),
            histograms: RwLock::new(HashMap::new()),
            max_histogram_size: 1000,
        }
    }
    
    /// 计数器增加
    pub fn inc_counter(&self, name: &str, delta: u64) {
        let counters = self.counters.read().await;
        
        if let Some(counter) = counters.get(name) {
            counter.fetch_add(delta, std::sync::atomic::Ordering::Relaxed);
        } else {
            drop(counters);
            let mut counters = self.counters.write().await;
            let counter = Arc::new(std::sync::atomic::AtomicU64::new(delta));
            counters.insert(name.to_string(), counter);
        }
    }
    
    /// 设置仪表值
    pub fn set_gauge(&self, name: &str, value: f64) {
        let gauges = self.gauges.read().await;
        
        if let Some(gauge) = gauges.get(name) {
            gauge.store(value as u64, std::sync::atomic::Ordering::Relaxed);
        } else {
            drop(gauges);
            let mut gauges = self.gauges.write().await;
            let gauge = Arc::new(std::sync::atomic::AtomicU64::new(value as u64));
            gauges.insert(name.to_string(), gauge);
        }
    }
    
    /// 记录直方图值
    pub fn record_histogram(&self, name: &str, value: f64) {
        let histograms = self.histograms.read().await;
        
        if let Some(hist) = histograms.get(name) {
            drop(hist);
            let mut hist = histograms.write().await;
            if let Some(hist) = hist.get(name) {
                let mut data = hist.write().await;
                if data.len() >= self.max_histogram_size {
                    data.pop_front();
                }
                data.push_back(value);
            }
        } else {
            drop(histograms);
            let mut histograms = self.histograms.write().await;
            let data = RwLock::new(VecDeque::with_capacity(self.max_histogram_size));
            data.write().await.push_back(value);
            histograms.insert(name.to_string(), data);
        }
    }
    
    /// 获取快照
    pub async fn snapshot(&self) -> MetricsSnapshot {
        let mut counters_map = HashMap::new();
        let counters = self.counters.read().await;
        for (name, counter) in counters.iter() {
            counters_map.insert(name.clone(), counter.load(std::sync::atomic::Ordering::Relaxed));
        }
        
        let mut gauges_map = HashMap::new();
        let gauges = self.gauges.read().await;
        for (name, gauge) in gauges.iter() {
            gauges_map.insert(name.clone(), gauge.load(std::sync::atomic::Ordering::Relaxed) as f64);
        }
        
        let mut histograms_map = HashMap::new();
        let histograms = self.histograms.read().await;
        for (name, data) in histograms.iter() {
            let data = data.read().await;
            let values: Vec<f64> = data.iter().cloned().collect();
            
            if !values.is_empty() {
                let mut sorted = values.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                
                let count = sorted.len() as u64;
                let sum: f64 = sorted.iter().sum();
                let p50 = sorted[(count as f32 * 0.5) as usize].min(sorted[(count as f32 * 0.5).ceil() as usize]);
                let p90 = sorted[(count as f32 * 0.9) as usize];
                let p99 = sorted[(count as f32 * 0.99) as usize].min(sorted[(count as f32 * 0.99).ceil() as usize]);
                
                histograms_map.insert(name.clone(), HistogramSnapshot {
                    count,
                    sum,
                    min: sorted.first().copied().unwrap_or(0.0),
                    max: sorted.last().copied().unwrap_or(0.0),
                    p50,
                    p90,
                    p99,
                });
            }
        }
        
        MetricsSnapshot {
            timestamp: Utc::now(),
            counters: counters_map,
            gauges: gauges_map,
            histograms: histograms_map,
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// 告警规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub name: String,
    pub condition: AlertCondition,
    pub severity: AlertSeverity,
    pub cooldown: Duration,
    pub last_triggered: Option<Instant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertCondition {
    MetricThreshold { metric: String, operator: String, value: f64 },
    ErrorRate { threshold: f64 },
    LatencyP99 { threshold_ms: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

/// 告警管理器
pub struct AlertManager {
    rules: RwLock<Vec<AlertRule>>,
    alerts: RwLock<VecDeque<Alert>>,
    metrics: Arc<MetricsCollector>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub rule_name: String,
    pub severity: AlertSeverity,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

impl AlertManager {
    pub fn new(metrics: Arc<MetricsCollector>) -> Self {
        Self {
            rules: RwLock::new(Vec::new()),
            alerts: RwLock::new(VecDeque::new()),
            metrics,
        }
    }
    
    /// 添加规则
    pub fn add_rule(&self, rule: AlertRule) {
        let mut rules = self.rules.write().await;
        rules.push(rule);
    }
    
    /// 检查规则
    pub async fn check_rules(&self) -> Vec<Alert> {
        let snapshot = self.metrics.snapshot().await;
        let rules = self.rules.read().await;
        let mut triggered = Vec::new();
        
        for rule in rules.iter() {
            let should_check = match rule.last_triggered {
                Some(last) => last.elapsed() > rule.cooldown,
                None => true,
            };
            
            if !should_check { continue; }
            
            let triggered_alert = self.evaluate_rule(rule, &snapshot).await;
            if let Some(alert) = triggered_alert {
                triggered.push(alert);
            }
        }
        
        triggered
    }
    
    async fn evaluate_rule(&self, rule: &AlertRule, snapshot: &MetricsSnapshot) -> Option<Alert> {
        match &rule.condition {
            AlertCondition::MetricThreshold { metric, operator, value } => {
                if let Some(gauge) = snapshot.gauges.get(metric) {
                    let triggered = match operator.as_str() {
                        ">" => gauge > value,
                        "<" => gauge < value,
                        ">=" => gauge >= value,
                        "<=" => gauge <= value,
                        "==" => (*gauge as i64) == (*value as i64),
                        _ => false,
                    };
                    
                    if triggered {
                        Some(self.create_alert(rule, &format!("{} {} {}", metric, operator, value)))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            AlertCondition::ErrorRate { threshold } => {
                if let Some(errors) = snapshot.counters.get("errors") {
                    if let Some(total) = snapshot.counters.get("requests") {
                        let rate = *errors as f64 / (*total as f64).max(1.0);
                        if rate > *threshold {
                            return Some(self.create_alert(rule, &format!("Error rate {} > {}", rate, threshold)));
                        }
                    }
                }
                None
            }
            AlertCondition::LatencyP99 { threshold_ms } => {
                if let Some(hist) = snapshot.histograms.get("latency_ms") {
                    if hist.p99 > *threshold_ms as f64 {
                        return Some(self.create_alert(rule, &format!("P99 latency {}ms > {}ms", hist.p99, threshold_ms)));
                    }
                }
                None
            }
        }
    }
    
    fn create_alert(&self, rule: &AlertRule, message: &str) -> Alert {
        Alert {
            id: Uuid::new_v4().to_string(),
            rule_name: rule.name.clone(),
            severity: rule.severity,
            message: message.to_string(),
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        }
    }
    
    /// 获取最近的告警
    pub async fn get_recent_alerts(&self, limit: usize) -> Vec<Alert> {
        let alerts = self.alerts.read().await;
        alerts.iter().take(limit).cloned().collect()
    }
}

/// 实时可观测性系统
pub struct ObservabilitySystem {
    tracer: Arc<Tracer>,
    metrics: Arc<MetricsCollector>,
    alerts: Arc<AlertManager>,
}

impl ObservabilitySystem {
    pub fn new(service_name: &str) -> Self {
        let metrics = Arc::new(MetricsCollector::new());
        
        Self {
            tracer: Arc::new(Tracer::new(service_name.to_string(), 10000)),
            metrics: metrics.clone(),
            alerts: Arc::new(AlertManager::new(metrics)),
        }
    }
    
    /// 获取追踪器
    pub fn tracer(&self) -> &Arc<Tracer> {
        &self.tracer
    }
    
    /// 获取指标收集器
    pub fn metrics(&self) -> &Arc<MetricsCollector> {
        &self.metrics
    }
    
    /// 获取告警管理器
    pub fn alerts(&self) -> &Arc<AlertManager> {
        &self.alerts
    }
    
    /// 记录请求
    pub fn record_request(&self, endpoint: &str, duration_ms: u64, status: &str) {
        self.metrics.inc_counter("requests", 1);
        self.metrics.inc_counter(&format!("requests.{}", status), 1);
        self.metrics.record_histogram("latency_ms", duration_ms as f64);
        
        if status == "error" {
            self.metrics.inc_counter("errors", 1);
        }
    }
}

/// 全局实例
use std::sync::OnceLock;
static OBSERVABILITY: OnceLock<ObservabilitySystem> = OnceLock::new();

pub fn init_observability(service_name: &str) -> &'static ObservabilitySystem {
    OBSERVABILITY.get_or_init(|| ObservabilitySystem::new(service_name))
}

pub fn get_observability() -> Option<&'static ObservabilitySystem> {
    OBSERVABILITY.get()
}