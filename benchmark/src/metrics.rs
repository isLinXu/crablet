// Performance Metrics Collection Module

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Memory usage tracking
pub fn get_memory_usage() -> u64 {
    // On macOS, use process stats
    #[cfg(target_os = "macos")]
    {
        use std::sys_info::proc_total_memory;
        proc_total_memory().unwrap_or(0)
    }
    
    // Fallback: estimate from resident set
    #[cfg(not(target_os = "macos"))]
    {
        // Default fallback
        0
    }
}

/// CPU usage measurement
#[derive(Debug, Clone, Default)]
pub struct CpuMetrics {
    pub user_time_ms: u64,
    pub system_time_ms: u64,
    pub total_time_ms: u64,
}

/// Throughput counter for concurrent operations
#[derive(Debug)]
pub struct ThroughputCounter {
    count: AtomicU64,
    start: Instant,
}

impl ThroughputCounter {
    pub fn new() -> Self {
        Self {
            count: AtomicU64::new(0),
            start: Instant::now(),
        }
    }
    
    pub fn increment(&self) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn get_throughput(&self) -> f64 {
        let elapsed = self.start.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.count.load(Ordering::Relaxed) as f64 / elapsed
        } else {
            0.0
        }
    }
    
    pub fn get_total(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }
}

/// Latency histogram for distribution analysis
#[derive(Debug, Clone, Default)]
pub struct LatencyHistogram {
    buckets: Vec<u64>,
    count: u64,
    sum: f64,
    min: f64,
    max: f64,
}

impl LatencyHistogram {
    pub fn new() -> Self {
        // Common latency buckets in milliseconds
        let buckets = vec![0u64; 21]; // 0-1, 1-2, 2-5, 5-10, 10-20, 20-50, 50-100, 100-200, 200-500, 500-1000ms+
        Self {
            buckets,
            count: 0,
            sum: 0.0,
            min: f64::MAX,
            max: 0.0,
        }
    }
    
    pub fn record(&mut self, latency_ms: f64) {
        self.count += 1;
        self.sum += latency_ms;
        self.min = self.min.min(latency_ms);
        self.max = self.max.max(latency_ms);
        
        // Bucket index calculation
        let bucket = match latency_ms as u64 {
            0 => 0,
            1 => 1,
            2...4 => 2,
            5...9 => 3,
            10...19 => 4,
            20...49 => 5,
            50...99 => 6,
            100...199 => 7,
            200...499 => 8,
            500...999 => 9,
            _ => 10,
        };
        
        if let Some(b) = self.buckets.get(bucket) {
            self.buckets[bucket] = b + 1;
        }
    }
    
    pub fn avg_ms(&self) -> f64 {
        if self.count > 0 {
            self.sum / self.count as f64
        } else {
            0.0
        }
    }
    
    pub fn percentiles(&self) -> (f64, f64, f64, f64) {
        // Returns (p50, p90, p95, p99)
        if self.count == 0 {
            return (0.0, 0.0, 0.0, 0.0);
        }
        
        let p50 = self.percentile(0.50);
        let p90 = self.percentile(0.90);
        let p95 = self.percentile(0.95);
        let p99 = self.percentile(0.99);
        
        (p50, p90, p95, p99)
    }
    
    fn percentile(&self, p: f64) -> f64 {
        // Approximation using histogram
        let target = (self.count as f64 * p) as u64;
        let mut cumulative = 0u64;
        
        for (i, &count) in self.buckets.iter().enumerate() {
            cumulative += count;
            if cumulative >= target {
                // Return bucket midpoint
                return match i {
                    0 => 0.5,
                    1 => 1.5,
                    2 => 3.0,
                    3 => 7.0,
                    4 => 15.0,
                    5 => 35.0,
                    6 => 75.0,
                    7 => 150.0,
                    8 => 350.0,
                    9 => 750.0,
                    _ => 1000.0,
                };
            }
        }
        
        self.max
    }
}

/// Resource usage snapshot
#[derive(Debug, Clone)]
pub struct ResourceSnapshot {
    pub timestamp_ms: u64,
    pub memory_bytes: u64,
    pub cpu_user_ms: u64,
    pub cpu_system_ms: u64,
}

impl ResourceSnapshot {
    pub fn capture() -> Self {
        Self {
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            memory_bytes: get_memory_usage(),
            cpu_user_ms: 0,
            cpu_system_ms: 0,
        }
    }
}

/// Performance counter for operations
#[derive(Debug)]
pub struct PerfCounter {
    name: String,
    start: Instant,
    iterations: AtomicU64,
}

impl PerfCounter {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            start: Instant::now(),
            iterations: AtomicU64::new(0),
        }
    }
    
    pub fn record(&self) {
        self.iterations.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
    
    pub fn ops_per_sec(&self) -> f64 {
        let elapsed = self.start.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.iterations.load(Ordering::Relaxed) as f64 / elapsed
        } else {
            0.0
        }
    }
}
