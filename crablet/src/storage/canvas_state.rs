//! Canvas State Store
//!
//! Hybrid storage for Canvas state with version control.
//! - Hot: Active canvas state in Redis (fast read/write)
//! - Cold: Canvas versions in SQLite (persistent)

use std::sync::Arc;
use sqlx::{SqlitePool, Row};
use serde::{Deserialize, Serialize};
use chrono::Utc;
use uuid::Uuid;

use super::redis_client::{RedisClient, canvas_key, canvas_lock_key, collab_key};

/// Canvas state stored in Redis (hot data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasState {
    pub canvas_id: String,
    pub name: String,
    pub nodes_json: String,
    pub edges_json: String,
    pub version: i32,
    pub folder_id: Option<String>,
    pub owner_id: String,
    pub last_updated: i64,
}

/// Canvas version snapshot (cold data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasVersion {
    pub id: String,
    pub canvas_id: String,
    pub version: i32,
    pub snapshot_json: String,
    pub diff_json: Option<String>,
    pub summary: Option<String>,
    pub created_by: String,
    pub created_at: i64,
}

/// Canvas lock info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasLock {
    pub canvas_id: String,
    pub user_id: String,
    pub locked_at: i64,
    pub expires_at: i64,
}

/// Collaboration state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollabState {
    pub canvas_id: String,
    pub users: Vec<String>,
    pub cursors: std::collections::HashMap<String, CursorPosition>,
    pub selections: std::collections::HashMap<String, Selection>,
}

/// Cursor position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorPosition {
    pub x: f64,
    pub y: f64,
    pub node_id: Option<String>,
}

/// Selection range
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Selection {
    pub node_ids: Vec<String>,
    pub edge_ids: Vec<String>,
}

/// Canvas state store with hybrid storage
pub struct CanvasStateStore {
    redis: Option<Arc<RedisClient>>,
    sqlite_pool: SqlitePool,
}

impl CanvasStateStore {
    /// Create a new canvas state store
    pub fn new(redis: Option<Arc<RedisClient>>, sqlite_pool: SqlitePool) -> Self {
        Self { redis, sqlite_pool }
    }

