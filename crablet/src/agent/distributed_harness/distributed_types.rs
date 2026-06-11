//! Distributed harness types, configuration, and error definitions
//!
//! Shared types used across the distributed harness subsystem:
//! - Node identity and status
//! - Backend configuration and factory
//! - Distributed error types
//! - Watch events for state synchronization

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::agent::harness_manager::HarnessInfo;

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

/// Generic harness resource limits for cross-node requests
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
    pub fn apply_to_config(&self, config: &mut super::super::harness::HarnessConfig) {
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

/// Generic harness agent specification for cross-node requests
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

/// Generic harness run request for cross-node execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericHarnessRunRequest {
    pub task: String,
    pub context: Vec<crate::types::Message>,
    pub agent: GenericHarnessAgentSpec,
    pub harness_config: Option<super::super::harness::HarnessConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_node_id: Option<String>,
}

/// Generic harness run response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericHarnessRunResponse {
    pub harness_id: String,
    pub result: super::super::harness_agent::HarnessAgentResult,
}

/// Generic harness resume request for cross-node resumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericHarnessResumeRequest {
    pub harness_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<GenericHarnessAgentSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness_config: Option<super::super::harness::HarnessConfig>,
}

/// Generic harness resume response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericHarnessResumeResponse {
    pub harness_id: String,
    pub result: super::super::harness_agent::HarnessAgentResult,
}

/// Create a backend instance from configuration
pub async fn create_backend(
    config: &DistributedConfig,
) -> Result<Arc<dyn HarnessBackend>, DistributedError> {
    match config.backend_type {
        BackendType::InMemory => Ok(Arc::new(
            super::distributed_backend::InMemoryBackend::new(),
        )),
        BackendType::Redis => Ok(Arc::new(
            super::distributed_backend::RedisHarnessBackend::new(&config.backend_uri).await?,
        )),
        BackendType::Etcd | BackendType::Consul => Err(DistributedError::ClusterError(format!(
            "{:?} backend is not implemented yet",
            config.backend_type
        ))),
    }
}
