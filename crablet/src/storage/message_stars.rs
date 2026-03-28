//! Message Stars Storage Module
//!
//! Hybrid storage for starred/favorited messages.
//! - Hot: Recently starred messages in Redis (fast access)
//! - Cold: All starred messages in SQLite (persistent)

use std::sync::Arc;
use sqlx::{SqlitePool, Row};
use serde::{Deserialize, Serialize};
use chrono::Utc;

use super::redis_client::RedisClient;

/// Starred message record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageStar {
    pub id: String,
    pub session_id: String,
    pub message_id: String,
    pub created_at: i64,
}

/// Starred message with details for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageStarWithContent {
    pub id: String,
    pub session_id: String,
    pub message_id: String,
    pub content_preview: String,
    pub created_at: i64,
}

/// Message stars store with hybrid storage
pub struct MessageStarsStore {
    redis: Option<Arc<RedisClient>>,
    sqlite_pool: SqlitePool,
}

impl MessageStarsStore {
    /// Create a new message stars store
    pub fn new(redis: Option<Arc<RedisClient>>, sqlite_pool: SqlitePool) -> Self {
        Self { redis, sqlite_pool }
    }

    /// Star a message
    pub async fn star_message(&self, session_id: &str, message_id: &str) -> anyhow::Result<Option<MessageStar>> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();

        // Check if already starred
        let existing = self.get_star(session_id, message_id).await?;
        if existing.is_some() {
            return Ok(existing);
        }

        // Save to SQLite
        sqlx::query(
            "INSERT INTO message_stars (id, session_id, message_id, created_at)
             VALUES (?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(session_id)
        .bind(message_id)
        .bind(now)
        .execute(&self.sqlite_pool)
        .await?;

        // Cache in Redis (hot data, 7 days TTL)
        if let Some(redis) = &self.redis {
            let key = format!("stars:{}:{}", session_id, message_id);
            let json = serde_json::to_string(&MessageStar {
                id: id.clone(),
                session_id: session_id.to_string(),
                message_id: message_id.to_string(),
                created_at: now,
            })?;
            let _ = redis.set(&key, &json, Some(7 * 86400)).await;
        }

        Ok(Some(MessageStar {
            id,
            session_id: session_id.to_string(),
            message_id: message_id.to_string(),
            created_at: now,
        }))
    }

    /// Unstar a message
    pub async fn unstar_message(&self, session_id: &str, message_id: &str) -> anyhow::Result<bool> {
        // Delete from SQLite
        let result = sqlx::query(
            "DELETE FROM message_stars WHERE session_id = ? AND message_id = ?"
        )
        .bind(session_id)
        .bind(message_id)
        .execute(&self.sqlite_pool)
        .await?;

        // Delete from Redis cache
        if let Some(redis) = &self.redis {
            let key = format!("stars:{}:{}", session_id, message_id);
            let _ = redis.del(&key).await;
        }

        Ok(result.rows_affected() > 0)
    }

    /// Get a specific star
    pub async fn get_star(&self, session_id: &str, message_id: &str) -> anyhow::Result<Option<MessageStar>> {
        // Try Redis first
        if let Some(redis) = &self.redis {
            let key = format!("stars:{}:{}", session_id, message_id);
            if let Ok(Some(json)) = redis.get(&key).await {
                if let Ok(star) = serde_json::from_str::<MessageStar>(&json) {
                    return Ok(Some(star));
                }
            }
        }

        // Fallback to SQLite
        let row = sqlx::query(
            "SELECT id, session_id, message_id, created_at FROM message_stars WHERE session_id = ? AND message_id = ?"
        )
        .bind(session_id)
        .bind(message_id)
        .fetch_optional(&self.sqlite_pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(MessageStar {
                id: row.get("id"),
                session_id: row.get("session_id"),
                message_id: row.get("message_id"),
                created_at: row.get("created_at"),
            }))
        } else {
            Ok(None)
        }
    }

    /// List all starred messages for a session
    pub async fn list_stars(&self, session_id: &str) -> anyhow::Result<Vec<MessageStar>> {
        let rows = sqlx::query(
            "SELECT id, session_id, message_id, created_at FROM message_stars WHERE session_id = ? ORDER BY created_at DESC"
        )
        .bind(session_id)
        .fetch_all(&self.sqlite_pool)
        .await?;

        Ok(rows.iter().map(|row| MessageStar {
            id: row.get("id"),
            session_id: row.get("session_id"),
            message_id: row.get("message_id"),
            created_at: row.get("created_at"),
        }).collect())
    }

    /// Check if a message is starred
    pub async fn is_starred(&self, session_id: &str, message_id: &str) -> anyhow::Result<bool> {
        let star = self.get_star(session_id, message_id).await?;
        Ok(star.is_some())
    }

    /// Get star count for a session
    pub async fn get_star_count(&self, session_id: &str) -> anyhow::Result<u32> {
        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM message_stars WHERE session_id = ?"
        )
        .bind(session_id)
        .fetch_one(&self.sqlite_pool)
        .await?;

        Ok(row.get::<i64, _>("count") as u32)
    }
}

/// Initialize message stars table
pub async fn init_message_stars_table(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS message_stars (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            message_id TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            UNIQUE(session_id, message_id)
        )"
    )
    .execute(pool)
    .await?;

    // Create index for faster lookups
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_message_stars_session ON message_stars(session_id)"
    )
    .execute(pool)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_star_serialization() {
        let star = MessageStar {
            id: "test-id".to_string(),
            session_id: "session-1".to_string(),
            message_id: "msg-1".to_string(),
            created_at: 1234567890,
        };

        let json = serde_json::to_string(&star).unwrap();
        let deserialized: MessageStar = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, star.id);
        assert_eq!(deserialized.session_id, star.session_id);
        assert_eq!(deserialized.message_id, star.message_id);
    }
}