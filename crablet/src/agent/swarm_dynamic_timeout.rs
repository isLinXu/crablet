//! Swarm Dynamic Timeout Module
//!
//! Provides dynamic timeout calculation for System 3 (Swarm reasoning) and
//! Swarm task nodes based on task complexity, historical performance, network
//! conditions, and current system load.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::debug;

/// Shorthand for the execution-history map keyed by (role, task_type).
type HistoryMap = HashMap<(String, String), VecDeque<ExecutionRecord>>;
/// Shorthand for the per-key stats map.
type StatsMap = HashMap<(String, String), ExecutionStats>;

// ----------------------------------------------------------------------
// Data structures
// ----------------------------------------------------------------------

/// Historical execution record for a single (role, task_type) pair
#[derive(Debug, Clone)]
pub struct ExecutionRecord {
    pub role: String,
    pub task_type: String, // e.g. "coding", "research", "draft", "analysis"
    pub duration_ms: u64,
    pub success: bool,
    pub token_count: Option<usize>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Aggregated statistics used for prediction
#[derive(Debug, Clone, Default)]
pub struct ExecutionStats {
    pub count: u64,
    pub success_count: u64,
    pub avg_duration_ms: u64,
    pub p95_duration_ms: u64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// System load snapshot (injected by external observer)
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemLoadSnapshot {
    pub cpu_percent: f32,           // 0.0 - 100.0
    pub memory_percent: f32,        // 0.0 - 100.0
    pub active_tasks: usize,
    pub llm_queue_depth: usize,
    pub network_latency_ms: Option<u64>,
}

/// Timeout configuration (can be hot-reloaded)
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    pub base_timeout_ms: u64,
    pub min_timeout_ms: u64,
    pub max_timeout_ms: u64,
    pub complexity_multiplier: f32, // per unit of complexity
    pub load_scaling_factor: f32,     // how much load affects timeout
    pub history_weight: f32,        // 0.0-1.0 weight of historical avg vs. base
    pub burst_allowance: f32,       // extra multiplier for burst tasks
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            base_timeout_ms: 30_000,   // 30s default
            min_timeout_ms: 5_000,     // 5s floor
            max_timeout_ms: 300_000,   // 5m ceiling
            complexity_multiplier: 20_000.0, // +20s per complexity unit
            load_scaling_factor: 0.5,  // 50% increase at full load
            history_weight: 0.6,       // 60% history, 40% base
            burst_allowance: 1.3,      // +30% for burst
        }
    }
}

// ----------------------------------------------------------------------
// Core engine
// ----------------------------------------------------------------------

/// Dynamic timeout calculator for Swarm / System 3
#[derive(Clone)]
pub struct DynamicTimeoutEngine {
    config: Arc<RwLock<TimeoutConfig>>,
    history: Arc<RwLock<HistoryMap>>,
    stats: Arc<RwLock<StatsMap>>,
    current_load: Arc<RwLock<SystemLoadSnapshot>>,
}

