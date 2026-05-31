//! Core types for distributed harness coordination

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use super::harness_manager::HarnessStatus;

/// Node identifier
pub type NodeId = String;

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
pub enum HarnessWatchEvent {
    Created { id: String },
    Updated { id: String },
    Deleted { id: String },
}

/// Distributed harness errors
#[derive(Debug)]
pub enum DistributedError {
    /// Backend connection error
    ConnectionError(String),
    /// Cluster coordination error
    ClusterError(String),
    /// Node not found
    NodeNotFound(String),
    /// Lock acquisition failed
    LockFailed { resource: String, owner: String },
    /// Serialization error
    SerializationError(String),
}

impl fmt::Display for DistributedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            Self::ClusterError(msg) => write!(f, "Cluster error: {}", msg),
            Self::NodeNotFound(id) => write!(f, "Node not found: {}", id),
            Self::LockFailed { resource, owner } => {
                write!(f, "Lock failed for resource {} by {}", resource, owner)
            }
            Self::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
        }
    }
}

impl std::error::Error for DistributedError {}

/// Configuration for distributed harness
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedConfig {
    /// This node's ID
    pub node_id: String,
    /// This node's address
    pub node_address: String,
    /// This node's port
    pub node_port: u16,
    /// Backend type
    pub backend_type: BackendType,
    /// Backend connection URI
    pub backend_uri: String,
    /// Key prefix for backend storage
    #[serde(default = "default_key_prefix")]
    pub key_prefix: String,
    /// Lock TTL in seconds
    #[serde(default = "default_lock_ttl")]
    pub lock_ttl_secs: u64,
    /// Heartbeat interval in seconds
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_secs: u64,
    /// Node timeout in seconds
    #[serde(default = "default_node_timeout")]
    pub node_timeout_secs: u64,
}

fn default_key_prefix() -> String {
    "crablet".to_string()
}
fn default_lock_ttl() -> u64 {
    30
}
fn default_heartbeat_interval() -> u64 {
    10
}
fn default_node_timeout() -> u64 {
    60
}

/// Backend type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum BackendType {
    Redis,
    Etcd,
    Consul,
    #[serde(rename = "in-memory")]
    InMemory,
}

impl BackendType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Redis => "redis",
            Self::Etcd => "etcd",
            Self::Consul => "consul",
            Self::InMemory => "in-memory",
        }
    }

    pub fn default_backend_uri(&self) -> &'static str {
        match self {
            Self::Redis => "redis://127.0.0.1:6379",
            Self::Etcd => "http://127.0.0.1:2379",
            Self::Consul => "http://127.0.0.1:8500",
            Self::InMemory => "memory://",
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
        match s {
            "redis" => Ok(Self::Redis),
            "etcd" => Ok(Self::Etcd),
            "consul" => Ok(Self::Consul),
            "in-memory" => Ok(Self::InMemory),
            _ => Err(DistributedError::ClusterError(format!(
                "Unknown backend type: {}",
                s
            ))),
        }
    }
}

/// Check if a harness status is terminal (completed, failed, or cancelled)
pub fn is_terminal_status(status: &HarnessStatus) -> bool {
    matches!(
        status,
        HarnessStatus::Completed | HarnessStatus::Failed | HarnessStatus::Cancelled
    )
}
