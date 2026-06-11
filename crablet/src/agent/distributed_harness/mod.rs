//! Distributed Harness Manager - Multi-node harness coordination
//!
//! This module provides distributed harness management capabilities:
//! - Pluggable backend support (Redis, etcd, Consul)
//! - Cross-node harness state synchronization
//! - Leader election for coordination
//! - Distributed lock management
//! - Health monitoring and failover

mod distributed_backend;
mod distributed_control_plane;
mod distributed_types;

// Re-export public API from sub-modules
pub use distributed_backend::{InMemoryBackend, LeaderElection, RedisHarnessBackend};
pub use distributed_control_plane::{
    HarnessControlPlane, HttpHarnessControlPlane, InProcessHarnessControlPlane,
};
pub use distributed_types::{
    create_backend, BackendType, DistributedConfig, DistributedError,
    GenericHarnessAgentSpec, GenericHarnessResourceLimits, GenericHarnessResumeRequest,
    GenericHarnessResumeResponse, GenericHarnessRunRequest, GenericHarnessRunResponse,
    HarnessBackend, HarnessWatchEvent, NodeId, NodeInfo, NodeStatus,
};

// Internal re-exports used by control plane and tests
use distributed_control_plane::{
    HarnessCreateRequest, HarnessCreateResponse, HarnessDeleteRequest, HarnessDeleteResponse,
    HarnessSignalRequest, HarnessSignalResponse, NoopHarnessControlPlane,
};

use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use super::harness::{AgentHarnessContext, HarnessConfig, HarnessError, HarnessSignal};
use super::harness_agent::{
    HarnessAgent, HarnessAgentBuilder, HarnessAgentResult, HarnessExecutionProgressSink,
    HarnessExecutionState,
};
use super::harness_manager::{HarnessInfo, HarnessManager, HarnessStats, HarnessStatus};
#[cfg(feature = "web")]
use crate::gateway::rpc::RpcDispatcher;
#[cfg(feature = "web")]
use crate::gateway::types::RpcError;
use crate::types::Message;

/// Distributed harness manager
pub struct DistributedHarnessManager {
    /// Local harness manager
    local: Arc<HarnessManager>,
    /// Distributed backend
    backend: Arc<dyn HarnessBackend>,
    /// This node's ID
    node_id: NodeId,
    /// This node's address
    node_address: String,
    /// This node's port
    node_port: u16,
    /// Distributed lock TTL in seconds
    lock_ttl_secs: u64,
    /// Local harness IDs
    local_harnesses: Arc<RwLock<HashMap<String, bool>>>,
    /// Shutdown signal
    shutdown_tx: Arc<RwLock<Option<broadcast::Sender<ShutdownSignal>>>>,
    /// Statistics
    stats: Arc<RwLock<DistributedStats>>,
    /// Optional live control plane for active remote owner forwarding
    control_plane: Arc<dyn HarnessControlPlane>,
}

struct DistributedHarnessProgressSink {
    harness_id: String,
    local: Arc<HarnessManager>,
    backend: Arc<dyn HarnessBackend>,
}

#[async_trait]
impl HarnessExecutionProgressSink for DistributedHarnessProgressSink {
    async fn persist(&self, state: HarnessExecutionState) {
        if self
            .local
            .set_execution_state(&self.harness_id, state)
            .await
            .is_err()
        {
            return;
        }

        if let Some(info) = self.local.get_info(&self.harness_id).await {
            let _ = self.backend.update_harness(&info).await;
        }
    }
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

/// Cluster-wide statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterStats {
    pub local_stats: HarnessStats,
    pub distributed_stats: DistributedStats,
    pub total_nodes: usize,
    pub active_nodes: usize,
    pub total_harnesses: usize,
}

