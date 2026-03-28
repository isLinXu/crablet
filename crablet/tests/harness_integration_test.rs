//! Harness Subsystem Integration Tests
//!
//! Comprehensive tests for the harness execution context, fusion engine,
//! adaptive timeout, circuit breaker, and manager.

use crablet::agent::adaptive_harness::{
    AdaptiveTimeout, Breakpoint, BreakpointAction, BreakpointCondition, BreakpointContext,
    BreakpointManager, StepHistory,
};
use crablet::agent::distributed_harness::{
    BackendType, DistributedConfig, DistributedHarnessManager, HarnessBackend, InMemoryBackend,
};
use crablet::agent::harness::{
    parse_tool_calls, AgentHarnessContext, CircuitBreaker, CircuitBreakerConfig, CircuitState,
    HarnessConfig, HarnessError, HarnessSignal,
    HarnessSignalChannel, RetryConfig, ToolExecResult,
};
use crablet::agent::harness_fusion::{EngineState, UnifiedHarnessFusion, UnifiedHarnessFusionBuilder};
use crablet::agent::harness_manager::{HarnessInfo, HarnessManager, HarnessStatus};
use std::sync::Arc;
use std::time::Duration;

// ============================================================================
// Core Harness Context Tests
// ============================================================================

#[test]
fn test_harness_config_defaults() {
    let config = HarnessConfig::default();
    assert_eq!(config.max_steps, 10);
    assert_eq!(config.tool_timeout, Duration::from_secs(30));
    assert!(config.enable_self_reflection);
    assert!(config.circuit_breaker.is_none());
}

#[test]
fn test_harness_config_serialization_roundtrip() {
    let config = HarnessConfig {
        max_steps: 50,
        tool_timeout: Duration::from_secs(120),
        step_timeout: Duration::from_secs(300),
        enable_self_reflection: false,
        circuit_breaker: Some(CircuitBreakerConfig {
            failure_threshold: 5,
            success_threshold: 3,
            timeout: Duration::from_secs(60),
        }),
        max_memory_bytes: Some(512 * 1024 * 1024),
        max_cpu_time_ms: Some(120000),
        metadata: Default::default(),
    };

    let json = serde_json::to_string(&config).unwrap();
    let parsed: HarnessConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.max_steps, 50);
    assert_eq!(parsed.tool_timeout, Duration::from_secs(120));
    assert!(!parsed.enable_self_reflection);
    assert!(parsed.circuit_breaker.is_some());
    assert_eq!(
        parsed.circuit_breaker.as_ref().unwrap().failure_threshold,
        5
    );
}

#[tokio::test]
async fn test_harness_full_lifecycle() {
    let config = HarnessConfig {
        max_steps: 20,
        enable_self_reflection: true,
        ..Default::default()
    };

    let mut ctx = AgentHarnessContext::new(config);

    // Initial state
    assert!(ctx.can_continue());
    assert_eq!(ctx.remaining_steps(), 20);
    assert!(!ctx.should_stop());
    assert!(!ctx.is_paused());

    // Execute steps
    for i in 1..=5 {
        ctx.record_step();
        assert_eq!(ctx.remaining_steps(), 20 - i);
    }

    // Record errors
    ctx.record_error(HarnessError::Timeout(Duration::from_secs(5)));
    ctx.record_error(HarnessError::ToolFailure(
        "search".to_string(),
        "not found".to_string(),
    ));
    assert!(ctx.has_recent_errors(2));

    // Pause/Resume
    ctx.pause();
    assert!(ctx.is_paused());

    // wait_if_paused should complete since we immediately resume
    let pause_clone = &ctx as &AgentHarnessContext;
    pause_clone.resume();
    assert!(!pause_clone.is_paused());

    // Cancel
    ctx.cancel();
    assert!(ctx.should_stop());
    assert!(!ctx.can_continue());

    // Metadata check
    let meta = ctx.metadata();
    assert_eq!(meta.step_count, 5);
}

#[tokio::test]
async fn test_checkpoint_save_load() {
    let mut ctx = AgentHarnessContext::new(HarnessConfig {
        max_steps: 10,
        ..Default::default()
    });

    // Record some state
    ctx.record_step();
    ctx.record_step();
    ctx.record_error(HarnessError::Timeout(Duration::from_secs(1)));

    // Save checkpoint
    let checkpoint = ctx.save_checkpoint().await;
    assert_eq!(checkpoint.step_number, 2);
    assert_eq!(checkpoint.error_history.len(), 1);

    // Load checkpoint
    let loaded = ctx.load_checkpoint().await;
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().step_number, 2);
}

