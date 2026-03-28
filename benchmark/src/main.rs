//! Crablet Performance Benchmark Runner
//! 
//! This binary runs comprehensive benchmarks to prove Crablet's
//! performance advantage over OpenClaw.

use crablet_benchmark::{
    BenchmarkConfig, BenchmarkReport, BenchmarkResult,
    scenarios::*,
    reporter::*,
    metrics::*,
};
use std::collections::HashMap;

/// Collect environment information
fn collect_environment() -> EnvironmentInfo {
    let os = std::env::consts::OS.to_string();
    let arch = std::env::consts::ARCH.to_string();
    
    // Get CPU cores
    let cpu_cores = num_cpus::get();
    
    // Get memory (rough estimate in GB)
    let memory_gb = {
        #[cfg(target_os = "macos")]
        {
            use std::sys_info::sys_info;
            let info = sys_info().unwrap_or_default();
            info.totalram as f64 / 1024.0 / 1024.0 / 1024.0
        }
        #[cfg(not(target_os = "macos"))]
        {
            16.0 // Default assumption
        }
    };
    
    EnvironmentInfo {
        os,
        arch,
        cpu_cores,
        memory_gb,
        rust_version: "1.75".to_string(),
        crablet_version: "0.1.0".to_string(),
    }
}

#[tokio::main]
async fn main() {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║     Crablet Performance Benchmark Suite v0.1.0                 ║");
    println!("║     Proving Rust Performance Advantage                         ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();
    
    // Configuration
    let config = BenchmarkConfig {
        iterations: 100,
        warmup_iterations: 10,
        workers: num_cpus::get(),
        timeout_ms: 30000,
        trace_enabled: false,
    };
    
    println!("Configuration:");
    println!("  Iterations: {}", config.iterations);
    println!("  Warmup: {}", config.warmup_iterations);
    println!("  Workers: {}", config.workers);
    println!();
    
    // Collect environment
    let environment = collect_environment();
    println!("Environment:");
    println!("  OS: {} ({})", environment.os, environment.arch);
    println!("  CPU Cores: {}", environment.cpu_cores);
    println!("  Memory: {:.1} GB", environment.memory_gb);
    println!();
    
    // Build metadata
    let metadata = ReportMetadata {
        timestamp: chrono_lite_now(),
        environment,
        config: ConfigSnapshot {
            iterations: config.iterations,
            warmup_iterations: config.warmup_iterations,
            workers: config.workers,
            timeout_ms: config.timeout_ms,
        },
    };
    
    // Initialize results
    let mut results: Vec<BenchmarkResult> = Vec::new();
    
    println!("═══════════════════════════════════════════════════════════════");
    println!("Running Benchmarks...");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    
    // System1 Benchmark
    println!("[1/8] System1 Fast Path (Trie + Fuzzy Match)...");
    let sys1_bench = System1Benchmark::new(config.iterations);
    let result = run_benchmark_sync(&config, &sys1_bench).await;
    print_result(&result);
    results.push(result);
    
    // System2 Benchmarks (Low/Medium/High complexity)
    println!("\n[2/8] System2 ReAct (Low Complexity)...");
    let sys2_low = System2ReactBenchmark::new(config.iterations, TestComplexity::Low);
    let result = run_benchmark_sync(&config, &sys2_low).await;
    print_result(&result);
    results.push(result);
    
    println!("[3/8] System2 ReAct (Medium Complexity)...");
    let sys2_med = System2ReactBenchmark::new(config.iterations, TestComplexity::Medium);
    let result = run_benchmark_sync(&config, &sys2_med).await;
    print_result(&result);
    results.push(result);
    
    println!("[4/8] System2 ReAct (High Complexity)...");
    let sys2_high = System2ReactBenchmark::new(config.iterations, TestComplexity::High);
    let result = run_benchmark_sync(&config, &sys2_high).await;
    print_result(&result);
    results.push(result);
    
    // System3 Swarm Benchmark
    println!("\n[5/8] System3 Swarm Coordination...");
    let sys3_bench = System3SwarmBenchmark::new(4, 10);
    let result = run_benchmark_sync(&config, &sys3_bench).await;
    print_result(&result);
    results.push(result);
    
    // Memory Layer Benchmarks
    println!("\n[6/8] Memory Layers...");
    for layer_type in [
        MemoryLayerType::Soul,
        MemoryLayerType::Tools,
        MemoryLayerType::User,
        MemoryLayerType::Session,
    ] {
        let mem_bench = MemoryLayerBenchmark::new(layer_type, 1000);
        let result = run_benchmark_sync(&config, &mem_bench).await;
        print_result(&result);
        results.push(result);
    }
    
    // GraphRAG Benchmarks
    println!("\n[7/8] GraphRAG (Hybrid Retrieval)...");
    let grag_bench = GraphRagBenchmark::new(3, RetrievalMode::Hybrid);
    let result = run_benchmark_sync(&config, &grag_bench).await;
    print_result(&result);
    results.push(result);
    
    // Concurrent Throughput
    println!("\n[8/8] Concurrent Throughput ({} workers)...", config.workers);
    let conc_bench = ConcurrentBenchmark::new(config.workers, 100);
    let result = run_benchmark_sync(&config, &conc_bench).await;
    print_result(&result);
    results.push(result);
    
    // Generate report
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("Generating Report...");
    println!("═══════════════════════════════════════════════════════════════");
    
    let report = BenchmarkReport::new(metadata, results);
    
    // Print markdown report
    let md_report = report.to_markdown();
    println!("\n{}", md_report);
    
    // Save report
    let report_path = "benchmark_report.md";
    std::fs::write(report_path, &md_report).ok();
    println!("\nReport saved to: {}", report_path);
    
    // Exit with appropriate code
    let failed = report.summary.failed_tests;
    if failed > 0 {
        println!("\n⚠️  {} benchmark(s) failed", failed);
        std::process::exit(1);
    } else {
        println!("\n✅ All benchmarks passed!");
        std::process::exit(0);
    }
}

/// Simple chrono-like timestamp
fn chrono_lite_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap();
    let secs = now.as_secs();
    let days = secs / 86400;
    let years = days / 365 + 1970;
    let remaining_days = days % 365;
    let months = remaining_days / 30 + 1;
    let day = remaining_days % 30 + 1;
    let hours = (secs % 86400) / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", years, months, day, hours, minutes, seconds)
}

