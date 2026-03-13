//! Task Scheduler
//!
//! Manages scheduled tasks with support for cron expressions, periodic execution,
//! and event-based triggers.

use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Row, sqlite::SqlitePool};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::error::Result;
use crate::auto_working::types::*;
use crate::auto_working::worker::TaskExecutor;

/// Task storage trait
#[async_trait]
pub trait TaskStore: Send + Sync {
    /// Save a task
    async fn save(&self, task: &ScheduledTask) -> Result<()>;
    
    /// Get a task by ID
    async fn get(&self, task_id: &str) -> Result<Option<ScheduledTask>>;
    
    /// List all tasks
    async fn list(&self) -> Result<Vec<ScheduledTask>>;
    
    /// List enabled tasks
    async fn list_enabled(&self) -> Result<Vec<ScheduledTask>>;
    
    /// Get tasks that are due for execution
    async fn get_due_tasks(&self, before: DateTime<Utc>) -> Result<Vec<ScheduledTask>>;
    
    /// Update next run time
    async fn update_next_run(&self, task_id: &str, next_run: Option<DateTime<Utc>>) -> Result<()>;
    
    /// Update last run time and run count
    async fn update_last_run(&self, task_id: &str, last_run: DateTime<Utc>, run_count: u32) -> Result<()>;
    
    /// Update retry count
    async fn update_retry_count(&self, task_id: &str, retry_count: u32) -> Result<()>;
    
    /// Delete a task
    async fn delete(&self, task_id: &str) -> Result<()>;
    
    /// Enable/disable a task
    async fn set_enabled(&self, task_id: &str, enabled: bool) -> Result<()>;
}

/// SQLite implementation of task storage
pub struct SqliteTaskStore {
    pool: SqlitePool,
}

impl SqliteTaskStore {
    /// Create a new SQLite task store
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = SqlitePool::connect(database_url).await?;
        Self::init_tables(&pool).await?;
        Ok(Self { pool })
    }
    
    /// Initialize database tables
    async fn init_tables(pool: &SqlitePool) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS scheduled_tasks (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                schedule_type TEXT NOT NULL,
                schedule_data TEXT NOT NULL,
                action_type TEXT NOT NULL,
                action_data TEXT NOT NULL,
                priority INTEGER DEFAULT 50,
                enabled BOOLEAN DEFAULT 1,
                created_at TEXT NOT NULL,
                next_run_at TEXT,
                last_run_at TEXT,
                run_count INTEGER DEFAULT 0,
                max_retries INTEGER DEFAULT 3,
                retry_count INTEGER DEFAULT 0
            )
            "#
        )
        .execute(pool)
        .await?;
        
        // Create index for efficient due task queries
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_scheduled_tasks_next_run 
            ON scheduled_tasks(next_run_at) 
            WHERE enabled = 1
            "#
        )
        .execute(pool)
        .await?;
        
        Ok(())
    }
    
    /// Serialize schedule type
    fn serialize_schedule(schedule: &ScheduleType) -> Result<(String, String)> {
        let schedule_type = match schedule {
            ScheduleType::Once { .. } => "once",
            ScheduleType::Periodic { .. } => "periodic",
            ScheduleType::Cron { .. } => "cron",
            ScheduleType::EventTriggered { .. } => "event",
        };
        
        let schedule_data = serde_json::to_string(schedule)?;
        Ok((schedule_type.to_string(), schedule_data))
    }
    
    /// Serialize action type
    fn serialize_action(action: &TaskAction) -> Result<(String, String)> {
        let action_type = match action {
            TaskAction::Cognitive { .. } => "cognitive",
            TaskAction::SystemCommand { .. } => "command",
            TaskAction::ApiCall { .. } => "api",
            TaskAction::Rpa { .. } => "rpa",
            TaskAction::Workflow { .. } => "workflow",
            TaskAction::Composite { .. } => "composite",
        };
        
        let action_data = serde_json::to_string(action)?;
        Ok((action_type.to_string(), action_data))
    }
    
    /// Deserialize a task from database row
    fn deserialize_task(row: &sqlx::sqlite::SqliteRow) -> Result<ScheduledTask> {
        let schedule_data: String = row.try_get("schedule_data")?;
        let action_data: String = row.try_get("action_data")?;
        
        let schedule: ScheduleType = serde_json::from_str(&schedule_data)?;
        let action: TaskAction = serde_json::from_str(&action_data)?;
        
        let created_at_str: String = row.try_get("created_at")?;
        let next_run_at_str: Option<String> = row.try_get("next_run_at")?;
        let last_run_at_str: Option<String> = row.try_get("last_run_at")?;
        
        Ok(ScheduledTask {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            description: row.try_get("description")?,
            schedule,
            action,
            priority: row.try_get::<i64, _>("priority")? as u8,
            enabled: row.try_get::<i64, _>("enabled")? != 0,
            created_at: DateTime::parse_from_rfc3339(&created_at_str)?.with_timezone(&Utc),
            next_run_at: next_run_at_str.map(|s| DateTime::parse_from_rfc3339(&s).ok()).flatten().map(|d| d.with_timezone(&Utc)),
            last_run_at: last_run_at_str.map(|s| DateTime::parse_from_rfc3339(&s).ok()).flatten().map(|d| d.with_timezone(&Utc)),
            run_count: row.try_get::<i64, _>("run_count")? as u32,
            max_retries: row.try_get::<i64, _>("max_retries")? as u32,
            retry_count: row.try_get::<i64, _>("retry_count")? as u32,
        })
    }
}

