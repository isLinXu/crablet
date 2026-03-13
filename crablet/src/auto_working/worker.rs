//! Worker Pool
//!
//! Manages a pool of workers that execute tasks from the queue.
//! Supports auto-scaling, health checks, and graceful shutdown.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use tokio::sync::{RwLock, mpsc, Semaphore};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::error::Result;
use crate::auto_working::types::*;
use crate::auto_working::queue::WorkQueue;

/// Worker configuration
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    /// Minimum number of workers
    pub min_workers: usize,
    /// Maximum number of workers
    pub max_workers: usize,
    /// Number of concurrent tasks per worker
    pub task_concurrency: usize,
    /// Idle timeout before worker shutdown
    pub idle_timeout: Duration,
    /// Task execution timeout
    pub task_timeout: Duration,
    /// Enable auto-scaling
    pub auto_scaling: bool,
    /// Scale up threshold (queue length)
    pub scale_up_threshold: usize,
    /// Scale down threshold (queue length)
    pub scale_down_threshold: usize,
    /// Graceful shutdown timeout
    pub graceful_shutdown_timeout: Duration,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            min_workers: 2,
            max_workers: 10,
            task_concurrency: 5,
            idle_timeout: Duration::from_secs(300),
            task_timeout: Duration::from_secs(300),
            auto_scaling: true,
            scale_up_threshold: 10,
            scale_down_threshold: 2,
            graceful_shutdown_timeout: Duration::from_secs(30),
        }
    }
}

/// Task executor trait
#[async_trait]
pub trait TaskExecutor: Send + Sync {
    /// Execute a task action
    async fn execute(&self, action: &TaskAction) -> Result<TaskResult>;
    
    /// Get executor name
    fn name(&self) -> &str;
}

/// Worker handle
#[derive(Debug)]
struct WorkerHandle {
    id: usize,
    handle: JoinHandle<()>,
    shutdown_tx: mpsc::Sender<()>,
    active_tasks: Arc<RwLock<usize>>,
    last_activity: Arc<RwLock<std::time::Instant>>,
}

/// Worker pool
pub struct WorkerPool {
    config: WorkerConfig,
    queue: Arc<WorkQueue>,
    workers: Arc<RwLock<Vec<WorkerHandle>>>,
    executor: Arc<RwLock<Option<Arc<dyn TaskExecutor>>>>,
    running: Arc<RwLock<bool>>,
    scaler_handle: Arc<RwLock<Option<JoinHandle<()>>>>,
}

impl WorkerPool {
    /// Create a new worker pool
    pub fn new(config: WorkerConfig, queue: Arc<WorkQueue>) -> Self {
        Self {
            config,
            queue,
            workers: Arc::new(RwLock::new(Vec::new())),
            executor: Arc::new(RwLock::new(None)),
            running: Arc::new(RwLock::new(false)),
            scaler_handle: Arc::new(RwLock::new(None)),
        }
    }
    
    /// Set the task executor
    pub async fn set_executor(&self, executor: Arc<dyn TaskExecutor>) {
        let mut ex = self.executor.write().await;
        *ex = Some(executor);
    }
    
    /// Start the worker pool
    pub async fn start(&self) -> Result<()> {
        let mut running = self.running.write().await;
        if *running {
            warn!("Worker pool is already running");
            return Ok(());
        }
        *running = true;
        drop(running);
        
        // Start minimum workers
        for i in 0..self.config.min_workers {
            self.spawn_worker(i).await?;
        }
        
        info!("Started worker pool with {} workers", self.config.min_workers);
        
        // Start auto-scaler if enabled
        if self.config.auto_scaling {
            self.start_scaler().await?;
        }
        
        Ok(())
    }
    
    /// Stop the worker pool
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        drop(running);
        
        // Stop scaler
        if let Some(handle) = self.scaler_handle.write().await.take() {
            handle.abort();
        }
        
        // Signal all workers to shutdown
        let workers = self.workers.read().await;
        for worker in workers.iter() {
            let _ = worker.shutdown_tx.send(()).await;
        }
        drop(workers);
        
        // Wait for workers to finish
        let timeout = self.config.graceful_shutdown_timeout;
        let start = std::time::Instant::now();
        