#[test]
fn test_retry_delay_exponential_backoff() {
    let config = RetryConfig {
        max_retries: 5,
        base_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(10),
        multiplier: 2.0,
    };

    assert_eq!(config.calculate_delay(0), Duration::from_millis(100));
    assert_eq!(config.calculate_delay(1), Duration::from_millis(200));
    assert_eq!(config.calculate_delay(2), Duration::from_millis(400));
    assert_eq!(config.calculate_delay(3), Duration::from_millis(800));
    // Capped at max_delay
    assert!(config.calculate_delay(20) <= Duration::from_secs(10));
}

// ============================================================================
// Circuit Breaker Tests
// ============================================================================

#[test]
fn test_circuit_breaker_state_transitions() {
    let cb = CircuitBreaker::new(CircuitBreakerConfig {
        failure_threshold: 3,
        success_threshold: 2,
        timeout: Duration::from_secs(30),
    });

    // Initial: Closed
    assert_eq!(cb.state(), CircuitState::Closed);
    assert!(cb.is_allowed());

    // Accumulate failures
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Closed);

    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Closed);

    cb.record_failure();
    // Now open
    assert_eq!(cb.state(), CircuitState::Open);
    assert!(!cb.is_allowed());

    // Reset
    cb.reset();
    assert_eq!(cb.state(), CircuitState::Closed);
    assert!(cb.is_allowed());
}

#[test]
fn test_circuit_breaker_respects_timeout_window() {
    let cb = CircuitBreaker::new(CircuitBreakerConfig {
        failure_threshold: 1,
        success_threshold: 1,
        timeout: Duration::from_millis(50),
    });

    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);
    assert!(!cb.is_allowed());

    std::thread::sleep(Duration::from_millis(60));

    assert!(cb.is_allowed());
    assert_eq!(cb.state(), CircuitState::HalfOpen);
}

#[tokio::test]
async fn test_per_tool_circuit_breaker_isolation() {
    let mut ctx = AgentHarnessContext::new(HarnessConfig::default());

    // Only tool_a should trip
    for _ in 0..3 {
        ctx.record_error(HarnessError::ToolFailure(
            "tool_a".to_string(),
            "err".to_string(),
        ));
    }

    assert!(ctx.is_circuit_open("tool_a"));
    assert!(!ctx.is_circuit_open("tool_b"));
    assert!(!ctx.is_circuit_open("tool_c"));
    assert_eq!(ctx.circuit_breaker_for("tool_a"), CircuitState::Open);
    assert_eq!(ctx.circuit_breaker_for("tool_b"), CircuitState::Closed);

    // Success on tool_a should reset its circuit
    ctx.record_tool_success("tool_a");
    // Note: record_success alone doesn't transition from Open to Closed
    // It only counts successes in HalfOpen state.
    // The circuit transitions Open->HalfOpen only after timeout expires.
    // For now just verify the method doesn't panic.
}

// ============================================================================
// Resource Tracker Tests
// ============================================================================

#[test]
fn test_resource_tracker_limits() {
    let tracker = crablet::agent::harness::ResourceTracker::new(
        1024, // max 1024 bytes
        5000, // max 5000 ms CPU
    );

    // Under limits
    tracker.update_memory(512);
    tracker.add_cpu_time(2000);
    assert!(tracker.check().is_ok());

    // Memory exceeded
    tracker.update_memory(2000);
    assert!(matches!(
        tracker.check(),
        Err(HarnessError::ResourceLimitExceeded(_))
    ));

    // Reset and check CPU
    tracker.reset();
    tracker.update_memory(512);
    tracker.add_cpu_time(6000);
    assert!(matches!(
        tracker.check(),
        Err(HarnessError::ResourceLimitExceeded(_))
    ));
}

// ============================================================================
// Signal Channel Tests
// ============================================================================

