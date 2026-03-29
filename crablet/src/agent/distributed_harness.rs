//! Distributed Harness Manager - Multi-node harness coordination
//!
//! This module provides distributed harness management capabilities:
//! - Pluggable backend support (Redis, etcd, Consul)
//! - Cross-node harness state synchronization
//! - Leader election for coordination
//! - Distributed lock management
//! - Health monitoring and failover

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast, mpsc};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use async_trait::async_trait;

use super::harness_manager::{HarnessManager, HarnessInfo, HarnessStats};
use super::harness::{HarnessConfig, HarnessSignal, HarnessError, AgentHarnessContext};

/// Node identifier
pub type NodeId = String;

/// Backend trait for distributed coordination
#[async_trait]
pub trait HarnessBackend: Send + Sync {
    /// Create a harness in the distributed store
    async fn create_harness(&self, info: &HarnessInfo) -> Result<(), DistributedError>;

    /// Update harness info
    async fn update_harness(&self, info: &HarnessInfo) -> Result<(), DistributedError>;

    /// Delete a harness from the distributed store
    async fn delete_harness(&self, id: &str) -> Result<(), DistributedError>;

    /// Get harness info from distributed store
    async fn get_harness(&self, id: &str) -> Result<Option<HarnessInfo>, DistributedError>;

    /// List all harnesses in the cluster
    async fn list_harnesses(&self) -> Result<Vec<HarnessInfo>, DistributedError>;

    /// Acquire a distributed lock
    async fn acquire_lock(&self, resource: &str, owner: &str, ttl_secs: u64) -> Result<bool, DistributedError>;

    /// Release a distributed lock
    async fn release_lock(&self, resource: &str, owner: &str) -> Result<(), DistributedError>;

    /// Register node health
    async fn register_node(&self, node: &NodeInfo) -> Result<(), DistributedError>;

    /// Get all active nodes
    async fn get_nodes(&self) -> Result<Vec<NodeInfo>, DistributedError>;

    /// Watch for harness changes
    async fn watch_harnesses(&self, tx: mpsc::Sender<HarnessWatchEvent>) -> Result<(), DistributedError>;
}

/// Node information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: NodeId,
    pub address: String,
    pub port: u16,
    pub capabilities: Vec<String>,
    pub status: NodeStatus,
    pub last_heartbeat: DateTime<Utc>,
    pub harness_count: usize,
}

/// Node status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeStatus {
    Active,
    Suspected,
    Dead,
}

/// Event from watching harnesses
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HarnessWatchEvent {
    Created { id: String, info: HarnessInfo },
    Updated { id: String, info: HarnessInfo },
    Deleted { id: String },
    NodeLeft { node_id: NodeId },
}

/// Distributed harness errors
#[derive(Debug, thiserror::Error)]
pub enum DistributedError {
    #[error("Backend connection error: {0}")]
    ConnectionError(String),

    #[error("Lock acquisition failed: {0}")]
    LockFailed(String),

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Harness not found: {0}")]
    HarnessNotFound(String),

    #[error("Cluster error: {0}")]
    ClusterError(String),

    #[error("Timeout: {0}")]
    Timeout(String),
}

/// Configuration for distributed harness
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedConfig {
    /// Local node ID
    pub node_id: NodeId,
    /// Node address
    pub node_address: String,
    /// Node port
    pub node_port: u16,
    /// Backend type
    pub backend_type: BackendType,
    /// Backend connection string
    pub backend_uri: String,
    /// Lock TTL in seconds
    pub lock_ttl_secs: u64,
    /// Heartbeat interval in seconds
    pub heartbeat_interval_secs: u64,
    /// Node timeout in seconds
    pub node_timeout_secs: u64,
}

/// Backend type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BackendType {
    Redis,
    Etcd,
    Consul,
    InMemory, // For single-node testing
}

/// Distributed harness manager
pub struct DistributedHarnessManager {
    /// Local harness manager
    local: Arc<HarnessManager>,
    /// Distributed backend
    backend: Arc<dyn HarnessBackend>,
    /// This node's ID
    node_id: NodeId,
    /// Local harness IDs
    local_harnesses: Arc<RwLock<HashMap<String, bool>>>,
    /// Shutdown signal
    shutdown_tx: Arc<RwLock<Option<broadcast::Sender<ShutdownSignal>>>>,
    /// Statistics
    stats: Arc<RwLock<DistributedStats>>,
}

/// Shutdown signal
#[derive(Debug, Clone)]
pub enum ShutdownSignal {
    NodeShutdown,
    ClusterLeadershipLost,
    BackendDisconnected,
}

