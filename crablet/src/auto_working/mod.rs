//! Auto-Working Module
//!
//! This module provides autonomous task execution capabilities for Crablet,
//! including task scheduling, work queues, and worker pools.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     Auto-Working System                          │
//! │                                                                  │
//! │   ┌──────────────┐    ┌──────────────┐    ┌──────────────┐     │
//! │   │   Task       │───►│   Work       │───►│   Worker     │     │
//! │   │   Scheduler  │    │   Queue      │    │   Pool       │     │
//! │   └──────────────┘    └──────────────┘    └──────────────┘     │
//! │          │                   │                   │              │
//! │          └───────────────────┴───────────────────┘              │
//! │                              │                                  │
//! │                              ▼                                  │
//! │                   ┌─────────────────────┐                       │
//! │                   │   Task Executor     │                       │
//! │                   │   (Cognitive/RPA)   │                       │
//! │                   └─────────────────────┘                       │
//! └─────────────────────────────────────────────────────────────────┘
//! ```

pub mod scheduler;
pub mod queue;
pub mod worker;
pub mod types;

pub use scheduler::{TaskScheduler, TaskStore, SqliteTaskStore};
pub use queue::{WorkQueue, QueueBackend, SqliteQueue};
pub use worker::{WorkerPool, WorkerConfig, TaskExecutor};
pub use types::*;

use std::sync::Arc;
use crate::error::Result;
use async_trait::async_trait;

/// Auto-Working system coordinator
pub struct AutoWorkingSystem {
    scheduler: Arc<TaskScheduler>,
    queue: Arc<WorkQueue>,
    worker_pool: Arc<WorkerPool>,
}

/// Task executor that pushes tasks to the work queue
pub struct QueueTaskExecutor {
    queue: Arc<WorkQueue>,
}

impl QueueTaskExecutor {
    pub fn new(queue: Arc<WorkQueue>) -> Self {
        Self { queue }
    }
}

#[async_trait]
impl TaskExecutor for QueueTaskExecutor {
    async fn execute(&self, action: &TaskAction) -> Result<TaskResult> {
        let payload = TaskPayload {
            task_type: "scheduled".to_string(),
            data: serde_json::to_value(action)?,
        };
        let task_id = self.queue.submit(payload, 50).await?;
        Ok(TaskResult::success(format!("Scheduled task in queue with ID: {}", task_id)))
    }
    
    fn name(&self) -> &str {
        "QueueTaskExecutor"
    }
}

impl AutoWorkingSystem {
    /// Create a new auto-working system
    pub async fn new(database_url: &str) -> Result<Self> {
        let store = Arc::new(SqliteTaskStore::new(database_url).await?);
        let scheduler = Arc::new(TaskScheduler::new(store.clone()));
        
        let queue_backend = Arc::new(SqliteQueue::new(database_url).await?);
        let queue = Arc::new(WorkQueue::new(queue_backend));
        
        let worker_pool = Arc::new(WorkerPool::new(WorkerConfig::default(), queue.clone()));
        
        Ok(Self {
            scheduler,
            queue,
            worker_pool,
        })
    }
    
    /// Start the auto-working system
    pub async fn start(&self) -> Result<()> {
        // Start worker pool
        self.worker_pool.start().await?;
        
        // Start scheduler with a queue-pushing executor
        let executor = Arc::new(QueueTaskExecutor::new(self.queue.clone()));
        self.scheduler.start(executor).await?;
        
        tracing::info!("Auto-working system started");
        Ok(())
    }
    
    /// Stop the auto-working system
    pub async fn stop(&self) {
        self.scheduler.stop().await;
        self.worker_pool.stop().await;
        tracing::info!("Auto-working system stopped");
    }
    
    /// Get the scheduler
    pub fn scheduler(&self) -> Arc<TaskScheduler> {
        self.scheduler.clone()
    }
    
    /// Get the queue
    pub fn queue(&self) -> Arc<WorkQueue> {
        self.queue.clone()
    }
    
    /// Get the worker pool
    pub fn worker_pool(&self) -> Arc<WorkerPool> {
        self.worker_pool.clone()
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_auto_working_system_creation() {
        // This would need a test database
        // let system = AutoWorkingSystem::new("sqlite::memory:").await.unwrap();
        // assert!(system.scheduler().is_some());
    }
}
