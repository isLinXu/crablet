//! Distributed Harness Manager - Main orchestrator for distributed harness coordination
//!
//! This module manages the lifecycle and coordination of distributed harnesses:
//! - Creates and destroys harness instances across nodes
//! - Monitors harness health and automatically fails over on node failure
//! - Provides a unified API for harness management

use std::collections::HashMap;
use std::sync::{Arc, RwLock, Weak};

use chrono::Utc;

use super::types::{DistributedConfig, DistributedError, NodeId, NodeInfo, NodeStatus};
use super::election::LeaderElection;
use super::health::{HealthMonitor, NodeHealth};

/// Manages distributed harness instances
pub struct DistributedHarnessManager {
    /// This node's identifier
    node_id: NodeId,
    /// Maps node ID -> harness info for each node
    nodes: Arc<RwLock<HashMap<NodeId, Weak<Self>>>>,
    /// Leader election for coordinating across nodes
    election: Option<LeaderElection>,
    /// Health monitor for detecting node failures
    health_monitor: Option<HealthMonitor>,
    /// Distributed statistics
    stats: DistributedStats,
    /// Signal for graceful shutdown
    shutdown_signal: Option<ShutdownSignal>,
}

/// Statistics for distributed harness
#[derive(Debug, Clone)]
pub struct DistributedStats {
    pub total_nodes: usize,
    pub active_nodes: usize,
    pub total_harnesses: usize,
}

/// Signal for graceful shutdown
#[derive(Debug, Clone)]
pub struct ShutdownSignal {
    /// Unique identifier for this shutdown signal
    pub signal_id: String,
    /// When the shutdown was initiated
    pub initiated_at: chrono::DateTime<Utc>,
    /// Which node initiated the shutdown
    pub source_node_id: String,
    /// The reason for the shutdown
    pub reason: String,
}

impl DistributedHarnessManager {
    /// Creates a new distributed harness manager
    pub fn new(
        node_id: NodeId,
        nodes: Arc<RwLock<HashMap<NodeId, Weak<Self>>>>,
        election: Option<LeaderElection>,
        health: Option<HealthMonitor>,
    ) -> Self {
        Self {
            node_id,
            nodes,
            election,
            health_monitor: health,
            stats: DistributedStats {
                total_nodes: 0,
                active_nodes: 0,
                total_harnesses: 0,
            },
            shutdown_signal: None,
        }
    }

    /// Creates a new distributed harness manager with default settings
    pub fn with_defaults() -> Self {
        Self::new(
            "default-node".to_string(),
            Arc::new(RwLock::new(HashMap::new())),
            None,
            None,
        )
    }

    /// Returns this node's identifier
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Creates a harness on this node
    pub async fn create_harness(
        &self,
        _config: Option<super::harness::HarnessConfig>,
    ) -> Result<String, DistributedError> {
        // In a full implementation, this would create a local harness instance
        // and register it with the backend
        let harness_id = format!("harness-{}", uuid::Uuid::new_v4());
        self.stats.total_harnesses += 1;
        Ok(harness_id)
    }

    /// Applies a local signal to a harness on this node
    pub async fn apply_local_signal(
        &self,
        _harness_id: &str,
        _signal: super::harness::HarnessSignal,
    ) -> Result<(), DistributedError> {
        // In a full implementation, this would forward the signal to the
        // local harness manager
        Ok(())
    }

    /// Removes a harness from this node
    pub async fn remove_harness(&self, _harness_id: &str) -> Result<(), DistributedError> {
        // In a full implementation, this would stop and remove the local harness
        if self.stats.total_harnesses > 0 {
            self.stats.total_harnesses -= 1;
        }
        Ok(())
    }

    /// Gets a harness by node ID
    pub fn get_harness(&self, node_id: &NodeId) -> Option<Arc<Self>> {
        self.nodes
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(node_id)
            .and_then(|weak| weak.upgrade())
    }

    /// Initiates a graceful shutdown
    pub fn initiate_shutdown(&mut self, reason: String) {
        self.shutdown_signal = Some(ShutdownSignal {
            signal_id: uuid::Uuid::new_v4().to_string(),
            initiated_at: Utc::now(),
            source_node_id: self.node_id.clone(),
            reason,
        });
    }

    /// Gets the current distributed statistics
    pub fn stats(&self) -> &DistributedStats {
        &self.stats
    }

    /// Updates statistics from cluster state
    pub fn update_stats(&mut self, total_nodes: usize, active_nodes: usize, total_harnesses: usize) {
        self.stats.total_nodes = total_nodes;
        self.stats.active_nodes = active_nodes;
        self.stats.total_harnesses = total_harnesses;
    }

    /// Checks if this node is the leader
    pub async fn is_leader(&self) -> bool {
        if let Some(ref election) = self.election {
            election.is_leader().await.unwrap_or(false)
        } else {
            false
        }
    }

    /// Gets health information for all known nodes
    pub fn compute_node_health(&self, nodes: &[NodeInfo]) -> Vec<NodeHealth> {
        match &self.health_monitor {
            Some(hm) => hm.compute_node_health(nodes),
            None => nodes
                .iter()
                .map(|node| NodeHealth {
                    node_id: node.id.clone(),
                    is_healthy: node.status == NodeStatus::Active,
                    harness_count: node.harness_count,
                    last_heartbeat_ago_secs: 0,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_manager() {
        let manager = DistributedHarnessManager::with_defaults();
        assert_eq!(manager.node_id(), "default-node");
        assert_eq!(manager.stats.total_nodes, 0);
        assert_eq!(manager.stats.active_nodes, 0);
        assert_eq!(manager.stats.total_harnesses, 0);
    }

    #[test]
    fn test_manager_stats() {
        let mut manager = DistributedHarnessManager::with_defaults();
        manager.update_stats(10, 8, 5);

        let stats = manager.stats();
        assert_eq!(stats.total_nodes, 10);
        assert_eq!(stats.active_nodes, 8);
        assert_eq!(stats.total_harnesses, 5);
    }

    #[test]
    fn test_shutdown_signal() {
        let mut manager = DistributedHarnessManager::with_defaults();
        assert!(manager.shutdown_signal.is_none());

        manager.initiate_shutdown("maintenance".to_string());
        assert!(manager.shutdown_signal.is_some());
        assert_eq!(manager.shutdown_signal.as_ref().unwrap().reason, "maintenance");
    }

    #[tokio::test]
    async fn test_create_harness() {
        let manager = DistributedHarnessManager::with_defaults();
        let result = manager.create_harness(None).await;
        assert!(result.is_ok());
        assert!(result.unwrap().starts_with("harness-"));
    }

    #[tokio::test]
    async fn test_remove_harness() {
        let mut manager = DistributedHarnessManager::with_defaults();
        manager.stats.total_harnesses = 1;
        let result = manager.remove_harness("test-harness").await;
        assert!(result.is_ok());
        assert_eq!(manager.stats.total_harnesses, 0);
    }
}
