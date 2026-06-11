//! Distributed harness backend implementations
//!
//! Backend implementations for the `HarnessBackend` trait:
//! - `InMemoryBackend` for single-node testing
//! - `RedisHarnessBackend` for multi-process and multi-node coordination
//! - `LeaderElection` for cluster coordination

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use redis::{aio::ConnectionManager, AsyncCommands, Script};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use super::distributed_types::{DistributedError, HarnessBackend, HarnessWatchEvent, NodeId, NodeInfo};
use crate::agent::harness_manager::HarnessInfo;

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
// Redis Backend
// ============================================

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
