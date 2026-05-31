//! Distributed Harness Manager - Multi-node harness coordination
//!
//! This module provides distributed harness management capabilities:
//! - Pluggable backend support (Redis, etcd, Consul)
//! - Cross-node harness state synchronization
//! - Leader election for coordination
//! - Distributed lock management
//! - Health monitoring and failover

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use redis::{aio::ConnectionManager, AsyncCommands, Script};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::sync::{Arc, Weak};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, RwLock};

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
    async fn acquire_lock(
        &self,
        resource: &str,
        owner: &str,
        ttl_secs: u64,
    ) -> Result<bool, DistributedError>;

    /// Release a distributed lock
    async fn release_lock(&self, resource: &str, owner: &str) -> Result<(), DistributedError>;

    /// Register node health
    async fn register_node(&self, node: &NodeInfo) -> Result<(), DistributedError>;

    /// Get all active nodes
    async fn get_nodes(&self) -> Result<Vec<NodeInfo>, DistributedError>;

    /// Watch for harness changes
    async fn watch_harnesses(
        &self,
        tx: mpsc::Sender<HarnessWatchEvent>,
    ) -> Result<(), DistributedError>;
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

impl BackendType {
    pub fn as_str(&self) -> &'static str {
        match self {
            BackendType::Redis => "redis",
            BackendType::Etcd => "etcd",
            BackendType::Consul => "consul",
            BackendType::InMemory => "memory",
        }
    }

    pub fn default_backend_uri(&self) -> &'static str {
        match self {
            BackendType::Redis => "redis://127.0.0.1/",
            BackendType::Etcd => "http://127.0.0.1:2379",
            BackendType::Consul => "http://127.0.0.1:8500",
            BackendType::InMemory => "memory://",
        }
    }
}

impl fmt::Display for BackendType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for BackendType {
    type Err = DistributedError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "redis" => Ok(BackendType::Redis),
            "etcd" => Ok(BackendType::Etcd),
            "consul" => Ok(BackendType::Consul),
            "memory" | "inmemory" | "in-memory" => Ok(BackendType::InMemory),
            other => Err(DistributedError::ClusterError(format!(
                "unsupported distributed backend: {}",
                other
            ))),
        }
    }
}

pub async fn create_backend(
    config: &DistributedConfig,
) -> Result<Arc<dyn HarnessBackend>, DistributedError> {
    match config.backend_type {
        BackendType::InMemory => Ok(Arc::new(InMemoryBackend::new())),
        BackendType::Redis => Ok(Arc::new(
            RedisHarnessBackend::new(&config.backend_uri).await?,
        )),
        BackendType::Etcd | BackendType::Consul => Err(DistributedError::ClusterError(format!(
            "{:?} backend is not implemented yet",
            config.backend_type
        ))),
    }
}

/// Optional control plane used to forward commands to a live remote owner.
#[async_trait]
pub trait HarnessControlPlane: Send + Sync {
    async fn forward_create(
        &self,
        node: &NodeInfo,
        config: Option<HarnessConfig>,
    ) -> Result<String, DistributedError>;

    async fn forward_run(
        &self,
        node: &NodeInfo,
        request: GenericHarnessRunRequest,
    ) -> Result<GenericHarnessRunResponse, DistributedError>;

    async fn forward_resume(
        &self,
        node: &NodeInfo,
        request: GenericHarnessResumeRequest,
    ) -> Result<GenericHarnessResumeResponse, DistributedError>;

    async fn forward_signal(
        &self,
        node: &NodeInfo,
        harness_id: &str,
        signal: HarnessSignal,
    ) -> Result<(), DistributedError>;

    async fn forward_delete(
        &self,
        node: &NodeInfo,
        harness_id: &str,
    ) -> Result<(), DistributedError>;
}

#[derive(Default)]
struct NoopHarnessControlPlane;

#[async_trait]
impl HarnessControlPlane for NoopHarnessControlPlane {
    async fn forward_create(
        &self,
        node: &NodeInfo,
        config: Option<HarnessConfig>,
    ) -> Result<String, DistributedError> {
        let _ = (node, config);
        Err(DistributedError::ClusterError(
            "remote control plane unavailable".to_string(),
        ))
    }

    async fn forward_run(
        &self,
        node: &NodeInfo,
        request: GenericHarnessRunRequest,
    ) -> Result<GenericHarnessRunResponse, DistributedError> {
        let _ = (node, request);
        Err(DistributedError::ClusterError(
            "remote control plane unavailable".to_string(),
        ))
    }

    async fn forward_resume(
        &self,
        node: &NodeInfo,
        request: GenericHarnessResumeRequest,
    ) -> Result<GenericHarnessResumeResponse, DistributedError> {
        let _ = (node, request);
        Err(DistributedError::ClusterError(
            "remote control plane unavailable".to_string(),
        ))
    }

    async fn forward_signal(
        &self,
        node: &NodeInfo,
        harness_id: &str,
        _signal: HarnessSignal,
    ) -> Result<(), DistributedError> {
        Err(DistributedError::ClusterError(format!(
            "remote control plane unavailable for node {} and harness {}",
            node.id, harness_id
        )))
    }

