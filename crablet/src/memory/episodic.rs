use sqlx::{sqlite::SqlitePool, Row};
use anyhow::Result;
use crate::types::Message;
use uuid::Uuid;
use chrono::Utc;

pub struct EpisodicMemory {
    pool: SqlitePool,
}

impl EpisodicMemory {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect(database_url).await?;
        
        // Ensure tables exist
        let schema = r#"
        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            channel TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            last_active INTEGER NOT NULL,
            message_count INTEGER DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            timestamp INTEGER NOT NULL,
            tokens INTEGER,
            latency_ms INTEGER,
            FOREIGN KEY (session_id) REFERENCES sessions(id)
        );
        "#;
        
        sqlx::query(schema).execute(&pool).await?;
        
        Ok(Self { pool })
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
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        
        // Ensure session exists (auto-create for fallback)
        sqlx::query("INSERT OR IGNORE INTO sessions (id, user_id, channel, created_at, last_active) VALUES (?, ?, ?, ?, ?)")
            .bind(session_id)
            .bind("user")
            .bind("unknown")
            .bind(now)
            .bind(now)
            .execute(&self.pool)
            .await?;

        sqlx::query("INSERT INTO messages (id, session_id, role, content, timestamp) VALUES (?, ?, ?, ?, ?)")
            .bind(id)
            .bind(session_id)
            .bind(role)
            .bind(content)
            .bind(now)
            .execute(&self.pool)
            .await?;
            
        Ok(())
    }

    pub async fn get_history(&self, session_id: &str, limit: i64) -> Result<Vec<Message>> {
        let rows = sqlx::query("SELECT role, content FROM messages WHERE session_id = ? ORDER BY timestamp DESC LIMIT ?")
            .bind(session_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;
            
        let mut messages: Vec<Message> = rows.into_iter().map(|row| {
            let content: String = row.get("content");
            Message::new(&row.get::<String, _>("role"), &content)
        }).collect();
        
        // Reverse to get chronological order
        messages.reverse();
        
        Ok(messages)
    }

    pub async fn get_context(&self, session_id: &str, limit: i64) -> Result<Vec<Message>> {
        self.get_history(session_id, limit).await
    }
}
