//! Trace Storage
//!
//! Storage backends for execution traces.

use super::{TraceSession, AgentSpan, ExecutionRecording};
use async_trait::async_trait;
use anyhow::Result;
use std::collections::HashMap;

use tokio::sync::RwLock;

/// Storage trait for trace persistence
#[async_trait]
pub trait TraceStorage: Send + Sync {
    /// Store a trace session
    async fn store_session(&self, session: &TraceSession) -> Result<()>;
    
    /// Retrieve a trace session
    async fn get_session(&self, execution_id: &str) -> Result<Option<TraceSession>>;
    
    /// Store spans for a session
    async fn store_spans(&self, execution_id: &str, spans: &[AgentSpan]) -> Result<()>;
    
    /// Retrieve spans for a session
    async fn get_spans(&self, execution_id: &str) -> Result<Vec<AgentSpan>>;
    
    /// Store a complete recording
    async fn store_recording(&self, recording: &ExecutionRecording) -> Result<()>;
    
    /// Retrieve a complete recording
    async fn get_recording(&self, execution_id: &str) -> Result<Option<ExecutionRecording>>;
    
    /// List all sessions
    async fn list_sessions(&self) -> Result<Vec<TraceSession>>;
    
    /// Delete a session and its spans
    async fn delete_session(&self, execution_id: &str) -> Result<()>;
}

/// In-memory storage for development and testing
pub struct InMemoryStorage {
    sessions: RwLock<HashMap<String, TraceSession>>,
    spans: RwLock<HashMap<String, Vec<AgentSpan>>>,
    recordings: RwLock<HashMap<String, ExecutionRecording>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            spans: RwLock::new(HashMap::new()),
            recordings: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl TraceStorage for InMemoryStorage {
    async fn store_session(&self, session: &TraceSession) -> Result<()> {
        self.sessions.write().await.insert(session.execution_id.clone(), session.clone());
        Ok(())
    }

    async fn get_session(&self, execution_id: &str) -> Result<Option<TraceSession>> {
        Ok(self.sessions.read().await.get(execution_id).cloned())
    }

    async fn store_spans(&self, execution_id: &str, spans: &[AgentSpan]) -> Result<()> {
        self.spans.write().await.insert(execution_id.to_string(), spans.to_vec());
        Ok(())
    }

    async fn get_spans(&self, execution_id: &str) -> Result<Vec<AgentSpan>> {
        Ok(self.spans.read().await.get(execution_id).cloned().unwrap_or_default())
    }

    async fn store_recording(&self, recording: &ExecutionRecording) -> Result<()> {
        self.recordings.write().await.insert(recording.session.execution_id.clone(), recording.clone());
        Ok(())
    }

    async fn get_recording(&self, execution_id: &str) -> Result<Option<ExecutionRecording>> {
        Ok(self.recordings.read().await.get(execution_id).cloned())
    }

    async fn list_sessions(&self) -> Result<Vec<TraceSession>> {
        Ok(self.sessions.read().await.values().cloned().collect())
    }

    async fn delete_session(&self, execution_id: &str) -> Result<()> {
        self.sessions.write().await.remove(execution_id);
        self.spans.write().await.remove(execution_id);
        self.recordings.write().await.remove(execution_id);
        Ok(())
    }
}

/// Persistent storage using SQLite
pub struct PersistentStorage {
    pool: sqlx::SqlitePool,
}

impl PersistentStorage {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = sqlx::sqlite::SqlitePool::connect(database_url).await?;
        
        // Initialize schema
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS trace_sessions (
                execution_id TEXT PRIMARY KEY,
                workflow_id TEXT NOT NULL,
                started_at INTEGER NOT NULL,
                ended_at INTEGER,
                status TEXT NOT NULL,
                metadata TEXT
            )
            "#
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS trace_spans (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                execution_id TEXT NOT NULL,
                span_index INTEGER NOT NULL,
                span_type TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                FOREIGN KEY (execution_id) REFERENCES trace_sessions(execution_id)
            )
            "#
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }
}

