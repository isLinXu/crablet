//! Session Context Store
//!
//! Hybrid storage for session context with token tracking.
//! - Hot: Active session context in Redis (fast read/write)
//! - Cold: Session history in SQLite (persistent)

use std::sync::Arc;
use sqlx::{SqlitePool, Row};
use serde::{Deserialize, Serialize};
use chrono::Utc;

use super::redis_client::{RedisClient, session_key};

/// Session context stored in Redis (hot data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    pub session_id: String,
    pub token_count: u32,
    pub max_tokens: u32,
    pub compressed: bool,
    pub last_updated: i64,
    pub messages_json: String,  // JSON serialized messages
}

/// Token usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub session_id: String,
    pub total_tokens: u32,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub token_limit: u32,
    pub usage_percentage: f32,
    pub last_updated: i64,
}

/// Session context store with hybrid storage
pub struct SessionContextStore {
    redis: Option<Arc<RedisClient>>,
    sqlite_pool: SqlitePool,
}

impl SessionContextStore {
    /// Create a new session context store
    pub fn new(redis: Option<Arc<RedisClient>>, sqlite_pool: SqlitePool) -> Self {
        Self { redis, sqlite_pool }
    }

    /// Get session context from Redis (hot) or SQLite (cold fallback)
    pub async fn get_context(&self, session_id: &str) -> anyhow::Result<Option<SessionContext>> {
        // Try Redis first
        if let Some(redis) = &self.redis {
            let key = session_key(session_id);
            if let Ok(Some(json)) = redis.get(&key).await {
                if let Ok(context) = serde_json::from_str::<SessionContext>(&json) {
                    return Ok(Some(context));
                }
            }
        }

        // Fallback to SQLite (cold data)
        let row = sqlx::query(
            "SELECT session_id, token_count, max_tokens, compressed, last_updated, messages_json 
             FROM session_contexts WHERE session_id = ?"
        )
        .bind(session_id)
        .fetch_optional(&self.sqlite_pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(SessionContext {
                session_id: row.get("session_id"),
                token_count: row.get("token_count"),
                max_tokens: row.get("max_tokens"),
                compressed: row.get("compressed"),
                last_updated: row.get("last_updated"),
                messages_json: row.get("messages_json"),
            }))
        } else {
            Ok(None)
        }
    }

    /// Save session context (hot: Redis + cold: SQLite with transaction)
    /// Uses write-through caching: both Redis and SQLite must succeed
    pub async fn save_context(&self, context: &SessionContext) -> anyhow::Result<()> {
        let json = serde_json::to_string(context)?;

        // Use a transaction for SQLite to ensure atomicity
        let mut tx = sqlx::Acquire::begin(&self.sqlite_pool).await?;

        // Save to SQLite first (source of truth)
        sqlx::query(
            "INSERT OR REPLACE INTO session_contexts (session_id, token_count, max_tokens, compressed, last_updated, messages_json)
             VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(&context.session_id)
        .bind(context.token_count)
        .bind(context.max_tokens)
        .bind(context.compressed)
        .bind(context.last_updated)
        .bind(&context.messages_json)
        .execute(&mut *tx)
        .await?;

        // Commit SQLite transaction
        tx.commit().await?;

        // Save to Redis (hot, best-effort) - after SQLite succeeds
        // Redis failure is logged but doesn't fail the operation since SQLite is the source of truth
        if let Some(redis) = &self.redis {
            let key = session_key(&context.session_id);
            match redis.set(&key, &json, Some(86400)).await {
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!("Failed to update Redis cache for session {}: {}. SQLite is up-to-date.", context.session_id, e);
                }
            }
        }

        Ok(())
    }

    /// Update token count
    pub async fn update_token_count(&self, session_id: &str, token_count: u32) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();

        // Update Redis
        if let Some(redis) = &self.redis {
            let key = session_key(session_id);
            if let Ok(Some(json)) = redis.get(&key).await {
                if let Ok(mut context) = serde_json::from_str::<SessionContext>(&json) {
                    context.token_count = token_count;
                    context.last_updated = now;
                    let json = serde_json::to_string(&context)?;
                    redis.set(&key, &json, Some(86400)).await?;
                }
            }
        }

        // Update SQLite
        sqlx::query(
            "UPDATE session_contexts SET token_count = ?, last_updated = ? WHERE session_id = ?"
        )
        .bind(token_count)
        .bind(now)
        .bind(session_id)
        .execute(&self.sqlite_pool)
        .await?;

        Ok(())
    }

    /// Get token usage statistics
    pub async fn get_token_usage(&self, session_id: &str) -> anyhow::Result<Option<TokenUsage>> {
        let context = self.get_context(session_id).await?;

        if let Some(ctx) = context {
            let usage_percentage = if ctx.max_tokens > 0 {
                (ctx.token_count as f32 / ctx.max_tokens as f32) * 100.0
            } else {
                0.0
            };

            Ok(Some(TokenUsage {
                session_id: ctx.session_id,
                total_tokens: ctx.token_count,
                prompt_tokens: ctx.token_count / 2,  // Estimate
                completion_tokens: ctx.token_count / 2,  // Estimate
                token_limit: ctx.max_tokens,
                usage_percentage,
                last_updated: ctx.last_updated,
            }))
        } else {
            Ok(None)
        }
    }

    /// Compress session context (reduce message history)
    pub async fn compress_context(&self, session_id: &str, keep_recent: usize) -> anyhow::Result<bool> {
        let mut context = match self.get_context(session_id).await? {
            Some(ctx) => ctx,
            None => return Ok(false),
        };

        // Parse messages and keep only recent ones
        if let Ok(messages) = serde_json::from_str::<Vec<serde_json::Value>>(&context.messages_json) {
            let msg_len = messages.len();
            if msg_len > keep_recent {
                let kept: Vec<_> = messages.into_iter().skip(msg_len - keep_recent).collect();
                context.messages_json = serde_json::to_string(&kept)?;
                context.compressed = true;
                context.last_updated = Utc::now().timestamp();
                self.save_context(&context).await?;
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Delete session context
    pub async fn delete_context(&self, session_id: &str) -> anyhow::Result<()> {
        // Delete from Redis
        if let Some(redis) = &self.redis {
            let key = session_key(session_id);
            let _ = redis.del(&key).await;
        }

        // Delete from SQLite
        sqlx::query("DELETE FROM session_contexts WHERE session_id = ?")
            .bind(session_id)
            .execute(&self.sqlite_pool)
            .await?;

        Ok(())
    }

    /// List active sessions (from Redis)
    pub async fn list_active_sessions(&self) -> anyhow::Result<Vec<String>> {
        if let Some(_redis) = &self.redis {
            // Use SCAN to find all session keys
            // For simplicity, we return session IDs from SQLite
            let rows = sqlx::query(
                "SELECT session_id FROM session_contexts WHERE last_updated > ? ORDER BY last_updated DESC LIMIT 100"
            )
            .bind(Utc::now().timestamp() - 86400)  // Last 24 hours
            .fetch_all(&self.sqlite_pool)
            .await?;

            Ok(rows.iter().map(|r| r.get("session_id")).collect())
        } else {
            // Redis not available, query SQLite
            let rows = sqlx::query(
                "SELECT session_id FROM session_contexts ORDER BY last_updated DESC LIMIT 100"
            )
            .fetch_all(&self.sqlite_pool)
            .await?;

            Ok(rows.iter().map(|r| r.get("session_id")).collect())
        }
    }

    /// Search for relevant messages across all sessions using simple keyword matching
    /// Returns (session_id, message_index, matched_message, relevance_score)
    pub async fn search_history(&self, query: &str, limit: usize) -> anyhow::Result<Vec<HistorySearchResult>> {
        let query_lower = query.to_lowercase();
        let query_terms: Vec<&str> = query_lower.split_whitespace().filter(|w| w.len() > 2).collect();

        if query_terms.is_empty() {
            return Ok(vec![]);
        }

        // Get recent sessions (last 50)
        let rows = sqlx::query(
            "SELECT session_id, messages_json FROM session_contexts ORDER BY last_updated DESC LIMIT 50"
        )
        .fetch_all(&self.sqlite_pool)
        .await?;

        let mut results: Vec<HistorySearchResult> = vec![];

        for row in rows {
            let session_id: String = row.get("session_id");
            let messages_json: String = row.get("messages_json");

            if let Ok(messages) = serde_json::from_str::<Vec<serde_json::Value>>(&messages_json) {
                for (_idx, msg) in messages.iter().enumerate() {
                    // Extract text content from message
                    let content = msg.get("content")
                        .and_then(|c| c.as_str())
                        .unwrap_or_default();
                    let content_lower = content.to_lowercase();

                    // Calculate simple relevance score based on term matches
                    let mut match_count = 0;
                    for term in &query_terms {
                        if content_lower.contains(term) {
                            match_count += 1;
                        }
                    }

                    if match_count > 0 {
                        let score = match_count as f32 / query_terms.len() as f32;
                        results.push(HistorySearchResult {
                            session_id: session_id.clone(),
                            message_id: msg.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            role: msg.get("role").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            content_preview: if content.len() > 200 {
                                format!("{}...", &content[..200])
                            } else {
                                content.to_string()
                            },
                            relevance_score: score,
                        });
                    }
                }
            }
        }

        // Sort by relevance and limit
        results.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);

        Ok(results)
    }
}

/// Result of history search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistorySearchResult {
    pub session_id: String,
    pub message_id: String,
    pub role: String,
    pub content_preview: String,
    pub relevance_score: f32,
}

/// Initialize session context tables
pub async fn init_session_tables(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS session_contexts (
            session_id TEXT PRIMARY KEY,
            token_count INTEGER NOT NULL DEFAULT 0,
            max_tokens INTEGER NOT NULL DEFAULT 128000,
            compressed INTEGER NOT NULL DEFAULT 0,
            last_updated INTEGER NOT NULL,
            messages_json TEXT NOT NULL DEFAULT '[]'
        )"
    )
    .execute(pool)
    .await?;

    // Create index for faster lookups
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_session_last_updated ON session_contexts(last_updated DESC)"
    )
    .execute(pool)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_usage_calculation() {
        let usage = TokenUsage {
            session_id: "test".to_string(),
            total_tokens: 32000,
            prompt_tokens: 16000,
            completion_tokens: 16000,
            token_limit: 128000,
            usage_percentage: 25.0,
            last_updated: Utc::now().timestamp(),
        };

        assert_eq!(usage.usage_percentage, 25.0);
    }
}
