//! Redis Client Wrapper
//!
//! Provides Redis connection management with connection pooling and error handling.

use redis::{AsyncCommands, Client};
use redis::aio::ConnectionManager;
use std::sync::Arc;
use anyhow::Result;

/// Connection pool configuration
#[derive(Clone, Debug)]
pub struct RedisPoolConfig {
    /// Maximum connections in pool
    pub max_connections: usize,
    /// Connection timeout
    pub connection_timeout_secs: u64,
    /// Read timeout
    pub read_timeout_secs: Option<u64>,
    /// Write timeout
    pub write_timeout_secs: Option<u64>,
    /// Keep-alive duration
    pub keepalive_secs: Option<u64>,
}

impl Default for RedisPoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 16,
            connection_timeout_secs: 5,
            read_timeout_secs: Some(3),
            write_timeout_secs: Some(3),
            keepalive_secs: Some(60),
        }
    }
}

/// Redis client wrapper with configurable connection pool
#[derive(Clone)]
pub struct RedisClient {
    connection_manager: Arc<tokio::sync::RwLock<ConnectionManager>>,
    pool_config: RedisPoolConfig,
    url: String,
}

impl RedisClient {
    /// Create a new Redis client with default configuration
    pub async fn new(url: &str) -> Result<Self> {
        Self::with_config(url, RedisPoolConfig::default()).await
    }

    /// Create a new Redis client with custom pool configuration
    pub async fn with_config(url: &str, config: RedisPoolConfig) -> Result<Self> {
        let client = Client::open(url)?;

        // Create connection manager with timeout configuration
        let connection_manager = ConnectionManager::new(client).await?;

        Ok(Self {
            connection_manager: Arc::new(tokio::sync::RwLock::new(connection_manager)),
            pool_config: config,
            url: url.to_string(),
        })
    }

    /// Get a cloned connection for operations
    async fn get_conn(&self) -> ConnectionManager {
        // For ConnectionManager, we clone to get a pooled connection
        // The underlying connection is automatically managed
        self.connection_manager.read().await.clone()
    }

    /// Set a key with optional TTL
    pub async fn set(&self, key: &str, value: &str, ttl_seconds: Option<u64>) -> Result<()> {
        let mut conn = self.get_conn().await;

        match ttl_seconds {
            Some(ttl) => {
                redis::cmd("SETEX")
                    .arg(key)
                    .arg(ttl)
                    .arg(value)
                    .query_async::<()>(&mut conn)
                    .await?;
            }
            None => {
                redis::cmd("SET")
                    .arg(key)
                    .arg(value)
                    .query_async::<()>(&mut conn)
                    .await?;
            }
        }

        Ok(())
    }

