//! Harness Manager - Manages multiple agent harness instances
//!
//! Provides:
//! - Multi-agent harness lifecycle management
//! - Shared harness pool for efficiency
//! - Harness state monitoring and metrics
//! - Graceful shutdown coordination

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, RwLock};

use super::harness::{
    AgentHarnessContext, ExecutionMetadata, HarnessConfig, HarnessError, HarnessSignal,
};
use super::harness_agent::{
    HarnessAgent, HarnessAgentBuilder, HarnessAgentResult, HarnessExecutionProgressSink,
    HarnessExecutionState,
};
use crate::types::Message;

/// Status of a managed harness
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HarnessStatus {
    Idle,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

/// Information about a managed harness instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessInfo {
    pub id: String,
    pub status: HarnessStatus,
    pub created_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
    pub step_count: usize,
    pub error_count: usize,
    #[serde(default)]
    pub execution_metadata: Option<ExecutionMetadata>,
    #[serde(default)]
    pub error_history: Vec<HarnessError>,
    #[serde(default)]
    pub execution_state: Option<HarnessExecutionState>,
    pub config: HarnessConfig,
}

impl HarnessInfo {
    pub fn new(id: String, config: HarnessConfig) -> Self {
        let now = Utc::now();
        Self {
            id,
            status: HarnessStatus::Idle,
            created_at: now,
            last_active_at: now,
            step_count: 0,
            error_count: 0,
            execution_metadata: None,
            error_history: Vec::new(),
            execution_state: None,
            config,
        }
    }
}

/// Statistics for harness usage
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HarnessStats {
    pub total_created: u64,
    pub total_completed: u64,
    pub total_failed: u64,
    pub total_cancelled: u64,
    pub total_steps_executed: u64,
    pub total_errors: u64,
    pub avg_execution_time_ms: f64,
}

impl HarnessStats {
    pub fn record_completion(&mut self, duration_ms: u64, steps: usize, errors: usize) {
        self.total_completed += 1;
        self.total_steps_executed += steps as u64;
        self.total_errors += errors as u64;

        // Update rolling average
        let n = self.total_completed as f64;
        let new_avg = (self.avg_execution_time_ms * (n - 1.0) + duration_ms as f64) / n;
        self.avg_execution_time_ms = new_avg;
    }
}

#[derive(Debug, Clone)]
struct HarnessSnapshot {
    last_active_at: DateTime<Utc>,
    step_count: usize,
    error_count: usize,
    duration_ms: u64,
    execution_metadata: ExecutionMetadata,
    error_history: Vec<HarnessError>,
}

struct HarnessManagerProgressSink {
    harness_id: String,
    info: Arc<RwLock<HashMap<String, HarnessInfo>>>,
}

#[async_trait]
impl HarnessExecutionProgressSink for HarnessManagerProgressSink {
    async fn persist(&self, state: HarnessExecutionState) {
        let mut info_map = self.info.write().await;
        if let Some(info) = info_map.get_mut(&self.harness_id) {
            info.execution_state = Some(state);
        }
    }
}

/// Manager for multiple agent harnesses
pub struct HarnessManager {
    /// Active harnesses by ID
    harnesses: Arc<RwLock<HashMap<String, Arc<RwLock<AgentHarnessContext>>>>>,
    /// Harness metadata
    info: Arc<RwLock<HashMap<String, HarnessInfo>>>,
    /// Statistics
    stats: Arc<RwLock<HarnessStats>>,
    /// Shutdown signal
    shutdown_tx: Arc<RwLock<Option<broadcast::Sender<HarnessSignal>>>>,
    /// Default configuration
    default_config: HarnessConfig,
}

