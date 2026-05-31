//! Backend implementations for distributed harness coordination
//!
//! Provides pluggable backend support:
//! - InMemoryBackend for single-node testing
//! - RedisHarnessBackend for multi-process and multi-node coordination

use async_trait::async_trait;
use redis::{aio::ConnectionManager, AsyncCommands, Script};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};

use super::harness_manager::HarnessInfo;
use super::types::{BackendType, DistributedError, HarnessWatchEvent, NodeInfo};

use super::control_plane::HarnessControlPlane;

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

/// Create a backend based on configuration
pub async fn create_backend(config: &super::types::DistributedConfig) -> Result<Arc<dyn HarnessBackend>, DistributedError> {
    let backend: Arc<dyn HarnessBackend> = match config.backend_type {
        BackendType::InMemory => Arc::new(InMemoryBackend::new()),
        BackendType::Redis => {
            Arc::new(
                RedisHarnessBackend::new(&config.backend_uri, &config.key_prefix).await?,
            )
        }
        BackendType::Etcd | BackendType::Consul => {
            return Err(DistributedError::ClusterError(format!(
                "Backend type {} is not yet implemented",
                config.backend_type
            )));
        }
    };
    Ok(backend)
}

// ============================================
// In-Memory Backend
// ============================================

/// In-memory backend for single-node testing
pub struct InMemoryBackend {
    harnesses: Arc<RwLock<HashMap<String, HarnessInfo>>>,
    locks: Arc<RwLock<HashMap<String, (String, std::time::Instant)>>>,
    nodes: Arc<RwLock<HashMap<String, NodeInfo>>>,
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
        self.harnesses
            .write()
            .await
            .insert(info.id.clone(), info.clone());
        Ok(())
    }

    async fn update_harness(&self, info: &HarnessInfo) -> Result<(), DistributedError> {
        self.harnesses
            .write()
            .await
            .insert(info.id.clone(), info.clone());
        Ok(())
    }

    async fn delete_harness(&self, id: &str) -> Result<(), DistributedError> {
        self.harnesses.write().await.remove(id);
        Ok(())
    }

    async fn get_harness(&self, id: &str) -> Result<Option<HarnessInfo>, DistributedError> {
        Ok(self.harnesses.read().await.get(id).cloned())
    }

    async fn list_harnesses(&self) -> Result<Vec<HarnessInfo>, DistributedError> {
        Ok(self.harnesses.read().await.values().cloned().collect())
    }

    async fn acquire_lock(
        &self,
        resource: &str,
        owner: &str,
        _ttl_secs: u64,
    ) -> Result<bool, DistributedError> {
        let mut locks = self.locks.write().await;
        if let Some((current_owner, expires_at)) = locks.get(resource) {
            if current_owner != owner && expires_at > &std::time::Instant::now() {
                return Ok(false);
            }
        }
        locks.insert(
            resource.to_string(),
            (owner.to_string(), std::time::Instant::now() + Duration::from_secs(300)),
        );
        Ok(true)
    }

    async fn release_lock(&self, resource: &str, owner: &str) -> Result<(), DistributedError> {
        let mut locks = self.locks.write().await;
        if let Some((current_owner, _)) = locks.get(resource) {
            if current_owner == owner {
                locks.remove(resource);
            }
        }
        Ok(())
    }

    async fn register_node(&self, node: &NodeInfo) -> Result<(), DistributedError> {
        self.nodes
            .write()
            .await
            .insert(node.id.clone(), node.clone());
        Ok(())
    }

    async fn get_nodes(&self) -> Result<Vec<NodeInfo>, DistributedError> {
        Ok(self.nodes.read().await.values().cloned().collect())
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
    pub async fn new(
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
