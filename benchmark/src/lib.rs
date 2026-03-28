// Crablet Performance Benchmark Suite
// P0-1: Establish Rust performance advantage over OpenClaw

pub mod scenarios;
pub mod metrics;
pub mod reporter;

use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

/// Benchmark configuration
#[derive(Debug, Clone, Deserialize)]
pub struct BenchmarkConfig {
    /// Number of iterations per test
    pub iterations: usize,
    /// Warmup iterations (results discarded)
    pub warmup_iterations: usize,
    /// Concurrent workers for parallel tests
    pub workers: usize,
    /// Timeout per operation (ms)
    pub timeout_ms: u64,
    /// Enable detailed tracing
    pub trace_enabled: bool,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            iterations: 100,
            warmup_iterations: 10,
            workers: 4,
            timeout_ms: 30000,
            trace_enabled: false,
        }
    }
}

/// Benchmark result for a single test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// Test name
    pub name: String,
    /// Number of successful runs
    pub successful_runs: usize,
    /// Number of failed runs
    pub failed_runs: usize,
    /// Average latency (ms)
    pub avg_latency_ms: f64,
    /// Median latency (ms)
    pub median_latency_ms: f64,
    /// Min latency (ms)
    pub min_latency_ms: f64,
    /// Max latency (ms)
    pub max_latency_ms: f64,
    /// P95 latency (ms)
    pub p95_latency_ms: f64,
    /// P99 latency (ms)
    pub p99_latency_ms: f64,
    /// Throughput (ops/sec)
    pub throughput: f64,
    /// Memory delta (bytes) - positive means increase
    pub memory_delta_bytes: i64,
    /// Peak memory (bytes)
    pub peak_memory_bytes: u64,
}

impl BenchmarkResult {
    pub fn new(name: String) -> Self {
        Self {
            name,
            successful_runs: 0,
            failed_runs: 0,
            avg_latency_ms: 0.0,
            median_latency_ms: 0.0,
            min_latency_ms: f64::MAX,
            max_latency_ms: 0.0,
            p95_latency_ms: 0.0,
            p99_latency_ms: 0.0,
            throughput: 0.0,
            memory_delta_bytes: 0,
            peak_memory_bytes: 0,
        }
    }
}

/// Run a benchmark scenario and collect metrics
pub async fn run_benchmark<S: BenchmarkScenario>(
    config: &BenchmarkConfig,
    scenario: &S,
) -> BenchmarkResult {
    let mut result = BenchmarkResult::new(scenario.name().to_string());
    
    // Memory measurement setup
    let baseline_memory = metrics::get_memory_usage();
    
    // Warmup phase
    for _ in 0..config.warmup_iterations {
        scenario.execute().await.ok();
    }
    
    // Main benchmark loop
    let mut latencies: Vec<f64> = Vec::with_capacity(config.iterations);
    
    for i in 0..config.iterations {
        let start = Instant::now();
        let exec_start = Instant::now();
        
        match scenario.execute().await {
            Ok(_) => {
                let elapsed = start.elapsed();
                let latency_ms = elapsed.as_secs_f64() * 1000.0;
                latencies.push(latency_ms);
                result.successful_runs += 1;
            }
            Err(e) => {
                result.failed_runs += 1;
                eprintln!("Benchmark iteration {} failed: {:?}", i, e);
            }
        }
        
        // Throughput calculation
        let total_time = exec_start.elapsed();
        if total_time.as_secs_f64() > 0.0 {
            result.throughput = (i + 1) as f64 / total_time.as_secs_f64();
        }
    }
    
    // Calculate statistics
    if !latencies.is_empty() {
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = latencies.len();
        
        result.avg_latency_ms = latencies.iter().sum::<f64>() / n as f64;
        result.median_latency_ms = latencies[n / 2];
        result.min_latency_ms = latencies.first().copied().unwrap_or(0.0);
        result.max_latency_ms = latencies.last().copied().unwrap_or(0.0);
        result.p95_latency_ms = latencies[(n as f64 * 0.95) as usize].min(result.max_latency_ms);
        result.p99_latency_ms = latencies[(n as f64 * 0.99) as usize].min(result.max_latency_ms);
    }
    
    // Memory measurements
    let final_memory = metrics::get_memory_usage();
    result.memory_delta_bytes = final_memory as i64 - baseline_memory as i64;
    result.peak_memory_bytes = final_memory;
    
    result
}

/// Trait for benchmark scenarios
#[async_trait::async_trait]
pub trait BenchmarkScenario: Send + Sync {
    /// Scenario name
    fn name(&self) -> &str;
    
    /// Execute a single iteration
    async fn execute(&self) -> anyhow::Result<()>;
}

pub use scenarios::*;
pub use metrics::*;
pub use reporter::*;
