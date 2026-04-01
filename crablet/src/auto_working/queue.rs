//! Work Queue
//!
//! Provides a persistent task queue with support for priorities, delays, and dead letter queues.

use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Row, sqlite::SqlitePool};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::error::Result;
use crate::auto_working::types::*;

/// Queue backend trait
#[async_trait]
pub trait QueueBackend: Send + Sync {
    /// Enqueue a task
    async fn enqueue(&self, task: QueuedTask) -> Result<()>;
    
    /// Dequeue a task (blocking with timeout)
    async fn dequeue(&self, timeout: Duration) -> Result<Option<QueuedTask>>;
    
    /// Acknowledge task completion
    async fn ack(&self, task_id: &str) -> Result<()>;
    
    /// Requeue a task with delay
    async fn requeue(&self, task_id: &str, delay: Duration) -> Result<()>;
    
    /// Get queue length
    async fn len(&self) -> Result<usize>;
    
    /// Check if queue is empty
    async fn is_empty(&self) -> Result<bool>;
    
    /// Move task to dead letter queue
    async fn move_to_dlq(&self, task: &QueuedTask, error: &str) -> Result<()>;
    
    /// Clear the queue
    async fn clear(&self) -> Result<()>;
}

/// SQLite queue backend
pub struct SqliteQueue {
    pub pool: SqlitePool,
    dlq_enabled: bool,
}

#[derive(Debug, Default, Clone)]
pub struct QueueStats {
    pub pending_count: usize,
    pub running_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
}

impl SqliteQueue {
    /// Create a new SQLite queue
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = SqlitePool::connect(database_url).await?;
        Self::init_tables(&pool).await?;
        Ok(Self { 
            pool,
            dlq_enabled: true,
        })
    }
    
    /// Initialize database tables
    async fn init_tables(pool: &SqlitePool) -> Result<()> {
        // Main queue table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS work_queue (
                id TEXT PRIMARY KEY,
                payload_type TEXT NOT NULL,
                payload_data TEXT NOT NULL,
                priority INTEGER DEFAULT 50,
                attempts INTEGER DEFAULT 0,
                max_attempts INTEGER DEFAULT 3,
                queued_at TEXT NOT NULL,
                scheduled_at TEXT NOT NULL,
                visibility_timeout INTEGER,
                processing BOOLEAN DEFAULT 0
            )
            "#
        )
        .execute(pool)
        .await?;
        
        // Dead letter queue table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS dead_letter_queue (
                id TEXT PRIMARY KEY,
                payload_type TEXT NOT NULL,
                payload_data TEXT NOT NULL,
                priority INTEGER DEFAULT 50,
                attempts INTEGER DEFAULT 0,
                max_attempts INTEGER DEFAULT 3,
                queued_at TEXT NOT NULL,
                failed_at TEXT NOT NULL,
                error_message TEXT,
                original_queue TEXT
            )
            "#
        )
        .execute(pool)
        .await?;
        
        // Create indexes
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_work_queue_priority 
            ON work_queue(priority DESC, scheduled_at ASC)
            WHERE processing = 0
            "#
        )
        .execute(pool)
        .await?;
        
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_work_queue_scheduled 
            ON work_queue(scheduled_at)
            WHERE processing = 0
            "#
        )
        .execute(pool)
        .await?;
        
        Ok(())
    }
    
    /// Serialize task payload
    fn serialize_payload(payload: &TaskPayload) -> Result<(String, String)> {
        let payload_data = serde_json::to_string(payload)?;
        Ok((payload.task_type.clone(), payload_data))
    }
    
    /// Deserialize a queued task from database row
    fn deserialize_task(row: &sqlx::sqlite::SqliteRow) -> Result<QueuedTask> {
        let payload_type: String = row.try_get("payload_type")?;
        let payload_data: String = row.try_get("payload_data")?;
        
        let data: serde_json::Value = serde_json::from_str(&payload_data)?;
        let payload = TaskPayload {
            task_type: payload_type,
            data,
        };
        
        let queued_at_str: String = row.try_get("queued_at")?;
        let scheduled_at_str: String = row.try_get("scheduled_at")?;
        let visibility_timeout: Option<i64> = row.try_get("visibility_timeout")?;
        
        Ok(QueuedTask {
            id: row.try_get("id")?,
            payload,
            priority: row.try_get::<i64, _>("priority")? as u8,
            attempts: row.try_get::<i64, _>("attempts")? as u32,
            max_attempts: row.try_get::<i64, _>("max_attempts")? as u32,
            queued_at: DateTime::parse_from_rfc3339(&queued_at_str)?.with_timezone(&Utc),
            scheduled_at: DateTime::parse_from_rfc3339(&scheduled_at_str)?.with_timezone(&Utc),
            visibility_timeout: visibility_timeout.map(|t| Duration::from_secs(t as u64)),
        })
    }

    pub async fn get_stats(&self) -> Result<QueueStats> {
        let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM work_queue WHERE processing = 0").fetch_one(&self.pool).await?;
        let running: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM work_queue WHERE processing = 1").fetch_one(&self.pool).await?;
        let failed: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM dead_letter_queue").fetch_one(&self.pool).await?;
        
        Ok(QueueStats {
            pending_count: pending as usize,
            running_count: running as usize,
            completed_count: 0, // We delete on ack, so we don't track completed here
            failed_count: failed as usize,
        })
    }

    pub async fn list_pending(&self, limit: usize) -> Result<Vec<QueuedTask>> {
        let rows = sqlx::query("SELECT * FROM work_queue WHERE processing = 0 ORDER BY priority DESC, scheduled_at ASC LIMIT ?1")
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await?;
            
        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(Self::deserialize_task(&row)?);
        }
        Ok(tasks)
    }
}

