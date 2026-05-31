//! Distributed Harness Manager - Multi-node harness coordination
//!
//! This module provides distributed harness management capabilities:
//! - Pluggable backend support (Redis, etcd, Consul)
//! - Cross-node harness state synchronization
//! - Leader election for coordination
//! - Distributed lock management
//! - Health monitoring and failover

mod backend;
mod control_plane;
mod election;
mod health;
mod manager;
mod types;

// Re-export all public items for backward compatibility
pub use backend::{create_backend, InMemoryBackend, RedisHarnessBackend};
pub use control_plane::{
    GenericHarnessAgentSpec, GenericHarnessResourceLimits, GenericHarnessResumeRequest,
    GenericHarnessResumeResponse, GenericHarnessRunRequest, GenericHarnessRunResponse,
    HarnessControlPlane, HttpHarnessControlPlane, InProcessHarnessControlPlane,
    NoopHarnessControlPlane,
};
pub use election::LeaderElection;
pub use health::{HealthMonitor, NodeHealth};
pub use manager::{DistributedHarnessManager, DistributedStats, ShutdownSignal};
pub use types::{BackendType, DistributedConfig, DistributedError, HarnessWatchEvent, NodeId, NodeInfo, NodeStatus};