impl DistributedHarnessManager {
    #[cfg(feature = "web")]
    pub async fn register_rpc_handlers(
        manager: &Arc<Self>,
        rpc: &RpcDispatcher,
    ) -> Result<(), DistributedError> {
        let signal_manager = Arc::clone(manager);
        rpc.register("distributed_harness.signal", move |params| {
            let manager = Arc::clone(&signal_manager);
            async move {
                let payload: HarnessSignalRequest = serde_json::from_value(
                    params.ok_or_else(|| RpcError::new(-32602, "Missing params", None))?,
                )
                .map_err(|error| {
                    RpcError::new(-32602, &format!("Invalid params: {}", error), None)
                })?;

                manager
                    .apply_local_signal(&payload.harness_id, payload.signal)
                    .await
                    .map_err(|error| RpcError::new(-32000, &error.to_string(), None))?;

                Ok(Some(
                    serde_json::to_value(HarnessSignalResponse { ok: true }).map_err(|error| {
                        RpcError::new(-32603, &format!("Serialization error: {}", error), None)
                    })?,
                ))
            }
        })
        .await;

        let create_manager = Arc::clone(manager);
        rpc.register("distributed_harness.create", move |params| {
            let manager = Arc::clone(&create_manager);
            async move {
                let payload: HarnessCreateRequest = serde_json::from_value(
                    params.ok_or_else(|| RpcError::new(-32602, "Missing params", None))?,
                )
                .map_err(|error| {
                    RpcError::new(-32602, &format!("Invalid params: {}", error), None)
                })?;

                let harness_id = manager
                    .create_harness(payload.config)
                    .await
                    .map_err(|error| RpcError::new(-32000, &error.to_string(), None))?;

                Ok(Some(
                    serde_json::to_value(HarnessCreateResponse { harness_id }).map_err(
                        |error| {
                            RpcError::new(-32603, &format!("Serialization error: {}", error), None)
                        },
                    )?,
                ))
            }
        })
        .await;

        let delete_manager = Arc::clone(manager);
        rpc.register("distributed_harness.delete", move |params| {
            let manager = Arc::clone(&delete_manager);
            async move {
                let payload: HarnessDeleteRequest = serde_json::from_value(
                    params.ok_or_else(|| RpcError::new(-32602, "Missing params", None))?,
                )
                .map_err(|error| {
                    RpcError::new(-32602, &format!("Invalid params: {}", error), None)
                })?;

                manager
                    .remove_harness(&payload.harness_id)
                    .await
                    .map_err(|error| RpcError::new(-32000, &error.to_string(), None))?;

                Ok(Some(
                    serde_json::to_value(HarnessDeleteResponse { deleted: true }).map_err(
                        |error| {
                            RpcError::new(-32603, &format!("Serialization error: {}", error), None)
                        },
                    )?,
                ))
            }
        })
        .await;

        Ok(())
    }

    /// Create a new distributed harness manager
    pub fn new(backend: Arc<dyn HarnessBackend>, config: DistributedConfig) -> Self {
        let local = HarnessManager::new();

        Self {
            local: Arc::new(local),
            backend,
            node_id: config.node_id,
            node_address: config.node_address,
            node_port: config.node_port,
            lock_ttl_secs: config.lock_ttl_secs,
            local_harnesses: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx: Arc::new(RwLock::new(None)),
            stats: Arc::new(RwLock::new(DistributedStats::default())),
            control_plane: Arc::new(NoopHarnessControlPlane),
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
            node_address: "127.0.0.1".to_string(),
            node_port: 0,
            lock_ttl_secs: 300,
            local_harnesses: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx: Arc::new(RwLock::new(None)),
            stats: Arc::new(RwLock::new(DistributedStats::default())),
            control_plane: Arc::new(NoopHarnessControlPlane),
        }
    }

    pub fn with_control_plane(mut self, control_plane: Arc<dyn HarnessControlPlane>) -> Self {
        self.control_plane = control_plane;
        self
    }

    /// Get local harness manager reference
    pub fn local_manager(&self) -> &Arc<HarnessManager> {
        &self.local
    }

    /// Get this node's ID
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Get this node's advertised address.
    pub fn node_address(&self) -> &str {
        &self.node_address
    }

    /// Get this node's advertised port.
    pub fn node_port(&self) -> u16 {
        self.node_port
    }

    fn local_node_info(&self, harness_count: usize) -> NodeInfo {
        NodeInfo {
            id: self.node_id.clone(),
            address: self.node_address.clone(),
            port: self.node_port,
            capabilities: vec![
                "harness".to_string(),
                "execution".to_string(),
                "failover".to_string(),
            ],
            status: NodeStatus::Active,
            last_heartbeat: Utc::now(),
            harness_count,
        }
    }