#[async_trait]
impl QueueBackend for SqliteQueue {
    async fn enqueue(&self, task: QueuedTask) -> Result<()> {
        let (payload_type, payload_data) = Self::serialize_payload(&task.payload)?;
        
        sqlx::query(
            r#"
            INSERT INTO work_queue 
            (id, payload_type, payload_data, priority, attempts, max_attempts, queued_at, scheduled_at, visibility_timeout, processing)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0)
            "#
        )
        .bind(&task.id)
        .bind(&payload_type)
        .bind(&payload_data)
        .bind(task.priority as i64)
        .bind(task.attempts as i64)
        .bind(task.max_attempts as i64)
        .bind(task.queued_at.to_rfc3339())
        .bind(task.scheduled_at.to_rfc3339())
        .bind(task.visibility_timeout.map(|d| d.as_secs() as i64))
        .execute(&self.pool)
        .await?;
        
        debug!("Enqueued task: {} (priority: {})", task.id, task.priority);
        Ok(())
    }
    
    async fn dequeue(&self, timeout: Duration) -> Result<Option<QueuedTask>> {
        let start = std::time::Instant::now();
        
        loop {
            let now = Utc::now();
            let now_str = now.to_rfc3339();
            
            // Try to get a task
            let result = sqlx::query(
                r#"
                SELECT * FROM work_queue 
                WHERE processing = 0 
                AND scheduled_at <= ?1
                ORDER BY priority DESC, scheduled_at ASC
                LIMIT 1
                "#
            )
            .bind(&now_str)
            .fetch_optional(&self.pool)
            .await?;
            
            if let Some(row) = result {
                let task = Self::deserialize_task(&row)?;
                
                // Mark as processing
                sqlx::query(
                    r#"
                    UPDATE work_queue 
                    SET processing = 1, attempts = attempts + 1
                    WHERE id = ?1
                    "#
                )
                .bind(&task.id)
                .execute(&self.pool)
                .await?;
                
                debug!("Dequeued task: {} (attempt: {})", task.id, task.attempts + 1);
                return Ok(Some(task));
            }
            
            // Check timeout
            if start.elapsed() >= timeout {
                return Ok(None);
            }
            
            // Wait a bit before retrying
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
    
    async fn ack(&self, task_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM work_queue WHERE id = ?1")
            .bind(task_id)
            .execute(&self.pool)
            .await?;
        
        debug!("Acknowledged task: {}", task_id);
        Ok(())
    }
    
    async fn requeue(&self, task_id: &str, delay: Duration) -> Result<()> {
        let scheduled_at = Utc::now() + chrono::Duration::from_std(delay).unwrap_or(chrono::Duration::seconds(0));
        
        sqlx::query(
            r#"
            UPDATE work_queue 
            SET processing = 0, scheduled_at = ?1
            WHERE id = ?2
            "#
        )
        .bind(scheduled_at.to_rfc3339())
        .bind(task_id)
        .execute(&self.pool)
        .await?;
        
        debug!("Requeued task: {} with delay {:?}", task_id, delay);
        Ok(())
    }
    
    async fn len(&self) -> Result<usize> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM work_queue WHERE processing = 0")
            .fetch_one(&self.pool)
            .await?;
        Ok(count as usize)
    }
    
    async fn is_empty(&self) -> Result<bool> {
        Ok(self.len().await? == 0)
    }
    
    async fn move_to_dlq(&self, task: &QueuedTask, error: &str) -> Result<()> {
        if !self.dlq_enabled {
            // Just delete the task if DLQ is disabled
            self.ack(&task.id).await?;
            return Ok(());
        }
        
        let (payload_type, payload_data) = Self::serialize_payload(&task.payload)?;
        
        sqlx::query(
            r#"
            INSERT INTO dead_letter_queue 
            (id, payload_type, payload_data, priority, attempts, max_attempts, queued_at, failed_at, error_message, original_queue)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'work_queue')
            "#
        )
        .bind(&task.id)
        .bind(&payload_type)
        .bind(&payload_data)
        .bind(task.priority as i64)
        .bind(task.attempts as i64)
        .bind(task.max_attempts as i64)
        .bind(task.queued_at.to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .bind(error)
        .execute(&self.pool)
        .await?;
        
        // Remove from main queue
        self.ack(&task.id).await?;
        
        warn!("Moved task {} to DLQ: {}", task.id, error);
        Ok(())
    }
    
    async fn clear(&self) -> Result<()> {
        sqlx::query("DELETE FROM work_queue").execute(&self.pool).await?;
        info!("Cleared work queue");
        Ok(())
    }
}