    /// Get canvas state from Redis (hot) or SQLite (cold fallback)
    pub async fn get_state(&self, canvas_id: &str) -> anyhow::Result<Option<CanvasState>> {
        // Try Redis first
        if let Some(redis) = &self.redis {
            let key = canvas_key(canvas_id);
            if let Ok(Some(json)) = redis.get(&key).await {
                if let Ok(state) = serde_json::from_str::<CanvasState>(&json) {
                    return Ok(Some(state));
                }
            }
        }

        // Fallback to SQLite
        let row = sqlx::query(
            "SELECT canvas_id, name, nodes_json, edges_json, version, folder_id, owner_id, last_updated
             FROM canvases WHERE canvas_id = ?"
        )
        .bind(canvas_id)
        .fetch_optional(&self.sqlite_pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(CanvasState {
                canvas_id: row.get("canvas_id"),
                name: row.get("name"),
                nodes_json: row.get("nodes_json"),
                edges_json: row.get("edges_json"),
                version: row.get("version"),
                folder_id: row.get("folder_id"),
                owner_id: row.get("owner_id"),
                last_updated: row.get("last_updated"),
            }))
        } else {
            Ok(None)
        }
    }

    /// Save canvas state (hot: Redis + cold: SQLite)
    pub async fn save_state(&self, state: &CanvasState) -> anyhow::Result<()> {
        // Save to Redis (hot, 1 hour TTL)
        if let Some(redis) = &self.redis {
            let key = canvas_key(&state.canvas_id);
            let json = serde_json::to_string(state)?;
            redis.set(&key, &json, Some(3600)).await?;
        }

        // Save to SQLite
        sqlx::query(
            "INSERT OR REPLACE INTO canvases (canvas_id, name, nodes_json, edges_json, version, folder_id, owner_id, last_updated)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&state.canvas_id)
        .bind(&state.name)
        .bind(&state.nodes_json)
        .bind(&state.edges_json)
        .bind(state.version)
        .bind(&state.folder_id)
        .bind(&state.owner_id)
        .bind(state.last_updated)
        .execute(&self.sqlite_pool)
        .await?;

        Ok(())
    }

    /// Create a new canvas
    pub async fn create_canvas(&self, name: &str, owner_id: &str, folder_id: Option<String>) -> anyhow::Result<CanvasState> {
        let canvas_id = format!("canvas_{}", Uuid::new_v4());
        let now = Utc::now().timestamp();

        let state = CanvasState {
            canvas_id: canvas_id.clone(),
            name: name.to_string(),
            nodes_json: "[]".to_string(),
            edges_json: "[]".to_string(),
            version: 1,
            folder_id,
            owner_id: owner_id.to_string(),
            last_updated: now,
        };

        self.save_state(&state).await?;

        // Create initial version
        self.create_version(&canvas_id, &state, owner_id, "Initial version").await?;

        Ok(state)
    }

    /// Create a new version snapshot
    pub async fn create_version(
        &self,
        canvas_id: &str,
        state: &CanvasState,
        created_by: &str,
        summary: &str,
    ) -> anyhow::Result<CanvasVersion> {
        let version_id = format!("{}_v{}", canvas_id, state.version);
        let now = Utc::now().timestamp();

        let version = CanvasVersion {
            id: version_id,
            canvas_id: canvas_id.to_string(),
            version: state.version,
            snapshot_json: serde_json::to_string(state)?,
            diff_json: None,
            summary: Some(summary.to_string()),
            created_by: created_by.to_string(),
            created_at: now,
        };

        sqlx::query(
            "INSERT INTO canvas_versions (id, canvas_id, version, snapshot_json, diff_json, summary, created_by, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&version.id)
        .bind(&version.canvas_id)
        .bind(version.version)
        .bind(&version.snapshot_json)
        .bind(&version.diff_json)
        .bind(&version.summary)
        .bind(&version.created_by)
        .bind(version.created_at)
        .execute(&self.sqlite_pool)
        .await?;

        Ok(version)
    }

    /// Get version history
    pub async fn get_versions(&self, canvas_id: &str) -> anyhow::Result<Vec<CanvasVersion>> {
        let rows = sqlx::query(
            "SELECT id, canvas_id, version, snapshot_json, diff_json, summary, created_by, created_at
             FROM canvas_versions WHERE canvas_id = ? ORDER BY version DESC"
        )
        .bind(canvas_id)
        .fetch_all(&self.sqlite_pool)
        .await?;

        let versions = rows.iter().map(|row| CanvasVersion {
            id: row.get("id"),
            canvas_id: row.get("canvas_id"),
            version: row.get("version"),
            snapshot_json: row.get("snapshot_json"),
            diff_json: row.get("diff_json"),
            summary: row.get("summary"),
            created_by: row.get("created_by"),
            created_at: row.get("created_at"),
        }).collect();

        Ok(versions)
    }

    /// Get specific version
    pub async fn get_version(&self, canvas_id: &str, version: i32) -> anyhow::Result<Option<CanvasVersion>> {
        let row = sqlx::query(
            "SELECT id, canvas_id, version, snapshot_json, diff_json, summary, created_by, created_at
             FROM canvas_versions WHERE canvas_id = ? AND version = ?"
        )
        .bind(canvas_id)
        .bind(version)
        .fetch_optional(&self.sqlite_pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(CanvasVersion {
                id: row.get("id"),
                canvas_id: row.get("canvas_id"),
                version: row.get("version"),
                snapshot_json: row.get("snapshot_json"),
                diff_json: row.get("diff_json"),
                summary: row.get("summary"),
                created_by: row.get("created_by"),
                created_at: row.get("created_at"),
            }))
        } else {
            Ok(None)
        }
    }

    /// Rollback to a specific version
    pub async fn rollback_to_version(&self, canvas_id: &str, version: i32) -> anyhow::Result<Option<CanvasState>> {
        let old_version = self.get_version(canvas_id, version).await?;

        if let Some(v) = old_version {
            let state: CanvasState = serde_json::from_str(&v.snapshot_json)?;
            let mut new_state = state;
            new_state.version = v.version + 1;
            new_state.last_updated = Utc::now().timestamp();

            self.save_state(&new_state).await?;
            self.create_version(canvas_id, &new_state, "system", &format!("Rollback to v{}", version)).await?;

            return Ok(Some(new_state));
        }

        Ok(None)
    }

    /// Acquire a lock on canvas
    pub async fn acquire_lock(&self, canvas_id: &str, user_id: &str) -> anyhow::Result<bool> {
        if let Some(redis) = &self.redis {
            let key = canvas_lock_key(canvas_id, user_id);
            let now = Utc::now().timestamp();

            let lock = CanvasLock {
                canvas_id: canvas_id.to_string(),
                user_id: user_id.to_string(),
                locked_at: now,
                expires_at: now + 30,  // 30 seconds TTL
            };

            let json = serde_json::to_string(&lock)?;
            redis.set(&key, &json, Some(30)).await?;

            return Ok(true);
        }

        Ok(false)
    }

    /// Release a lock on canvas
    pub async fn release_lock(&self, canvas_id: &str, user_id: &str) -> anyhow::Result<()> {
        if let Some(redis) = &self.redis {
            let key = canvas_lock_key(canvas_id, user_id);
            let _ = redis.del(&key).await;
        }

        Ok(())
    }

    /// Check if canvas is locked
    pub async fn is_locked(&self, canvas_id: &str) -> anyhow::Result<bool> {
        if let Some(redis) = &self.redis {
            let key = format!("canvas_lock:{}:placeholder", canvas_id);
            return redis.exists(&key).await;
        }

        Ok(false)
    }

    /// Save collaboration state
    pub async fn save_collab_state(&self, state: &CollabState) -> anyhow::Result<()> {
        if let Some(redis) = &self.redis {
            let key = collab_key(&state.canvas_id);
            let json = serde_json::to_string(state)?;
            redis.set(&key, &json, Some(300)).await?;  // 5 min TTL
        }

        Ok(())
    }

    /// Get collaboration state
    pub async fn get_collab_state(&self, canvas_id: &str) -> anyhow::Result<Option<CollabState>> {
        if let Some(redis) = &self.redis {
            let key = collab_key(canvas_id);
            if let Ok(Some(json)) = redis.get(&key).await {
                return Ok(serde_json::from_str(&json)?);
            }
        }

        Ok(None)
    }

    /// Search canvases (uses SQLite FTS)
    pub async fn search_canvases(&self, query: &str, folder_id: Option<String>) -> anyhow::Result<Vec<CanvasState>> {
        let search_pattern = format!("%{}%", query);

        let rows = if let Some(fid) = folder_id {
            sqlx::query(
                "SELECT canvas_id, name, nodes_json, edges_json, version, folder_id, owner_id, last_updated
                 FROM canvases WHERE folder_id = ? AND (name LIKE ? OR nodes_json LIKE ?) ORDER BY last_updated DESC LIMIT 50"
            )
            .bind(&fid)
            .bind(&search_pattern)
            .bind(&search_pattern)
            .fetch_all(&self.sqlite_pool)
            .await?
        } else {
            sqlx::query(
                "SELECT canvas_id, name, nodes_json, edges_json, version, folder_id, owner_id, last_updated
                 FROM canvases WHERE name LIKE ? OR nodes_json LIKE ? ORDER BY last_updated DESC LIMIT 50"
            )
            .bind(&search_pattern)
            .bind(&search_pattern)
            .fetch_all(&self.sqlite_pool)
            .await?
        };

        let canvases = rows.iter().map(|row| CanvasState {
            canvas_id: row.get("canvas_id"),
            name: row.get("name"),
            nodes_json: row.get("nodes_json"),
            edges_json: row.get("edges_json"),
            version: row.get("version"),
            folder_id: row.get("folder_id"),
            owner_id: row.get("owner_id"),
            last_updated: row.get("last_updated"),
        }).collect();

        Ok(canvases)
    }

    /// List canvases by folder
    pub async fn list_by_folder(&self, folder_id: Option<String>) -> anyhow::Result<Vec<CanvasState>> {
        let rows = if let Some(fid) = folder_id {
            sqlx::query(
                "SELECT canvas_id, name, nodes_json, edges_json, version, folder_id, owner_id, last_updated
                 FROM canvases WHERE folder_id = ? ORDER BY last_updated DESC"
            )
            .bind(&fid)
            .fetch_all(&self.sqlite_pool)
            .await?
        } else {
            sqlx::query(
                "SELECT canvas_id, name, nodes_json, edges_json, version, folder_id, owner_id, last_updated
                 FROM canvases WHERE folder_id IS NULL ORDER BY last_updated DESC"
            )
            .fetch_all(&self.sqlite_pool)
            .await?
        };

        let canvases = rows.iter().map(|row| CanvasState {
            canvas_id: row.get("canvas_id"),
            name: row.get("name"),
            nodes_json: row.get("nodes_json"),
            edges_json: row.get("edges_json"),
            version: row.get("version"),
            folder_id: row.get("folder_id"),
            owner_id: row.get("owner_id"),
            last_updated: row.get("last_updated"),
        }).collect();

        Ok(canvases)
    }

    /// Delete canvas
    pub async fn delete_canvas(&self, canvas_id: &str) -> anyhow::Result<()> {
        // Delete from Redis
        if let Some(redis) = &self.redis {
            let key = canvas_key(canvas_id);
            let _ = redis.del(&key).await;
        }

        // Delete from SQLite (versions + canvas)
        sqlx::query("DELETE FROM canvas_versions WHERE canvas_id = ?")
            .bind(canvas_id)
            .execute(&self.sqlite_pool)
            .await?;

        sqlx::query("DELETE FROM canvases WHERE canvas_id = ?")
            .bind(canvas_id)
            .execute(&self.sqlite_pool)
            .await?;

        Ok(())
    }
}