impl HarnessManager {
    /// Create a new harness manager
    pub fn new() -> Self {
        Self {
            harnesses: Arc::new(RwLock::new(HashMap::new())),
            info: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(HarnessStats::default())),
            shutdown_tx: Arc::new(RwLock::new(None)),
            default_config: HarnessConfig::default(),
        }
    }

    /// Create with custom default configuration
    pub fn with_config(config: HarnessConfig) -> Self {
        Self {
            default_config: config,
            ..Self::new()
        }
    }

    /// Generate a unique harness ID
    fn generate_id(&self) -> String {
        format!("harness_{}", uuid::Uuid::new_v4())
    }

    /// Create a new harness instance
    pub async fn create_harness(
        &self,
        config: Option<HarnessConfig>,
    ) -> Result<String, HarnessError> {
        let id = self.generate_id();
        let cfg = config.unwrap_or_else(|| self.default_config.clone());

        let harness = AgentHarnessContext::new(cfg.clone());
        let info = HarnessInfo::new(id.clone(), cfg);

        let mut harnesses = self.harnesses.write().await;
        harnesses.insert(id.clone(), Arc::new(RwLock::new(harness)));

        let mut info_map = self.info.write().await;
        info_map.insert(id.clone(), info);

        let mut stats = self.stats.write().await;
        stats.total_created += 1;

        Ok(id)
    }

    pub fn default_config(&self) -> &HarnessConfig {
        &self.default_config
    }

    /// Get a harness by ID
    pub async fn get_harness(&self, id: &str) -> Option<Arc<RwLock<AgentHarnessContext>>> {
        let harnesses = self.harnesses.read().await;
        harnesses.get(id).cloned()
    }

    async fn snapshot_harness(&self, id: &str) -> Option<HarnessSnapshot> {
        let harness = {
            let harnesses = self.harnesses.read().await;
            harnesses.get(id).cloned()
        }?;

        let harness = harness.read().await;
        let metadata = harness.metadata();
        Some(HarnessSnapshot {
            last_active_at: metadata.last_activity_at,
            step_count: metadata.step_count,
            error_count: metadata.error_count,
            duration_ms: metadata.total_duration_ms,
            execution_metadata: metadata,
            error_history: harness.error_history(),
        })
    }

    /// Get harness info
    pub async fn get_info(&self, id: &str) -> Option<HarnessInfo> {
        let snapshot = self.snapshot_harness(id).await;
        let mut info_map = self.info.write().await;
        if let Some(info) = info_map.get_mut(id) {
            if let Some(snapshot) = snapshot {
                info.last_active_at = snapshot.last_active_at;
                info.step_count = snapshot.step_count;
                info.error_count = snapshot.error_count;
                info.execution_metadata = Some(snapshot.execution_metadata);
                info.error_history = snapshot.error_history;
            }
            return Some(info.clone());
        }
        None
    }

    pub async fn get_execution_state(&self, id: &str) -> Option<HarnessExecutionState> {
        let info_map = self.info.read().await;
        info_map
            .get(id)
            .and_then(|info| info.execution_state.clone())
    }

    pub async fn set_harness_config(
        &self,
        id: &str,
        config: HarnessConfig,
    ) -> Result<(), HarnessError> {
        let harness = self
            .get_harness(id)
            .await
            .ok_or(HarnessError::ContextClosed)?;
        let mut merged_config = config;
        {
            let info_map = self.info.read().await;
            let existing = info_map.get(id).ok_or(HarnessError::ContextClosed)?;
            for (key, value) in &existing.config.metadata {
                merged_config
                    .metadata
                    .entry(key.clone())
                    .or_insert_with(|| value.clone());
            }
        }
        {
            let mut harness_guard = harness.write().await;
            harness_guard.replace_config(merged_config.clone());
        }

        let mut info_map = self.info.write().await;
        let info = info_map.get_mut(id).ok_or(HarnessError::ContextClosed)?;
        info.config = merged_config;
        info.last_active_at = Utc::now();
        Ok(())
    }

    /// List all harness IDs
    pub async fn list_harnesses(&self) -> Vec<String> {
        let info_map = self.info.read().await;
        info_map.keys().cloned().collect()
    }

    /// Get all harness info
    pub async fn list_harness_info(&self) -> Vec<HarnessInfo> {
        let ids = {
            let info_map = self.info.read().await;
            info_map.keys().cloned().collect::<Vec<_>>()
        };

        for id in ids {
            if let Some(snapshot) = self.snapshot_harness(&id).await {
                let mut info_map = self.info.write().await;
                if let Some(info) = info_map.get_mut(&id) {
                    info.last_active_at = snapshot.last_active_at;
                    info.step_count = snapshot.step_count;
                    info.error_count = snapshot.error_count;
                    info.execution_metadata = Some(snapshot.execution_metadata);
                    info.error_history = snapshot.error_history;
                }
            }
        }

        let info_map = self.info.read().await;
        info_map.values().cloned().collect()
    }

    /// Update harness status
    pub async fn update_status(&self, id: &str, status: HarnessStatus) {
        let snapshot = self.snapshot_harness(id).await;
        let mut info_map = self.info.write().await;
        if let Some(info) = info_map.get_mut(id) {
            info.status = status;
            if let Some(snapshot) = snapshot {
                info.last_active_at = snapshot.last_active_at;
                info.step_count = snapshot.step_count;
                info.error_count = snapshot.error_count;
                info.execution_metadata = Some(snapshot.execution_metadata);
                info.error_history = snapshot.error_history;
            } else {
                info.last_active_at = Utc::now();
            }
        }
    }

    fn fallback_execution_metadata(info: &HarnessInfo) -> ExecutionMetadata {
        let total_duration_ms = info
            .last_active_at
            .signed_duration_since(info.created_at)
            .num_milliseconds()
            .max(0) as u64;

        ExecutionMetadata {
            started_at: info.created_at,
            last_activity_at: info.last_active_at,
            total_duration_ms,
            step_count: info.step_count,
            error_count: info.error_count.max(info.error_history.len()),
            tool_call_count: 0,
            successful_tool_calls: 0,
            paused: matches!(info.status, HarnessStatus::Paused),
            cancelled: matches!(info.status, HarnessStatus::Cancelled),
        }
    }

    pub async fn set_execution_state(
        &self,
        id: &str,
        state: HarnessExecutionState,
    ) -> Result<(), HarnessError> {
        let mut info_map = self.info.write().await;
        if let Some(info) = info_map.get_mut(id) {
            info.execution_state = Some(state);
            Ok(())
        } else {
            Err(HarnessError::ContextClosed)
        }
    }

    fn progress_sink(&self, id: &str) -> Arc<dyn HarnessExecutionProgressSink> {
        Arc::new(HarnessManagerProgressSink {
            harness_id: id.to_string(),
            info: self.info.clone(),
        })
    }

    pub(crate) async fn prepare_agent_execution(
        &self,
        id: &str,
        state: HarnessExecutionState,
    ) -> Result<Arc<RwLock<AgentHarnessContext>>, HarnessError> {
        let harness = self
            .get_harness(id)
            .await
            .ok_or(HarnessError::ContextClosed)?;

        self.update_status(id, HarnessStatus::Running).await;
        self.set_execution_state(id, state).await?;

        {
            let harness_guard = harness.write().await;
            if harness_guard.is_paused() {
                harness_guard.resume();
            }
        }

        Ok(harness)
    }

    pub(crate) async fn execute_agent_on_prepared_harness<A: HarnessAgent>(
        &self,
        id: &str,
        builder: &HarnessAgentBuilder<A>,
        harness: Arc<RwLock<AgentHarnessContext>>,
        state: HarnessExecutionState,
    ) -> Result<HarnessAgentResult, HarnessError> {
        let result = builder
            .clone()
            .with_progress_sink(self.progress_sink(id))
            .with_shared_harness(harness)
            .execute_from_state(state.clone())
            .await
            .map_err(|error| HarnessError::LlmFailure(error.to_string()))?;
        let updated_state = self
            .get_execution_state(id)
            .await
            .unwrap_or(state)
            .with_trace(result.trace.clone());
        self.set_execution_state(id, updated_state).await?;

        if result.success {
            self.complete_harness(id).await?;
        } else {
            self.fail_harness(id).await?;
        }

        Ok(result)
    }

    pub(crate) async fn run_agent_on_harness<A: HarnessAgent>(
        &self,
        id: &str,
        builder: &HarnessAgentBuilder<A>,
        task: &str,
        context: &[Message],
    ) -> Result<HarnessAgentResult, HarnessError> {
        self.set_harness_config(id, builder.config().clone())
            .await?;
        let state = builder.execution_state(task, context);
        let harness = self.prepare_agent_execution(id, state.clone()).await?;
        self.execute_agent_on_prepared_harness(id, builder, harness, state)
            .await
    }

    pub(crate) async fn resume_agent_on_harness<A: HarnessAgent>(
        &self,
        id: &str,
        builder: &HarnessAgentBuilder<A>,
    ) -> Result<HarnessAgentResult, HarnessError> {
        self.set_harness_config(id, builder.config().clone())
            .await?;
        let state = self
            .get_execution_state(id)
            .await
            .ok_or_else(|| HarnessError::ResumeStateUnavailable(id.to_string()))?;
        let harness = self.prepare_agent_execution(id, state.clone()).await?;
        self.execute_agent_on_prepared_harness(id, builder, harness, state)
            .await
    }

    /// Adopt an externally tracked harness into the local manager using the provided ID.
    pub async fn adopt_harness(
        &self,
        info: HarnessInfo,
    ) -> Result<Arc<RwLock<AgentHarnessContext>>, HarnessError> {
        let id = info.id.clone();

        if let Some(existing) = self.get_harness(&id).await {
            {
                let mut info_map = self.info.write().await;
                info_map.insert(id, info);
            }
            return Ok(existing);
        }

        let harness = Arc::new(RwLock::new(AgentHarnessContext::new(info.config.clone())));
        let metadata = info
            .execution_metadata
            .clone()
            .unwrap_or_else(|| Self::fallback_execution_metadata(&info));

        {
            let guard = harness.write().await;
            guard.restore_snapshot(metadata, info.error_history.clone());
        }

        {
            let mut harnesses = self.harnesses.write().await;
            harnesses.insert(id.clone(), harness.clone());
        }

        {
            let mut info_map = self.info.write().await;
            info_map.insert(id, info);
        }

        Ok(harness)
    }

    /// Cancel a harness
    pub async fn cancel_harness(&self, id: &str) -> Result<(), HarnessError> {
        let harnesses = self.harnesses.read().await;
        if let Some(harness) = harnesses.get(id) {
            harness.write().await.cancel();
            drop(harnesses);

            self.update_status(id, HarnessStatus::Cancelled).await;

            let mut stats = self.stats.write().await;
            stats.total_cancelled += 1;

            Ok(())
        } else {
            Err(HarnessError::ContextClosed)
        }
    }

    /// Pause a harness
    pub async fn pause_harness(&self, id: &str) -> Result<(), HarnessError> {
        let harnesses = self.harnesses.read().await;
        if let Some(harness) = harnesses.get(id) {
            harness.write().await.pause();
            self.update_status(id, HarnessStatus::Paused).await;
            Ok(())
        } else {
            Err(HarnessError::ContextClosed)
        }
    }

    /// Resume a harness
    pub async fn resume_harness(&self, id: &str) -> Result<(), HarnessError> {
        let harnesses = self.harnesses.read().await;
        if let Some(harness) = harnesses.get(id) {
            harness.write().await.resume();
            self.update_status(id, HarnessStatus::Running).await;
            Ok(())
        } else {
            Err(HarnessError::ContextClosed)
        }
    }

    /// Mark a harness as completed and roll its execution into aggregate stats
    pub async fn complete_harness(&self, id: &str) -> Result<(), HarnessError> {
        let snapshot = self
            .snapshot_harness(id)
            .await
            .ok_or(HarnessError::ContextClosed)?;
        let mut should_record_stats = false;

        {
            let mut info_map = self.info.write().await;
            if let Some(info) = info_map.get_mut(id) {
                if !matches!(info.status, HarnessStatus::Completed) {
                    should_record_stats = true;
                }
                info.status = HarnessStatus::Completed;
                info.last_active_at = snapshot.last_active_at;
                info.step_count = snapshot.step_count;
                info.error_count = snapshot.error_count;
                info.execution_metadata = Some(snapshot.execution_metadata);
                info.error_history = snapshot.error_history;
            } else {
                return Err(HarnessError::ContextClosed);
            }
        }

        if should_record_stats {
            let mut stats = self.stats.write().await;
            stats.record_completion(
                snapshot.duration_ms,
                snapshot.step_count,
                snapshot.error_count,
            );
        }
        Ok(())
    }

    /// Mark a harness as failed and roll the failure into aggregate stats once
    pub async fn fail_harness(&self, id: &str) -> Result<(), HarnessError> {
        let snapshot = self
            .snapshot_harness(id)
            .await
            .ok_or(HarnessError::ContextClosed)?;
        let mut should_record_failure = false;

        {
            let mut info_map = self.info.write().await;
            if let Some(info) = info_map.get_mut(id) {
                if !matches!(info.status, HarnessStatus::Failed) {
                    should_record_failure = true;
                }
                info.status = HarnessStatus::Failed;
                info.last_active_at = snapshot.last_active_at;
                info.step_count = snapshot.step_count;
                info.error_count = snapshot.error_count;
                info.execution_metadata = Some(snapshot.execution_metadata);
                info.error_history = snapshot.error_history;
            } else {
                return Err(HarnessError::ContextClosed);
            }
        }

        if should_record_failure {
            let mut stats = self.stats.write().await;
            stats.total_failed += 1;
            stats.total_steps_executed += snapshot.step_count as u64;
            stats.total_errors += snapshot.error_count as u64;
        }

        Ok(())
    }

    /// Execute a harness-aware agent under manager lifecycle control.
    pub async fn run_agent<A: HarnessAgent>(
        &self,
        builder: &HarnessAgentBuilder<A>,
        task: &str,
        context: &[Message],
    ) -> Result<(String, HarnessAgentResult), HarnessError> {
        let id = self.create_harness(Some(builder.config().clone())).await?;
        let result = self
            .run_agent_on_harness(&id, builder, task, context)
            .await?;

        Ok((id, result))
    }

    /// Resume a previously tracked harness-aware agent from its saved execution state.
    pub async fn resume_agent<A: HarnessAgent>(
        &self,
        id: &str,
        builder: &HarnessAgentBuilder<A>,
    ) -> Result<HarnessAgentResult, HarnessError> {
        self.resume_agent_on_harness(id, builder).await
    }

    /// Remove a harness
    pub async fn remove_harness(&self, id: &str) -> bool {
        let mut harnesses = self.harnesses.write().await;
        let mut info_map = self.info.write().await;

        if harnesses.remove(id).is_some() {
            info_map.remove(id);
            true
        } else {
            false
        }
    }

    /// Get statistics
    pub async fn get_stats(&self) -> HarnessStats {
        self.stats.read().await.clone()
    }

    /// Cancel all harnesses (graceful shutdown)
    pub async fn cancel_all(&self) {
        let harnesses = self.harnesses.read().await;
        for (_, harness) in harnesses.iter() {
            harness.write().await.cancel();
        }
    }

    /// Shutdown the manager
    pub async fn shutdown(&self) {
        self.cancel_all().await;

        let mut harnesses = self.harnesses.write().await;
        harnesses.clear();

        let mut info_map = self.info.write().await;
        info_map.clear();
    }
}

