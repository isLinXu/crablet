//! Telemetry Module - 可观测性模块
//!
//! 提供指标收集、追踪和日志记录功能

pub mod metrics;
pub mod performance;

use tracing::{info, debug};
use std::time::Instant;

/// 计时器守卫 - 自动记录执行时间
pub struct TimerGuard {
    name: String,
    start: Instant,
}

impl TimerGuard {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            start: Instant::now(),
        }
    }
}

impl Drop for TimerGuard {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        debug!(target: "timing", "{} took {:?}", self.name, duration);
    }
}

/// 创建一个计时器守卫
#[macro_export]
macro_rules! timed_scope {
    ($name:expr) => {
        let _timer = $crate::telemetry::TimerGuard::new($name);
    };
}

/// 记录函数调用
#[macro_export]
macro_rules! trace_fn {
    ($name:expr) => {
        tracing::trace!(target: "function_calls", "Entering: {}", $name);
    };
}

/// 性能计数器
#[derive(Debug)]
pub struct PerformanceCounter {
    name: String,
    count: std::sync::atomic::AtomicU64,
    total_time_ms: std::sync::atomic::AtomicU64,
}

impl PerformanceCounter {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            count: std::sync::atomic::AtomicU64::new(0),
            total_time_ms: std::sync::atomic::AtomicU64::new(0),
        }
    }

    pub fn record(&self, duration_ms: u64) {
        self.count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.total_time_ms.fetch_add(duration_ms, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn get_stats(&self) -> (u64, f64) {
        let count = self.count.load(std::sync::atomic::Ordering::Relaxed);
        let total = self.total_time_ms.load(std::sync::atomic::Ordering::Relaxed);
        let avg = if count > 0 { total as f64 / count as f64 } else { 0.0 };
        (count, avg)
    }

    pub fn report(&self) {
        let (count, avg) = self.get_stats();
        info!(target: "performance", "{}: {} calls, avg {:.2}ms", self.name, count, avg);
    }
}

/// 日志级别过滤器
pub fn init_tracing(level: &str) {
    let _ = init_telemetry(level);
}

pub fn init_telemetry(log_level: &str) -> anyhow::Result<()> {
    #[cfg(feature = "telemetry")]
    {
        use opentelemetry::global;
        use opentelemetry_sdk::propagation::TraceContextPropagator;
        use opentelemetry_sdk::trace::{self, Sampler};
        use opentelemetry_sdk::Resource;
        use opentelemetry_sdk::runtime;
        use opentelemetry::KeyValue;
        use opentelemetry_otlp::WithExportConfig;
        use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Registry};

        // Set global propagator
        global::set_text_map_propagator(TraceContextPropagator::new());

        // Check if OTEL_EXPORTER_OTLP_ENDPOINT is set, if not, skip OTEL setup or use stdout
        let otlp_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok();
        
        // Env filter for logs
        let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| log_level.into());

        if let Some(endpoint) = otlp_endpoint {
            println!("Initializing OpenTelemetry with endpoint: {}", endpoint);
            
            // Initialize OTLP pipeline
            let tracer = opentelemetry_otlp::new_pipeline()
                .tracing()
                .with_exporter(
                    opentelemetry_otlp::new_exporter()
                        .tonic()
                        .with_endpoint(endpoint),
                )
                .with_trace_config(
                    trace::config()
                        .with_sampler(Sampler::AlwaysOn)
                        .with_resource(Resource::new(vec![
                            KeyValue::new("service.name", "crablet"),
                            KeyValue::new("service.version", "0.1.0"),
                        ])),
                )
                .install_batch(runtime::Tokio)?;

            // Create tracing layer
            let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

            // Standard stdout fmt layer
            let fmt_layer = tracing_subscriber::fmt::layer();

            // Register everything
            Registry::default()
                .with(env_filter)
                .with(fmt_layer)
                .with(telemetry)
                .init();
                
        } else {
            // Fallback to just stdout logging
            Registry::default()
                .with(env_filter)
                .with(tracing_subscriber::fmt::layer())
                .init();
        }
    }

    #[cfg(not(feature = "telemetry"))]
    {
        use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Registry};
        let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| log_level.into());
        Registry::default()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer())
            .init();
    }

    Ok(())
}

pub fn shutdown_telemetry() {
    #[cfg(feature = "telemetry")]
    opentelemetry::global::shutdown_tracer_provider();
}

/// 健康检查状态
#[derive(Debug, Clone)]
pub enum HealthStatus {
    Healthy,
    Degraded(String),
    Unhealthy(String),
}

impl HealthStatus {
    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthStatus::Healthy)
    }

    pub fn is_degraded(&self) -> bool {
        matches!(self, HealthStatus::Degraded(_))
    }

    pub fn is_unhealthy(&self) -> bool {
        matches!(self, HealthStatus::Unhealthy(_))
    }
}

/// 健康检查器
pub struct HealthChecker {
    checks: Vec<Box<dyn Fn() -> HealthStatus + Send + Sync>>,
}

impl std::fmt::Debug for HealthChecker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HealthChecker")
            .field("checks_count", &self.checks.len())
            .finish()
    }
}

impl HealthChecker {
    pub fn new() -> Self {
        Self { checks: Vec::new() }
    }

    pub fn add_check<F>(&mut self, check: F)
    where
        F: Fn() -> HealthStatus + Send + Sync + 'static,
    {
        self.checks.push(Box::new(check));
    }

    pub fn check_all(&self) -> Vec<(&str, HealthStatus)> {
        self.checks
            .iter()
            .enumerate()
            .map(|(_i, check)| ("check", check()))
            .collect()
    }

    pub fn overall_status(&self) -> HealthStatus {
        let results: Vec<_> = self.checks.iter().map(|c| c()).collect();
        
        if results.iter().any(|r| r.is_unhealthy()) {
            HealthStatus::Unhealthy("One or more checks failed".to_string())
        } else if results.iter().any(|r| r.is_degraded()) {
            HealthStatus::Degraded("One or more checks degraded".to_string())
        } else {
            HealthStatus::Healthy
        }
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status() {
        assert!(HealthStatus::Healthy.is_healthy());
        assert!(!HealthStatus::Degraded("test".to_string()).is_healthy());
        assert!(HealthStatus::Degraded("test".to_string()).is_degraded());
        assert!(HealthStatus::Unhealthy("test".to_string()).is_unhealthy());
    }

    #[test]
    fn test_performance_counter() {
        let counter = PerformanceCounter::new("test");
        counter.record(100);
        counter.record(200);
        
        let (count, avg) = counter.get_stats();
        assert_eq!(count, 2);
        assert_eq!(avg, 150.0);
    }
}
