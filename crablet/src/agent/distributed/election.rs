//! Leader election for cluster coordination

use std::sync::Arc;

use super::backend::HarnessBackend;
use super::types::{DistributedError, NodeId};

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
        let acquired = self
            .backend
            .acquire_lock(&self.election_key, &self.node_id, self.ttl_secs)
            .await?;

        if acquired {
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