#[async_trait]
impl TraceStorage for PersistentStorage {
    async fn store_session(&self, session: &TraceSession) -> Result<()> {
        let metadata = serde_json::to_string(&session.metadata)?;
        
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO trace_sessions 
            (execution_id, workflow_id, started_at, ended_at, status, metadata)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#
        )
        .bind(&session.execution_id)
        .bind(&session.workflow_id)
        .bind(session.started_at as i64)
        .bind(session.ended_at.map(|t| t as i64))
        .bind(serde_json::to_string(&session.status)?)
        .bind(metadata)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    async fn get_session(&self, execution_id: &str) -> Result<Option<TraceSession>> {
        let row = sqlx::query_as::<_, SessionRow>(
            "SELECT * FROM trace_sessions WHERE execution_id = ?1"
        )
        .bind(execution_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    async fn store_spans(&self, execution_id: &str, spans: &[AgentSpan]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        
        for (index, span) in spans.iter().enumerate() {
            let (span_type, content) = serialize_span(span);
            let timestamp = get_span_timestamp(span);
            
            sqlx::query(
                r#"
                INSERT INTO trace_spans 
                (execution_id, span_index, span_type, content, timestamp)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#
            )
            .bind(execution_id)
            .bind(index as i64)
            .bind(span_type)
            .bind(content)
            .bind(timestamp as i64)
            .execute(&mut *tx)
            .await?;
        }
        
        tx.commit().await?;
        Ok(())
    }

    async fn get_spans(&self, execution_id: &str) -> Result<Vec<AgentSpan>> {
        let rows = sqlx::query_as::<_, SpanRow>(
            "SELECT * FROM trace_spans WHERE execution_id = ?1 ORDER BY span_index"
        )
        .bind(execution_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|r| deserialize_span(&r.span_type, &r.content, r.timestamp as u64))
            .collect()
    }

    async fn store_recording(&self, recording: &ExecutionRecording) -> Result<()> {
        // Store session
        self.store_session(&recording.session).await?;
        
        // Store spans
        self.store_spans(&recording.session.execution_id, &recording.spans).await?;
        
        Ok(())
    }

    async fn get_recording(&self, execution_id: &str) -> Result<Option<ExecutionRecording>> {
        let session = self.get_session(execution_id).await?;
        let spans = self.get_spans(execution_id).await?;
        
        Ok(session.map(|s| ExecutionRecording {
            session: s,
            spans,
            checkpoints: Vec::new(),
            metadata: super::RecordingMetadata {
                total_steps: 0,
                total_duration_ms: 0,
                tool_calls: 0,
                errors: 0,
                final_output: None,
            },
        }))
    }

    async fn list_sessions(&self) -> Result<Vec<TraceSession>> {
        let rows = sqlx::query_as::<_, SessionRow>(
            "SELECT * FROM trace_sessions ORDER BY started_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    async fn delete_session(&self, execution_id: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        
        sqlx::query("DELETE FROM trace_spans WHERE execution_id = ?1")
            .bind(execution_id)
            .execute(&mut *tx)
            .await?;
        
        sqlx::query("DELETE FROM trace_sessions WHERE execution_id = ?1")
            .bind(execution_id)
            .execute(&mut *tx)
            .await?;
        
        tx.commit().await?;
        Ok(())
    }
}

// Helper structs for database operations
#[derive(sqlx::FromRow)]
struct SessionRow {
    execution_id: String,
    workflow_id: String,
    started_at: i64,
    ended_at: Option<i64>,
    status: String,
    metadata: String,
}

impl From<SessionRow> for TraceSession {
    fn from(row: SessionRow) -> Self {
        Self {
            execution_id: row.execution_id,
            workflow_id: row.workflow_id,
            started_at: row.started_at as u64,
            ended_at: row.ended_at.map(|t| t as u64),
            status: serde_json::from_str(&row.status).unwrap_or(super::SessionStatus::Running),
            metadata: serde_json::from_str(&row.metadata).unwrap_or_default(),
            spans: Vec::new(),
            metrics: super::ExecutionMetrics::default(),
        }
    }
}

#[derive(sqlx::FromRow)]
struct SpanRow {
    execution_id: String,
    span_index: i64,
    span_type: String,
    content: String,
    timestamp: i64,
}

fn serialize_span(span: &AgentSpan) -> (String, String) {
    let span_type = match span {
        AgentSpan::Thought { .. } => "thought",
        AgentSpan::Action { .. } => "action",
        AgentSpan::Observation { .. } => "observation",
        AgentSpan::Reflection { .. } => "reflection",
        AgentSpan::Decision { .. } => "decision",
        AgentSpan::LoopDetected { .. } => "loop_detected",
        AgentSpan::Error { .. } => "error",
    };

    let content = serde_json::to_string(span).unwrap_or_default();
    (span_type.to_string(), content)
}

fn deserialize_span(_span_type: &str, content: &str, timestamp: u64) -> Result<AgentSpan> {
    // For simplicity, just deserialize the full JSON
    // In production, you might want more controlled deserialization
    let mut span: AgentSpan = serde_json::from_str(content)?;
    
    // Ensure timestamp is correct
    match &mut span {
        AgentSpan::Thought { timestamp: t, .. } => *t = timestamp,
        AgentSpan::Action { timestamp: t, .. } => *t = timestamp,
        AgentSpan::Observation { timestamp: t, .. } => *t = timestamp,
        AgentSpan::Reflection { timestamp: t, .. } => *t = timestamp,
        AgentSpan::Decision { timestamp: t, .. } => *t = timestamp,
        AgentSpan::LoopDetected { timestamp: t, .. } => *t = timestamp,
        AgentSpan::Error { timestamp: t, .. } => *t = timestamp,
    }
    
    Ok(span)
}

fn get_span_timestamp(span: &AgentSpan) -> u64 {
    match span {
        AgentSpan::Thought { timestamp, .. } => *timestamp,
        AgentSpan::Action { timestamp, .. } => *timestamp,
        AgentSpan::Observation { timestamp, .. } => *timestamp,
        AgentSpan::Reflection { timestamp, .. } => *timestamp,
        AgentSpan::Decision { timestamp, .. } => *timestamp,
        AgentSpan::LoopDetected { timestamp, .. } => *timestamp,
        AgentSpan::Error { timestamp, .. } => *timestamp,
    }
}