/// Work queue with high-level operations
pub struct WorkQueue {
    backend: Arc<dyn QueueBackend>,
    metrics: Arc<RwLock<QueueMetrics>>,
}

/// Queue metrics
#[derive(Debug, Default, Clone)]
pub struct QueueMetrics {
    pub total_enqueued: u64,
    pub total_dequeued: u64,
    pub total_acknowledged: u64,
    pub total_failed: u64,
    pub total_dlq: u64,
}

impl WorkQueue {
    /// Create a new work queue
    pub fn new(backend: Arc<dyn QueueBackend>) -> Self {
        Self {
            backend,
            metrics: Arc::new(RwLock::new(QueueMetrics::default())),
        }
    }
    
    /// Submit a task to the queue
    pub async fn submit(&self, payload: TaskPayload, priority: u8) -> Result<String> {
        let task = QueuedTask::new(payload, priority);
        let task_id = task.id.clone();
        
        self.backend.enqueue(task).await?;
        
        let mut metrics = self.metrics.write().await;
        metrics.total_enqueued += 1;
        drop(metrics);
        
        Ok(task_id)
    }
    
    /// Submit a task with custom scheduling
    pub async fn submit_scheduled(
        &self, 
        payload: TaskPayload, 
        priority: u8, 
        scheduled_at: DateTime<Utc>
    ) -> Result<String> {
        let mut task = QueuedTask::new(payload, priority);
        task.scheduled_at = scheduled_at;
        let task_id = task.id.clone();
        
        self.backend.enqueue(task).await?;
        
        let mut metrics = self.metrics.write().await;
        metrics.total_enqueued += 1;
        drop(metrics);
        
        Ok(task_id)
    }
    
    /// Get a task from the queue
    pub async fn get(&self, timeout: Duration) -> Result<Option<QueuedTask>> {
        let task = self.backend.dequeue(timeout).await?;
        
        if task.is_some() {
            let mut metrics = self.metrics.write().await;
            metrics.total_dequeued += 1;
        }
        
        Ok(task)
    }
    
    /// Mark a task as completed
    pub async fn complete(&self, task_id: &str) -> Result<()> {
        self.backend.ack(task_id).await?;
        
        let mut metrics = self.metrics.write().await;
        metrics.total_acknowledged += 1;
        drop(metrics);
        
        Ok(())
    }
    
    /// Mark a task as failed with retry
    pub async fn fail_with_retry(&self, task: &QueuedTask, error: &str, retry_delay: Duration) -> Result<()> {
        if task.attempts >= task.max_attempts {
            // Max retries reached, move to DLQ
            self.backend.move_to_dlq(task, error).await?;
            
            let mut metrics = self.metrics.write().await;
            metrics.total_dlq += 1;
        } else {
            // Requeue with delay
            self.backend.requeue(&task.id, retry_delay).await?;
            
            let mut metrics = self.metrics.write().await;
            metrics.total_failed += 1;
        }
        
        Ok(())
    }
    
    /// Mark a task as permanently failed (move to DLQ immediately)
    pub async fn fail_permanent(&self, task: &QueuedTask, error: &str) -> Result<()> {
        self.backend.move_to_dlq(task, error).await?;
        
        let mut metrics = self.metrics.write().await;
        metrics.total_dlq += 1;
        drop(metrics);
        
        Ok(())
    }
    
    /// Get queue length
    pub async fn len(&self) -> Result<usize> {
        self.backend.len().await
    }
    
    /// Check if queue is empty
    pub async fn is_empty(&self) -> Result<bool> {
        self.backend.is_empty().await
    }
    
    /// Get current metrics
    pub async fn metrics(&self) -> QueueMetrics {
        let metrics = self.metrics.read().await;
        (*metrics).clone()
    }
    
    /// Clear the queue
    pub async fn clear(&self) -> Result<()> {
        self.backend.clear().await
    }
}

/// Priority queue implementation (in-memory)
pub struct PriorityQueue<T: Ord> {
    items: std::collections::BinaryHeap<T>,
}

impl<T: Ord> PriorityQueue<T> {
    /// Create a new priority queue
    pub fn new() -> Self {
        Self {
            items: std::collections::BinaryHeap::new(),
        }
    }
    