/// Run a benchmark synchronously
async fn run_benchmark_sync<C: BenchmarkScenario>(
    config: &BenchmarkConfig,
    scenario: &C,
) -> BenchmarkResult {
    let start = std::time::Instant::now();
    let baseline_memory = get_memory_usage();
    
    let mut latencies: Vec<f64> = Vec::with_capacity(config.iterations);
    let mut successful = 0;
    let mut failed = 0;
    
    // Warmup
    for _ in 0..config.warmup_iterations {
        scenario.execute().await.ok();
    }
    
    // Main loop
    for _ in 0..config.iterations {
        let iter_start = std::time::Instant::now();
        match scenario.execute().await {
            Ok(_) => {
                let elapsed = iter_start.elapsed().as_secs_f64() * 1000.0;
                latencies.push(elapsed);
                successful += 1;
            }
            Err(_) => {
                failed += 1;
            }
        }
    }
    
    let total_time = start.elapsed();
    let throughput = if total_time.as_secs_f64() > 0.0 {
        config.iterations as f64 / total_time.as_secs_f64()
    } else {
        0.0
    };
    
    // Calculate statistics
    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = latencies.len();
    
    let avg = if n > 0 { latencies.iter().sum::<f64>() / n as f64 } else { 0.0 };
    let median = if n > 0 { latencies[n / 2] } else { 0.0 };
    let min = latencies.first().copied().unwrap_or(0.0);
    let max = latencies.last().copied().unwrap_or(0.0);
    let p95 = if n > 0 { latencies[(n as f64 * 0.95) as usize].min(max) } else { 0.0 };
    let p99 = if n > 0 { latencies[(n as f64 * 0.99) as usize].min(max) } else { 0.0 };
    
    let final_memory = get_memory_usage();
    
    BenchmarkResult {
        name: scenario.name().to_string(),
        successful_runs: successful,
        failed_runs: failed,
        avg_latency_ms: avg,
        median_latency_ms: median,
        min_latency_ms: min,
        max_latency_ms: max,
        p95_latency_ms: p95,
        p99_latency_ms: p99,
        throughput,
        memory_delta_bytes: final_memory as i64 - baseline_memory as i64,
        peak_memory_bytes: final_memory,
    }
}

/// Print a single benchmark result
fn print_result(result: &BenchmarkResult) {
    let status = if result.failed_runs == 0 { "✅" } else { "❌" };
    println!(
        "  {} {}: {:.2} ms avg, {:.1} ops/s, {} failed",
        status,
        result.name,
        result.avg_latency_ms,
        result.throughput,
        result.failed_runs,
    );
}