        loop {
            let workers = self.workers.read().await;
            if workers.is_empty() || start.elapsed() >= timeout {
                break;
            }
            drop(workers);
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        // Abort remaining workers
        let mut workers = self.workers.write().await;
        for worker in workers.drain(..) {
            worker.handle.abort();
        }
        
        info!("Worker pool stopped");
    }
    
    /// Spawn a new worker
    async fn spawn_worker(&self, id: usize) -> Result<()> {
        let queue = self.queue.clone();
        let executor = self.executor.clone();
        let running = self.running.clone();
        let config = self.config.clone();
        let workers = self.workers.clone();
        
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        let active_tasks = Arc::new(RwLock::new(0usize));
        let last_activity = Arc::new(RwLock::new(std::time::Instant::now()));
        
        let active_tasks_clone = active_tasks.clone();
        let last_activity_clone = last_activity.clone();
        
        let handle = tokio::spawn(async move {
            info!("Worker {} started", id);
            
            let semaphore = Arc::new(Semaphore::new(config.task_concurrency));
            
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        debug!("Worker {} received shutdown signal", id);
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {
                        if !*running.read().await {
                            break;
                        }
                        
                        // Check idle timeout
                        let idle_duration = {
                            let last = last_activity_clone.read().await;
                            last.elapsed()
                        };
                        
                        let active = *active_tasks_clone.read().await;
                        if active == 0 && idle_duration > config.idle_timeout {
                            // Check if we're above minimum workers
                            let worker_count = workers.read().await.len();
                            if worker_count > config.min_workers {
                                debug!("Worker {} idle for {:?}, shutting down", id, idle_duration);
                                break;
                            }
                        }
                        
                        // Try to get a task
                        let permit = match semaphore.clone().try_acquire_owned() {
                            Ok(permit) => permit,
                            Err(_) => continue, // At max concurrency
                        };
                        
                        let task = match queue.get(Duration::from_millis(100)).await {
                            Ok(Some(task)) => task,
                            Ok(None) => continue,
                            Err(e) => {
                                error!("Worker {} failed to get task: {}", id, e);
                                continue;
                            }
                        };
                        
                        // Update activity
                        *last_activity_clone.write().await = std::time::Instant::now();
                        
                        // Increment active tasks
                        *active_tasks_clone.write().await += 1;
                        
                        // Execute task
                        let queue_clone = queue.clone();
                        let executor_clone = executor.clone();
                        let active_tasks_inner = active_tasks_clone.clone();
                        let task_timeout = config.task_timeout;
                        
                        tokio::spawn(async move {
                            let task_id = task.id.clone();
                            debug!("Worker {} executing task {}", id, task_id);
                            
                            let start = std::time::Instant::now();
                            
                            // Get executor
                            let exec = {
                                let ex = executor_clone.read().await;
                                ex.clone()
                            };
                            
                            let result = if let Some(executor) = exec {
                                // Execute with timeout
                                match tokio::time::timeout(
                                    task_timeout,
                                    executor.execute(&TaskAction::Cognitive { 
                                        prompt: format!("Execute queued task: {:?}", task.payload),
                                        context: None 
                                    })
                                ).await {
                                    Ok(Ok(result)) => {
                                        debug!("Task {} completed in {:?}", task_id, start.elapsed());
                                        if result.success {
                                            Ok(())
                                        } else {
                                            Err(result.error.unwrap_or_else(|| "Unknown error".to_string()))
                                        }
                                    }
                                    Ok(Err(e)) => {
                                        Err(format!("Execution error: {}", e))
                                    }
                                    Err(_) => {
                                        Err("Task timeout".to_string())
                                    }
                                }
                            } else {
                                Err("No executor configured".to_string())
                            };
                            
                            // Handle result
                            match result {
                                Ok(()) => {
                                    if let Err(e) = queue_clone.complete(&task_id).await {
                                        error!("Failed to ack task {}: {}", task_id, e);
                                    }
                                }
                                Err(error) => {
                                    let retry_delay = Duration::from_secs(60);
                                    if let Err(e) = queue_clone.fail_with_retry(&task, &error, retry_delay).await {
                                        error!("Failed to handle task failure for {}: {}", task_id, e);
                                    }
                                }
                            }
                            
                            // Decrement active tasks
                            *active_tasks_inner.write().await -= 1;
                            drop(permit);
                        });
                    }
                }
            }
            
            info!("Worker {} stopped", id);
            
            // Remove self from workers list
            let mut workers = workers.write().await;
            workers.retain(|w| w.id != id);
        });
        
        let worker = WorkerHandle {
            id,
            handle,
            shutdown_tx,
            active_tasks,
            last_activity,
        };
        
        self.workers.write().await.push(worker);
        
        Ok(())
    }
    
    /// Start the auto-scaler
    async fn start_scaler(&self) -> Result<()> {
        let workers = self.workers.clone();
        let queue = self.queue.clone();
        let config = self.config.clone();
        let running = self.running.clone();
        let self_ref = Arc::new(RwLock::new(None as Option<Arc<WorkerPool>>));
        
        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(10));
            let mut next_worker_id = config.max_workers; // Start IDs after initial workers
            
            loop {
                ticker.tick().await;
                
                if !*running.read().await {
                    break;
                }
                
                let queue_len = match queue.len().await {
                    Ok(len) => len,
                    Err(e) => {
                        error!("Failed to get queue length: {}", e);
                        continue;
                    }
                };
                
                let worker_count = workers.read().await.len();
                
                // Scale up
                if queue_len > config.scale_up_threshold && worker_count < config.max_workers {
                    let to_add = ((queue_len / config.scale_up_threshold).min(config.max_workers - worker_count)).min(3);
                    
                    info!("Scaling up: adding {} workers (queue: {}, current: {})", 
                        to_add, queue_len, worker_count);
                    
                    for _ in 0..to_add {
                        if let Some(pool) = self_ref.read().await.clone() {
                            if let Err(e) = pool.spawn_worker(next_worker_id).await {
                                error!("Failed to spawn worker {}: {}", next_worker_id, e);
                            } else {
                                next_worker_id += 1;
                            }
                        }
                    }
                }
                
                // Scale down (workers will self-terminate when idle)
                if queue_len < config.scale_down_threshold && worker_count > config.min_workers {
                    debug!("Scale down condition met (queue: {}, workers: {})", 
                        queue_len, worker_count);
                    // Workers will self-terminate when idle
                }
            }
        });
        
        *self.scaler_handle.write().await = Some(handle);
        
        Ok(())
    }
    
    /// Get current worker count
    pub async fn worker_count(&self) -> usize {
        self.workers.read().await.len()
    }
    
    /// Get total active tasks across all workers
    pub async fn active_task_count(&self) -> usize {
        let workers = self.workers.read().await;
        let mut total = 0;
        for worker in workers.iter() {
            total += *worker.active_tasks.read().await;
        }
        total
    }
    
    /// Check if the pool is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

