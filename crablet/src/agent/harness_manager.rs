//! Harness Manager - Manages multiple agent harness instances
//!
//! Provides:
//! - Multi-agent harness lifecycle management
//! - Shared harness pool for efficiency
//! - Harness state monitoring and metrics
//! - Graceful shutdown coordination

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, RwLock};

use super::harness::{AgentHarnessContext, HarnessConfig, HarnessError, HarnessSignal};

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
        Some(HarnessSnapshot {
            last_active_at: harness.metadata().last_activity_at,
            step_count: harness.metadata().step_count,
            error_count: harness.error_count(),
            duration_ms: harness.metadata().total_duration_ms,
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
            }
            return Some(info.clone());
        }
        None
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
            } else {
                info.last_active_at = Utc::now();
            }
        }
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
            let mut harness = harness.write().await;
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