#[tokio::test]
async fn test_signal_channel_broadcast() {
    let channel = HarnessSignalChannel::new();
    let mut rx = channel.subscribe();

    // Cancel signal
    assert!(channel.cancel());
    let signal = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await;
    assert!(signal.is_ok());
    assert!(matches!(signal.unwrap(), Ok(HarnessSignal::Cancel)));

    // Pause/Resume
    let mut rx2 = channel.subscribe();
    assert!(channel.pause());
    let signal = tokio::time::timeout(Duration::from_millis(100), rx2.recv()).await;
    assert!(matches!(signal.unwrap(), Ok(HarnessSignal::Pause)));

    assert!(channel.resume());
    let signal = tokio::time::timeout(Duration::from_millis(100), rx2.recv()).await;
    assert!(matches!(signal.unwrap(), Ok(HarnessSignal::Resume)));

    // Checkpoint
    assert!(channel.checkpoint());
    let signal = tokio::time::timeout(Duration::from_millis(100), rx2.recv()).await;
    assert!(matches!(signal.unwrap(), Ok(HarnessSignal::Checkpoint)));
}

// ============================================================================
// Tool Call Parsing Tests
// ============================================================================

#[test]
fn test_parse_tool_calls_json_format() {
    let response = r#"{"actions": [{"name": "search", "args": {"query": "rust"}}, {"name": "read_file", "args": {"path": "/etc/config"}}]}"#;
    let calls = parse_tool_calls(response);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].0, "search");
    assert_eq!(calls[1].0, "read_file");
}

#[test]
fn test_parse_tool_calls_text_format() {
    let response = r#"Action: search {"query": "hello"}
Action: read {path: "/file.txt"}"#;
    let calls = parse_tool_calls(response);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].0, "search");
    assert_eq!(calls[1].0, "read");
}

#[test]
fn test_parse_tool_calls_empty() {
    let calls = parse_tool_calls("no tool calls here");
    assert!(calls.is_empty());
}

// ============================================================================
// Tool Execution Result Tests
// ============================================================================

#[test]
fn test_tool_exec_result_success() {
    let result = ToolExecResult::success(
        "search".to_string(),
        r#"{"q": "test"}"#.to_string(),
        "results...".to_string(),
        1,
        150,
    );
    assert!(result.is_success());
    assert!(!result.can_retry(&RetryConfig::default()));
    assert_eq!(result.attempts, 1);
}

#[test]
fn test_tool_exec_result_retryable_failure() {
    let result = ToolExecResult::failure(
        "api_call".to_string(),
        "{}".to_string(),
        HarnessError::Timeout(Duration::from_secs(5)),
        1,
        100,
    );
    assert!(!result.is_success());
    assert!(result.can_retry(&RetryConfig::default())); // 1 < 3 max retries
}

#[test]
fn test_tool_exec_result_max_retries() {
    let result = ToolExecResult::failure(
        "api_call".to_string(),
        "{}".to_string(),
        HarnessError::Timeout(Duration::from_secs(5)),
        3,
        100,
    );
    assert!(!result.is_success());
    assert!(!result.can_retry(&RetryConfig::default())); // 3 >= 3 max retries
}

// ============================================================================
// Harness Error Tests
// ============================================================================

#[test]
fn test_harness_error_retryable_classification() {
    assert!(HarnessError::Timeout(Duration::from_secs(1)).is_retryable());
    assert!(HarnessError::ToolFailure("x".to_string(), "y".to_string()).is_retryable());
    assert!(HarnessError::LlmFailure("err".to_string()).is_retryable());

    assert!(!HarnessError::Cancelled.is_retryable());
    assert!(!HarnessError::ContextClosed.is_retryable());
    assert!(!HarnessError::CircuitBreakerOpen("x".to_string()).is_retryable());
    assert!(!HarnessError::ResourceLimitExceeded("mem".to_string()).is_retryable());
    assert!(!HarnessError::MaxRetriesExceeded("x".to_string()).is_retryable());
}

// ============================================================================
// Adaptive Harness Tests
// ============================================================================

#[test]
fn test_adaptive_timeout_calculation() {
    let mut timeout = AdaptiveTimeout::with_default();

    // Record step durations
    timeout.record_step(Duration::from_millis(100));
    timeout.record_step(Duration::from_millis(150));
    timeout.record_step(Duration::from_millis(120));
    timeout.record_step(Duration::from_millis(130));

    // Calculate timeout for later steps
    let t5 = timeout.calculate_timeout(5);
    assert!(t5 >= Duration::from_secs(10)); // min bound
    assert!(t5 <= Duration::from_secs(300)); // max bound

    // Higher step count should increase timeout
    let t20 = timeout.calculate_timeout(20);
    assert!(t20 >= t5); // More steps → more complex → longer timeout
}