    /// Register or refresh this node in the shared backend.
    pub async fn register_local_node(&self) -> Result<(), DistributedError> {
        self.sync_local_node_registration().await
    }

    async fn sync_local_node_registration(&self) -> Result<(), DistributedError> {
        let harness_count = self.local_harnesses.read().await.len();
        self.backend
            .register_node(&self.local_node_info(harness_count))
            .await
    }

    fn owner_node_id(info: &HarnessInfo) -> Option<&str> {
        info.config.metadata.get("node_id").map(String::as_str)
    }

    fn is_terminal_status(status: &HarnessStatus) -> bool {
        matches!(
            status,
            HarnessStatus::Completed | HarnessStatus::Failed | HarnessStatus::Cancelled
        )
    }

    async fn sync_backend_harness_state(&self, id: &str) {
        if let Some(info) = self.local.get_info(id).await {
            if let Err(error) = self.backend.update_harness(&info).await {
                tracing::warn!("Failed to sync harness {} to backend: {}", id, error);
            }
        }
    }

    fn progress_sink(&self, id: &str) -> Arc<dyn HarnessExecutionProgressSink> {
        Arc::new(DistributedHarnessProgressSink {
            harness_id: id.to_string(),
            local: self.local.clone(),
            backend: self.backend.clone(),
        })
    }

    pub async fn select_target_node(
        &self,
        preferred_node_id: Option<&str>,
    ) -> Result<NodeInfo, DistributedError> {
        if preferred_node_id == Some(self.node_id.as_str()) {
            let harness_count = self.local_harnesses.read().await.len();
            return Ok(self.local_node_info(harness_count));
        }

        let mut nodes: Vec<NodeInfo> = self
            .backend
            .get_nodes()
            .await?
            .into_iter()
            .filter(|node| {
                node.status == NodeStatus::Active
                    && node
                        .capabilities
                        .iter()
                        .any(|capability| capability == "harness")
            })
            .collect();

        if let Some(preferred) = preferred_node_id {
            return nodes
                .into_iter()
                .find(|node| node.id == preferred)
                .ok_or_else(|| DistributedError::NodeNotFound(preferred.to_string()));
        }

        if nodes.is_empty() {
            let harness_count = self.local_harnesses.read().await.len();
            return Ok(self.local_node_info(harness_count));
        }

        nodes.sort_by_key(|node| {
            (
                node.harness_count,
                if node.id == self.node_id { 0u8 } else { 1u8 },
                node.id.clone(),
            )
        });

        Ok(nodes.remove(0))
    }

    pub async fn create_harness_distributed(
        &self,
        config: Option<HarnessConfig>,
        preferred_node_id: Option<&str>,
    ) -> Result<String, DistributedError> {
        let target_node = self.select_target_node(preferred_node_id).await?;
        if target_node.id == self.node_id {
            return self.create_harness(config).await;
        }

        let harness_id = self
            .control_plane
            .forward_create(&target_node, config)
            .await?;

        let mut stats = self.stats.write().await;
        stats.remote_operations += 1;
        stats.cross_node_delegations += 1;

        Ok(harness_id)
    }

    pub async fn forward_generic_run(
        &self,
        node: &NodeInfo,
        request: GenericHarnessRunRequest,
    ) -> Result<GenericHarnessRunResponse, DistributedError> {
        let response = self.control_plane.forward_run(node, request).await?;

        let mut stats = self.stats.write().await;
        stats.remote_operations += 1;
        stats.cross_node_delegations += 1;

        Ok(response)
    }

    pub async fn forward_generic_resume(
        &self,
        node: &NodeInfo,
        request: GenericHarnessResumeRequest,
    ) -> Result<GenericHarnessResumeResponse, DistributedError> {
        let response = self.control_plane.forward_resume(node, request).await?;

        let mut stats = self.stats.write().await;
        stats.remote_operations += 1;
        stats.cross_node_delegations += 1;

        Ok(response)
    }

