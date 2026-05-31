//! Health monitoring for distributed harness nodes

use serde::{Deserialize, Serialize};

use super::harness_manager::HarnessStats;
use super::manager::DistributedStats;

/// Cluster-wide statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterStats {
    pub local_stats: HarnessStats,
    pub distributed_stats: DistributedStats,
    pub total_nodes: usize,
    pub active_nodes: usize,
    pub total_harnesses: usize,
}

/// Node health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHealth {
    pub node_id: String,
    pub is_healthy: bool,
    pub harness_count: usize,
    pub last_heartbeat_ago_secs: u64,
}

/// Health monitor for tracking node health in the cluster
pub struct HealthMonitor {
    node_timeout_secs: u64,
}

impl HealthMonitor {
    pub fn new(node_timeout_secs: u64) -> Self {
        Self { node_timeout_secs }
    }

    /// Check if a node is healthy based on its last heartbeat
    pub fn is_node_healthy(&self, last_heartbeat_ago_secs: u64) -> bool {
        last_heartbeat_ago_secs < self.node_timeout_secs
    }

    /// Compute health info for all known nodes
    pub fn compute_node_health(
        &self,
        nodes: &[super::types::NodeInfo],
    ) -> Vec<NodeHealth> {
        let now = chrono::Utc::now();
        nodes
            .iter()
            .map(|node| {
                let ago = (now - node.last_heartbeat).num_seconds().max(0) as u64;
                NodeHealth {
                    node_id: node.id.clone(),
                    is_healthy: self.is_node_healthy(ago),
                    harness_count: node.harness_count,
                    last_heartbeat_ago_secs: ago,
                }
            })
            .collect()
    }
}
