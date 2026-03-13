//! Testing Framework - 测试框架
//!
//! 提供全面的单元测试、集成测试和性能测试支持

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::time::timeout;

/// 测试结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    pub duration_ms: u64,
    pub message: Option<String>,
    pub assertions: Vec<AssertionResult>,
}

/// 断言结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    pub description: String,
    pub passed: bool,
    pub expected: String,
    pub actual: String,
}

/// 测试套件
pub struct TestSuite {
    name: String,
    tests: Vec<TestCase>,
    setup: Option<Box<dyn Fn() -> Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync>>,
    teardown: Option<Box<dyn Fn() -> Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync>>,
}

/// 测试用例
pub struct TestCase {
    name: String,
    test_fn: Box<dyn Fn() -> Pin<Box<dyn Future<Output = TestResult> + Send>> + Send + Sync>,
    timeout: Duration,
    skip: bool,
}

impl TestSuite {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tests: Vec::new(),
            setup: None,
            teardown: None,
        }
    }

    pub fn with_setup<F, Fut>(mut self, setup: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.setup = Some(Box::new(move || Box::pin(setup())));
        self
    }

    pub fn with_teardown<F, Fut>(mut self, teardown: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.teardown = Some(Box::new(move || Box::pin(teardown())));
        self
    }

    pub fn add_test<F, Fut>(mut self, name: impl Into<String>, test_fn: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = TestResult> + Send + 'static,
    {
        self.tests.push(TestCase {
            name: name.into(),
            test_fn: Box::new(move || Box::pin(test_fn())),
            timeout: Duration::from_secs(30),
            skip: false,
        });
        self
    }

    pub fn add_test_with_timeout<F, Fut>(
        mut self,
        name: impl Into<String>,
        test_fn: F,
        timeout: Duration,
    ) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = TestResult> + Send + 'static,
    {
        self.tests.push(TestCase {
            name: name.into(),
            test_fn: Box::new(move || Box::pin(test_fn())),
            timeout,
            skip: false,
        });
        self
    }

    pub fn skip_test(mut self, name: impl Into<String> + Clone) -> Self {
        let name_str: String = name.into();
        if let Some(test) = self.tests.iter_mut().find(|t| t.name == name_str) {
            test.skip = true;
        }
        self
    }

    /// 运行所有测试
    pub async fn run(&self) -> TestSuiteResult {
        let start = Instant::now();
        let mut results = Vec::new();

        // 执行 setup
        if let Some(ref setup) = self.setup {
            if let Err(e) = setup().await {
                return TestSuiteResult {
                    suite_name: self.name.clone(),
                    results: vec![],
                    passed: false,
                    total_tests: 0,
                    passed_tests: 0,
                    failed_tests: 0,
                    skipped_tests: 0,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error: Some(format!("Setup failed: {}", e)),
                };
            }
        }

        // 执行测试
        for test in &self.tests {
            if test.skip {
                results.push(TestResult {
                    name: test.name.clone(),
                    passed: true,
                    duration_ms: 0,
                    message: Some("Skipped".to_string()),
                    assertions: vec![],
                });
                continue;
            }

            let _test_start = Instant::now();
            let result = match timeout(test.timeout, (test.test_fn)()).await {
                Ok(r) => r,
                Err(_) => TestResult {
                    name: test.name.clone(),
                    passed: false,
                    duration_ms: test.timeout.as_millis() as u64,
                    message: Some("Timeout".to_string()),
                    assertions: vec![],
                },
            };
            results.push(result);
        }

        // 执行 teardown
        if let Some(ref teardown) = self.teardown {
            let _ = teardown().await;
        }

        let passed_tests = results.iter().filter(|r| r.passed && r.message != Some("Skipped".to_string())).count();
        let failed_tests = results.iter().filter(|r| !r.passed).count();
        let skipped_tests = results.iter().filter(|r| r.message == Some("Skipped".to_string())).count();

        TestSuiteResult {
            suite_name: self.name.clone(),
            results,
            passed: failed_tests == 0,
            total_tests: self.tests.len(),
            passed_tests,
            failed_tests,
            skipped_tests,
            duration_ms: start.elapsed().as_millis() as u64,
            error: None,
        }
    }
}

