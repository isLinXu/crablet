use sqlx::{sqlite::SqlitePool, Row};
use crate::error::Result;
use crate::types::Message;
use uuid::Uuid;
use chrono::Utc;

pub struct EpisodicMemory {
    pub pool: SqlitePool,
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
        
        // Run migrations
        // sqlx::migrate!("./migrations") can return sqlx::migrate::MigrateError
        // which might not be sqlx::Error directly?
        // Let's check.
        // migrate! returns Migrator. run returns Result<(), MigrateError>.
        // MigrateError implements Error.
        // CrabletError has Other(anyhow::Error).
        // So `?` works via anyhow conversion?
        // No, `?` tries `From`.
        // If MigrateError impl From<MigrateError> for CrabletError? No.
        // So `?` on migrate might fail if not mapped.
        // Let's wrap it.
        sqlx::migrate!("./migrations").run(&pool).await.map_err(|e| crate::error::CrabletError::Database(sqlx::Error::Migrate(Box::new(e))))?;
        
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
        self.save_message_transactional(session_id, role, content).await
    }

    pub async fn save_message_transactional(&self, session_id: &str, role: &str, content: &str) -> Result<()> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        
        let mut tx = self.pool.begin().await?;

        // Ensure session exists (auto-create for fallback)
        sqlx::query("INSERT OR IGNORE INTO sessions (id, user_id, channel, created_at, last_active) VALUES (?, ?, ?, ?, ?)")
            .bind(session_id)
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
            
        tx.commit().await?;
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
            Message::new(row.get::<String, _>("role"), &content)
        }).collect();
        
        // Reverse to get chronological order
        messages.reverse();
        
        Ok(messages)
    }

    pub async fn get_context(&self, session_id: &str, limit: i64) -> Result<Vec<Message>> {
        self.get_history(session_id, limit).await
    }
}
