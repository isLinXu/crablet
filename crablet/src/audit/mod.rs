use sqlx::{SqlitePool, Row};
use crate::events::AgentEvent;
use chrono::Utc;
use uuid::Uuid;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AuditEventType {
    SwarmActivity,
    SkillExecution,
    MemoryConsolidation,
    SecurityAlert,
    SystemMaintenance,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuditEvent {
    pub event_id: String,
    pub event_type: AuditEventType,
    pub timestamp: i64,
    pub user_id: Option<String>,
    pub details: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
pub struct AuditLog {
    pub id: String,
    pub task_id: String,
    pub from_agent: String,
    pub to_agent: String,
    pub message_type: String,
    pub content: String,
    pub timestamp: i64,
}

#[derive(Clone)]
pub struct AuditLogger {
    pool: SqlitePool,
}

impl AuditLogger {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn log_event(&self, event: &AuditEvent) -> anyhow::Result<()> {
        let details = serde_json::to_string(&event.details)?;
        let event_type = format!("{:?}", event.event_type);
        
        sqlx::query("INSERT INTO audit_events (id, event_type, timestamp, user_id, details) VALUES (?, ?, ?, ?, ?)")
            .bind(&event.event_id)
            .bind(event_type)
            .bind(event.timestamp)
            .bind(&event.user_id)
            .bind(details)
            .execute(&self.pool)
            .await?;
            
        Ok(())
    }

    pub async fn log_swarm_activity(&self, event: &AgentEvent) -> anyhow::Result<()> {
        if let AgentEvent::SwarmActivity { task_id, from, to, message_type, content, .. } = event {
            let id = Uuid::new_v4().to_string();
            let now = Utc::now().timestamp();
            
            sqlx::query("INSERT INTO swarm_logs (id, task_id, from_agent, to_agent, message_type, content, timestamp) VALUES (?, ?, ?, ?, ?, ?, ?)")
                .bind(id)
                .bind(task_id)
                .bind(from)
                .bind(to)
                .bind(message_type)
                .bind(content)
                .bind(now)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    pub async fn cleanup_logs(&self, retention_days: i64) -> anyhow::Result<u64> {
        let cutoff = Utc::now().timestamp() - (retention_days * 24 * 3600);
        let result = sqlx::query("DELETE FROM swarm_logs WHERE timestamp < ?")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;
            
        Ok(result.rows_affected())
    }
    
    pub async fn list_logs(&self, limit: i64, offset: i64) -> anyhow::Result<Vec<AuditLog>> {
        let rows = sqlx::query("SELECT * FROM swarm_logs ORDER BY timestamp DESC LIMIT ? OFFSET ?")
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
            
        let logs = rows.iter().map(|row| AuditLog {
            id: row.get("id"),
            task_id: row.get("task_id"),
            from_agent: row.get("from_agent"),
            to_agent: row.get("to_agent"),
            message_type: row.get("message_type"),
            content: row.get("content"),
            timestamp: row.get("timestamp"),
        }).collect();
        
        Ok(logs)
    }
}

pub fn start_audit_worker(pool: SqlitePool, event_bus: Arc<crate::events::EventBus>) {
    let logger = AuditLogger::new(pool.clone());
    let logger_clone = logger.clone();
    
    // Task 1: Event Listener
    let mut rx = event_bus.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if let AgentEvent::SwarmActivity { .. } = &event.payload {
                if let Err(e) = logger.log_swarm_activity(&event.payload).await {
                    tracing::error!("Failed to audit swarm activity: {}", e);
                }
            }
        }
    });
    
    // Task 2: Log Rotation (Cleanup every 24 hours)
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(86400)); // 24h
        loop {
            interval.tick().await;
            match logger_clone.cleanup_logs(7).await { // Keep 7 days
                Ok(count) => tracing::info!("Cleaned up {} old audit logs", count),
                Err(e) => tracing::error!("Failed to cleanup logs: {}", e),
            }
        }
    });
}