    pub(crate) async fn apply_local_signal(
        &self,
        id: &str,
        signal: HarnessSignal,
    ) -> Result<(), DistributedError> {
        match signal {
            HarnessSignal::Cancel => self
                .local
                .cancel_harness(id)
                .await
                .map_err(|error| DistributedError::ClusterError(error.to_string()))?,
            HarnessSignal::Pause => self
                .local
                .pause_harness(id)
                .await
                .map_err(|error| DistributedError::ClusterError(error.to_string()))?,
            HarnessSignal::Resume => self
                .local
                .resume_harness(id)
                .await
                .map_err(|error| DistributedError::ClusterError(error.to_string()))?,
            HarnessSignal::Checkpoint => {
                let harness = self
                    .local
                    .get_harness(id)
                    .await
                    .ok_or_else(|| DistributedError::HarnessNotFound(id.to_string()))?;
                let (checkpoint, path) = {
                    let harness = harness.read().await;
                    harness.checkpoint_artifact()
                };
                AgentHarnessContext::persist_checkpoint_artifact(&checkpoint, path).await;
            }
        }

        self.sync_backend_harness_state(id).await;
        if let Err(error) = self.sync_local_node_registration().await {
            tracing::warn!("Failed to refresh local node {}: {}", self.node_id, error);
        }

        let mut stats = self.stats.write().await;
        stats.local_operations += 1;

        Ok(())
    }

    async fn claim_remote_harness(
        &self,
        mut info: HarnessInfo,
    ) -> Result<Option<Arc<RwLock<AgentHarnessContext>>>, DistributedError> {
        if let Some(local_harness) = self.local.get_harness(&info.id).await {
            return Ok(Some(local_harness));
        }

        let owner_node = Self::owner_node_id(&info).map(str::to_string);
        let nodes = self.backend.get_nodes().await?;
        if owner_node
            .as_ref()
            .and_then(|owner| nodes.iter().find(|node| node.id == *owner))
            .map(|node| node.status == NodeStatus::Active && node.id != self.node_id)
            .unwrap_or(false)
        {
            let mut stats = self.stats.write().await;
            stats.remote_operations += 1;
            return Ok(None);
        }

        let lock_name = format!("harness:{}", info.id);
        let acquired = self
            .backend
            .acquire_lock(&lock_name, &self.node_id, self.lock_ttl_secs)
            .await?;
        if !acquired {
            return Ok(None);
        }

        info.config
            .metadata
            .insert("node_id".to_string(), self.node_id.clone());
        if matches!(info.status, HarnessStatus::Running) {
            info.status = HarnessStatus::Paused;
            if let Some(metadata) = info.execution_metadata.as_mut() {
                metadata.paused = true;
                metadata.cancelled = false;
                metadata.last_activity_at = Utc::now();
            }
        }
        info.last_active_at = Utc::now();

        let adopted = self
            .local
            .adopt_harness(info.clone())
            .await
            .map_err(|error| DistributedError::ClusterError(error.to_string()))?;

        {
            let mut harnesses = self.local_harnesses.write().await;
            harnesses.insert(info.id.clone(), true);
        }

        if let Err(error) = self.sync_local_node_registration().await {
            tracing::warn!("Failed to register local node {}: {}", self.node_id, error);
        }
        self.sync_backend_harness_state(&info.id).await;

        let mut stats = self.stats.write().await;
        stats.remote_operations += 1;
        if owner_node
            .as_deref()
            .map(|owner| owner != self.node_id.as_str())
            .unwrap_or(false)
        {
            stats.node_failovers += 1;
        }

        Ok(Some(adopted))
    }

    /// Create a new harness (local or remote based on load)
    pub async fn create_harness(
        &self,
        config: Option<HarnessConfig>,
    ) -> Result<String, DistributedError> {
        let mut effective_config = config.unwrap_or_else(|| self.local.default_config().clone());
        effective_config
            .metadata
            .insert("node_id".to_string(), self.node_id.clone());

        // First try to create locally
        let create_result: Result<String, HarnessError> =
            self.local.create_harness(Some(effective_config)).await;
        let id = create_result
            .map_err(|e: HarnessError| DistributedError::ClusterError(e.to_string()))?;

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

        if let Err(error) = self.sync_local_node_registration().await {
            tracing::warn!("Failed to register local node {}: {}", self.node_id, error);
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.local_operations += 1;
        }

        Ok(id)
    }