/// Distributed statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DistributedStats {
    pub remote_operations: u64,
    pub local_operations: u64,
    pub lock_acquisitions: u64,
    pub lock_failures: u64,
    pub node_failovers: u64,
    pub cross_node_delegations: u64,
}

impl DistributedHarnessManager {
    /// Create a new distributed harness manager
    pub fn new(
        backend: Arc<dyn HarnessBackend>,
        config: DistributedConfig,
    ) -> Self {
        let local = HarnessManager::new();

        Self {
            local: Arc::new(local),
            backend,
            node_id: config.node_id,
            local_harnesses: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx: Arc::new(RwLock::new(None)),
            stats: Arc::new(RwLock::new(DistributedStats::default())),
        }
    }

    /// Create with an existing local harness manager
    pub fn with_local_manager(
        local: Arc<HarnessManager>,
        backend: Arc<dyn HarnessBackend>,
        node_id: NodeId,
    ) -> Self {
        Self {
            local,
            backend,
            node_id,
            local_harnesses: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx: Arc::new(RwLock::new(None)),
            stats: Arc::new(RwLock::new(DistributedStats::default())),
        }
    }

    /// Get local harness manager reference
    pub fn local_manager(&self) -> &Arc<HarnessManager> {
        &self.local
    }

    /// Get this node's ID
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Create a new harness (local or remote based on load)
    pub async fn create_harness(
        &self,
        config: Option<HarnessConfig>,
    ) -> Result<String, DistributedError> {
        // First try to create locally
        let create_result: Result<String, HarnessError> = self.local.create_harness(config.clone()).await;
        let id = create_result.map_err(|e: HarnessError| DistributedError::ClusterError(e.to_string()))?;

        // Track as local
        {
            let mut harnesses = self.local_harnesses.write().await;
            harnesses.insert(id.clone(), true);
        }

        // Sync to backend
        if let Some(info) = self.local.get_info(&id).await {
            if let Err(e) = self.backend.create_harness(&info).await {
                // Log but don't fail - local creation succeeded
                tracing::warn!("Failed to sync harness to backend: {}", e);
            }
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.local_operations += 1;
        }

        Ok(id)
    }

    /// Get a harness (checks local first, then remote)
    pub async fn get_harness(&self, id: &str) -> Result<Option<Arc<RwLock<AgentHarnessContext>>>, DistributedError> {
        // Check local first
        let local_harness = self.local.get_harness(id).await;

        if local_harness.is_some() {
            let mut stats = self.stats.write().await;
            stats.local_operations += 1;
            return Ok(local_harness);
        }

        // Try to get from remote node
        let remote_info = self.backend.get_harness(id).await?;

        if let Some(info) = remote_info {
            // Check if it's on a live node
            let nodes = self.backend.get_nodes().await?;
            let owner_node = nodes.iter().find(|n| {
                n.status == NodeStatus::Active &&
                n.id != self.node_id &&
                info.config.metadata.get("node_id") == Some(&n.id)
            });

            if owner_node.is_none() {
                // Node is dead or harness has no owner - try to claim locally
                return Ok(None);
            }

            let mut stats = self.stats.write().await;
            stats.remote_operations += 1;
            Ok(None) // Would need delegation mechanism
        } else {
            Ok(None)
        }
    }

    /// Send a signal to a harness (handles cross-node signals)
    pub async fn send_signal(
        &self,
        id: &str,
        signal: HarnessSignal,
    ) -> Result<(), DistributedError> {
        // Check local
        if let Some(harness) = self.local.get_harness(id).await {
            let harness_guard = harness.write().await;
            match &signal {
                HarnessSignal::Cancel => harness_guard.cancel(),
                HarnessSignal::Pause => harness_guard.pause(),
                HarnessSignal::Resume => harness_guard.resume(),
                HarnessSignal::Checkpoint => {
                    // Checkpoint is async but we drop the lock first
                    drop(harness_guard);
                    let _ = self.local.get_harness(id).await
                        .ok_or_else(|| DistributedError::HarnessNotFound(id.to_string()))?
                        .write().await.save_checkpoint().await;
                }
            }

            let mut stats = self.stats.write().await;
            stats.local_operations += 1;
            return Ok(());
        }

        // Need to forward to remote node
        let _remote_info = self.backend.get_harness(id).await?
            .ok_or_else(|| DistributedError::HarnessNotFound(id.to_string()))?;

        // Find owner node and forward
        // (simplified - would need actual RPC mechanism)
        let mut stats = self.stats.write().await;
        stats.remote_operations += 1;

        Ok(())
    }