impl Default for HarnessManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Scope guard for harness execution
pub struct HarnessScope<'a> {
    manager: &'a HarnessManager,
    id: String,
    start_time: Instant,
}

impl<'a> HarnessScope<'a> {
    pub async fn new(
        manager: &'a HarnessManager,
        config: Option<HarnessConfig>,
    ) -> Result<Self, HarnessError> {
        let id = manager.create_harness(config).await?;
        manager.update_status(&id, HarnessStatus::Running).await;

        Ok(Self {
            manager,
            id,
            start_time: Instant::now(),
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the harness ID for this scope
    pub fn get_harness_id(&self) -> &str {
        &self.id
    }

    /// Elapsed time since scope creation
    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }
}

impl<'a> Drop for HarnessScope<'a> {
    fn drop(&mut self) {
        // Note: Cannot use async in drop, so status update is best-effort
        // In practice, the harness itself tracks completion
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_get_harness() {
        let manager = HarnessManager::new();

        let id = manager.create_harness(None).await.unwrap();
        assert!(manager.get_harness(&id).await.is_some());

        manager.shutdown().await;
    }

    #[tokio::test]
    async fn test_harness_lifecycle() {
        let manager = HarnessManager::new();
        let config = HarnessConfig {
            max_steps: 5,
            ..Default::default()
        };

        let id = manager.create_harness(Some(config)).await.unwrap();

        // Check initial status
        let info = manager.get_info(&id).await.unwrap();
        assert!(matches!(info.status, HarnessStatus::Idle));

        // Update to running
        manager.update_status(&id, HarnessStatus::Running).await;
        let info = manager.get_info(&id).await.unwrap();
        assert!(matches!(info.status, HarnessStatus::Running));

        // Cancel
        manager.cancel_harness(&id).await.unwrap();
        let info = manager.get_info(&id).await.unwrap();
        assert!(matches!(info.status, HarnessStatus::Cancelled));

        // Remove
        assert!(manager.remove_harness(&id).await);
        assert!(manager.get_harness(&id).await.is_none());

        manager.shutdown().await;
    }

    #[tokio::test]
    async fn test_stats_tracking() {
        let manager = HarnessManager::new();

        manager.create_harness(None).await.unwrap();
        manager.create_harness(None).await.unwrap();

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_created, 2);

        manager.shutdown().await;
    }

    #[tokio::test]
    async fn test_complete_harness_updates_snapshot_and_stats() {
        let manager = HarnessManager::new();
        let id = manager.create_harness(None).await.unwrap();

        let harness = manager.get_harness(&id).await.unwrap();
        {
            let harness = harness.write().await;
            harness.record_step();
            harness.record_error(HarnessError::Timeout(std::time::Duration::from_millis(5)));
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

        manager.shutdown().await;
    }
}