#[async_trait]
impl TaskStore for SqliteTaskStore {
    async fn save(&self, task: &ScheduledTask) -> Result<()> {
        let (schedule_type, schedule_data) = Self::serialize_schedule(&task.schedule)?;
        let (action_type, action_data) = Self::serialize_action(&task.action)?;
        
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO scheduled_tasks 
            (id, name, description, schedule_type, schedule_data, action_type, action_data,
             priority, enabled, created_at, next_run_at, last_run_at, run_count, max_retries, retry_count)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            "#
        )
        .bind(&task.id)
        .bind(&task.name)
        .bind(&task.description)
        .bind(&schedule_type)
        .bind(&schedule_data)
        .bind(&action_type)
        .bind(&action_data)
        .bind(task.priority as i64)
        .bind(task.enabled)
        .bind(task.created_at.to_rfc3339())
        .bind(task.next_run_at.map(|d| d.to_rfc3339()))
        .bind(task.last_run_at.map(|d| d.to_rfc3339()))
        .bind(task.run_count as i64)
        .bind(task.max_retries as i64)
        .bind(task.retry_count as i64)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn get(&self, task_id: &str) -> Result<Option<ScheduledTask>> {
        let row = sqlx::query("SELECT * FROM scheduled_tasks WHERE id = ?1")
            .bind(task_id)
            .fetch_optional(&self.pool)
            .await?;
        
        match row {
            Some(row) => Ok(Some(Self::deserialize_task(&row)?)),
            None => Ok(None),
        }
    }
    
    async fn list(&self) -> Result<Vec<ScheduledTask>> {
        let rows = sqlx::query("SELECT * FROM scheduled_tasks ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await?;
        
        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(Self::deserialize_task(&row)?);
        }
        
        Ok(tasks)
    }
    
    async fn list_enabled(&self) -> Result<Vec<ScheduledTask>> {
        let rows = sqlx::query("SELECT * FROM scheduled_tasks WHERE enabled = 1 ORDER BY priority DESC, created_at DESC")
            .fetch_all(&self.pool)
            .await?;
        
        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(Self::deserialize_task(&row)?);
        }
        