#[test]
fn test_step_history_avg_and_std() {
    let mut history = StepHistory::new(10);

    assert_eq!(history.avg_duration(), Duration::from_secs(60)); // default

    history.record_step(100);
    history.record_step(200);
    history.record_step(300);

    assert_eq!(history.avg_duration(), Duration::from_millis(200));

    let std = history.std_deviation();
    // Std should be non-zero for varied data
    assert!(std.as_millis() > 0);
}

#[test]
fn test_step_history_sliding_window() {
    let mut history = StepHistory::new(3);

    history.record_step(100);
    history.record_step(200);
    history.record_step(300);
    assert_eq!(history.avg_duration(), Duration::from_millis(200));

    // This should push out the oldest value
    history.record_step(400);
    // Now we have [200, 300, 400]
    assert_eq!(history.avg_duration(), Duration::from_millis(300));
}

#[test]
fn test_step_history_is_slowing_down() {
    let mut history = StepHistory::new(10);

    // Need at least 4 items, split into two halves
    history.record_step(100); // older half: [100, 200]
    history.record_step(200);
    history.record_step(500); // recent half: [500, 600]
    history.record_step(600);

    // Recent avg (550) > 2x older avg (150)
    assert!(history.is_slowing_down());

    // No slowdown
    let mut history2 = StepHistory::new(10);
    history2.record_step(100);
    history2.record_step(200);
    history2.record_step(150);
    history2.record_step(180);
    assert!(!history2.is_slowing_down());
}

#[tokio::test]
async fn test_breakpoint_manager_full_lifecycle() {
    let manager = BreakpointManager::new();

    // Add breakpoints
    manager
        .add_breakpoint(Breakpoint::new(
            "step_limit",
            "Step Limit",
            BreakpointCondition::StepCount { count: 5 },
            BreakpointAction::Cancel,
        ))
        .await;

    manager
        .add_breakpoint(Breakpoint::new(
            "error_rate",
            "High Error Rate",
            BreakpointCondition::ErrorRate { threshold: 0.5 },
            BreakpointAction::Reflect,
        ))
        .await;

    // List
    let breakpoints = manager.list_breakpoints().await;
    assert_eq!(breakpoints.len(), 2);

    // Check step limit NOT triggered yet
    let ctx = BreakpointContext {
        step_count: 3,
        ..Default::default()
    };
    let events = manager.check_all(&ctx).await;
    assert!(events.is_empty());

    // Check step limit triggered
    let ctx = BreakpointContext {
        step_count: 5,
        ..Default::default()
    };
    let events = manager.check_all(&ctx).await;
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0].action, BreakpointAction::Cancel));

    // Check error rate triggered
    let ctx = BreakpointContext {
        total_calls: 10,
        failed_calls: 6,
        ..Default::default()
    };
    let events = manager.check_all(&ctx).await;
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0].action, BreakpointAction::Reflect));

    // Remove
    let removed = manager.remove_breakpoint("step_limit").await;
    assert!(removed);
    assert_eq!(manager.list_breakpoints().await.len(), 1);

    // Remove non-existent
    assert!(!manager.remove_breakpoint("nonexistent").await);
}

#[tokio::test]
async fn test_breakpoint_event_subscription() {
    let manager = BreakpointManager::new();
    let mut rx = manager.subscribe();

    manager
        .add_breakpoint(Breakpoint::new(
            "test",
            "Test",
            BreakpointCondition::StepCount { count: 1 },
            BreakpointAction::LogAndContinue {
                message: "Reached step 1".to_string(),
            },
        ))
        .await;

    let ctx = BreakpointContext {
        step_count: 1,
        ..Default::default()
    };
    manager.check_all(&ctx).await;

    let event = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await;
    assert!(event.is_ok());
    assert_eq!(event.unwrap().unwrap().breakpoint_id, "test");
}

// ============================================================================
// Harness Manager Tests
// ============================================================================