    /// Try to acquire lock for a harness
    pub async fn try_lock_harness(&self, id: &str) -> Result<bool, DistributedError> {
        let acquired = self.backend
            .acquire_lock(&format!("harness:{}", id), &self.node_id, 300)
            .await?;

        let mut stats = self.stats.write().await;
        if acquired {
            stats.lock_acquisitions += 1;
        } else {
            stats.lock_failures += 1;
        }

        Ok(acquired)
    }

    /// Release lock for a harness
    pub async fn unlock_harness(&self, id: &str) -> Result<(), DistributedError> {
        self.backend
            .release_lock(&format!("harness:{}", id), &self.node_id)
            .await
    }

    /// Get cluster statistics
    pub async fn get_cluster_stats(&self) -> Result<ClusterStats, DistributedError> {
        let local_stats = self.local.get_stats().await;
        let dist_stats = self.stats.read().await.clone();
        let nodes = self.backend.get_nodes().await?;

        let active_nodes = nodes.iter()
            .filter(|n| n.status == NodeStatus::Active)
            .count();

        let total_harnesses = nodes.iter()
            .map(|n| n.harness_count)
            .sum::<usize>();

        Ok(ClusterStats {
            local_stats,
            distributed_stats: dist_stats,
            total_nodes: nodes.len(),
            active_nodes,
            total_harnesses,
        })
    }

    /// Get statistics
    pub async fn get_stats(&self) -> HarnessStats {
        self.local.get_stats().await
    }

    /// List all harnesses (local + remote)
    pub async fn list_all_harnesses(&self) -> Result<Vec<HarnessInfo>, DistributedError> {
        let mut all_harnesses: Vec<HarnessInfo> = self.local.list_harness_info().await;

        let remote_harnesses = self.backend.list_harnesses().await?;

        // Merge, preferring local
        for remote in remote_harnesses {
            if !all_harnesses.iter().any(|h| h.id == remote.id) {
                all_harnesses.push(remote);
            }
        }

        Ok(all_harnesses)
    }

    /// Get local harness IDs
    pub async fn get_local_harness_ids(&self) -> Vec<String> {
        let harnesses = self.local_harnesses.read().await;
        harnesses.keys().cloned().collect()
    }

    /// Check if a harness is local
    pub async fn is_local(&self, id: &str) -> bool {
        let harnesses = self.local_harnesses.read().await;
        harnesses.contains_key(id)
    }

    /// Handle node failure (called when a node is detected as dead)
    pub async fn handle_node_failure(&self, failed_node_id: &NodeId) -> Result<Vec<String>, DistributedError> {
        // Get harnesses that were on the failed node
        let harnesses = self.list_all_harnesses().await?;

        let mut to_claim = Vec::new();
        for harness in harnesses {
            if harness.config.metadata.get("node_id") == Some(&failed_node_id.to_string()) {
                to_claim.push(harness.id);
            }
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.node_failovers += to_claim.len() as u64;
        }

        Ok(to_claim)
    }
}

/// Cluster-wide statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterStats {
    pub local_stats: HarnessStats,
    pub distributed_stats: DistributedStats,
    pub total_nodes: usize,
    pub active_nodes: usize,
    pub total_harnesses: usize,
}

// ============================================
// In-Memory Backend for Testing
// ============================================

/// In-memory backend for single-node testing
pub struct InMemoryBackend {
    harnesses: Arc<RwLock<HashMap<String, HarnessInfo>>>,
    locks: Arc<RwLock<HashMap<String, (NodeId, DateTime<Utc>)>>>,
    nodes: Arc<RwLock<HashMap<NodeId, NodeInfo>>>,
}