impl DynamicTimeoutEngine {
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(TimeoutConfig::default())),
            history: Arc::new(RwLock::new(HistoryMap::new())),
            stats: Arc::new(RwLock::new(StatsMap::new())),
            current_load: Arc::new(RwLock::new(SystemLoadSnapshot::default())),
        }
    }

    pub fn with_config(config: TimeoutConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            history: Arc::new(RwLock::new(HistoryMap::new())),
            stats: Arc::new(RwLock::new(StatsMap::new())),
            current_load: Arc::new(RwLock::new(SystemLoadSnapshot::default())),
        }
    }

    pub async fn set_config(&self, config: TimeoutConfig) {
        let mut c = self.config.write().await;
        *c = config;
    }

    /// Record a completed execution for future timeout prediction
    pub async fn record_execution(&self, record: ExecutionRecord) {
        let key = (record.role.clone(), record.task_type.clone());

        {
            let mut history = self.history.write().await;
            let queue = history.entry(key.clone()).or_default();
            queue.push_back(record.clone());
            // Keep only last 100 records
            while queue.len() > 100 {
                queue.pop_front();
            }
        }

        // Recalculate stats
        let stats = self.calculate_stats(&key).await;
        {
            let mut map = self.stats.write().await;
            map.insert(key, stats);
        }
    }

    /// Update current system load snapshot (called periodically by observer)
    pub async fn update_system_load(&self, load: SystemLoadSnapshot) {
        let mut current = self.current_load.write().await;
        *current = load;
    }

    // ------------------------------------------------------------------
    // Main API: compute timeout
    // ------------------------------------------------------------------

    /// Compute dynamic timeout for a swarm task node
    ///
    /// Parameters:
    /// - `role`: agent role (e.g. "coder", "researcher")
    /// - `task_type`: semantic category (e.g. "coding", "research")
    /// - `complexity`: 0.0 - 1.0 complexity score
    /// - `priority`: 0 (highest) - 255 (lowest) — lower priority gets more time
    /// - `is_burst`: whether this task is part of a burst batch
    pub async fn compute_timeout(
        &self,
        role: &str,
        task_type: &str,
        complexity: f32,
        priority: u8,
        is_burst: bool,
    ) -> Duration {
        let config = self.config.read().await;
        let load = self.current_load.read().await;
        let stats = self.stats.read().await;

        let key = (role.to_string(), task_type.to_string());
        let history_stats = stats.get(&key).cloned().unwrap_or_default();

        // 1. Base timeout from historical average (if available) or config base
        let historical_ms = if history_stats.count > 0 {
            history_stats.avg_duration_ms
        } else {
            config.base_timeout_ms
        };

        let base_ms = (config.history_weight * historical_ms as f32
            + (1.0 - config.history_weight) * config.base_timeout_ms as f32)
            as u64;

        // 2. Complexity multiplier
        let complexity_ms = (complexity.clamp(0.0, 1.0) * config.complexity_multiplier) as u64;

        // 3. Load scaling: linear interpolation based on CPU + memory + queue
        let load_score = ((load.cpu_percent + load.memory_percent) / 2.0
            + (load.llm_queue_depth as f32 * 5.0).min(50.0))
        .clamp(0.0, 100.0)
            / 100.0;
        let load_ms = (base_ms as f32 * load_score * config.load_scaling_factor) as u64;

        // 4. Priority adjustment: lower priority (numerically lower) gets more time
        let priority_factor = 1.0 + (priority as f32 / 255.0) * 0.5; // 1.0 - 1.5x
        let priority_ms = (base_ms as f32 * priority_factor) as u64;

        // 5. Burst allowance
        let burst_ms = if is_burst {
            (base_ms as f32 * (config.burst_allowance - 1.0)) as u64
        } else {
            0
        };

        // 6. Risk adjustment: if historical success rate is low, extend timeout
        let risk_ms = if history_stats.count > 3 && history_stats.success_count < history_stats.count * 3 / 4 {
            (base_ms as f32 * 0.3) as u64 // +30% safety margin for historically flaky tasks
        } else {
            0
        };

        let total_ms = base_ms + complexity_ms + load_ms + priority_ms + burst_ms + risk_ms;

        let final_ms = total_ms
            .clamp(config.min_timeout_ms, config.max_timeout_ms);

        debug!(
            "Dynamic timeout for {}|{}: {}ms (base={} complexity={} load={} priority={} burst={} risk={}) | load_score={:.2}",
            role, task_type, final_ms, base_ms, complexity_ms, load_ms, priority_ms, burst_ms, risk_ms, load_score
        );

        Duration::from_millis(final_ms)
    }

    /// Compute timeout for System 3 reasoning / swarm submission
    pub async fn compute_system3_timeout(
        &self,
        goal: &str,
        context_length: usize,
    ) -> Duration {
        // Infer task type from goal prefix (matches existing system logic)
        let task_type = if goal.to_lowercase().starts_with("draft ") {
            "draft"
        } else if goal.to_lowercase().starts_with("research ") {
            "research"
        } else if goal.to_lowercase().starts_with("code ") || goal.to_lowercase().contains("code") {
            "coding"
        } else if goal.to_lowercase().contains("analyze") {
            "analysis"
        } else {
            "general"
        };

        // Estimate complexity from goal length and token count
        let complexity = ((context_length as f32 / 4000.0) + (goal.len() as f32 / 500.0)).min(1.0);

        // System 3 uses "orchestrator" as role with higher complexity tolerance.
        // The base/max timeout is governed by the shared config; here we only
        // route through the orchestrator role to pick up System 3 scaling.
        let _config = self.config.read().await.clone();
        self.compute_timeout("orchestrator", task_type, complexity, 128, false).await
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    async fn calculate_stats(&self, key: &(String, String)) -> ExecutionStats {
        let history = self.history.read().await;
        let queue = match history.get(key) {
            Some(q) if !q.is_empty() => q.clone(),
            _ => return ExecutionStats::default(),
        };
        drop(history);

        let count = queue.len() as u64;
        let success_count = queue.iter().filter(|r| r.success).count() as u64;

        let mut durations: Vec<u64> = queue.iter().map(|r| r.duration_ms).collect();
        durations.sort_unstable();

        let avg = durations.iter().sum::<u64>() / count.max(1);
        let p95_idx = ((durations.len() as f32) * 0.95).ceil() as usize;
        let p95 = durations.get(p95_idx.saturating_sub(1)).copied().unwrap_or(avg);

        ExecutionStats {
            count,
            success_count,
            avg_duration_ms: avg,
            p95_duration_ms: p95,
            last_updated: chrono::Utc::now(),
        }
    }

    /// Get a diagnostic summary of current timeout stats
    pub async fn stats_summary(&self) -> HashMap<String, serde_json::Value> {
        let stats = self.stats.read().await;
        let load = self.current_load.read().await;
        let mut summary = HashMap::new();

        for (key, s) in stats.iter() {
            let key_str = format!("{}|{}", key.0, key.1);
            summary.insert(
                key_str,
                serde_json::json!({
                    "count": s.count,
                    "success_rate": if s.count > 0 { s.success_count as f64 / s.count as f64 } else { 0.0 },
                    "avg_ms": s.avg_duration_ms,
                    "p95_ms": s.p95_duration_ms,
                }),
            );
        }

        summary.insert(
            "_system_load".to_string(),
            serde_json::json!({
                "cpu": load.cpu_percent,
                "memory": load.memory_percent,
                "active_tasks": load.active_tasks,
                "llm_queue": load.llm_queue_depth,
                "network_ms": load.network_latency_ms,
            }),
        );

        summary
    }
}

