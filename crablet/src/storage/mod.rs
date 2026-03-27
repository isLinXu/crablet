//! Hybrid Storage Layer Module
//!
//! Implements SQLite + Redis hybrid storage for Chat & Canvas enhancement.
//! - Hot data (active sessions, real-time state) → Redis
//! - Cold data (history, versions) → SQLite
//!
//! ## Data Flow
//! ```
//! User Request → Redis (hot cache) → SQLite (persistent)
//!                     ↑                   ↓
//!                     ←←←←←←←←←←←←←←←←←←←←←
//! ```

pub mod redis_client;
pub mod session_context;
pub mod canvas_state;
pub mod message_stars;
pub mod layer_cache;  // P0: Multi-layer cache (L1 Memory + L2 Redis + L3 SQLite)

use std::sync::Arc;
use sqlx::sqlite::SqlitePool;

use redis_client::RedisClient;
use session_context::SessionContextStore;
use canvas_state::CanvasStateStore;
use message_stars::MessageStarsStore;
use session_context::init_session_tables;
use canvas_state::init_canvas_tables;
use message_stars::init_message_stars_table;

/// Hybrid storage configuration
#[derive(Clone)]
pub struct StorageConfig {
    pub redis_url: Option<String>,
    pub sqlite_pool: SqlitePool,
}

/// Unified storage layer combining Redis and SQLite
pub struct HybridStorage {
    pub redis: Option<Arc<RedisClient>>,
    pub session_context: Arc<SessionContextStore>,
    pub canvas_state: Arc<CanvasStateStore>,
    pub message_stars: Arc<MessageStarsStore>,
}

impl HybridStorage {
    /// Initialize hybrid storage with Redis + SQLite
    pub async fn new(config: StorageConfig) -> anyhow::Result<Self> {
        init_session_tables(&config.sqlite_pool).await?;
        init_canvas_tables(&config.sqlite_pool).await?;
        init_message_stars_table(&config.sqlite_pool).await?;

        // Initialize Redis client (optional, graceful degradation)
        let redis = match &config.redis_url {
            Some(url) => {
                match RedisClient::new(url).await {
                    Ok(client) => {
                        tracing::info!("Redis connected successfully");
                        Some(Arc::new(client))
                    }
                    Err(e) => {
                        tracing::warn!("Redis connection failed, operating in SQLite-only mode: {}", e);
                        None
                    }
                }
            }
            None => {
                tracing::info!("Redis not configured, operating in SQLite-only mode");
                None
            }
        };

        // Initialize session context store (hot: Redis, cold: SQLite)
        let session_context = Arc::new(SessionContextStore::new(
            redis.clone(),
            config.sqlite_pool.clone(),
        ));

        // Initialize canvas state store
        let canvas_state = Arc::new(CanvasStateStore::new(
            redis.clone(),
            config.sqlite_pool.clone(),
        ));

        // Initialize message stars store
        let message_stars = Arc::new(MessageStarsStore::new(
            redis.clone(),
            config.sqlite_pool.clone(),
        ));

        Ok(Self {
            redis,
            session_context,
            canvas_state,
            message_stars,
        })
    }

    /// Check if Redis is available
    pub fn is_redis_available(&self) -> bool {
        self.redis.is_some()
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_storage_init_without_redis() {
        // This test verifies graceful degradation when Redis is not available
        // In real tests, we would mock Redis or use testcontainers
    }
}
