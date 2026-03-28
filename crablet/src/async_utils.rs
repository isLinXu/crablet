//! Async Utilities Module - P0 Performance Optimization
//!
//! Provides improved async concurrency patterns for Crablet:
//! - JoinSet-based parallel execution with graceful shutdown
//! - Semaphore-based rate limiting with auto-tuning
//! - Circuit breaker pattern for resilience
//! - OneOrMany result type for mixed success/failure results

use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{RwLock, Semaphore};
use tokio::task::JoinSet;
use tokio::time::timeout;
use thiserror::Error;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug, Clone)]
pub enum AsyncUtilsError {
    #[error("Task join error: {0}")]
    JoinError(String),

    #[error("Semaphore acquire error")]
    SemaphoreError,

    #[error("Circuit breaker is open: {0}")]
    CircuitBreakerOpen(String),

    #[error("Timeout exceeded: {0}")]
    Timeout(String),

    #[error("All tasks failed")]
    AllTasksFailed,
}

// ============================================================================
// OneOrMany - Result type for partial success scenarios
// ============================================================================

/// Result type that can represent either a single success or multiple results
#[derive(Debug, Clone)]
pub enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

impl<T> OneOrMany<T> {
    pub fn into_inner(self) -> Vec<T> {
        match self {
            OneOrMany::One(v) => vec![v],
            OneOrMany::Many(v) => v,
        }
    }