    /// Get a value by key
    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.get_conn().await;
        let result: Option<String> = conn.get(key).await?;
        Ok(result)
    }

    /// Delete a key
    pub async fn del(&self, key: &str) -> Result<bool> {
        let mut conn = self.get_conn().await;
        let deleted: i64 = conn.del(key).await?;
        Ok(deleted > 0)
    }

    /// Check if key exists
    pub async fn exists(&self, key: &str) -> Result<bool> {
        let mut conn = self.get_conn().await;
        let exists: i64 = conn.exists(key).await?;
        Ok(exists > 0)
    }

    /// Set hash field
    pub async fn hset(&self, key: &str, field: &str, value: &str) -> Result<()> {
        let mut conn = self.get_conn().await;
        redis::cmd("HSET")
            .arg(key)
            .arg(field)
            .arg(value)
            .query_async::<()>(&mut conn)
            .await?;
        Ok(())
    }

    /// Get hash field
    pub async fn hget(&self, key: &str, field: &str) -> Result<Option<String>> {
        let mut conn = self.get_conn().await;
        let result: Option<String> = conn.hget(key, field).await?;
        Ok(result)
    }

    /// Get all hash fields
    pub async fn hgetall(&self, key: &str) -> Result<std::collections::HashMap<String, String>> {
        let mut conn = self.get_conn().await;
        let result: std::collections::HashMap<String, String> = conn.hgetall(key).await?;
        Ok(result)
    }

    /// Delete hash field
    pub async fn hdel(&self, key: &str, field: &str) -> Result<bool> {
        let mut conn = self.get_conn().await;
        let deleted: i64 = conn.hdel(key, field).await?;
        Ok(deleted > 0)
    }

    /// Set sorted set member with score
    pub async fn zadd(&self, key: &str, member: &str, score: f64) -> Result<()> {
        let mut conn = self.get_conn().await;
        redis::cmd("ZADD")
            .arg(key)
            .arg(score)
            .arg(member)
            .query_async::<()>(&mut conn)
            .await?;
        Ok(())
    }

    /// Get sorted set members by rank range
    pub async fn zrange(&self, key: &str, start: isize, stop: isize) -> Result<Vec<String>> {
        let mut conn = self.get_conn().await;
        let result: Vec<String> = conn.zrange(key, start, stop).await?;
        Ok(result)
    }

    /// Get sorted set members with scores
    pub async fn zrange_with_scores(&self, key: &str, start: isize, stop: isize) -> Result<Vec<(String, f64)>> {
        let mut conn = self.get_conn().await;
        let result: Vec<(String, f64)> = conn.zrange_withscores(key, start, stop).await?;
        Ok(result)
    }

    /// Set key expiration
    pub async fn expire(&self, key: &str, ttl_seconds: u64) -> Result<()> {
        let mut conn = self.get_conn().await;
        redis::cmd("EXPIRE")
            .arg(key)
            .arg(ttl_seconds)
            .query_async::<()>(&mut conn)
            .await?;
        Ok(())
    }

    /// Increment a counter
    pub async fn incr(&self, key: &str) -> Result<i64> {
        let mut conn = self.get_conn().await;
        let result: i64 = conn.incr(key, 1).await?;
        Ok(result)
    }

    /// Ping to check connection
    pub async fn ping(&self) -> Result<String> {
        let mut conn = self.get_conn().await;
        let result: String = redis::cmd("PING").query_async(&mut conn).await?;
        Ok(result)
    }

    /// Get pool statistics (for monitoring)
    pub fn pool_stats(&self) -> RedisPoolStats {
        RedisPoolStats {
            max_connections: self.pool_config.max_connections,
            url: self.url.clone(),
        }
    }
}

/// Redis connection pool statistics
#[derive(Debug, Clone)]
pub struct RedisPoolStats {
    pub max_connections: usize,
    pub url: String,
}

/// Generate Redis key with prefix
pub fn make_key(prefix: &str, id: &str) -> String {
    format!("{}:{}", prefix, id)
}

/// Session context Redis keys
pub fn session_key(session_id: &str) -> String {
    make_key("session", session_id)
}

/// Canvas state Redis keys
pub fn canvas_key(canvas_id: &str) -> String {
    make_key("canvas", canvas_id)
}

/// Canvas lock Redis keys
pub fn canvas_lock_key(canvas_id: &str, user_id: &str) -> String {
    make_key("canvas_lock", &format!("{}:{}", canvas_id, user_id))
}

/// Collaboration state Redis keys
pub fn collab_key(canvas_id: &str) -> String {
    make_key("collab", canvas_id)
}

/// Template hot data Redis keys
pub fn template_key(template_id: &str) -> String {
    make_key("template", template_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_key() {
        assert_eq!(make_key("session", "123"), "session:123");
        assert_eq!(make_key("canvas", "abc"), "canvas:abc");
    }

    #[test]
    fn test_session_key() {
        assert_eq!(session_key("sess_001"), "session:sess_001");
    }

    #[test]
    fn test_canvas_key() {
        assert_eq!(canvas_key("canvas_001"), "canvas:canvas_001");
    }
}