/// Initialize canvas tables
pub async fn init_canvas_tables(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS canvases (
            canvas_id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            nodes_json TEXT NOT NULL DEFAULT '[]',
            edges_json TEXT NOT NULL DEFAULT '[]',
            version INTEGER NOT NULL DEFAULT 1,
            folder_id TEXT,
            owner_id TEXT NOT NULL,
            last_updated INTEGER NOT NULL
        )"
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS canvas_versions (
            id TEXT PRIMARY KEY,
            canvas_id TEXT NOT NULL,
            version INTEGER NOT NULL,
            snapshot_json TEXT NOT NULL,
            diff_json TEXT,
            summary TEXT,
            created_by TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (canvas_id) REFERENCES canvases(canvas_id)
        )"
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS canvas_folders (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            parent_id TEXT,
            owner_id TEXT NOT NULL,
            created_at INTEGER NOT NULL
        )"
    )
    .execute(pool)
    .await?;

    // Create indexes
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_canvas_folder ON canvases(folder_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_canvas_versions ON canvas_versions(canvas_id, version)")
        .execute(pool)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canvas_state_serialization() {
        let state = CanvasState {
            canvas_id: "test_canvas".to_string(),
            name: "Test Canvas".to_string(),
            nodes_json: "[]".to_string(),
            edges_json: "[]".to_string(),
            version: 1,
            folder_id: None,
            owner_id: "user1".to_string(),
            last_updated: Utc::now().timestamp(),
        };

        let json = serde_json::to_string(&state).unwrap();
        let deserialized: CanvasState = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.canvas_id, state.canvas_id);
        assert_eq!(deserialized.name, state.name);
    }
}