/// 测试套件结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSuiteResult {
    pub suite_name: String,
    pub results: Vec<TestResult>,
    pub passed: bool,
    pub total_tests: usize,
    pub passed_tests: usize,
    pub failed_tests: usize,
    pub skipped_tests: usize,
    pub duration_ms: u64,
    pub error: Option<String>,
}

impl TestSuiteResult {
    pub fn print_report(&self) {
        println!("\n{:=^60}", format!(" Test Suite: {} ", self.suite_name));
        println!("Total: {} | Passed: {} | Failed: {} | Skipped: {}",
            self.total_tests, self.passed_tests, self.failed_tests, self.skipped_tests);
        println!("Duration: {}ms", self.duration_ms);
        
        if let Some(ref error) = self.error {
            println!("Error: {}", error);
        }

        for result in &self.results {
            let status = if result.message == Some("Skipped".to_string()) {
                "SKIP"
            } else if result.passed {
                "PASS"
            } else {
                "FAIL"
            };
            
            println!("  [{}] {} ({}ms)", status, result.name, result.duration_ms);
            
            if let Some(ref message) = result.message {
                if !result.passed || message != "Skipped" {
                    println!("       {}", message);
                }
            }

            for assertion in &result.assertions {
                let symbol = if assertion.passed { "✓" } else { "✗" };
                println!("       {} {}", symbol, assertion.description);
                if !assertion.passed {
                    println!("         Expected: {}", assertion.expected);
                    println!("         Actual:   {}", assertion.actual);
                }
            }
        }

        println!("{:=^60}\n", "");
    }
}

/// 测试构建器
pub struct TestBuilder {
    name: String,
    assertions: Vec<AssertionResult>,
    start_time: Instant,
}

impl TestBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            assertions: Vec::new(),
            start_time: Instant::now(),
        }
    }

    pub fn assert<T: PartialEq + std::fmt::Debug>(
        mut self,
        description: impl Into<String>,
        expected: T,
        actual: T,
    ) -> Self {
        let passed = expected == actual;
        self.assertions.push(AssertionResult {
            description: description.into(),
            passed,
            expected: format!("{:?}", expected),
            actual: format!("{:?}", actual),
        });
        self
    }

    pub fn assert_true(mut self, description: impl Into<String>, condition: bool) -> Self {
        self.assertions.push(AssertionResult {
            description: description.into(),
            passed: condition,
            expected: "true".to_string(),
            actual: condition.to_string(),
        });
        self
    }

    pub fn assert_false(mut self, description: impl Into<String>, condition: bool) -> Self {
        self.assertions.push(AssertionResult {
            description: description.into(),
            passed: !condition,
            expected: "false".to_string(),
            actual: condition.to_string(),
        });
        self
    }

    pub fn assert_ok<T, E: std::fmt::Debug>(
        mut self,
        description: impl Into<String>,
        result: Result<T, E>,
    ) -> Self {
        let passed = result.is_ok();
        self.assertions.push(AssertionResult {
            description: description.into(),
            passed,
            expected: "Ok".to_string(),
            actual: if let Err(e) = result {
                format!("Err({:?})", e)
            } else {
                "Ok".to_string()
            },
        });
        self
    }

    pub fn assert_err<T, E>(
        mut self,
        description: impl Into<String>,
        result: Result<T, E>,
    ) -> Self {
        let passed = result.is_err();
        self.assertions.push(AssertionResult {
            description: description.into(),
            passed,
            expected: "Err".to_string(),
            actual: if passed { "Err".to_string() } else { "Ok".to_string() },
        });
        self
    }

    pub fn build(self) -> TestResult {
        let passed = self.assertions.iter().all(|a| a.passed);
        TestResult {
            name: self.name,
            passed,
            duration_ms: self.start_time.elapsed().as_millis() as u64,
            message: None,
            assertions: self.assertions,
        }
    }
}

/// 性能测试
pub struct Benchmark {
    name: String,
    iterations: usize,
    warmup_iterations: usize,
}