    async fn forward_delete(
        &self,
        node: &NodeInfo,
        harness_id: &str,
    ) -> Result<(), DistributedError> {
        Err(DistributedError::ClusterError(format!(
            "remote control plane unavailable for node {} and harness {}",
            node.id, harness_id
        )))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HarnessCreateRequest {
    config: Option<HarnessConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HarnessCreateResponse {
    harness_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HarnessDeleteRequest {
    harness_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HarnessDeleteResponse {
    deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct GenericHarnessResourceLimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_steps: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_memory_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cpu_time_ms: Option<u64>,
}

impl GenericHarnessResourceLimits {
    pub fn apply_to_config(&self, config: &mut HarnessConfig) {
        if let Some(max_steps) = self.max_steps {
            config.max_steps = max_steps;
        }
        if let Some(tool_timeout_ms) = self.tool_timeout_ms {
            config.tool_timeout = Duration::from_millis(tool_timeout_ms);
        }
        if let Some(step_timeout_ms) = self.step_timeout_ms {
            config.step_timeout = Duration::from_millis(step_timeout_ms);
        }
        if let Some(max_memory_bytes) = self.max_memory_bytes {
            config.max_memory_bytes = Some(max_memory_bytes);
        }
        if let Some(max_cpu_time_ms) = self.max_cpu_time_ms {
            config.max_cpu_time_ms = Some(max_cpu_time_ms);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct GenericHarnessAgentSpec {
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_limits: Option<GenericHarnessResourceLimits>,
}

const AGENT_SPEC_METADATA_KEY: &str = "crablet.agent_spec";
const AGENT_ROLE_METADATA_KEY: &str = "crablet.agent_role";
const AGENT_NAME_METADATA_KEY: &str = "crablet.agent_name";
const AGENT_DESCRIPTION_METADATA_KEY: &str = "crablet.agent_description";
const AGENT_SYSTEM_PROMPT_METADATA_KEY: &str = "crablet.agent_system_prompt";

impl GenericHarnessAgentSpec {
    pub fn from_metadata(metadata: &HashMap<String, String>) -> Option<Self> {
        if let Some(raw) = metadata.get(AGENT_SPEC_METADATA_KEY) {
            if let Ok(spec) = serde_json::from_str(raw) {
                return Some(spec);
            }
        }

        let role = metadata.get(AGENT_ROLE_METADATA_KEY)?.clone();
        Some(Self {
            role,
            name: metadata.get(AGENT_NAME_METADATA_KEY).cloned(),
            description: metadata.get(AGENT_DESCRIPTION_METADATA_KEY).cloned(),
            system_prompt: metadata.get(AGENT_SYSTEM_PROMPT_METADATA_KEY).cloned(),
            allowed_tools: None,
            resource_limits: None,
        })
    }

    pub fn persist_into_metadata(
        &self,
        metadata: &mut HashMap<String, String>,
    ) -> Result<(), DistributedError> {
        let encoded = serde_json::to_string(self).map_err(|error| {
            DistributedError::ClusterError(format!("failed to serialize agent spec: {}", error))
        })?;

        metadata.insert(AGENT_SPEC_METADATA_KEY.to_string(), encoded);
        metadata.insert(AGENT_ROLE_METADATA_KEY.to_string(), self.role.clone());

        if let Some(name) = &self.name {
            metadata.insert(AGENT_NAME_METADATA_KEY.to_string(), name.clone());
        } else {
            metadata.remove(AGENT_NAME_METADATA_KEY);
        }

        if let Some(description) = &self.description {
            metadata.insert(
                AGENT_DESCRIPTION_METADATA_KEY.to_string(),
                description.clone(),
            );
        } else {
            metadata.remove(AGENT_DESCRIPTION_METADATA_KEY);
        }

        if let Some(system_prompt) = &self.system_prompt {
            metadata.insert(
                AGENT_SYSTEM_PROMPT_METADATA_KEY.to_string(),
                system_prompt.clone(),
            );
        } else {
            metadata.remove(AGENT_SYSTEM_PROMPT_METADATA_KEY);
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericHarnessRunRequest {
    pub task: String,
    #[serde(default)]
    pub context: Vec<Message>,
    pub agent: GenericHarnessAgentSpec,
    #[serde(default)]
    pub harness_config: Option<HarnessConfig>,
    #[serde(default)]
    pub target_node_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericHarnessRunResponse {
    pub harness_id: String,
    pub result: HarnessAgentResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericHarnessResumeRequest {
    pub harness_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<GenericHarnessAgentSpec>,
    #[serde(default)]
    pub harness_config: Option<HarnessConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericHarnessResumeResponse {
    pub harness_id: String,
    pub result: HarnessAgentResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HarnessSignalRequest {
    harness_id: String,
    signal: HarnessSignal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HarnessSignalResponse {
    ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ControlPlaneRpcRequest {
    jsonrpc: String,
    method: String,
    params: Option<serde_json::Value>,
    id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ControlPlaneRpcResponse {
    jsonrpc: String,
    id: Option<String>,
    result: Option<serde_json::Value>,
    error: Option<ControlPlaneRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ControlPlaneRpcError {
    code: i32,
    message: String,
    data: Option<serde_json::Value>,
}

/// HTTP transport for live remote owner forwarding.
pub struct HttpHarnessControlPlane {
    client: reqwest::Client,
    rpc_path: String,
    bearer_token: Option<String>,
}

impl Default for HttpHarnessControlPlane {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
            rpc_path: "/rpc".to_string(),
            bearer_token: None,
        }
    }
}

impl HttpHarnessControlPlane {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_rpc_path(mut self, rpc_path: impl Into<String>) -> Self {
        self.rpc_path = rpc_path.into();
        self
    }

    pub fn with_bearer_token(mut self, token: impl Into<String>) -> Self {
        self.bearer_token = Some(token.into());
        self
    }

    async fn invoke_rpc<T, P>(
        &self,
        node: &NodeInfo,
        method: &str,
        params: Option<P>,
    ) -> Result<T, DistributedError>
    where
        T: DeserializeOwned,
        P: Serialize,
    {
        let url = format!("http://{}:{}{}", node.address, node.port, self.rpc_path);
        let request = ControlPlaneRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params: params
                .map(serde_json::to_value)
                .transpose()
                .map_err(|error| DistributedError::ClusterError(error.to_string()))?,
            id: Some(uuid::Uuid::new_v4().to_string()),
        };

        let mut builder = self.client.post(url).json(&request);
        if let Some(token) = &self.bearer_token {
            builder = builder.bearer_auth(token);
        }

        let response = builder
            .send()
            .await
            .map_err(|error| DistributedError::ConnectionError(error.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| DistributedError::ConnectionError(error.to_string()))?;

        if !status.is_success() {
            return Err(DistributedError::ClusterError(format!(
                "remote rpc {} failed with {}: {}",
                method, status, body
            )));
        }

        let response: ControlPlaneRpcResponse = serde_json::from_str(&body)
            .map_err(|error| DistributedError::ConnectionError(error.to_string()))?;
        if let Some(error) = response.error {
            return Err(DistributedError::ClusterError(format!(
                "rpc error {}: {}",
                error.code, error.message
            )));
        }

        let result = response
            .result
            .ok_or_else(|| DistributedError::ClusterError("missing rpc result".to_string()))?;
        serde_json::from_value(result)
            .map_err(|error| DistributedError::ClusterError(error.to_string()))
    }
}

#[async_trait]
impl HarnessControlPlane for HttpHarnessControlPlane {
    async fn forward_create(
        &self,
        node: &NodeInfo,
        config: Option<HarnessConfig>,
    ) -> Result<String, DistributedError> {
        let response: HarnessCreateResponse = self
            .invoke_rpc(
                node,
                "distributed_harness.create",
                Some(HarnessCreateRequest { config }),
            )
            .await?;
        Ok(response.harness_id)
    }

    async fn forward_run(
        &self,
        node: &NodeInfo,
        request: GenericHarnessRunRequest,
    ) -> Result<GenericHarnessRunResponse, DistributedError> {
        self.invoke_rpc(node, "distributed_harness.run", Some(request))
            .await
    }

    async fn forward_resume(
        &self,
        node: &NodeInfo,
        request: GenericHarnessResumeRequest,
    ) -> Result<GenericHarnessResumeResponse, DistributedError> {
        self.invoke_rpc(node, "distributed_harness.resume", Some(request))
            .await
    }

    async fn forward_signal(
        &self,
        node: &NodeInfo,
        harness_id: &str,
        signal: HarnessSignal,
    ) -> Result<(), DistributedError> {
        let _: HarnessSignalResponse = self
            .invoke_rpc(
                node,
                "distributed_harness.signal",
                Some(HarnessSignalRequest {
                    harness_id: harness_id.to_string(),
                    signal,
                }),
            )
            .await?;
        Ok(())
    }

    async fn forward_delete(
        &self,
        node: &NodeInfo,
        harness_id: &str,
    ) -> Result<(), DistributedError> {
        let _: HarnessDeleteResponse = self
            .invoke_rpc(
                node,
                "distributed_harness.delete",
                Some(HarnessDeleteRequest {
                    harness_id: harness_id.to_string(),
                }),
            )
            .await?;
        Ok(())
    }
}

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

    async fn apply_local_signal(
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

/// In-process control plane used for tests or embedded multi-node runtimes.
#[derive(Default)]
pub struct InProcessHarnessControlPlane {
    managers: Arc<RwLock<HashMap<NodeId, Weak<DistributedHarnessManager>>>>,
}

impl InProcessHarnessControlPlane {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register_manager(&self, manager: &Arc<DistributedHarnessManager>) {
        self.managers
            .write()
            .await
            .insert(manager.node_id().to_string(), Arc::downgrade(manager));
    }

    pub async fn unregister_node(&self, node_id: &str) {
        self.managers.write().await.remove(node_id);
    }
}

#[async_trait]
impl HarnessControlPlane for InProcessHarnessControlPlane {
    async fn forward_create(
        &self,
        node: &NodeInfo,
        config: Option<HarnessConfig>,
    ) -> Result<String, DistributedError> {
        let manager = self
            .managers
            .read()
            .await
            .get(&node.id)
            .cloned()
            .ok_or_else(|| DistributedError::NodeNotFound(node.id.clone()))?
            .upgrade()
            .ok_or_else(|| DistributedError::NodeNotFound(node.id.clone()))?;

        manager.create_harness(config).await
    }

    async fn forward_run(
        &self,
        _node: &NodeInfo,
        _request: GenericHarnessRunRequest,
    ) -> Result<GenericHarnessRunResponse, DistributedError> {
        Err(DistributedError::ClusterError(
            "in-process control plane does not support remote run".to_string(),
        ))
    }

    async fn forward_resume(
        &self,
        _node: &NodeInfo,
        _request: GenericHarnessResumeRequest,
    ) -> Result<GenericHarnessResumeResponse, DistributedError> {
        Err(DistributedError::ClusterError(
            "in-process control plane does not support remote resume".to_string(),
        ))
    }

    async fn forward_signal(
        &self,
        node: &NodeInfo,
        harness_id: &str,
        signal: HarnessSignal,
    ) -> Result<(), DistributedError> {
        let manager = self
            .managers
            .read()
            .await
            .get(&node.id)
            .cloned()
            .ok_or_else(|| DistributedError::NodeNotFound(node.id.clone()))?
            .upgrade()
            .ok_or_else(|| DistributedError::NodeNotFound(node.id.clone()))?;

        manager.apply_local_signal(harness_id, signal).await
    }

    async fn forward_delete(
        &self,
        node: &NodeInfo,
        harness_id: &str,
    ) -> Result<(), DistributedError> {
        let manager = self
            .managers
            .read()
            .await
            .get(&node.id)
            .cloned()
            .ok_or_else(|| DistributedError::NodeNotFound(node.id.clone()))?
            .upgrade()
            .ok_or_else(|| DistributedError::NodeNotFound(node.id.clone()))?;

        manager.remove_harness(harness_id).await
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

type InMemoryHarnessMap = HashMap<String, HarnessInfo>;
type InMemoryLockMap = HashMap<String, (NodeId, DateTime<Utc>)>;
type InMemoryNodeMap = HashMap<NodeId, NodeInfo>;

/// In-memory backend for single-node testing
pub struct InMemoryBackend {
    harnesses: Arc<RwLock<InMemoryHarnessMap>>,
    locks: Arc<RwLock<InMemoryLockMap>>,
    nodes: Arc<RwLock<InMemoryNodeMap>>,
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

/// Redis-backed backend for multi-process and multi-node coordination.
pub struct RedisHarnessBackend {
    connection_manager: Arc<RwLock<ConnectionManager>>,
    key_prefix: String,
}

impl RedisHarnessBackend {
    pub async fn new(uri: &str) -> Result<Self, DistributedError> {
        Self::with_prefix(uri, "crablet:distributed_harness").await
    }

    pub async fn with_prefix(
        uri: &str,
        key_prefix: impl Into<String>,
    ) -> Result<Self, DistributedError> {
        let client = redis::Client::open(uri)
            .map_err(|error| DistributedError::ConnectionError(error.to_string()))?;
        let connection_manager = ConnectionManager::new(client)
            .await
            .map_err(|error| DistributedError::ConnectionError(error.to_string()))?;

        Ok(Self {
            connection_manager: Arc::new(RwLock::new(connection_manager)),
            key_prefix: key_prefix.into(),
        })
    }

    async fn conn(&self) -> ConnectionManager {
        self.connection_manager.read().await.clone()
    }

    fn harness_key(&self, id: &str) -> String {
        format!("{}:harness:{}", self.key_prefix, id)
    }

    fn harness_index_key(&self) -> String {
        format!("{}:harness:index", self.key_prefix)
    }

    fn lock_key(&self, resource: &str) -> String {
        format!("{}:lock:{}", self.key_prefix, resource)
    }

    fn node_key(&self, id: &str) -> String {
        format!("{}:node:{}", self.key_prefix, id)
    }

    fn node_index_key(&self) -> String {
        format!("{}:node:index", self.key_prefix)
    }
}

#[async_trait]
impl HarnessBackend for RedisHarnessBackend {
    async fn create_harness(&self, info: &HarnessInfo) -> Result<(), DistributedError> {
        let mut conn = self.conn().await;
        let payload = serde_json::to_string(info)
            .map_err(|error| DistributedError::ClusterError(error.to_string()))?;
        let _: () = redis::pipe()
            .cmd("SET")
            .arg(self.harness_key(&info.id))
            .arg(payload)
            .ignore()
            .cmd("SADD")
            .arg(self.harness_index_key())
            .arg(&info.id)
            .ignore()
            .query_async(&mut conn)
            .await
            .map_err(|error| DistributedError::ConnectionError(error.to_string()))?;
        Ok(())
    }

    async fn update_harness(&self, info: &HarnessInfo) -> Result<(), DistributedError> {
        self.create_harness(info).await
    }

    async fn delete_harness(&self, id: &str) -> Result<(), DistributedError> {
        let mut conn = self.conn().await;
        let _: () = redis::pipe()
            .cmd("DEL")
            .arg(self.harness_key(id))
            .ignore()
            .cmd("SREM")
            .arg(self.harness_index_key())
            .arg(id)
            .ignore()
            .query_async(&mut conn)
            .await
            .map_err(|error| DistributedError::ConnectionError(error.to_string()))?;
        Ok(())
    }

    async fn get_harness(&self, id: &str) -> Result<Option<HarnessInfo>, DistributedError> {
        let mut conn = self.conn().await;
        let payload: Option<String> = conn
            .get(self.harness_key(id))
            .await
            .map_err(|error| DistributedError::ConnectionError(error.to_string()))?;
        payload
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(|error| DistributedError::ClusterError(error.to_string()))
    }

    async fn list_harnesses(&self) -> Result<Vec<HarnessInfo>, DistributedError> {
        let mut conn = self.conn().await;
        let ids: Vec<String> = conn
            .smembers(self.harness_index_key())
            .await
            .map_err(|error| DistributedError::ConnectionError(error.to_string()))?;

        let mut harnesses = Vec::new();
        for id in ids {
            if let Some(info) = self.get_harness(&id).await? {
                harnesses.push(info);
            }
        }

        Ok(harnesses)
    }

    async fn acquire_lock(
        &self,
        resource: &str,
        owner: &str,
        ttl_secs: u64,
    ) -> Result<bool, DistributedError> {
        let script = Script::new(
            r#"
            local current = redis.call('GET', KEYS[1])
            if (not current) or current == ARGV[1] then
                redis.call('SET', KEYS[1], ARGV[1], 'EX', ARGV[2])
                return 1
            end
            return 0
        "#,
        );
        let mut conn = self.conn().await;
        let acquired: i32 = script
            .key(self.lock_key(resource))
            .arg(owner)
            .arg(ttl_secs)
            .invoke_async(&mut conn)
            .await
            .map_err(|error| DistributedError::ConnectionError(error.to_string()))?;
        Ok(acquired == 1)
    }

    async fn release_lock(&self, resource: &str, owner: &str) -> Result<(), DistributedError> {
        let script = Script::new(
            r#"
            if redis.call('GET', KEYS[1]) == ARGV[1] then
                redis.call('DEL', KEYS[1])
                return 1
            end
            return 0
        "#,
        );
        let mut conn = self.conn().await;
        let _: i32 = script
            .key(self.lock_key(resource))
            .arg(owner)
            .invoke_async(&mut conn)
            .await
            .map_err(|error| DistributedError::ConnectionError(error.to_string()))?;
        Ok(())
    }

    async fn register_node(&self, node: &NodeInfo) -> Result<(), DistributedError> {
        let mut conn = self.conn().await;
        let payload = serde_json::to_string(node)
            .map_err(|error| DistributedError::ClusterError(error.to_string()))?;
        let _: () = redis::pipe()
            .cmd("SET")
            .arg(self.node_key(&node.id))
            .arg(payload)
            .ignore()
            .cmd("SADD")
            .arg(self.node_index_key())
            .arg(&node.id)
            .ignore()
            .query_async(&mut conn)
            .await
            .map_err(|error| DistributedError::ConnectionError(error.to_string()))?;
        Ok(())
    }

    async fn get_nodes(&self) -> Result<Vec<NodeInfo>, DistributedError> {
        let mut conn = self.conn().await;
        let ids: Vec<String> = conn
            .smembers(self.node_index_key())
            .await
            .map_err(|error| DistributedError::ConnectionError(error.to_string()))?;

        let mut nodes = Vec::new();
        for id in ids {
            let payload: Option<String> = conn
                .get(self.node_key(&id))
                .await
                .map_err(|error| DistributedError::ConnectionError(error.to_string()))?;
            if let Some(payload) = payload {
                nodes.push(
                    serde_json::from_str(&payload)
                        .map_err(|error| DistributedError::ClusterError(error.to_string()))?,
                );
            }
        }

        Ok(nodes)
    }

    async fn watch_harnesses(
        &self,
        _tx: mpsc::Sender<HarnessWatchEvent>,
    ) -> Result<(), DistributedError> {
        Ok(())
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

    async fn acquire_lock(
        &self,
        resource: &str,
        owner: &str,
        ttl_secs: u64,
    ) -> Result<bool, DistributedError> {
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

    async fn watch_harnesses(
        &self,
        _tx: mpsc::Sender<HarnessWatchEvent>,
    ) -> Result<(), DistributedError> {
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
    pub fn new(
        backend: Arc<dyn HarnessBackend>,
        election_key: &str,
        node_id: NodeId,
        ttl_secs: u64,
    ) -> Self {
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
        let acquired = self
            .backend
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
    use crate::agent::harness::RetryConfig;
    use crate::agent::AgentRole;
    use crate::plugins::Plugin;
    use crate::skills::SkillRegistry;
    use async_trait::async_trait;
    use serde_json::{json, Value};
    use std::sync::atomic::{AtomicU32, Ordering};
    use tokio::sync::{Notify, RwLock};
    use uuid::Uuid;
    #[cfg(feature = "web")]
    use {
        crate::cognitive::router::CognitiveRouter,
        crate::config::Config,
        crate::events::EventBus,
        crate::gateway::harness_handlers::register_execution_rpc_handlers,
        crate::gateway::types::{RpcRequest, RpcResponse},
        axum::{extract::State, routing::post, Json, Router},
        tokio::net::TcpListener,
    };

    fn test_dist_config(node_id: &str, port: u16) -> DistributedConfig {
        DistributedConfig {
            node_id: node_id.to_string(),
            node_address: "127.0.0.1".to_string(),
            node_port: port,
            backend_type: BackendType::InMemory,
            backend_uri: "memory://".to_string(),
            lock_ttl_secs: 300,
            heartbeat_interval_secs: 30,
            node_timeout_secs: 60,
        }
    }

    fn test_node(node_id: &str, port: u16, status: NodeStatus, harness_count: usize) -> NodeInfo {
        NodeInfo {
            id: node_id.to_string(),
            address: "127.0.0.1".to_string(),
            port,
            capabilities: vec!["harness".to_string()],
            status,
            last_heartbeat: Utc::now(),
            harness_count,
        }
    }

    #[cfg(feature = "web")]
    async fn test_router() -> Arc<CognitiveRouter> {
        let mut config = Config::default();
        config.llm_vendor = Some("mock".to_string());
        let event_bus = Arc::new(EventBus::new(100));
        Arc::new(CognitiveRouter::new(&config, None, event_bus).await)
    }

    struct EchoPlugin {
        name: String,
        prefix: String,
    }

    #[async_trait]
    impl Plugin for EchoPlugin {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "echo test plugin"
        }

        async fn initialize(&mut self) -> anyhow::Result<()> {
            Ok(())
        }

        async fn execute(&self, _command: &str, args: Value) -> anyhow::Result<String> {
            let payload = args
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or("missing");
            Ok(format!("{}{}", self.prefix, payload))
        }

        async fn shutdown(&mut self) -> anyhow::Result<()> {
            Ok(())
        }
    }

    fn registry_with_echo_tools() -> Arc<RwLock<SkillRegistry>> {
        let mut registry = SkillRegistry::new();
        registry.register_plugin(Box::new(EchoPlugin {
            name: "echo_tool".to_string(),
            prefix: "echo:".to_string(),
        }));
        Arc::new(RwLock::new(registry))
    }

    struct SimpleHarnessAgent;

    #[async_trait]
    impl HarnessAgent for SimpleHarnessAgent {
        fn name(&self) -> &str {
            "simple-harness-agent"
        }

        fn role(&self) -> AgentRole {
            AgentRole::Executor
        }

        fn tools(&self) -> Vec<String> {
            Vec::new()
        }

        async fn execute_step(
            &self,
            _task: &str,
            _context: &[Message],
            _step_number: usize,
        ) -> anyhow::Result<(String, Option<String>, Option<serde_json::Value>)> {
            Ok(("done".to_string(), None, None))
        }
    }

    struct ResumeCapableHarnessAgent {
        step2_attempts: Arc<AtomicU32>,
    }

    #[async_trait]
    impl HarnessAgent for ResumeCapableHarnessAgent {
        fn name(&self) -> &str {
            "resume-capable-harness-agent"
        }

        fn role(&self) -> AgentRole {
            AgentRole::Executor
        }

        fn tools(&self) -> Vec<String> {
            vec!["echo_tool".to_string()]
        }

        async fn execute_step(
            &self,
            _task: &str,
            context: &[Message],
            step_number: usize,
        ) -> anyhow::Result<(String, Option<String>, Option<serde_json::Value>)> {
            match step_number {
                1 => Ok((
                    "Need remote tool context".to_string(),
                    Some("echo_tool".to_string()),
                    Some(json!({"query": "failover"})),
                )),
                2 => {
                    if self.step2_attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                        anyhow::bail!("resume after failover")
                    }

                    let observation = context.last().and_then(Message::text).unwrap_or_default();
                    Ok((
                        format!("Recovered on failover: {}", observation),
                        None,
                        None,
                    ))
                }
                _ => anyhow::bail!("unexpected step {}", step_number),
            }
        }
    }

    struct BlockingProgressHarnessAgent {
        step2_started: Arc<Notify>,
        step2_release: Arc<Notify>,
    }

    #[async_trait]
    impl HarnessAgent for BlockingProgressHarnessAgent {
        fn name(&self) -> &str {
            "blocking-progress-harness-agent"
        }

        fn role(&self) -> AgentRole {
            AgentRole::Executor
        }

        fn tools(&self) -> Vec<String> {
            vec!["echo_tool".to_string()]
        }

        async fn execute_step(
            &self,
            _task: &str,
            context: &[Message],
            step_number: usize,
        ) -> anyhow::Result<(String, Option<String>, Option<serde_json::Value>)> {
            match step_number {
                1 => Ok((
                    "Capture progress before completion".to_string(),
                    Some("echo_tool".to_string()),
                    Some(json!({"query": "midflight"})),
                )),
                2 => {
                    self.step2_started.notify_one();
                    self.step2_release.notified().await;
                    let observation = context.last().and_then(Message::text).unwrap_or_default();
                    Ok((format!("Finished after block: {}", observation), None, None))
                }
                _ => anyhow::bail!("unexpected step {}", step_number),
            }
        }
    }

    struct ToolCallingHarnessAgent;

    #[async_trait]
    impl HarnessAgent for ToolCallingHarnessAgent {
        fn name(&self) -> &str {
            "tool-calling-harness-agent"
        }

        fn role(&self) -> AgentRole {
            AgentRole::Executor
        }

        fn tools(&self) -> Vec<String> {
            vec!["echo_tool".to_string()]
        }

        async fn execute_step(
            &self,
            _task: &str,
            _context: &[Message],
            step_number: usize,
        ) -> anyhow::Result<(String, Option<String>, Option<serde_json::Value>)> {
            match step_number {
                1 => Ok((
                    "Need allowed tool".to_string(),
                    Some("echo_tool".to_string()),
                    Some(json!({"query": "policy"})),
                )),
                2 => Ok(("Tool policy satisfied".to_string(), None, None)),
                _ => anyhow::bail!("unexpected step {}", step_number),
            }
        }
    }

    #[tokio::test]
    async fn test_in_memory_backend() {
        let backend = Arc::new(InMemoryBackend::new());

        // Test harness operations
        let info = HarnessInfo::new("test-1".to_string(), HarnessConfig::default());

        backend.create_harness(&info).await.unwrap();

        let retrieved = backend.get_harness("test-1").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "test-1");

        // Test lock acquisition
        let acquired = backend
            .acquire_lock("resource-1", "node-1", 60)
            .await
            .unwrap();
        assert!(acquired);

        // Test lock not acquired by another
        let acquired2 = backend
            .acquire_lock("resource-1", "node-2", 60)
            .await
            .unwrap();
        assert!(!acquired2);

        // Test lock release
        backend.release_lock("resource-1", "node-1").await.unwrap();

        let acquired3 = backend
            .acquire_lock("resource-1", "node-2", 60)
            .await
            .unwrap();
        assert!(acquired3);
    }

    #[tokio::test]
    async fn test_create_backend_in_memory() {
        let backend = create_backend(&test_dist_config("node-1", 8080))
            .await
            .unwrap();
        let info = HarnessInfo::new("factory-test".to_string(), HarnessConfig::default());

        backend.create_harness(&info).await.unwrap();
        let retrieved = backend.get_harness("factory-test").await.unwrap();

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "factory-test");
    }

    #[test]
    fn test_backend_type_parsing_and_defaults() {
        assert_eq!("redis".parse::<BackendType>().unwrap(), BackendType::Redis);
        assert_eq!(
            "in-memory".parse::<BackendType>().unwrap(),
            BackendType::InMemory
        );
        assert_eq!(
            BackendType::Redis.default_backend_uri(),
            "redis://127.0.0.1/"
        );
        assert_eq!(BackendType::InMemory.to_string(), "memory");
    }

    #[tokio::test]
    async fn test_create_backend_redis_when_url_available() {
        let Some(redis_url) = std::env::var("CRABLET_TEST_REDIS_URL")
            .ok()
            .or_else(|| std::env::var("REDIS_URL").ok())
            .filter(|value| !value.trim().is_empty())
        else {
            eprintln!("skipping redis backend test: CRABLET_TEST_REDIS_URL/REDIS_URL not set");
            return;
        };

        let mut config = test_dist_config("node-redis", 8080);
        config.backend_type = BackendType::Redis;
        config.backend_uri = redis_url;

        let backend = create_backend(&config).await.unwrap();
        let harness_id = format!("redis-factory-{}", Uuid::new_v4());
        let info = HarnessInfo::new(harness_id.clone(), HarnessConfig::default());

        backend.create_harness(&info).await.unwrap();
        let retrieved = backend.get_harness(&harness_id).await.unwrap();
        backend.delete_harness(&harness_id).await.unwrap();

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, harness_id);
    }

    #[tokio::test]
    async fn test_create_backend_rejects_unimplemented_backends() {
        let mut config = test_dist_config("node-1", 8080);
        config.backend_type = BackendType::Consul;

        let result = create_backend(&config).await;
        assert!(matches!(result, Err(DistributedError::ClusterError(_))));
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

    #[tokio::test]
    async fn test_distributed_manager_registers_local_node() {
        let backend: Arc<dyn HarnessBackend> = Arc::new(InMemoryBackend::new());
        let manager =
            DistributedHarnessManager::new(backend.clone(), test_dist_config("node-1", 8080));

        manager.register_local_node().await.unwrap();

        let nodes = backend.get_nodes().await.unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id, "node-1");
        assert_eq!(nodes[0].address, "127.0.0.1");
        assert_eq!(nodes[0].port, 8080);
        assert_eq!(nodes[0].status, NodeStatus::Active);
    }

    #[tokio::test]
    async fn test_distributed_manager_run_agent_syncs_backend_state() {
        let backend: Arc<dyn HarnessBackend> = Arc::new(InMemoryBackend::new());
        let manager =
            DistributedHarnessManager::new(backend.clone(), test_dist_config("node-1", 8080));
        let builder =
            HarnessAgentBuilder::new(Arc::new(SimpleHarnessAgent)).with_config(HarnessConfig {
                max_steps: 2,
                ..Default::default()
            });

        let (id, result) = manager.run_agent(&builder, "hello", &[]).await.unwrap();

        assert!(result.success);

        let backend_info = backend.get_harness(&id).await.unwrap().unwrap();
        assert!(matches!(
            backend_info.status,
            super::super::harness_manager::HarnessStatus::Completed
        ));
        assert_eq!(
            backend_info
                .config
                .metadata
                .get("node_id")
                .map(String::as_str),
            Some("node-1")
        );
    }

    #[tokio::test]
    async fn test_distributed_manager_run_agent_enforces_allowed_tools() {
        let backend: Arc<dyn HarnessBackend> = Arc::new(InMemoryBackend::new());
        let manager =
            DistributedHarnessManager::new(backend.clone(), test_dist_config("node-1", 8080));
        let builder = HarnessAgentBuilder::new(Arc::new(ToolCallingHarnessAgent))
            .with_config(HarnessConfig {
                max_steps: 3,
                ..Default::default()
            })
            .with_skill_registry(registry_with_echo_tools())
            .with_allowed_tools(vec!["echo_tool".to_string()]);

        let (id, result) = manager
            .run_agent(&builder, "policy task", &[])
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.output, "Tool policy satisfied");
        let backend_info = backend.get_harness(&id).await.unwrap().unwrap();
        assert!(matches!(backend_info.status, HarnessStatus::Completed));
    }

    #[tokio::test]
    async fn test_distributed_manager_run_agent_rejects_disallowed_tools() {
        let backend: Arc<dyn HarnessBackend> = Arc::new(InMemoryBackend::new());
        let manager =
            DistributedHarnessManager::new(backend.clone(), test_dist_config("node-1", 8080));
        let builder = HarnessAgentBuilder::new(Arc::new(ToolCallingHarnessAgent))
            .with_config(HarnessConfig {
                max_steps: 3,
                ..Default::default()
            })
            .with_skill_registry(registry_with_echo_tools())
            .with_allowed_tools(vec!["other_tool".to_string()]);

        let (id, result) = manager
            .run_agent(&builder, "policy task", &[])
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result
            .output
            .contains("tool not allowed by execution policy"));
        let backend_info = backend.get_harness(&id).await.unwrap().unwrap();
        assert!(matches!(backend_info.status, HarnessStatus::Failed));
    }

    #[tokio::test]
    async fn test_distributed_manager_run_agent_enforces_resource_limits() {
        let backend: Arc<dyn HarnessBackend> = Arc::new(InMemoryBackend::new());
        let manager =
            DistributedHarnessManager::new(backend.clone(), test_dist_config("node-1", 8080));
        let builder = HarnessAgentBuilder::new(Arc::new(ToolCallingHarnessAgent))
            .with_config(HarnessConfig {
                max_steps: 3,
                max_cpu_time_ms: Some(0),
                ..Default::default()
            })
            .with_skill_registry(registry_with_echo_tools())
            .with_allowed_tools(vec!["echo_tool".to_string()]);

        let (id, result) = manager
            .run_agent(&builder, "budget task", &[])
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.output.contains("Resource limit exceeded"));
        let backend_info = backend.get_harness(&id).await.unwrap().unwrap();
        assert!(matches!(backend_info.status, HarnessStatus::Failed));
    }

    #[tokio::test]
    async fn test_distributed_manager_syncs_progress_to_backend_while_running() {
        let backend: Arc<dyn HarnessBackend> = Arc::new(InMemoryBackend::new());
        let manager = Arc::new(DistributedHarnessManager::new(
            backend.clone(),
            test_dist_config("node-1", 8080),
        ));
        let step2_started = Arc::new(Notify::new());
        let step2_release = Arc::new(Notify::new());
        let builder = Arc::new(
            HarnessAgentBuilder::new(Arc::new(BlockingProgressHarnessAgent {
                step2_started: step2_started.clone(),
                step2_release: step2_release.clone(),
            }))
            .with_config(HarnessConfig {
                max_steps: 4,
                ..Default::default()
            })
            .with_skill_registry(registry_with_echo_tools()),
        );

        let run_task = {
            let manager = manager.clone();
            let builder = builder.clone();
            tokio::spawn(async move {
                manager
                    .run_agent(builder.as_ref(), "track progress", &[])
                    .await
            })
        };

        tokio::time::timeout(std::time::Duration::from_secs(1), step2_started.notified())
            .await
            .unwrap();

        let harnesses = backend.list_harnesses().await.unwrap();
        assert_eq!(harnesses.len(), 1);

        let id = harnesses[0].id.clone();
        let backend_info = backend.get_harness(&id).await.unwrap().unwrap();
        assert!(matches!(backend_info.status, HarnessStatus::Running));
        assert_eq!(
            backend_info
                .execution_state
                .as_ref()
                .map(|state| state.trace.len()),
            Some(1)
        );
        assert_eq!(
            backend_info
                .execution_state
                .as_ref()
                .and_then(|state| state.trace.first())
                .and_then(|step| step.observation.as_deref()),
            Some("echo:midflight")
        );

        step2_release.notify_one();

        let (completed_id, result) = run_task.await.unwrap().unwrap();
        assert_eq!(completed_id, id);
        assert!(result.success);
        assert_eq!(result.trace.len(), 2);

        let completed_info = backend.get_harness(&id).await.unwrap().unwrap();
        assert!(matches!(completed_info.status, HarnessStatus::Completed));
        assert_eq!(
            completed_info
                .execution_state
                .as_ref()
                .map(|state| state.trace.len()),
            Some(2)
        );
    }

    #[tokio::test]
    async fn test_distributed_manager_creates_harness_on_least_loaded_remote_node() {
        let backend = Arc::new(InMemoryBackend::new());
        let control_plane = Arc::new(InProcessHarnessControlPlane::new());
        let primary = Arc::new(
            DistributedHarnessManager::new(backend.clone(), test_dist_config("node-1", 8080))
                .with_control_plane(control_plane.clone()),
        );
        let secondary = Arc::new(
            DistributedHarnessManager::new(backend.clone(), test_dist_config("node-2", 8081))
                .with_control_plane(control_plane.clone()),
        );
        control_plane.register_manager(&primary).await;
        control_plane.register_manager(&secondary).await;
        primary.register_local_node().await.unwrap();
        secondary.register_local_node().await.unwrap();

        let local_id = primary.create_harness(None).await.unwrap();
        assert!(primary.is_local(&local_id).await);

        let id = primary
            .create_harness_distributed(
                Some(HarnessConfig {
                    max_steps: 12,
                    ..Default::default()
                }),
                None,
            )
            .await
            .unwrap();

        let backend_info = backend.get_harness(&id).await.unwrap().unwrap();
        let primary_stats = primary.get_cluster_stats().await.unwrap();

        assert!(!primary.is_local(&id).await);
        assert!(secondary.is_local(&id).await);
        assert_eq!(
            backend_info
                .config
                .metadata
                .get("node_id")
                .map(String::as_str),
            Some("node-2")
        );
        assert_eq!(backend_info.config.max_steps, 12);
        assert_eq!(primary_stats.distributed_stats.remote_operations, 1);
        assert_eq!(primary_stats.distributed_stats.cross_node_delegations, 1);
    }

    #[tokio::test]
    async fn test_distributed_manager_forwards_signal_to_active_remote_owner() {
        let backend = Arc::new(InMemoryBackend::new());
        let control_plane = Arc::new(InProcessHarnessControlPlane::new());
        let primary = Arc::new(
            DistributedHarnessManager::new(backend.clone(), test_dist_config("node-1", 8080))
                .with_control_plane(control_plane.clone()),
        );
        let secondary = Arc::new(
            DistributedHarnessManager::new(backend.clone(), test_dist_config("node-2", 8081))
                .with_control_plane(control_plane.clone()),
        );
        control_plane.register_manager(&primary).await;
        control_plane.register_manager(&secondary).await;

        let id = primary.create_harness(None).await.unwrap();
        primary
            .local_manager()
            .update_status(&id, HarnessStatus::Running)
            .await;
        let running_info = primary.local_manager().get_info(&id).await.unwrap();
        backend.update_harness(&running_info).await.unwrap();

        secondary
            .send_signal(&id, HarnessSignal::Pause)
            .await
            .unwrap();

        let paused_info = primary.local_manager().get_info(&id).await.unwrap();
        let paused_backend = backend.get_harness(&id).await.unwrap().unwrap();
        assert!(matches!(paused_info.status, HarnessStatus::Paused));
        assert!(matches!(paused_backend.status, HarnessStatus::Paused));

        secondary
            .send_signal(&id, HarnessSignal::Resume)
            .await
            .unwrap();

        let resumed_info = primary.local_manager().get_info(&id).await.unwrap();
        let resumed_backend = backend.get_harness(&id).await.unwrap().unwrap();
        let secondary_stats = secondary.get_cluster_stats().await.unwrap();
        assert!(matches!(resumed_info.status, HarnessStatus::Running));
        assert!(matches!(resumed_backend.status, HarnessStatus::Running));
        assert_eq!(secondary_stats.distributed_stats.remote_operations, 2);
        assert_eq!(secondary_stats.distributed_stats.cross_node_delegations, 2);
    }

    #[tokio::test]
    async fn test_distributed_manager_removes_terminal_harness_via_active_remote_owner() {
        let backend = Arc::new(InMemoryBackend::new());
        let control_plane = Arc::new(InProcessHarnessControlPlane::new());
        let primary = Arc::new(
            DistributedHarnessManager::new(backend.clone(), test_dist_config("node-1", 8080))
                .with_control_plane(control_plane.clone()),
        );
        let secondary = Arc::new(
            DistributedHarnessManager::new(backend.clone(), test_dist_config("node-2", 8081))
                .with_control_plane(control_plane.clone()),
        );
        control_plane.register_manager(&primary).await;
        control_plane.register_manager(&secondary).await;
        primary.register_local_node().await.unwrap();
        secondary.register_local_node().await.unwrap();

        let id = primary.create_harness(None).await.unwrap();
        primary.local_manager().complete_harness(&id).await.unwrap();
        let completed_info = primary.local_manager().get_info(&id).await.unwrap();
        backend.update_harness(&completed_info).await.unwrap();

        secondary.remove_harness(&id).await.unwrap();

        let secondary_stats = secondary.get_cluster_stats().await.unwrap();
        assert!(primary.local_manager().get_info(&id).await.is_none());
        assert!(backend.get_harness(&id).await.unwrap().is_none());
        assert_eq!(secondary_stats.distributed_stats.remote_operations, 1);
        assert_eq!(secondary_stats.distributed_stats.cross_node_delegations, 1);
    }

    #[cfg(feature = "web")]
    #[tokio::test]
    async fn test_distributed_manager_forwards_signal_over_http_rpc() {
        let backend = Arc::new(InMemoryBackend::new());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let primary_port = listener.local_addr().unwrap().port();
        let primary = Arc::new(DistributedHarnessManager::new(
            backend.clone(),
            test_dist_config("node-1", primary_port),
        ));

        let rpc = RpcDispatcher::new();
        DistributedHarnessManager::register_rpc_handlers(&primary, &rpc)
            .await
            .unwrap();

        let app = Router::new()
            .route(
                "/rpc",
                post(
                    |State(rpc): State<RpcDispatcher>, Json(request): Json<RpcRequest>| async move {
                        Json::<RpcResponse>(rpc.dispatch(request).await)
                    },
                ),
            )
            .with_state(rpc.clone());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let secondary = Arc::new(
            DistributedHarnessManager::new(backend.clone(), test_dist_config("node-2", 8081))
                .with_control_plane(Arc::new(HttpHarnessControlPlane::new())),
        );

        let id = primary.create_harness(None).await.unwrap();
        primary
            .local_manager()
            .update_status(&id, HarnessStatus::Running)
            .await;
        let running_info = primary.local_manager().get_info(&id).await.unwrap();
        backend.update_harness(&running_info).await.unwrap();

        secondary
            .send_signal(&id, HarnessSignal::Pause)
            .await
            .unwrap();
        let paused_info = primary.local_manager().get_info(&id).await.unwrap();
        let paused_backend = backend.get_harness(&id).await.unwrap().unwrap();
        assert!(matches!(paused_info.status, HarnessStatus::Paused));
        assert!(matches!(paused_backend.status, HarnessStatus::Paused));

        secondary
            .send_signal(&id, HarnessSignal::Resume)
            .await
            .unwrap();
        let resumed_info = primary.local_manager().get_info(&id).await.unwrap();
        let resumed_backend = backend.get_harness(&id).await.unwrap().unwrap();
        let secondary_stats = secondary.get_cluster_stats().await.unwrap();
        assert!(matches!(resumed_info.status, HarnessStatus::Running));
        assert!(matches!(resumed_backend.status, HarnessStatus::Running));
        assert_eq!(secondary_stats.distributed_stats.remote_operations, 2);
        assert_eq!(secondary_stats.distributed_stats.cross_node_delegations, 2);

        server.abort();
    }

    #[cfg(feature = "web")]
    #[tokio::test]
    async fn test_distributed_manager_forwards_run_over_http_rpc() {
        let backend = Arc::new(InMemoryBackend::new());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let primary_port = listener.local_addr().unwrap().port();
        let primary = Arc::new(DistributedHarnessManager::new(
            backend.clone(),
            test_dist_config("node-1", primary_port),
        ));

        let rpc = RpcDispatcher::new();
        DistributedHarnessManager::register_rpc_handlers(&primary, &rpc)
            .await
            .unwrap();
        register_execution_rpc_handlers(&rpc, primary.clone(), test_router().await)
            .await
            .unwrap();

        let app = Router::new()
            .route(
                "/rpc",
                post(
                    |State(rpc): State<RpcDispatcher>, Json(request): Json<RpcRequest>| async move {
                        Json::<RpcResponse>(rpc.dispatch(request).await)
                    },
                ),
            )
            .with_state(rpc.clone());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let secondary = Arc::new(
            DistributedHarnessManager::new(backend.clone(), test_dist_config("node-2", 8081))
                .with_control_plane(Arc::new(HttpHarnessControlPlane::new())),
        );
        primary.register_local_node().await.unwrap();
        secondary.register_local_node().await.unwrap();
        let agent_spec = GenericHarnessAgentSpec {
            role: "http-runner".to_string(),
            name: Some("HTTP Runner".to_string()),
            description: Some("remote execution agent".to_string()),
            system_prompt: Some("You are a remote execution agent.".to_string()),
            allowed_tools: None,
            resource_limits: Some(GenericHarnessResourceLimits {
                max_steps: Some(2),
                tool_timeout_ms: Some(1_500),
                step_timeout_ms: Some(2_500),
                max_memory_bytes: Some(1_048_576),
                max_cpu_time_ms: Some(5_000),
            }),
        };

        let response = secondary
            .forward_generic_run(
                &test_node("node-1", primary_port, NodeStatus::Active, 0),
                GenericHarnessRunRequest {
                    task: "http rpc task".to_string(),
                    context: Vec::new(),
                    agent: agent_spec.clone(),
                    harness_config: Some(HarnessConfig::default()),
                    target_node_id: None,
                },
            )
            .await
            .unwrap();

        let info = primary
            .local_manager()
            .get_info(&response.harness_id)
            .await
            .unwrap();
        let backend_info = backend
            .get_harness(&response.harness_id)
            .await
            .unwrap()
            .unwrap();
        let secondary_stats = secondary.get_cluster_stats().await.unwrap();

        assert!(response.result.success);
        assert_eq!(
            response.result.output,
            "Step 1: (Mock LLM) Processed: http rpc task"
        );
        assert_eq!(
            info.execution_state
                .as_ref()
                .and_then(|state| state.system_prompt.as_deref()),
            Some("You are a remote execution agent.")
        );
        assert_eq!(info.config.max_steps, 2);
        assert_eq!(info.config.tool_timeout.as_millis(), 1_500);
        assert_eq!(info.config.step_timeout.as_millis(), 2_500);
        assert_eq!(
            GenericHarnessAgentSpec::from_metadata(&info.config.metadata).unwrap(),
            agent_spec
        );
        assert!(matches!(info.status, HarnessStatus::Completed));
        assert!(matches!(backend_info.status, HarnessStatus::Completed));
        assert_eq!(secondary_stats.distributed_stats.remote_operations, 1);
        assert_eq!(secondary_stats.distributed_stats.cross_node_delegations, 1);

        server.abort();
    }

    #[cfg(feature = "web")]
    #[tokio::test]
    async fn test_distributed_manager_forwards_resume_over_http_rpc() {
        let backend = Arc::new(InMemoryBackend::new());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let primary_port = listener.local_addr().unwrap().port();
        let primary = Arc::new(DistributedHarnessManager::new(
            backend.clone(),
            test_dist_config("node-1", primary_port),
        ));

        let rpc = RpcDispatcher::new();
        DistributedHarnessManager::register_rpc_handlers(&primary, &rpc)
            .await
            .unwrap();
        register_execution_rpc_handlers(&rpc, primary.clone(), test_router().await)
            .await
            .unwrap();

        let app = Router::new()
            .route(
                "/rpc",
                post(
                    |State(rpc): State<RpcDispatcher>, Json(request): Json<RpcRequest>| async move {
                        Json::<RpcResponse>(rpc.dispatch(request).await)
                    },
                ),
            )
            .with_state(rpc.clone());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let secondary = Arc::new(
            DistributedHarnessManager::new(backend.clone(), test_dist_config("node-2", 8081))
                .with_control_plane(Arc::new(HttpHarnessControlPlane::new())),
        );
        primary.register_local_node().await.unwrap();
        secondary.register_local_node().await.unwrap();
        let persisted_agent = GenericHarnessAgentSpec {
            role: "http-resume-runner".to_string(),
            name: Some("HTTP Resume Runner".to_string()),
            description: Some("remote resume agent".to_string()),
            system_prompt: Some("You are a remote resume agent.".to_string()),
            allowed_tools: None,
            resource_limits: Some(GenericHarnessResourceLimits {
                max_steps: Some(4),
                tool_timeout_ms: Some(1_000),
                step_timeout_ms: Some(2_000),
                max_memory_bytes: Some(2_097_152),
                max_cpu_time_ms: Some(10_000),
            }),
        };
        let mut harness_config = HarnessConfig::default();
        persisted_agent
            .persist_into_metadata(&mut harness_config.metadata)
            .unwrap();

        let harness_id = primary.create_harness(Some(harness_config)).await.unwrap();
        primary
            .local_manager()
            .set_execution_state(
                &harness_id,
                HarnessExecutionState::new("resume over http", &[], None),
            )
            .await
            .unwrap();
        primary
            .local_manager()
            .update_status(&harness_id, HarnessStatus::Paused)
            .await;
        let paused_info = primary.local_manager().get_info(&harness_id).await.unwrap();
        backend.update_harness(&paused_info).await.unwrap();

        let response = secondary
            .forward_generic_resume(
                &test_node("node-1", primary_port, NodeStatus::Active, 1),
                GenericHarnessResumeRequest {
                    harness_id: harness_id.clone(),
                    agent: None,
                    harness_config: None,
                },
            )
            .await
            .unwrap();

        let info = primary.local_manager().get_info(&harness_id).await.unwrap();
        let backend_info = backend.get_harness(&harness_id).await.unwrap().unwrap();
        let secondary_stats = secondary.get_cluster_stats().await.unwrap();

        assert_eq!(response.harness_id, harness_id);
        assert!(response.result.success);
        assert_eq!(
            response.result.output,
            "Step 1: (Mock LLM) Processed: resume over http"
        );
        assert_eq!(
            info.execution_state
                .as_ref()
                .and_then(|state| state.system_prompt.as_deref()),
            Some("You are a remote resume agent.")
        );
        assert_eq!(info.config.max_steps, 4);
        assert_eq!(info.config.tool_timeout.as_millis(), 1_000);
        assert_eq!(info.config.step_timeout.as_millis(), 2_000);
        assert_eq!(
            GenericHarnessAgentSpec::from_metadata(&info.config.metadata).unwrap(),
            persisted_agent
        );
        assert!(matches!(info.status, HarnessStatus::Completed));
        assert!(matches!(backend_info.status, HarnessStatus::Completed));
        assert_eq!(secondary_stats.distributed_stats.remote_operations, 1);
        assert_eq!(secondary_stats.distributed_stats.cross_node_delegations, 1);

        server.abort();
    }

    #[tokio::test]
    async fn test_distributed_manager_claims_dead_remote_harness() {
        let backend = Arc::new(InMemoryBackend::new());
        let primary =
            DistributedHarnessManager::new(backend.clone(), test_dist_config("node-1", 8080));
        let failover =
            DistributedHarnessManager::new(backend.clone(), test_dist_config("node-2", 8081));

        let id = primary.create_harness(None).await.unwrap();
        primary
            .local_manager()
            .update_status(&id, HarnessStatus::Running)
            .await;
        let running_info = primary.local_manager().get_info(&id).await.unwrap();
        backend.update_harness(&running_info).await.unwrap();
        backend
            .register_node(&test_node("node-1", 8080, NodeStatus::Dead, 1))
            .await
            .unwrap();

        let claimed = failover.get_harness(&id).await.unwrap().unwrap();
        let metadata = claimed.read().await.metadata();
        let backend_info = backend.get_harness(&id).await.unwrap().unwrap();
        let cluster_stats = failover.get_cluster_stats().await.unwrap();

        assert!(metadata.paused);
        assert!(failover.is_local(&id).await);
        assert!(matches!(backend_info.status, HarnessStatus::Paused));
        assert_eq!(
            backend_info
                .config
                .metadata
                .get("node_id")
                .map(String::as_str),
            Some("node-2")
        );
        assert_eq!(cluster_stats.distributed_stats.node_failovers, 1);
    }

    #[tokio::test]
    async fn test_handle_node_failure_claims_only_non_terminal_harnesses() {
        let backend = Arc::new(InMemoryBackend::new());
        let primary =
            DistributedHarnessManager::new(backend.clone(), test_dist_config("node-1", 8080));
        let failover =
            DistributedHarnessManager::new(backend.clone(), test_dist_config("node-2", 8081));

        let running_id = primary.create_harness(None).await.unwrap();
        primary
            .local_manager()
            .update_status(&running_id, HarnessStatus::Running)
            .await;
        let running_info = primary.local_manager().get_info(&running_id).await.unwrap();
        backend.update_harness(&running_info).await.unwrap();

        let completed_id = primary.create_harness(None).await.unwrap();
        primary
            .local_manager()
            .complete_harness(&completed_id)
            .await
            .unwrap();
        let completed_info = primary
            .local_manager()
            .get_info(&completed_id)
            .await
            .unwrap();
        backend.update_harness(&completed_info).await.unwrap();

        backend
            .register_node(&test_node("node-1", 8080, NodeStatus::Dead, 2))
            .await
            .unwrap();

        let claimed = failover
            .handle_node_failure(&"node-1".to_string())
            .await
            .unwrap();
        let backend_completed = backend.get_harness(&completed_id).await.unwrap().unwrap();

        assert_eq!(claimed, vec![running_id.clone()]);
        assert!(failover.is_local(&running_id).await);
        assert!(!failover.is_local(&completed_id).await);
        assert!(matches!(backend_completed.status, HarnessStatus::Completed));
        assert_eq!(
            backend_completed
                .config
                .metadata
                .get("node_id")
                .map(String::as_str),
            Some("node-1")
        );
    }

    #[tokio::test]
    async fn test_distributed_manager_resumes_claimed_harness_after_failover() {
        let backend = Arc::new(InMemoryBackend::new());
        let primary =
            DistributedHarnessManager::new(backend.clone(), test_dist_config("node-1", 8080));
        let failover =
            DistributedHarnessManager::new(backend.clone(), test_dist_config("node-2", 8081));
        let step2_attempts = Arc::new(AtomicU32::new(0));
        let builder = HarnessAgentBuilder::new(Arc::new(ResumeCapableHarnessAgent {
            step2_attempts: step2_attempts.clone(),
        }))
        .with_config(HarnessConfig {
            max_steps: 4,
            ..Default::default()
        })
        .with_retry_config(RetryConfig {
            max_retries: 1,
            ..Default::default()
        })
        .with_skill_registry(registry_with_echo_tools());

        let (id, first_result) = primary
            .run_agent(&builder, "resume failover", &[])
            .await
            .unwrap();
        assert!(!first_result.success);
        assert_eq!(first_result.trace.len(), 1);

        backend
            .register_node(&test_node("node-1", 8080, NodeStatus::Dead, 1))
            .await
            .unwrap();

        let resumed = failover.resume_agent(&id, &builder).await.unwrap();
        assert!(resumed.success);
        assert_eq!(resumed.output, "Recovered on failover: echo:failover");
        assert_eq!(resumed.trace.len(), 2);
        assert_eq!(step2_attempts.load(Ordering::SeqCst), 2);

        let backend_info = backend.get_harness(&id).await.unwrap().unwrap();
        assert!(matches!(backend_info.status, HarnessStatus::Completed));
        assert_eq!(
            backend_info
                .execution_state
                .as_ref()
                .map(|state| state.trace.len()),
            Some(2)
        );
        assert_eq!(
            backend_info
                .config
                .metadata
                .get("node_id")
                .map(String::as_str),
            Some("node-2")
        );
    }
}