        Ok(tasks)
    }
    
    async fn get_due_tasks(&self, before: DateTime<Utc>) -> Result<Vec<ScheduledTask>> {
        let before_str = before.to_rfc3339();
        
        let rows = sqlx::query(
            r#"
            SELECT * FROM scheduled_tasks 
            WHERE enabled = 1 
            AND next_run_at IS NOT NULL 
            AND next_run_at <= ?1
            ORDER BY priority DESC, next_run_at ASC
            "#
        )
        .bind(&before_str)
        .fetch_all(&self.pool)
        .await?;
        
        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(Self::deserialize_task(&row)?);
        }
        
        Ok(tasks)
    }
    
    async fn update_next_run(&self, task_id: &str, next_run: Option<DateTime<Utc>>) -> Result<()> {
        sqlx::query("UPDATE scheduled_tasks SET next_run_at = ?1 WHERE id = ?2")
            .bind(next_run.map(|d| d.to_rfc3339()))
            .bind(task_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    
    async fn update_last_run(&self, task_id: &str, last_run: DateTime<Utc>, run_count: u32) -> Result<()> {
        sqlx::query("UPDATE scheduled_tasks SET last_run_at = ?1, run_count = ?2, retry_count = 0 WHERE id = ?3")
            .bind(last_run.to_rfc3339())
            .bind(run_count as i64)
            .bind(task_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    
    async fn update_retry_count(&self, task_id: &str, retry_count: u32) -> Result<()> {
        sqlx::query("UPDATE scheduled_tasks SET retry_count = ?1 WHERE id = ?2")
            .bind(retry_count as i64)
            .bind(task_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    
    async fn delete(&self, task_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM scheduled_tasks WHERE id = ?1")
            .bind(task_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    
    async fn set_enabled(&self, task_id: &str, enabled: bool) -> Result<()> {
        sqlx::query("UPDATE scheduled_tasks SET enabled = ?1 WHERE id = ?2")
            .bind(enabled)
            .bind(task_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

/// Task scheduler
pub struct TaskScheduler {
    store: Arc<dyn TaskStore>,
    running: Arc<RwLock<bool>>,
    check_interval: Duration,
}

impl TaskScheduler {
    /// Create a new task scheduler
    pub fn new(store: Arc<dyn TaskStore>) -> Self {
        Self {
            store,
            running: Arc::new(RwLock::new(false)),
            check_interval: Duration::from_secs(10),
        }
    }
    
    /// Set the check interval
    pub fn with_check_interval(mut self, interval: Duration) -> Self {
        self.check_interval = interval;
        self
    }
    
    /// Schedule a new task
    pub async fn schedule(&self, mut task: ScheduledTask) -> Result<String> {
        // Ensure task has an ID
        if task.id.is_empty() {
            task.id = uuid::Uuid::new_v4().to_string();
        }
        
        // Calculate initial next run time
        task.next_run_at = task.calculate_next_run();
        
        self.store.save(&task).await?;
        
        info!("Scheduled task: {} (id: {}, next_run: {:?})", 
            task.name, task.id, task.next_run_at);
        
        Ok(task.id)
    }
    
    /// Get a task by ID
    pub async fn get_task(&self, task_id: &str) -> Result<Option<ScheduledTask>> {
        self.store.get(task_id).await
    }
    
    /// List all tasks
    pub async fn list_tasks(&self) -> Result<Vec<ScheduledTask>> {
        self.store.list().await
    }
    
    /// List enabled tasks
    pub async fn list_enabled_tasks(&self) -> Result<Vec<ScheduledTask>> {
        self.store.list_enabled().await
    }
    
    /// Cancel a task
    pub async fn cancel_task(&self, task_id: &str) -> Result<()> {
        self.store.delete(task_id).await?;
        info!("Cancelled task: {}", task_id);
        Ok(())
    }
    
    /// Enable/disable a task
    pub async fn set_task_enabled(&self, task_id: &str, enabled: bool) -> Result<()> {
        self.store.set_enabled(task_id, enabled).await?;
        info!("Task {} {}", task_id, if enabled { "enabled" } else { "disabled" });
        Ok(())
    }
    
    /// Start the scheduler
    pub async fn start(&self, executor: Arc<dyn TaskExecutor>) -> Result<()> {
        let mut running = self.running.write().await;
        if *running {
            warn!("Scheduler is already running");
            return Ok(());
        }
        *running = true;
        drop(running);
        
        let store = self.store.clone();
        let running = self.running.clone();
        let interval = self.check_interval;
        
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            
            info!("Task scheduler started with {:?} check interval", interval);
            
            loop {
                ticker.tick().await;
                
                if !*running.read().await {
                    info!("Task scheduler stopping");
                    break;
                }
                
                // Get due tasks
                let now = Utc::now();
                match store.get_due_tasks(now).await {
                    Ok(tasks) => {
                        if !tasks.is_empty() {
                            debug!("Found {} due tasks", tasks.len());
                        }
                        
                        for task in tasks {
                            let executor = executor.clone();
                            let store = store.clone();
                            let task_id = task.id.clone();
                            let task_name = task.name.clone();
                            
                            // Update next run time immediately to prevent duplicate execution
                            if let Err(e) = store.update_next_run(&task_id, None).await {
                                error!("Failed to update next_run for task {}: {}", task_id, e);
                                continue;
                            }
                            
                            tokio::spawn(async move {
                                info!("Executing scheduled task: {} ({})", task_name, task_id);
                                
                                let start = std::time::Instant::now();
                                
                                // Execute the task
                                match executor.execute(&task.action).await {
                                    Ok(result) => {
                                        let elapsed = start.elapsed();
                                        
                                        if result.success {
                                            info!("Task {} completed successfully in {:?}", task_id, elapsed);
                                        } else {
                                            warn!("Task {} completed with errors in {:?}: {:?}", 
                                                task_id, elapsed, result.error);
                                        }
                                        
                                        // Update task status
                                        let next_run = if result.success {
                                            task.schedule.next_run(Utc::now())
                                        } else if task.retry_count < task.max_retries {
                                            // Retry after delay
                                            Some(Utc::now() + Duration::from_secs(300))
                                        } else {
                                            task.schedule.next_run(Utc::now())
                                        };
                                        
                                        let run_count = task.run_count + 1;
                                        let retry_count = if result.success { 0 } else { task.retry_count + 1 };
                                        
                                        if let Err(e) = store.update_last_run(&task_id, Utc::now(), run_count).await {
                                            error!("Failed to update last_run for task {}: {}", task_id, e);
                                        }
                                        
                                        if let Err(e) = store.update_next_run(&task_id, next_run).await {
                                            error!("Failed to update next_run for task {}: {}", task_id, e);
                                        }
                                        
                                        if retry_count != 0 {
                                            if let Err(e) = store.update_retry_count(&task_id, retry_count).await {
                                                error!("Failed to update retry_count for task {}: {}", task_id, e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!("Task {} execution failed: {}", task_id, e);
                                        
                                        // Schedule retry
                                        let retry_count = task.retry_count + 1;
                                        let next_run = if retry_count < task.max_retries {
                                            Some(Utc::now() + Duration::from_secs(300))
                                        } else {
                                            task.schedule.next_run(Utc::now())
                                        };
                                        
                                        if let Err(e) = store.update_next_run(&task_id, next_run).await {
                                            error!("Failed to update next_run for task {}: {}", task_id, e);
                                        }
                                        
                                        if let Err(e) = store.update_retry_count(&task_id, retry_count).await {
                                            error!("Failed to update retry_count for task {}: {}", task_id, e);
                                        }
                                    }
                                }
                            });
                        }
                    }
                    Err(e) => {
                        error!("Failed to get due tasks: {}", e);
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Stop the scheduler
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        info!("Task scheduler stop requested");
    }
    
    /// Check if the scheduler is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_sqlite_task_store() {
        let store = SqliteTaskStore::new("sqlite::memory:").await.unwrap();
        
        // Create a test task
        let task = ScheduledTask::new(
            "Test Task".to_string(),
            ScheduleType::Periodic { interval: Duration::from_secs(60) },
            TaskAction::Cognitive { 
                prompt: "test".to_string(), 
                context: None 
            }
        );
        
        // Save task
        store.save(&task).await.unwrap();
        
        // Retrieve task
        let retrieved = store.get(&task.id).await.unwrap().unwrap();
        assert_eq!(retrieved.name, task.name);
        assert_eq!(retrieved.priority, task.priority);
        
        // List tasks
        let tasks = store.list().await.unwrap();
        assert_eq!(tasks.len(), 1);
        
        // Update next run
        let next_run = Utc::now() + Duration::from_secs(3600);
        store.update_next_run(&task.id, Some(next_run)).await.unwrap();
        
        let updated = store.get(&task.id).await.unwrap().unwrap();
        assert!(updated.next_run_at.is_some());
        
        // Delete task
        store.delete(&task.id).await.unwrap();
        let deleted = store.get(&task.id).await.unwrap();
        assert!(deleted.is_none());
    }
    
    #[tokio::test]
    async fn test_get_due_tasks() {
        let store = SqliteTaskStore::new("sqlite::memory:").await.unwrap();
        
        // Create a task that is due
        let mut due_task = ScheduledTask::new(
            "Due Task".to_string(),
            ScheduleType::Periodic { interval: Duration::from_secs(60) },
            TaskAction::Cognitive { 
                prompt: "test".to_string(), 
                context: None 
            }
        );
        due_task.next_run_at = Some(Utc::now() - Duration::from_secs(10));
        store.save(&due_task).await.unwrap();
        
        // Create a task that is not due
        let mut future_task = ScheduledTask::new(
            "Future Task".to_string(),
            ScheduleType::Periodic { interval: Duration::from_secs(60) },
            TaskAction::Cognitive { 
                prompt: "test".to_string(), 
                context: None 
            }
        );
        future_task.next_run_at = Some(Utc::now() + Duration::from_secs(3600));
        store.save(&future_task).await.unwrap();
        
        // Get due tasks
        let due_tasks = store.get_due_tasks(Utc::now()).await.unwrap();
        assert_eq!(due_tasks.len(), 1);
        assert_eq!(due_tasks[0].name, "Due Task");
    }
}