impl Benchmark {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            iterations: 1000,
            warmup_iterations: 100,
        }
    }

    pub fn with_iterations(mut self, iterations: usize) -> Self {
        self.iterations = iterations;
        self
    }

    pub fn with_warmup(mut self, warmup: usize) -> Self {
        self.warmup_iterations = warmup;
        self
    }

    pub async fn run<F, Fut, T>(self, f: F) -> BenchmarkResult
    where
        F: Fn() -> Fut,
        Fut: Future<Output = T>,
    {
        // Warmup
        for _ in 0..self.warmup_iterations {
            let _ = f().await;
        }

        // Benchmark
        let mut durations = Vec::with_capacity(self.iterations);
        for _ in 0..self.iterations {
            let start = Instant::now();
            let _ = f().await;
            durations.push(start.elapsed());
        }

        // Calculate statistics
        durations.sort();
        let total: Duration = durations.iter().sum();
        let min = durations[0];
        let max = durations[durations.len() - 1];
        let median = durations[durations.len() / 2];
        let mean = total / self.iterations as u32;

        let p95_idx = (self.iterations as f64 * 0.95) as usize;
        let p99_idx = (self.iterations as f64 * 0.99) as usize;
        let p95 = durations[p95_idx.min(durations.len() - 1)];
        let p99 = durations[p99_idx.min(durations.len() - 1)];

        BenchmarkResult {
            name: self.name,
            iterations: self.iterations,
            total_duration_ms: total.as_millis() as u64,
            min_ms: min.as_micros() as f64 / 1000.0,
            max_ms: max.as_micros() as f64 / 1000.0,
            mean_ms: mean.as_micros() as f64 / 1000.0,
            median_ms: median.as_micros() as f64 / 1000.0,
            p95_ms: p95.as_micros() as f64 / 1000.0,
            p99_ms: p99.as_micros() as f64 / 1000.0,
            throughput_per_sec: self.iterations as f64 / total.as_secs_f64(),
        }
    }
}

/// 性能测试结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub name: String,
    pub iterations: usize,
    pub total_duration_ms: u64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub mean_ms: f64,
    pub median_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub throughput_per_sec: f64,
}

impl BenchmarkResult {
    pub fn print_report(&self) {
        println!("\n{:=^60}", format!(" Benchmark: {} ", self.name));
        println!("Iterations: {}", self.iterations);
        println!("Total Duration: {}ms", self.total_duration_ms);
        println!("Throughput: {:.2} ops/sec", self.throughput_per_sec);
        println!("\nLatency Statistics:");
        println!("  Min:    {:.3}ms", self.min_ms);
        println!("  Max:    {:.3}ms", self.max_ms);
        println!("  Mean:   {:.3}ms", self.mean_ms);
        println!("  Median: {:.3}ms", self.median_ms);
        println!("  P95:    {:.3}ms", self.p95_ms);
        println!("  P99:    {:.3}ms", self.p99_ms);
        println!("{:=^60}\n", "");
    }
}

/// 模拟对象宏
#[macro_export]
macro_rules! mock {
    ($trait:ident, $method:ident, $return_type:ty) => {{
        use std::sync::Arc;
        use tokio::sync::Mutex;
        
        struct Mock$trait {
            calls: Arc<Mutex<Vec<Vec<Box<dyn std::any::Any + Send>>>>>,
            return_values: Arc<Mutex<Vec<$return_type>>>,
        }
        
        impl Mock$trait {
            fn new() -> Self {
                Self {
                    calls: Arc::new(Mutex::new(Vec::new())),
                    return_values: Arc::new(Mutex::new(Vec::new())),
                }
            }
            
            async fn when(&self, return_value: $return_type) {
                self.return_values.lock().await.push(return_value);
            }
            
            async fn call_count(&self) -> usize {
                self.calls.lock().await.len()
            }
        }
        
        Mock$trait::new()
    }};
}

/// 测试夹具
pub struct TestFixture<T> {
    data: T,
    cleanup: Option<Box<dyn FnOnce(&T)>>,
}

impl<T> TestFixture<T> {
    pub fn new(data: T) -> Self {
        Self {
            data,
            cleanup: None,
        }
    }

    pub fn with_cleanup<F>(mut self, cleanup: F) -> Self
    where
        F: FnOnce(&T) + 'static,
    {
        self.cleanup = Some(Box::new(cleanup));
        self
    }

    pub fn get(&self) -> &T {
        &self.data
    }

    pub fn get_mut(&mut self) -> &mut T {
        &mut self.data
    }
}