    /// Push an item
    pub fn push(&mut self, item: T) {
        self.items.push(item);
    }
    
    /// Pop the highest priority item
    pub fn pop(&mut self) -> Option<T> {
        self.items.pop()
    }
    
    /// Peek at the highest priority item
    pub fn peek(&self) -> Option<&T> {
        self.items.peek()
    }
    
    /// Get length
    pub fn len(&self) -> usize {
        self.items.len()
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl<T: Ord> Default for PriorityQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_sqlite_queue_basic() {
        let queue = SqliteQueue::new("sqlite::memory:").await.expect("failed to create sqlite queue");
        
        // Enqueue a task
        let task = QueuedTask::new(
            TaskPayload {
                task_type: "test".to_string(),
                data: serde_json::json!({"key": "value"}),
            },
            50
        );
        
        queue.enqueue(task.clone()).await.expect("failed to enqueue task");
        assert_eq!(queue.len().await.expect("failed to get queue len"), 1);
        
        // Dequeue the task
        let dequeued = queue.dequeue(Duration::from_secs(1)).await.expect("failed to dequeue task");
        assert!(dequeued.is_some());
        let dequeued = dequeued.expect("dequeued task should be Some");
        assert_eq!(dequeued.id, task.id);
        assert_eq!(dequeued.priority, task.priority);
        
        // Acknowledge
        queue.ack(&dequeued.id).await.expect("failed to ack task");
        assert_eq!(queue.len().await.expect("failed to get queue len"), 0);
    }
    
    #[tokio::test]
    async fn test_priority_ordering() {
        let queue = SqliteQueue::new("sqlite::memory:").await.expect("failed to create sqlite queue");
        
        // Enqueue tasks with different priorities
        let low_priority = QueuedTask::new(
            TaskPayload { task_type: "low".to_string(), data: serde_json::Value::Null },
            10
        );
        let high_priority = QueuedTask::new(
            TaskPayload { task_type: "high".to_string(), data: serde_json::Value::Null },
            90
        );
        let medium_priority = QueuedTask::new(
            TaskPayload { task_type: "medium".to_string(), data: serde_json::Value::Null },
            50
        );
        
        queue.enqueue(low_priority).await.expect("failed to enqueue low priority task");
        queue.enqueue(high_priority).await.expect("failed to enqueue high priority task");
        queue.enqueue(medium_priority).await.expect("failed to enqueue medium priority task");
        
        // Dequeue should return highest priority first
        let first = queue.dequeue(Duration::from_secs(1)).await.expect("failed to dequeue first task").expect("first dequeued task should be Some");
        assert_eq!(first.priority, 90);
        queue.ack(&first.id).await.expect("failed to ack first task");
        
        let second = queue.dequeue(Duration::from_secs(1)).await.expect("failed to dequeue second task").expect("second dequeued task should be Some");
        assert_eq!(second.priority, 50);
        queue.ack(&second.id).await.expect("failed to ack second task");
        
        let third = queue.dequeue(Duration::from_secs(1)).await.expect("failed to dequeue third task").expect("third dequeued task should be Some");
        assert_eq!(third.priority, 10);
    }
    
    #[tokio::test]
    async fn test_scheduled_task() {
        let queue = SqliteQueue::new("sqlite::memory:").await.expect("failed to create sqlite queue");
        
        // Create a future task
        let future_time = Utc::now() + chrono::Duration::seconds(2);
        let mut future_task = QueuedTask::new(
            TaskPayload { task_type: "future".to_string(), data: serde_json::Value::Null },
            50
        );
        future_task.scheduled_at = future_time;
        
        queue.enqueue(future_task).await.expect("failed to enqueue future task");
        
        // Should not get the task immediately
        let immediate = queue.dequeue(Duration::from_millis(100)).await.expect("failed to dequeue (should be none)");
        assert!(immediate.is_none());
        
        // Wait and try again
        tokio::time::sleep(Duration::from_secs(3)).await;
        let later = queue.dequeue(Duration::from_secs(1)).await.expect("failed to dequeue later task");
        assert!(later.is_some());
    }
    
    #[tokio::test]
    async fn test_dead_letter_queue() {
        let queue = SqliteQueue::new("sqlite::memory:").await.expect("failed to create sqlite queue");
        
        let task = QueuedTask::new(
            TaskPayload { task_type: "failing".to_string(), data: serde_json::Value::Null },
            50
        );
        
        queue.enqueue(task.clone()).await.expect("failed to enqueue task for dlq test");
        
        // Move to DLQ
        queue.move_to_dlq(&task, "Test error").await.expect("failed to move task to dlq");
        
        // Task should be removed from main queue
        assert_eq!(queue.len().await.expect("failed to get queue len"), 0);
    }
}