    /// Execute a harness-aware agent locally and sync its lifecycle to the backend.
    pub async fn run_agent<A: HarnessAgent>(
        &self,
        builder: &HarnessAgentBuilder<A>,
        task: &str,
        context: &[Message],
    ) -> Result<(String, HarnessAgentResult), DistributedError> {
        let id = self.create_harness(Some(builder.config().clone())).await?;
        let state = builder.execution_state(task, context);
        let harness = self
            .local
            .prepare_agent_execution(&id, state.clone())
            .await
            .map_err(|error| DistributedError::ClusterError(error.to_string()))?;
        self.sync_backend_harness_state(&id).await;

        let result = self
            .local
            .execute_agent_on_prepared_harness(
                &id,
                &builder.clone().with_progress_sink(self.progress_sink(&id)),
                harness,
                state,
            )
            .await
            .map_err(|error| DistributedError::ClusterError(error.to_string()))?;

        self.sync_backend_harness_state(&id).await;
        if let Err(error) = self.sync_local_node_registration().await {
            tracing::warn!("Failed to refresh local node {}: {}", self.node_id, error);
        }

        Ok((id, result))
    }

    /// Resume a harness-aware agent from locally stored or claimed execution state.
    pub async fn resume_agent<A: HarnessAgent>(
        &self,
        id: &str,
        builder: &HarnessAgentBuilder<A>,
    ) -> Result<HarnessAgentResult, DistributedError> {
        if self.local.get_harness(id).await.is_none() {
            let claimed = self.get_harness(id).await?;
            if claimed.is_none() {
                return Err(DistributedError::HarnessNotFound(id.to_string()));
            }
        }

        let state = self.local.get_execution_state(id).await.ok_or_else(|| {
            DistributedError::ClusterError(format!("resume state unavailable: {}", id))
        })?;
        self.local
            .set_harness_config(id, builder.config().clone())
            .await
            .map_err(|error| DistributedError::ClusterError(error.to_string()))?;
        let harness = self
            .local
            .prepare_agent_execution(id, state.clone())
            .await
            .map_err(|error| DistributedError::ClusterError(error.to_string()))?;
        self.sync_backend_harness_state(id).await;

        let result = self
            .local
            .execute_agent_on_prepared_harness(
                id,
                &builder.clone().with_progress_sink(self.progress_sink(id)),
                harness,
                state,
            )
            .await
            .map_err(|error| DistributedError::ClusterError(error.to_string()))?;

        self.sync_backend_harness_state(id).await;
        if let Err(error) = self.sync_local_node_registration().await {
            tracing::warn!("Failed to refresh local node {}: {}", self.node_id, error);
        }

        Ok(result)
    }