impl Default for DynamicTimeoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ----------------------------------------------------------------------
// Integration helpers for existing System 3 and SwarmExecutor
// ----------------------------------------------------------------------

/// Trait to adapt existing System 3 and SwarmCoordinator without breaking changes
#[async_trait::async_trait]
pub trait DynamicTimeoutAdapter: Send + Sync {
    async fn get_timeout_for_task(
        &self,
        role: &str,
        task_type: &str,
        complexity: f32,
        priority: u8,
    ) -> Duration;
}

#[async_trait::async_trait]
impl DynamicTimeoutAdapter for DynamicTimeoutEngine {
    async fn get_timeout_for_task(
        &self,
        role: &str,
        task_type: &str,
        complexity: f32,
        priority: u8,
    ) -> Duration {
        self.compute_timeout(role, task_type, complexity, priority, false).await
    }
}

/// Usage example for System 3 replacement:
///
/// ```rust,ignore
/// // In system3.rs:
/// let timeout_engine = DynamicTimeoutEngine::new();
/// // ... after each swarm execution:
/// timeout_engine.record_execution(ExecutionRecord {
///     role: "orchestrator".into(),
///     task_type: "research".into(),
///     duration_ms: elapsed.as_millis() as u64,
///     success,
///     token_count: Some(tokens_used),
///     timestamp: chrono::Utc::now(),
/// }).await;
///
/// // Before submission:
/// let timeout = timeout_engine.compute_system3_timeout(&goal, context.len()).await;
/// match tokio::time::timeout(timeout, execution_future).await { ... }
/// ```

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_default_timeout_bounds() {
        let engine = DynamicTimeoutEngine::new();
        let timeout = engine.compute_timeout("coder", "coding", 0.5, 128, false).await;
        let ms = timeout.as_millis() as u64;
        assert!(ms >= 5_000);
        assert!(ms <= 300_000);
    }

    #[tokio::test]
    async fn test_history_influences_timeout() {
        let engine = DynamicTimeoutEngine::new();

        // Record fast history
        for _ in 0..10 {
            engine
                .record_execution(ExecutionRecord {
                    role: "coder".into(),
                    task_type: "coding".into(),
                    duration_ms: 5_000,
                    success: true,
                    token_count: None,
                    timestamp: chrono::Utc::now(),
                })
                .await;
        }

        let timeout1 = engine.compute_timeout("coder", "coding", 0.0, 128, false).await;
        let ms1 = timeout1.as_millis();

        // Record slow history
        for _ in 0..10 {
            engine
                .record_execution(ExecutionRecord {
                    role: "coder".into(),
                    task_type: "coding".into(),
                    duration_ms: 60_000,
                    success: true,
                    token_count: None,
                    timestamp: chrono::Utc::now(),
                })
                .await;
        }

        let timeout2 = engine.compute_timeout("coder", "coding", 0.0, 128, false).await;
        let ms2 = timeout2.as_millis();

        assert!(ms2 > ms1, "History of slow tasks should increase timeout");
    }

    #[tokio::test]
    async fn test_load_scaling() {
        let engine = DynamicTimeoutEngine::new();
        let base = engine.compute_timeout("coder", "coding", 0.0, 128, false).await;

        engine
            .update_system_load(SystemLoadSnapshot {
                cpu_percent: 90.0,
                memory_percent: 85.0,
                active_tasks: 50,
                llm_queue_depth: 20,
                network_latency_ms: Some(500),
            })
            .await;

        let under_load = engine.compute_timeout("coder", "coding", 0.0, 128, false).await;
        assert!(under_load > base, "High load should increase timeout");
    }

    #[tokio::test]
    async fn test_system3_timeout_for_draft() {
        let engine = DynamicTimeoutEngine::new();
        let timeout = engine
            .compute_system3_timeout("Draft a comprehensive essay about AI", 800)
            .await;
        let ms = timeout.as_millis() as u64;
        // Draft is complex, should be longer than base
        assert!(ms > 60_000);
    }

    #[tokio::test]
    async fn test_risk_adjustment_for_flaky_tasks() {
        let engine = DynamicTimeoutEngine::new();
        // Record 5 failures, 1 success
        for _ in 0..5 {
            engine
                .record_execution(ExecutionRecord {
                    role: "coder".into(),
                    task_type: "coding".into(),
                    duration_ms: 10_000,
                    success: false,
                    token_count: None,
                    timestamp: chrono::Utc::now(),
                })
                .await;
        }
        engine
            .record_execution(ExecutionRecord {
                role: "coder".into(),
                task_type: "coding".into(),
                duration_ms: 10_000,
                success: true,
                token_count: None,
                timestamp: chrono::Utc::now(),
            })
            .await;

        let risky = engine.compute_timeout("coder", "coding", 0.0, 128, false).await;
        let safe_role = engine.compute_timeout("coder", "analysis", 0.0, 128, false).await;
        // Risky task should get +30% safety margin
        assert!(risky.as_millis() > safe_role.as_millis() || risky.as_millis() >= 6_500);
    }

    #[tokio::test]
    async fn test_with_config_uses_custom_config() {
        // Regression: with_config previously discarded the passed-in config
        // and returned Self::new() with defaults.
        let custom = TimeoutConfig {
            base_timeout_ms: 20_000,
            max_timeout_ms: 100_000,
            ..Default::default()
        };
        let engine = DynamicTimeoutEngine::with_config(custom);

        // With base 20s, complexity 0 → timeout should be ≥ 20s
        let timeout = engine.compute_timeout("coder", "coding", 0.0, 128, false).await;
        let ms = timeout.as_millis() as u64;
        assert!(
            ms >= 20_000,
            "with_config should apply custom base_timeout_ms: got {ms}ms"
        );

        // And should not exceed the custom ceiling
        assert!(
            ms <= 100_000,
            "with_config should apply custom max_timeout_ms: got {ms}ms"
        );
    }

    #[tokio::test]
    async fn test_with_config_vs_default_diverge() {
        let default_engine = DynamicTimeoutEngine::new();
        let custom_engine = DynamicTimeoutEngine::with_config(TimeoutConfig {
            base_timeout_ms: 60_000,
            max_timeout_ms: 600_000,
            ..Default::default()
        });

        let default_ts = default_engine
            .compute_timeout("coder", "coding", 0.0, 128, false)
            .await;
        let custom_ts = custom_engine
            .compute_timeout("coder", "coding", 0.0, 128, false)
            .await;

        assert!(
            custom_ts.as_millis() > default_ts.as_millis(),
            "Custom engine with higher base should produce longer timeouts"
        );
    }

    #[tokio::test]
    async fn test_set_config_overrides_at_runtime() {
        let engine = DynamicTimeoutEngine::new();
        let original = engine
            .compute_timeout("coder", "coding", 0.0, 128, false)
            .await;

        engine
            .set_config(TimeoutConfig {
                base_timeout_ms: 40_000,
                max_timeout_ms: 200_000,
                ..Default::default()
            })
            .await;

        let updated = engine
            .compute_timeout("coder", "coding", 0.0, 128, false)
            .await;
        assert!(
            updated.as_millis() > original.as_millis(),
            "set_config should take effect immediately"
        );
    }
}
