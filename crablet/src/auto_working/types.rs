//! Auto-Working Types
//!
//! Core type definitions for the auto-working system.

use std::collections::HashMap;
use std::time::Duration;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Schedule type for tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScheduleType {
    /// Execute once at a specific time
    Once { at: DateTime<Utc> },
    /// Execute periodically with a fixed interval
    Periodic { interval: Duration },
    /// Execute based on a cron expression
    Cron { expression: String, timezone: Option<String> },
    /// Execute when a specific event occurs
    EventTriggered { event_type: String, condition: String },
}

impl ScheduleType {
    /// Calculate the next execution time based on this schedule
    pub fn next_run(&self, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match self {
            ScheduleType::Once { at } => {
                if *at > after {
                    Some(*at)
                } else {
                    None // Already executed
                }
            }
            ScheduleType::Periodic { interval } => {
                Some(after + *interval)
            }
            ScheduleType::Cron { expression, .. } => {
                parse_cron_next(expression, after)
            }
            ScheduleType::EventTriggered { .. } => {
                None // Event-triggered tasks don't have a fixed next run time
            }
        }
    }
}

/// Parse a cron expression and get the next execution time
fn parse_cron_next(expression: &str, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
    #[cfg(feature = "auto-working")]
    {
        use cron::Schedule;
        use std::str::FromStr;
        
        if let Ok(schedule) = Schedule::from_str(expression) {
            schedule.after(&after).next()
        } else {
            None
        }
    }
    #[cfg(not(feature = "auto-working"))]
    {
        // Fallback when auto-working feature is disabled
        Some(after + Duration::from_secs(3600))
    }
}

/// Task action types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TaskAction {
    /// Execute cognitive processing
    Cognitive { 
        prompt: String, 
        context: Option<String> 
    },
    /// Execute a system command
    SystemCommand { 
        command: String, 
        args: Vec<String> 
    },
    /// Make an HTTP API call
    ApiCall { 
        endpoint: String, 
        method: HttpMethod, 
        headers: Option<HashMap<String, String>>,
        body: Option<serde_json::Value> 
    },
    /// Execute an RPA workflow
    Rpa { 
        workflow_id: String, 
        parameters: serde_json::Value 
    },
    /// Execute a named workflow
    Workflow { 
        workflow_name: String, 
        parameters: serde_json::Value 
    },
    /// Execute a composite task (multiple actions)
    Composite { 
        actions: Vec<TaskAction> 
    },
}

/// System type for cognitive processing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutoWorkingSystemType {
    System1,
    System2,
    System3,
}
/// HTTP methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpMethod::GET => write!(f, "GET"),
            HttpMethod::POST => write!(f, "POST"),
            HttpMethod::PUT => write!(f, "PUT"),
            HttpMethod::DELETE => write!(f, "DELETE"),
            HttpMethod::PATCH => write!(f, "PATCH"),
        }
    }
}

/// Scheduled task definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub id: String,
    pub name: String,
    pub description: String,
    pub schedule: ScheduleType,
    pub action: TaskAction,
    pub priority: u8,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub next_run_at: Option<DateTime<Utc>>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub run_count: u32,
    pub max_retries: u32,
    pub retry_count: u32,
}

impl ScheduledTask {
    /// Create a new scheduled task
    pub fn new(name: String, schedule: ScheduleType, action: TaskAction) -> Self {
        let now = Utc::now();
        let next_run_at = schedule.next_run(now);
        
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            description: String::new(),
            schedule,
            action,
            priority: 50,
            enabled: true,
            created_at: now,
            next_run_at,
            last_run_at: None,
            run_count: 0,
            max_retries: 3,
            retry_count: 0,
        }
    }
    
    /// Calculate the next execution time
    pub fn calculate_next_run(&self) -> Option<DateTime<Utc>> {
        let base = self.last_run_at.unwrap_or(self.created_at);
        self.schedule.next_run(base)
    }
    
    /// Update after execution
    pub fn mark_executed(&mut self, success: bool) {
        self.last_run_at = Some(Utc::now());
        self.run_count += 1;
        
        if success {
            self.retry_count = 0;
            self.next_run_at = self.calculate_next_run();
        } else {
            self.retry_count += 1;
            if self.retry_count < self.max_retries {
                // Retry after 5 minutes
                self.next_run_at = Some(Utc::now() + Duration::from_secs(300));
            } else {
                // Max retries reached, schedule next normal run
                self.retry_count = 0;
                self.next_run_at = self.calculate_next_run();
            }
        }
    }
}

/// Task payload for queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPayload {
    pub task_type: String,
    pub data: serde_json::Value,
}

/// Queued task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedTask {
    pub id: String,
    pub payload: TaskPayload,
    pub priority: u8,
    pub attempts: u32,
    pub max_attempts: u32,
    pub queued_at: DateTime<Utc>,
    pub scheduled_at: DateTime<Utc>,
    pub visibility_timeout: Option<Duration>,
}