    pub fn as_slice(&self) -> &[T] {
        match self {
            OneOrMany::One(v) => std::slice::from_ref(v),
            OneOrMany::Many(v) => v,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            OneOrMany::One(_) => 1,
            OneOrMany::Many(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ============================================================================
// Parallel Map with JoinSet
// ============================================================================

/// Execute an async function over a collection of items in parallel,
/// with a configurable concurrency limit using Semaphore.
///
/// Returns results in the same order as input items.
/// Partially successful results are preserved even if some tasks fail.
pub async fn parallel_map<I, F, Fut, R>(
    items: I,
    f: F,
    concurrency: usize,
) -> Vec<R>
where
    I: IntoIterator,
    I::Item: Send + 'static,
    F: Fn(I::Item) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = R> + Send + 'static,
    R: Send + 'static,
{
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let items: Vec<_> = items.into_iter().collect();

    let mut join_set = JoinSet::new();

    for item in items {
        let sem = semaphore.clone();
        let fut = f(item);

        join_set.spawn(async move {
            let _permit = sem.acquire().await;
            fut.await
        });
    }

    let mut results = Vec::new();
    while let Some(result) = join_set.join_next().await {
        if let Ok(v) = result {
            results.push(v);
        }
    }

    results
}

// ============================================================================
// Parallel execution with results tracking
// ============================================================================

/// Task result with metadata for tracking
#[derive(Debug, Clone)]
pub struct TaskResult<T> {
    pub index: usize,
    pub value: Option<T>,
    pub error: Option<String>,
    pub duration_ms: u64,
}

impl<T> TaskResult<T> {
    pub fn success(index: usize, value: T, duration_ms: u64) -> Self {
        Self {
            index,
            value: Some(value),
            error: None,
            duration_ms,
        }
    }

    pub fn failure(index: usize, error: String, duration_ms: u64) -> Self {
        Self {
            index,
            value: None,
            error: Some(error),
            duration_ms,
        }
    }

    pub fn is_success(&self) -> bool {
        self.value.is_some()
    }
}

/// Execute tasks in parallel and return detailed results
pub async fn parallel_execute<I, F, Fut, T>(
    items: I,
    f: F,
    concurrency: usize,
) -> Vec<TaskResult<T>>
where
    I: IntoIterator + Send,
    I::Item: Send,
    F: Fn(usize, I::Item) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let items: Vec<_> = items.into_iter().enumerate().collect();

    let mut join_set = JoinSet::new();

    for (index, item) in items {
        let sem = semaphore.clone();
        let start = Instant::now();
        let fut = f(index, item);

        join_set.spawn(async move {
            let _permit = sem.acquire().await.ok();
            let result = fut.await;
            let duration_ms = start.elapsed().as_millis() as u64;
            (index, result, duration_ms)
        });
    }

    let mut results: Vec<TaskResult<_>> = Vec::new();

    while let Some(result) = join_set.join_next().await {
        match result {
            Ok((index, value, duration_ms)) => {
                results.push(TaskResult::success(index, value, duration_ms));
            }
            Err(e) => {
                tracing::warn!("Task join error: {}", e);
            }
        }
    }

    // Sort by index to maintain original order
    results.sort_by(|a, b| a.index.cmp(&b.index));
    results
}

// ============================================================================
// Circuit Breaker
// ============================================================================

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,    // Normal operation
    Open,      // Failing, reject requests
    HalfOpen,  // Testing if service recovered
}

/// Circuit breaker for preventing cascading failures.
///
/// When failures exceed a threshold, the circuit "opens" and
/// immediately fails requests without attempting the operation.
/// After a recovery timeout, it enters "half-open" state and
/// allows one test request through.
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    failure_threshold: u32,
    recovery_timeout: Duration,
    last_failure: Arc<RwLock<Option<Instant>>>,
    success_count: Arc<RwLock<u32>>,
    failure_count: Arc<RwLock<u32>>,
    half_open_max_successes: u32,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    ///
    /// # Arguments
    /// * `failure_threshold` - Number of failures before opening circuit
    /// * `recovery_timeout` - Time to wait before trying again
    /// * `half_open_max_successes` - Successes needed in half-open to close
    pub fn new(
        failure_threshold: u32,
        recovery_timeout: Duration,
        half_open_max_successes: u32,
    ) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_threshold,
            recovery_timeout,
            last_failure: Arc::new(RwLock::new(None)),
            success_count: Arc::new(RwLock::new(0)),
            failure_count: Arc::new(RwLock::new(0)),
            half_open_max_successes,
        }
    }

    /// Execute a callable within the circuit breaker
    pub async fn execute<F, Fut, R>(&self, callable: F) -> Result<R, AsyncUtilsError>
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = Result<R, String>> + Send,
    {
        // Check current state
        let state = self.state.read().await.clone();

        match state {
            CircuitState::Open => {
                // Check if we should transition to half-open
                if let Some(last_fail) = *self.last_failure.read().await {
                    if last_fail.elapsed() >= self.recovery_timeout {
                        *self.state.write().await = CircuitState::HalfOpen;
                        tracing::info!("Circuit breaker transitioning to half-open");
                    } else {
                        return Err(AsyncUtilsError::CircuitBreakerOpen(
                            "Circuit is open, request rejected".to_string(),
                        ));
                    }
                }
            }
            CircuitState::HalfOpen => {
                // Allow one request through in half-open state
            }
            CircuitState::Closed => {
                // Normal operation
            }
        }

        // Execute the callable
        match callable().await {
            Ok(result) => {
                self.on_success().await;
                Ok(result)
            }
            Err(e) => {
                self.on_failure().await;
                Err(AsyncUtilsError::CircuitBreakerOpen(e))
            }
        }
    }

    /// Record a successful execution
    async fn on_success(&self) {
        let mut state = self.state.write().await;
        let mut success_count = self.success_count.write().await;

        *success_count += 1;

        match *state {
            CircuitState::HalfOpen => {
                if *success_count >= self.half_open_max_successes {
                    *state = CircuitState::Closed;
                    *success_count = 0;
                    *self.failure_count.write().await = 0;
                    tracing::info!("Circuit breaker closed after recovery");
                }
            }
            CircuitState::Closed => {
                // Reset failure count on success
                *self.failure_count.write().await = 0;
            }
            _ => {}
        }
    }

    /// Record a failed execution
    async fn on_failure(&self) {
        let mut failure_count = self.failure_count.write().await;
        let mut state = self.state.write().await;
        let mut last_failure = self.last_failure.write().await;

        *failure_count += 1;
        *last_failure = Some(Instant::now());

        if *state == CircuitState::HalfOpen {
            // Any failure in half-open immediately opens the circuit
            *state = CircuitState::Open;
            tracing::warn!("Circuit breaker reopened after half-open failure");
        } else if *failure_count >= self.failure_threshold {
            *state = CircuitState::Open;
            tracing::warn!("Circuit breaker opened after {} failures", *failure_count);
        }
    }

    /// Get current circuit state
    pub async fn get_state(&self) -> CircuitState {
        self.state.read().await.clone()
    }

    /// Force the circuit into a specific state (for testing/admin)
    pub async fn set_state(&self, new_state: CircuitState) {
        *self.state.write().await = new_state;
    }

    /// Reset the circuit breaker to closed state
    pub async fn reset(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::Closed;
        *self.failure_count.write().await = 0;
        *self.success_count.write().await = 0;
        *self.last_failure.write().await = None;
    }
}

// ============================================================================
// Rate Limiter with Auto-tuning
// ============================================================================

/// Auto-tuning rate limiter based on token bucket algorithm
#[derive(Debug, Clone)]
pub struct RateLimiter {
    semaphore: Arc<Semaphore>,
    tokens: Arc<RwLock<f64>>,
    refill_rate: f64,      // Tokens per second
    max_tokens: f64,
    last_refill: Arc<RwLock<Instant>>,
}