/// Simple task executor implementation for testing
pub struct SimpleTaskExecutor {
    name: String,
    handler: Box<dyn Fn(&TaskAction) -> Result<TaskResult> + Send + Sync>,
}

impl SimpleTaskExecutor {
    /// Create a new simple executor
    pub fn new<F>(name: impl Into<String>, handler: F) -> Self
    where
        F: Fn(&TaskAction) -> Result<TaskResult> + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            handler: Box::new(handler),
        }
    }
}

#[async_trait]
impl TaskExecutor for SimpleTaskExecutor {
    async fn execute(&self, action: &TaskAction) -> Result<TaskResult> {
        (self.handler)(action)
    }
    
    fn name(&self) -> &str {
        &self.name
    }
}

/// Cognitive task executor (integrates with Crablet's cognitive system)
pub struct CognitiveTaskExecutor {
    router: Arc<crate::cognitive::router::CognitiveRouter>,
}

impl CognitiveTaskExecutor {
    /// Create a new cognitive task executor
    pub fn new(router: Arc<crate::cognitive::router::CognitiveRouter>) -> Self {
        Self { router }
    }
}

#[async_trait]
impl TaskExecutor for CognitiveTaskExecutor {
    async fn execute(&self, action: &TaskAction) -> Result<TaskResult> {
        match action {
            TaskAction::Cognitive { prompt, context: _ } => {
                let session_id = format!("auto_{}", uuid::Uuid::new_v4());
                
                let start = std::time::Instant::now();
                
                match self.router.process(prompt, &session_id).await {
                    Ok((response, _traces)) => {
                        Ok(TaskResult {
                            success: true,
                            output: Some(response),
                            error: None,
                            execution_time: start.elapsed(),
                            metadata: HashMap::new(),
                        })
                    }
                    Err(e) => {
                        Ok(TaskResult {
                            success: false,
                            output: None,
                            error: Some(e.to_string()),
                            execution_time: start.elapsed(),
                            metadata: HashMap::new(),
                        })
                    }
                }
            }
            TaskAction::SystemCommand { command, args } => {
                let start = std::time::Instant::now();
                
                match tokio::process::Command::new(command)
                    .args(args)
                    .output()
                    .await
                {
                    Ok(output) => {
                        let success = output.status.success();
                        let output_str = String::from_utf8_lossy(&output.stdout).to_string();
                        let error_str = String::from_utf8_lossy(&output.stderr).to_string();
                        
                        Ok(TaskResult {
                            success,
                            output: Some(output_str),
                            error: if success { None } else { Some(error_str) },
                            execution_time: start.elapsed(),
                            metadata: HashMap::new(),
                        })
                    }
                    Err(e) => {
                        Ok(TaskResult {
                            success: false,
                            output: None,
                            error: Some(e.to_string()),
                            execution_time: start.elapsed(),
                            metadata: HashMap::new(),
                        })
                    }
                }
            }
            TaskAction::ApiCall { endpoint, method, headers: _, body } => {
                let start = std::time::Instant::now();
                
                let client = reqwest::Client::new();
                let mut request = match method {
                    HttpMethod::GET => client.get(endpoint),
                    HttpMethod::POST => client.post(endpoint),
                    HttpMethod::PUT => client.put(endpoint),
                    HttpMethod::DELETE => client.delete(endpoint),
                    HttpMethod::PATCH => client.patch(endpoint),
                };
                
                if let Some(body) = body {
                    request = request.json(body);
                }
                
                match request.send().await {
                    Ok(response) => {
                        let success = response.status().is_success();
                        let text = response.text().await.unwrap_or_default();
                        
                        Ok(TaskResult {
                            success,
                            output: Some(text),
                            error: if success { None } else { Some("HTTP error".to_string()) },
                            execution_time: start.elapsed(),
                            metadata: HashMap::new(),
                        })
                    }
                    Err(e) => {
                        Ok(TaskResult {
                            success: false,
                            output: None,
                            error: Some(e.to_string()),
                            execution_time: start.elapsed(),
                            metadata: HashMap::new(),
                        })
                    }
                }
            }
            TaskAction::Rpa { .. } => {
                Ok(TaskResult {
                    success: false,
                    output: None,
                    error: Some("RPA actions require RpaTaskExecutor".to_string()),
                    execution_time: Duration::default(),
                    metadata: HashMap::new(),
                })
            }
            TaskAction::Workflow { .. } => {
                Ok(TaskResult {
                    success: false,
                    output: None,
                    error: Some("Workflow actions require WorkflowTaskExecutor".to_string()),
                    execution_time: Duration::default(),
                    metadata: HashMap::new(),
                })
            }
            TaskAction::Composite { .. } => {
                Ok(TaskResult {
                    success: false,
                    output: None,
                    error: Some("Composite actions not yet implemented".to_string()),
                    execution_time: Duration::default(),
                    metadata: HashMap::new(),
                })
            }
        }
    }
    
    fn name(&self) -> &str {
        "CognitiveTaskExecutor"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_worker_config_default() {
        let config = WorkerConfig::default();
        assert_eq!(config.min_workers, 2);
        assert_eq!(config.max_workers, 10);
        assert!(config.auto_scaling);
    }
    
    #[tokio::test]
    async fn test_simple_task_executor() {
        let executor = SimpleTaskExecutor::new("test", |action| {
            match action {
                TaskAction::Cognitive { prompt, .. } => {
                    Ok(TaskResult::success(format!("Processed: {}", prompt)))
                }
                _ => Ok(TaskResult::failure("Unsupported action")),
            }
        });
        
        let action = TaskAction::Cognitive {
            prompt: "Hello".to_string(),
            context: None,
        };
        
        let result = executor.execute(&action).await.unwrap();
        assert!(result.success);
        assert!(result.output.unwrap().contains("Processed: Hello"));
    }
}