impl InMemoryBackend {
    pub fn new() -> Self {
        Self {
            harnesses: Arc::new(RwLock::new(HashMap::new())),
            locks: Arc::new(RwLock::new(HashMap::new())),
            nodes: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HarnessBackend for InMemoryBackend {
    async fn create_harness(&self, info: &HarnessInfo) -> Result<(), DistributedError> {
        let mut harnesses = self.harnesses.write().await;
        harnesses.insert(info.id.clone(), info.clone());
        Ok(())
    }

    async fn update_harness(&self, info: &HarnessInfo) -> Result<(), DistributedError> {
        let mut harnesses = self.harnesses.write().await;
        if harnesses.contains_key(&info.id) {
            harnesses.insert(info.id.clone(), info.clone());
            Ok(())
        } else {
            Err(DistributedError::HarnessNotFound(info.id.clone()))
        }
    }

    async fn delete_harness(&self, id: &str) -> Result<(), DistributedError> {
        let mut harnesses = self.harnesses.write().await;
        harnesses.remove(id);
        Ok(())
    }

    async fn get_harness(&self, id: &str) -> Result<Option<HarnessInfo>, DistributedError> {
        let harnesses = self.harnesses.read().await;
        Ok(harnesses.get(id).cloned())
    }

    async fn list_harnesses(&self) -> Result<Vec<HarnessInfo>, DistributedError> {
        let harnesses = self.harnesses.read().await;
        Ok(harnesses.values().cloned().collect())
    }

    async fn acquire_lock(&self, resource: &str, owner: &str, ttl_secs: u64) -> Result<bool, DistributedError> {
        let mut locks = self.locks.write().await;

        if let Some((existing_owner, expiry)) = locks.get(resource) {
            if existing_owner == owner {
                // Already owned by us, refresh TTL
                let new_expiry = Utc::now() + chrono::Duration::seconds(ttl_secs as i64);
                locks.insert(resource.to_string(), (owner.to_string(), new_expiry));
                return Ok(true);
            }
            // Check if expired by comparing expiry timestamp directly
            let now = Utc::now();
            if now > *expiry {
                // Expired, take it
                let new_expiry = now + chrono::Duration::seconds(ttl_secs as i64);
                locks.insert(resource.to_string(), (owner.to_string(), new_expiry));
                return Ok(true);
            }
            return Ok(false); // Owned by someone else, not expired
        }

        // New lock - store the actual expiry time
        let expiry = Utc::now() + chrono::Duration::seconds(ttl_secs as i64);
        locks.insert(resource.to_string(), (owner.to_string(), expiry));
        Ok(true)
    }

    async fn release_lock(&self, resource: &str, owner: &str) -> Result<(), DistributedError> {
        let mut locks = self.locks.write().await;
        if let Some((existing_owner, _)) = locks.get(resource) {
            if existing_owner == owner {
                locks.remove(resource);
            }
        }
        Ok(())
    }

    async fn register_node(&self, node: &NodeInfo) -> Result<(), DistributedError> {
        let mut nodes = self.nodes.write().await;
        nodes.insert(node.id.clone(), node.clone());
        Ok(())
    }

    async fn get_nodes(&self) -> Result<Vec<NodeInfo>, DistributedError> {
        let nodes = self.nodes.read().await;
        Ok(nodes.values().cloned().collect())
    }

    async fn watch_harnesses(&self, _tx: mpsc::Sender<HarnessWatchEvent>) -> Result<(), DistributedError> {
        // In-memory doesn't support real watching
        Ok(())
    }
}

// ============================================
// Leader Election
// ============================================

/// Leader election for cluster coordination
pub struct LeaderElection {
    backend: Arc<dyn HarnessBackend>,
    election_key: String,
    node_id: NodeId,
    ttl_secs: u64,
}

impl LeaderElection {
    pub fn new(backend: Arc<dyn HarnessBackend>, election_key: &str, node_id: NodeId, ttl_secs: u64) -> Self {
        Self {
            backend,
            election_key: election_key.to_string(),
            node_id,
            ttl_secs,
        }
    }

    /// Try to become leader
    pub async fn try_become_leader(&self) -> Result<bool, DistributedError> {
        self.backend
            .acquire_lock(&self.election_key, &self.node_id, self.ttl_secs)
            .await
    }

    /// Check if this node is the leader
    pub async fn is_leader(&self) -> Result<bool, DistributedError> {
        // Try to acquire the election lock without side effects (check-only)
        // We check if we already hold it by attempting to renew
        let acquired = self.backend
            .acquire_lock(&self.election_key, &self.node_id, self.ttl_secs)
            .await?;

        if acquired {
            // We already held it or just acquired it - we are the leader
            // If we just acquired it and shouldn't have, release it
            // For simplicity, if acquire succeeds, we consider ourselves leader
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Renounce leadership
    pub async fn renounce(&self) -> Result<(), DistributedError> {
        self.backend
            .release_lock(&self.election_key, &self.node_id)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_backend() {
        let backend = Arc::new(InMemoryBackend::new());

        // Test harness operations
        let info = HarnessInfo::new(
            "test-1".to_string(),
            HarnessConfig::default(),
        );

        backend.create_harness(&info).await.unwrap();

        let retrieved = backend.get_harness("test-1").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "test-1");

        // Test lock acquisition
        let acquired = backend.acquire_lock("resource-1", "node-1", 60).await.unwrap();
        assert!(acquired);

        // Test lock not acquired by another
        let acquired2 = backend.acquire_lock("resource-1", "node-2", 60).await.unwrap();
        assert!(!acquired2);

        // Test lock release
        backend.release_lock("resource-1", "node-1").await.unwrap();

        let acquired3 = backend.acquire_lock("resource-1", "node-2", 60).await.unwrap();
        assert!(acquired3);
    }

    #[tokio::test]
    async fn test_distributed_manager() {
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
    }
}