#[tokio::test]
async fn test_manager_create_and_list() {
    let manager = HarnessManager::new();

    let id1 = manager.create_harness(None).await.unwrap();
    let id2 = manager.create_harness(None).await.unwrap();

    assert_ne!(id1, id2);

    let harnesses = manager.list_harnesses().await;
    assert_eq!(harnesses.len(), 2);

    let stats = manager.get_stats().await;
    assert_eq!(stats.total_created, 2);
}

#[tokio::test]
async fn test_manager_lifecycle_operations() {
    let manager = HarnessManager::new();
    let id = manager.create_harness(None).await.unwrap();

    // Check initial status
    let info = manager.get_info(&id).await.unwrap();
    assert!(matches!(info.status, HarnessStatus::Idle));

    // Pause
    manager.pause_harness(&id).await.unwrap();
    let info = manager.get_info(&id).await.unwrap();
    assert!(matches!(info.status, HarnessStatus::Paused));

    // Resume
    manager.resume_harness(&id).await.unwrap();
    let info = manager.get_info(&id).await.unwrap();
    assert!(matches!(info.status, HarnessStatus::Running));

    // Cancel
    manager.cancel_harness(&id).await.unwrap();
    let info = manager.get_info(&id).await.unwrap();
    assert!(matches!(info.status, HarnessStatus::Cancelled));

    // Remove
    assert!(manager.remove_harness(&id).await);
    assert!(manager.get_harness(&id).await.is_none());
}

#[tokio::test]
async fn test_manager_graceful_shutdown() {
    let manager = HarnessManager::new();

    manager.create_harness(None).await.unwrap();
    manager.create_harness(None).await.unwrap();

    manager.shutdown().await;

    assert!(manager.list_harnesses().await.is_empty());
}

#[tokio::test]
async fn test_manager_with_custom_config() {
    let config = HarnessConfig {
        max_steps: 100,
        tool_timeout: Duration::from_secs(60),
        ..Default::default()
    };

    let manager = HarnessManager::with_config(config);
    let id = manager.create_harness(None).await.unwrap();

    let info = manager.get_info(&id).await.unwrap();
    assert_eq!(info.config.max_steps, 100);
}

#[tokio::test]
async fn test_manager_completion_tracks_live_snapshot() {
    let manager = HarnessManager::new();
    let id = manager.create_harness(None).await.unwrap();
    let harness = manager.get_harness(&id).await.unwrap();

    {
        let mut harness = harness.write().await;
        harness.record_step();
        harness.record_error(HarnessError::Timeout(Duration::from_millis(5)));
        harness.metadata_mut().update_duration();
    }

    manager.complete_harness(&id).await.unwrap();

    let info = manager.get_info(&id).await.unwrap();
    assert!(matches!(info.status, HarnessStatus::Completed));
    assert_eq!(info.step_count, 1);
    assert_eq!(info.error_count, 1);

    let stats = manager.get_stats().await;
    assert_eq!(stats.total_completed, 1);
    assert_eq!(stats.total_steps_executed, 1);
    assert_eq!(stats.total_errors, 1);
}

// ============================================================================
// Fusion Engine Tests
// ============================================================================

#[tokio::test]
async fn test_fusion_engine_state_machine() {
    let engine = UnifiedHarnessFusion::with_default();

    assert_eq!(engine.state().await, EngineState::Idle);

    engine.start().await;
    assert_eq!(engine.state().await, EngineState::Running);

    engine.pause().await;
    assert_eq!(engine.state().await, EngineState::Paused);

    engine.resume().await;
    assert_eq!(engine.state().await, EngineState::Running);

    engine.stop().await;
    assert_eq!(engine.state().await, EngineState::Stopped);
}

#[tokio::test]
async fn test_fusion_builder() {
    let engine = UnifiedHarnessFusionBuilder::new()
        .with_self_healing(true)
        .with_adaptive_timeout(false)
        .with_metrics(true)
        .with_max_repair_attempts(5)
        .with_circuit_sensitivity(0.9)
        .build()
        .await;

    let metrics = engine.metrics().await;
    assert_eq!(metrics.steps_completed, 0);
    assert_eq!(metrics.steps_failed, 0);
}