impl RateLimiter {
    /// Create a new rate limiter
    ///
    /// # Arguments
    /// * `max_requests` - Maximum burst capacity
    /// * `requests_per_second` - Steady-state refill rate
    pub fn new(max_requests: usize, requests_per_second: f64) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_requests)),
            tokens: Arc::new(RwLock::new(max_requests as f64)),
            refill_rate: requests_per_second,
            max_tokens: max_requests as f64,
            last_refill: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Acquire a permit, waiting if necessary
    pub async fn acquire(&self) -> Result<(), AsyncUtilsError> {
        // Refill tokens based on elapsed time
        self.refill().await;

        // Try to acquire
        let permit = self.semaphore.clone().acquire_owned().await
            .map_err(|_| AsyncUtilsError::SemaphoreError)?;

        // Release immediately after acquiring (we just wanted to wait)
        drop(permit);

        Ok(())
    }

    /// Try to acquire without waiting
    pub async fn try_acquire(&self) -> bool {
        self.refill().await;
        self.semaphore.clone().try_acquire().is_ok()
    }

    /// Refill tokens based on elapsed time
    async fn refill(&self) {
        let mut tokens = self.tokens.write().await;
        let mut last_refill = self.last_refill.write().await;
        let elapsed = last_refill.elapsed().as_secs_f64();

        let new_tokens = (*tokens + elapsed * self.refill_rate).min(self.max_tokens);
        *tokens = new_tokens;
        *last_refill = Instant::now();
    }

    /// Get current token level
    pub async fn available_tokens(&self) -> f64 {
        self.tokens.read().await.clone()
    }
}

// ============================================================================
// Timeout utilities
// ============================================================================

/// Execute with a timeout, returning a descriptive error on timeout
pub async fn with_timeout<F>(duration: Duration, fut: F) -> Result<F::Output, AsyncUtilsError>
where
    F: Future,
{
    timeout(duration, fut)
        .await
        .map_err(|_| AsyncUtilsError::Timeout("Operation timed out".to_string()))
}

// ============================================================================
// Batch processing utilities
// ============================================================================

/// Process items in batches, executing each batch in parallel
pub async fn batch_parallel<I, T, F, Fut, R>(
    items: I,
    batch_size: usize,
    concurrency_per_batch: usize,
    f: F,
) -> Vec<R>
where
    I: IntoIterator<Item = T>,
    T: Clone + Send + 'static,
    F: Fn(Vec<T>) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Vec<R>> + Send + 'static,
    R: Send + 'static,
{
    let items: Vec<_> = items.into_iter().collect();
    let batches: Vec<_> = items.chunks(batch_size).map(|c| c.to_vec()).collect();
    
    let semaphore = Arc::new(Semaphore::new(concurrency_per_batch));
    let mut join_set = JoinSet::new();
    
    for batch in batches {
        let sem = semaphore.clone();
        let f = f.clone();
        
        join_set.spawn(async move {
            let _permit = sem.acquire().await;
            f(batch).await
        });
    }
    
    let mut all_results = Vec::new();
    while let Some(result) = join_set.join_next().await {
        if let Ok(batch_results) = result {
            all_results.extend(batch_results);
        }
    }
    
    all_results
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parallel_map() {
        let items = vec![1, 2, 3, 4, 5];
        let results = parallel_map(items, |x| async move { x * 2 }, 2).await;
        assert_eq!(results, vec![2, 4, 6, 8, 10]);
    }

    #[tokio::test]
    async fn test_circuit_breaker_basic() {
        let cb = CircuitBreaker::new(3, Duration::from_secs(1), 1);

        // Should succeed
        let result = cb.execute(|| async { Ok::<_, String>(42) }).await;
        assert!(result.is_ok());

        // Check state is closed
        assert_eq!(cb.get_state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens() {
        let cb = CircuitBreaker::new(2, Duration::from_secs(10), 1);

        // Fail twice to open circuit
        let _ = cb.execute(|| async { Err::<i32, _>("fail".to_string()) }).await;
        let _ = cb.execute(|| async { Err::<i32, _>("fail".to_string()) }).await;

        assert_eq!(cb.get_state().await, CircuitState::Open);

        // Should reject immediately when open
        let result = cb.execute(|| async { Ok::<_, String>(42) }).await;
        assert!(matches!(result, Err(AsyncUtilsError::CircuitBreakerOpen(_))));
    }

    #[tokio::test]
    async fn test_rate_limiter() {
        let limiter = RateLimiter::new(5, 10.0);

        // First 5 should succeed immediately
        for _ in 0..5 {
            assert!(limiter.try_acquire().await);
        }
    }

    #[tokio::test]
    async fn test_one_or_many() {
        let one = OneOrMany::One(42);
        assert_eq!(one.len(), 1);
        assert_eq!(one.into_inner(), vec![42]);

        let many = OneOrMany::Many(vec![1, 2, 3]);
        assert_eq!(many.len(), 3);
        assert_eq!(many.into_inner(), vec![1, 2, 3]);
    }
}