    /// Get a harness (checks local first, then remote)
    pub async fn get_harness(
        &self,
        id: &str,
    ) -> Result<Option<Arc<RwLock<AgentHarnessContext>>>, DistributedError> {
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
            self.claim_remote_harness(info).await
        } else {
            Ok(None)
        }
    }

    /// Get harness metadata without claiming or moving ownership.
    pub async fn get_harness_info(
        &self,
        id: &str,
    ) -> Result<Option<HarnessInfo>, DistributedError> {
        if let Some(info) = self.local.get_info(id).await {
            let mut stats = self.stats.write().await;
            stats.local_operations += 1;
            return Ok(Some(info));
        }

        let remote_info = self.backend.get_harness(id).await?;
        if remote_info.is_some() {
            let mut stats = self.stats.write().await;
            stats.remote_operations += 1;
        }

        Ok(remote_info)
    }

    /// Remove a harness from both the backend and local manager when safe.
    pub async fn remove_harness(&self, id: &str) -> Result<(), DistributedError> {
        let local_info = self.local.get_info(id).await;
        let remote_info = if local_info.is_none() {
            self.backend.get_harness(id).await?
        } else {
            None
        };

        let info = local_info
            .as_ref()
            .or(remote_info.as_ref())
            .ok_or_else(|| DistributedError::HarnessNotFound(id.to_string()))?;

        if !Self::is_terminal_status(&info.status) {
            return Err(DistributedError::ClusterError(format!(
                "harness {} must be terminal before removal",
                id
            )));
        }

        let owner_node_id = Self::owner_node_id(info).map(str::to_string);
        let active_owner = if owner_node_id
            .as_ref()
            .map(|owner| owner != &self.node_id)
            .unwrap_or(false)
        {
            self.backend.get_nodes().await?.into_iter().find(|node| {
                node.id == owner_node_id.as_deref().unwrap_or_default()
                    && node.status == NodeStatus::Active
            })
        } else {
            None
        };

        if let Some(owner_node) = active_owner.as_ref() {
            self.control_plane.forward_delete(owner_node, id).await?;

            let mut stats = self.stats.write().await;
            stats.remote_operations += 1;
            stats.cross_node_delegations += 1;
        } else {
            self.backend.delete_harness(id).await?;
        }

        if local_info.is_some() {
            self.local.remove_harness(id).await;
            {
                let mut local_harnesses = self.local_harnesses.write().await;
                local_harnesses.remove(id);
            }

            if let Err(error) = self.sync_local_node_registration().await {
                tracing::warn!("Failed to refresh local node {}: {}", self.node_id, error);
            }
        }

        Ok(())
    }

    /// Send a signal to a harness (handles cross-node signals)
    pub async fn send_signal(
        &self,
        id: &str,
        signal: HarnessSignal,
    ) -> Result<(), DistributedError> {
        // Check local
        if self.local.get_harness(id).await.is_some() {
            return self.apply_local_signal(id, signal).await;
        }

        let remote_info = self
            .backend
            .get_harness(id)
            .await?
            .ok_or_else(|| DistributedError::HarnessNotFound(id.to_string()))?;
        let owner_node = Self::owner_node_id(&remote_info)
            .map(str::to_string)
            .ok_or_else(|| DistributedError::NodeNotFound(format!("owner for harness {}", id)))?;
        let owner_node_info = self
            .backend
            .get_nodes()
            .await?
            .into_iter()
            .find(|node| node.id == owner_node);

        if let Some(owner_node_info) = owner_node_info
            .filter(|node| node.status == NodeStatus::Active && node.id != self.node_id)
        {
            self.control_plane
                .forward_signal(&owner_node_info, id, signal)
                .await?;

            let mut stats = self.stats.write().await;
            stats.remote_operations += 1;
            stats.cross_node_delegations += 1;
            return Ok(());
        }

        let claimed = self.get_harness(id).await?;
        if claimed.is_none() {
            return Err(DistributedError::HarnessNotFound(id.to_string()));
        }

        self.apply_local_signal(id, signal).await
    }

    /// Try to acquire lock for a harness
    pub async fn try_lock_harness(&self, id: &str) -> Result<bool, DistributedError> {
        let acquired = self
            .backend
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

        let active_nodes = nodes
            .iter()
            .filter(|n| n.status == NodeStatus::Active)
            .count();

        let total_harnesses = nodes.iter().map(|n| n.harness_count).sum::<usize>();

        Ok(ClusterStats {
            local_stats,
            distributed_stats: dist_stats,
            total_nodes: nodes.len(),
            active_nodes,
            total_harnesses,
        })
    }

    /// List currently visible cluster nodes from the shared backend.
    pub async fn list_nodes(&self) -> Result<Vec<NodeInfo>, DistributedError> {
        self.backend.get_nodes().await
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
    pub async fn handle_node_failure(
        &self,
        failed_node_id: &NodeId,
    ) -> Result<Vec<String>, DistributedError> {
        // Get harnesses that were on the failed node
        let harnesses = self.list_all_harnesses().await?;

        let mut claimed = Vec::new();
        for harness in harnesses {
            let owned_by_failed_node =
                harness.config.metadata.get("node_id") == Some(&failed_node_id.to_string());
            if owned_by_failed_node
                && !Self::is_terminal_status(&harness.status)
                && self.claim_remote_harness(harness.clone()).await?.is_some()
            {
                claimed.push(harness.id);
            }
        }

        Ok(claimed)
    }
}