#[tokio::test]
async fn test_fusion_metrics_collection() {
    let engine = UnifiedHarnessFusion::with_default();
    let snapshot = engine.metrics().await;

    assert_eq!(snapshot.steps_completed, 0);
    assert_eq!(snapshot.steps_failed, 0);
    assert_eq!(snapshot.self_healing_attempts, 0);
    assert_eq!(snapshot.circuit_breaker_trips, 0);
    assert_eq!(snapshot.current_step_duration_ms, 0.0);
}

#[tokio::test]
async fn test_fusion_breakpoint_integration() {
    let engine = UnifiedHarnessFusion::with_default();

    engine
        .add_breakpoint(Breakpoint::new(
            "test_bp",
            "Test Breakpoint",
            BreakpointCondition::StepCount { count: 100 },
            BreakpointAction::LogAndContinue {
                message: "Reached 100 steps".to_string(),
            },
        ))
        .await;

    let breakpoints = engine.list_breakpoints().await;
    assert_eq!(breakpoints.len(), 1);
}

#[tokio::test]
async fn test_fusion_execute_step_tracks_harness_state() {
    let engine = UnifiedHarnessFusion::with_default();
    engine.start().await;

    let result = engine
        .execute_step(|_, _| async { Ok("ok".to_string()) })
        .await
        .unwrap();
    assert_eq!(result, "ok");

    let harness = engine.harness().await;
    let harness = harness.read().await;
    assert_eq!(harness.metadata().step_count, 1);
    drop(harness);

    let summary = engine.summary().await;
    assert_eq!(summary.step_count, 1);
}

// ============================================================================
// Distributed Harness Tests
// ============================================================================

#[tokio::test]
async fn test_in_memory_backend_basic() {
    let backend = Arc::new(InMemoryBackend::new());

    let info = HarnessInfo::new("test-1".to_string(), HarnessConfig::default());

    // Create
    backend.create_harness(&info).await.unwrap();

    // Get
    let retrieved = backend.get_harness("test-1").await.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id, "test-1");

    // Update
    let mut updated = info.clone();
    updated.step_count = 10;
    backend.update_harness(&updated).await.unwrap();

    // List
    let all = backend.list_harnesses().await.unwrap();
    assert_eq!(all.len(), 1);

    // Delete
    backend.delete_harness("test-1").await.unwrap();
    assert!(backend.get_harness("test-1").await.unwrap().is_none());
}

#[tokio::test]
async fn test_in_memory_distributed_lock() {
    let backend = Arc::new(InMemoryBackend::new());

    // Acquire lock
    let acquired = backend
        .acquire_lock("resource-1", "node-1", 60)
        .await
        .unwrap();
    assert!(acquired);

    // Same owner can re-acquire
    let reacquired = backend
        .acquire_lock("resource-1", "node-1", 60)
        .await
        .unwrap();
    assert!(reacquired);

    // Different owner cannot acquire
    let blocked = backend
        .acquire_lock("resource-1", "node-2", 60)
        .await
        .unwrap();
    assert!(!blocked);

    // Release and retry
    backend.release_lock("resource-1", "node-1").await.unwrap();
    let acquired2 = backend
        .acquire_lock("resource-1", "node-2", 60)
        .await
        .unwrap();
    assert!(acquired2);
}

#[tokio::test]
async fn test_distributed_manager_basic() {
    let backend: Arc<dyn HarnessBackend> = Arc::new(InMemoryBackend::new());
    let config = DistributedConfig {
        node_id: "node-1".to_string(),
        node_address: "127.0.0.1".to_string(),
        node_port: 8080,
        backend_type: BackendType::InMemory,
        backend_uri: "memory://".to_string(),
        lock_ttl_secs: 300,
        heartbeat_interval_secs: 30,
        node_timeout_secs: 60,
    };

    let manager = DistributedHarnessManager::new(backend, config);

    // Create harness
    let id = manager.create_harness(None).await.unwrap();
    assert!(manager.is_local(&id).await);

    // Get harness
    let harness = manager.get_harness(&id).await.unwrap();
    assert!(harness.is_some());

    // Lock harness
    let locked = manager.try_lock_harness(&id).await.unwrap();
    assert!(locked);

    // Unlock
    manager.unlock_harness(&id).await.unwrap();

    // Cluster stats
    let stats = manager.get_cluster_stats().await.unwrap();
    assert_eq!(stats.active_nodes, 0); // No nodes registered
}
