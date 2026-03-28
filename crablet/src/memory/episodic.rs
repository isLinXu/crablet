use sqlx::{sqlite::SqlitePool, Row};
use crate::error::Result;
use crate::types::Message;
use uuid::Uuid;
use chrono::Utc;

use tokio::sync::mpsc;

pub struct EpisodicMemory {
    pub pool: SqlitePool,
    write_tx: mpsc::Sender<WriteTask>,
}

enum WriteTask {
    SaveMessage {
        session_id: String,
        role: String,
        content: String,
    },
}

impl EpisodicMemory {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(10)
            .min_connections(2)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .idle_timeout(std::time::Duration::from_secs(300))
            .after_connect(|conn, _meta| Box::pin(async move {
                sqlx::query("PRAGMA journal_mode=WAL").execute(&mut *conn).await?;
                sqlx::query("PRAGMA synchronous=NORMAL").execute(&mut *conn).await?;
                sqlx::query("PRAGMA cache_size=-64000").execute(&mut *conn).await?; // 64MB cache
                Ok(())
            }))
            .connect(database_url).await?;
        
        let migrations_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
        let migrator = sqlx::migrate::Migrator::new(migrations_dir.as_path())
            .await
            .map_err(|e| crate::error::CrabletError::Other(e.into()))?;
        migrator
            .run(&pool)
            .await
            .map_err(|e| crate::error::CrabletError::Other(e.into()))?;
        
        // Background write loop
        let (write_tx, mut write_rx) = mpsc::channel(100);
        let pool_clone = pool.clone();
        
        tokio::spawn(async move {
            while let Some(task) = write_rx.recv().await {
                match task {
                    WriteTask::SaveMessage { session_id, role, content } => {
                        // Batch multiple messages if they are in the queue
                        let mut tasks = vec![WriteTask::SaveMessage { session_id, role, content }];
                        while let Ok(next) = write_rx.try_recv() {
                            tasks.push(next);
                            if tasks.len() >= 20 { break; }
                        }
                        
                        // Execute batch write in one transaction
                        if let Err(e) = Self::save_messages_batch(&pool_clone, tasks).await {
                            tracing::error!("Failed to save messages batch to SQLite: {}", e);
                        }
                    }
                }
            }
        });
        
        Ok(Self { pool, write_tx })
    }

    async fn save_messages_batch(pool: &SqlitePool, tasks: Vec<WriteTask>) -> Result<()> {
        let mut tx = pool.begin().await?;
        let now = Utc::now().timestamp();
        
        for task in tasks {
            let WriteTask::SaveMessage { session_id, role, content } = task;
            let id = Uuid::new_v4().to_string();
                
            // Ensure session exists
            sqlx::query("INSERT OR IGNORE INTO sessions (id, user_id, channel, created_at, last_active) VALUES (?, ?, ?, ?, ?)")
                .bind(&session_id)
                .bind("user")
                .bind("unknown")
                .bind(now)
                .bind(now)
                .execute(&mut *tx)
                .await?;

            sqlx::query("INSERT INTO messages (id, session_id, role, content, timestamp) VALUES (?, ?, ?, ?, ?)")
                .bind(id)
                .bind(session_id)
                .bind(role)
                .bind(content)
                .bind(now)
                .execute(&mut *tx)
                .await?;
        }
        
        tx.commit().await?;
        Ok(())
    }

    pub async fn create_session(&self, user_id: &str, channel: &str) -> Result<String> {
        let session_id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        
        sqlx::query("INSERT INTO sessions (id, user_id, channel, created_at, last_active, message_count) VALUES (?, ?, ?, ?, ?, 0)")
            .bind(&session_id)
            .bind(user_id)
            .bind(channel)
            .bind(now)
            .bind(now)
            .execute(&self.pool)
            .await?;
            
        Ok(session_id)
    }

    pub async fn save_message(&self, session_id: &str, role: &str, content: &str) -> Result<()> {
        self.write_tx.send(WriteTask::SaveMessage { 
            session_id: session_id.to_string(), 
            role: role.to_string(), 
            content: content.to_string() 
        }).await.map_err(|e| anyhow::anyhow!("Failed to queue write task: {}", e))?;
        Ok(())
    }

    pub async fn save_message_transactional(&self, session_id: &str, role: &str, content: &str) -> Result<()> {
        // Immediate write if transactional is strictly required (e.g., for tests)
        Self::save_messages_batch(&self.pool, vec![WriteTask::SaveMessage { 
            session_id: session_id.to_string(), 
            role: role.to_string(), 
            content: content.to_string() 
        }]).await
    }

    pub async fn get_history(&self, session_id: &str, limit: i64) -> Result<Vec<Message>> {
        let rows = sqlx::query("SELECT role, content FROM messages WHERE session_id = ? ORDER BY timestamp DESC LIMIT ?")
            .bind(session_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;
            
        let mut messages: Vec<Message> = rows.into_iter().map(|row| {
            let content: String = row.get("content");
            Message::new(row.get::<String, _>("role"), &content)
        }).collect();
        
        // Reverse to get chronological order
        messages.reverse();
        
        Ok(messages)
    }

    pub async fn get_context(&self, session_id: &str, limit: i64) -> Result<Vec<Message>> {
        self.get_history(session_id, limit).await
    }

    /// Search messages across all sessions for content matching the query
    /// Returns tuples of (session_id, role, content, timestamp)
    pub async fn search_messages(
        &self,
        query: &str,
        limit: usize,
        page: usize,
    ) -> Result<Vec<(String, String, String, chrono::DateTime<chrono::Utc>)>> {
        let offset = page * limit;
        let search_pattern = format!("%{}%", query);
        
        let rows = sqlx::query(
            "SELECT session_id, role, content, timestamp 
             FROM messages 
             WHERE content LIKE ? 
             ORDER BY timestamp DESC 
             LIMIT ? OFFSET ?"
        )
        .bind(&search_pattern)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await?;

        let results: Vec<(String, String, String, chrono::DateTime<chrono::Utc>)> = rows
            .into_iter()
            .filter_map(|row| {
                let timestamp_i64: i64 = row.get("timestamp");
                let timestamp = chrono::DateTime::from_timestamp(timestamp_i64, 0)?;
                Some((
                    row.get("session_id"),
                    row.get("role"),
                    row.get("content"),
                    timestamp,
                ))
            })
            .collect();

        Ok(results)
    }
}