impl<T> Drop for TestFixture<T> {
    fn drop(&mut self) {
        if let Some(cleanup) = self.cleanup.take() {
            cleanup(&self.data);
        }
    }
}

/// 并发测试
pub struct ConcurrencyTest {
    name: String,
    concurrency: usize,
    iterations: usize,
}

impl ConcurrencyTest {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            concurrency: 10,
            iterations: 100,
        }
    }

    pub fn with_concurrency(mut self, concurrency: usize) -> Self {
        self.concurrency = concurrency;
        self
    }

    pub fn with_iterations(mut self, iterations: usize) -> Self {
        self.iterations = iterations;
        self
    }

    pub async fn run<F, Fut, T>(self, f: F) -> ConcurrencyTestResult
    where
        F: Fn() -> Fut + Send + Sync + Clone + 'static,
        Fut: Future<Output = Result<T>> + Send,
        T: Send,
    {
        use tokio::task::JoinSet;
        
        let start = Instant::now();
        let mut join_set = JoinSet::new();
        let f = Arc::new(f);

        for _ in 0..self.concurrency {
            let f = f.clone();
            join_set.spawn(async move {
                let mut successes = 0;
                let mut failures = 0;
                
                for _ in 0..self.iterations {
                    match f().await {
                        Ok(_) => successes += 1,
                        Err(_) => failures += 1,
                    }
                }
                
                (successes, failures)
            });
        }

        let mut total_successes = 0;
        let mut total_failures = 0;

        while let Some(result) = join_set.join_next().await {
            if let Ok((successes, failures)) = result {
                total_successes += successes;
                total_failures += failures;
            }
        }

        let duration = start.elapsed();
        let total_ops = total_successes + total_failures;

        ConcurrencyTestResult {
            name: self.name,
            concurrency: self.concurrency,
            iterations: self.iterations,
            total_operations: total_ops,
            successes: total_successes,
            failures: total_failures,
            duration_ms: duration.as_millis() as u64,
            ops_per_sec: total_ops as f64 / duration.as_secs_f64(),
            success_rate: total_successes as f64 / total_ops as f64,
        }
    }
}

/// 并发测试结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrencyTestResult {
    pub name: String,
    pub concurrency: usize,
    pub iterations: usize,
    pub total_operations: usize,
    pub successes: usize,
    pub failures: usize,
    pub duration_ms: u64,
    pub ops_per_sec: f64,
    pub success_rate: f64,
}

impl ConcurrencyTestResult {
    pub fn print_report(&self) {
        println!("\n{:=^60}", format!(" Concurrency Test: {} ", self.name));
        println!("Concurrency: {}", self.concurrency);
        println!("Iterations per worker: {}", self.iterations);
        println!("Total Operations: {}", self.total_operations);
        println!("Successes: {} | Failures: {}", self.successes, self.failures);
        println!("Success Rate: {:.2}%", self.success_rate * 100.0);
        println!("Duration: {}ms", self.duration_ms);
        println!("Throughput: {:.2} ops/sec", self.ops_per_sec);
        println!("{:=^60}\n", "");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_test_builder() {
        let result = TestBuilder::new("example test")
            .assert_true("true is true", true)
            .assert("equality", 42, 42)
            .assert_ok("result is ok", Ok::<_, ()>("success"))
            .build();

        assert!(result.passed);
        assert_eq!(result.assertions.len(), 3);
    }

    #[tokio::test]
    async fn test_test_suite() {
        let suite = TestSuite::new("example suite")
            .add_test("test 1", || async {
                TestBuilder::new("test 1")
                    .assert_true("pass", true)
                    .build()
            })
            .add_test("test 2", || async {
                TestBuilder::new("test 2")
                    .assert("fail", 1, 2)
                    .build()
            });

        let result = suite.run().await;
        assert_eq!(result.total_tests, 2);
        assert_eq!(result.passed_tests, 1);
        assert_eq!(result.failed_tests, 1);
    }

    #[tokio::test]
    async fn test_benchmark() {
        let result = Benchmark::new("example benchmark")
            .with_iterations(100)
            .with_warmup(10)
            .run(|| async {
                // 模拟工作
                tokio::time::sleep(Duration::from_micros(100)).await;
            })
            .await;

        assert_eq!(result.iterations, 100);
        assert!(result.mean_ms > 0.0);
    }
}