impl QueuedTask {
    /// Create a new queued task
    pub fn new(payload: TaskPayload, priority: u8) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            payload,
            priority,
            attempts: 0,
            max_attempts: 3,
            queued_at: now,
            scheduled_at: now,
            visibility_timeout: None,
        }
    }
    
    /// Check if the task is visible (not in visibility timeout)
    pub fn is_visible(&self) -> bool {
        if let Some(timeout) = self.visibility_timeout {
            let elapsed = Utc::now() - self.scheduled_at;
            elapsed > chrono::Duration::from_std(timeout).unwrap_or(chrono::Duration::MAX)
        } else {
            true
        }
    }
}

/// Task execution result
#[derive(Debug, Clone)]
pub struct TaskResult {
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub execution_time: Duration,
    pub metadata: HashMap<String, String>,
}

impl TaskResult {
    /// Create a successful result
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: Some(output.into()),
            error: None,
            execution_time: Duration::default(),
            metadata: HashMap::new(),
        }
    }
    
    /// Create a failed result
    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            success: false,
            output: None,
            error: Some(error.into()),
            execution_time: Duration::default(),
            metadata: HashMap::new(),
        }
    }
}

/// Retry policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub backoff_strategy: BackoffStrategy,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            backoff_strategy: BackoffStrategy::Exponential {
                base: 2,
                max_delay: Duration::from_secs(300),
            },
        }
    }
}

/// Backoff strategy for retries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackoffStrategy {
    /// Fixed delay between retries
    Fixed { delay: Duration },
    /// Exponential backoff
    Exponential { base: u32, max_delay: Duration },
    /// Linear backoff
    Linear { increment: Duration, max_delay: Duration },
}

impl BackoffStrategy {
    /// Calculate the delay for a specific retry attempt
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        match self {
            BackoffStrategy::Fixed { delay } => *delay,
            BackoffStrategy::Exponential { base, max_delay } => {
                let delay = Duration::from_secs(base.pow(attempt) as u64);
                delay.min(*max_delay)
            }
            BackoffStrategy::Linear { increment, max_delay } => {
                let delay = *increment * attempt;
                delay.min(*max_delay)
            }
        }
    }
}

/// Resource requirements for task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirements {
    pub cpu_cores: f32,
    pub memory_mb: u32,
    pub estimated_duration: Duration,
}

impl Default for ResourceRequirements {
    fn default() -> Self {
        Self {
            cpu_cores: 0.5,
            memory_mb: 256,
            estimated_duration: Duration::from_secs(60),
        }
    }
}

/// Task status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Execution record for tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub task_id: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: TaskStatus,
    pub output: Option<String>,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_schedule_type_next_run() {
        let now = Utc::now();
        
        // Test Once schedule
        let future = now + Duration::from_secs(3600);
        let once = ScheduleType::Once { at: future };
        assert_eq!(once.next_run(now), Some(future));
        
        let past = now - Duration::from_secs(3600);
        let once_past = ScheduleType::Once { at: past };
        assert_eq!(once_past.next_run(now), None);
        
        // Test Periodic schedule
        let periodic = ScheduleType::Periodic { interval: Duration::from_secs(60) };
        let next = periodic.next_run(now).unwrap();
        assert!(next > now);
        assert_eq!((next - now).num_seconds(), 60);
    }
    
    #[test]
    fn test_backoff_strategy() {
        // Test fixed backoff
        let fixed = BackoffStrategy::Fixed { delay: Duration::from_secs(10) };
        assert_eq!(fixed.calculate_delay(0), Duration::from_secs(10));
        assert_eq!(fixed.calculate_delay(5), Duration::from_secs(10));
        
        // Test exponential backoff
        let exp = BackoffStrategy::Exponential { 
            base: 2, 
            max_delay: Duration::from_secs(60) 
        };
        assert_eq!(exp.calculate_delay(0), Duration::from_secs(1));
        assert_eq!(exp.calculate_delay(1), Duration::from_secs(2));
        assert_eq!(exp.calculate_delay(2), Duration::from_secs(4));
        assert_eq!(exp.calculate_delay(10), Duration::from_secs(60)); // Capped at max
    }
    
    #[test]
    fn test_scheduled_task_mark_executed() {
        let mut task = ScheduledTask::new(
            "Test Task".to_string(),
            ScheduleType::Periodic { interval: Duration::from_secs(60) },
            TaskAction::Cognitive { 
                prompt: "test".to_string(), 
                context: None 
            }
        );
        
        assert_eq!(task.run_count, 0);
        
        task.mark_executed(true);
        assert_eq!(task.run_count, 1);
        assert!(task.last_run_at.is_some());
        assert!(task.next_run_at.is_some());
    }
}
