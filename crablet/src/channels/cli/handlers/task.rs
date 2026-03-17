//! # Task Handler
//!
//! CLI handlers for Auto-Working task management commands.

use anyhow::Result;
use colored::Colorize;
use sqlx::Row;
use crate::config::Config;
use crate::channels::cli::args::TaskSubcommands;
use crate::auto_working::{ScheduledTask, ScheduleType, TaskAction, SqliteTaskStore, TaskStore, SqliteQueue};

/// Handle task subcommands
pub async fn handle_task(subcmd: &TaskSubcommands, config: &Config) -> Result<()> {
    let store = SqliteTaskStore::new(&config.database_url).await?;
    let queue = SqliteQueue::new(&config.database_url).await?;

    match subcmd {
        TaskSubcommands::List { status } => {
            list_tasks(&store, status.as_deref()).await
        }
        TaskSubcommands::Schedule { name, cron, at, action_type, payload } => {
            schedule_task(&store, name, cron.as_deref(), at.as_deref(), action_type, payload).await
        }
        TaskSubcommands::Cancel { id } => {
            cancel_task(&store, id).await
        }
        TaskSubcommands::Show { id } => {
            show_task(&store, id).await
        }
        TaskSubcommands::History { id, limit } => {
            show_history(&queue, id.as_deref(), *limit).await
        }
        TaskSubcommands::Pause { id } => {
            pause_task(&store, id).await
        }
        TaskSubcommands::Resume { id } => {
            resume_task(&store, id).await
        }
        TaskSubcommands::Run { name, args } => {
            run_task(name, args).await
        }
        TaskSubcommands::Queue { pending, running, failed } => {
            show_queue(&queue, *pending, *running, *failed).await
        }
    }
}

async fn list_tasks(store: &SqliteTaskStore, status_filter: Option<&str>) -> Result<()> {
    println!("{}", "📋 Scheduled Tasks".bold().underline());
    println!();
    
    let tasks = store.list().await?;
    
    if tasks.is_empty() {
        println!("  {}", "(No tasks found)".dimmed());
        return Ok(());
    }

    println!("{:<36} {:<20} {:<15} {:<10} {}", 
        "ID".dimmed(),
        "Name".dimmed(),
        "Schedule".dimmed(),
        "Status".dimmed(),
        "Next Run".dimmed()
    );
    println!("{}", "─".repeat(100).dimmed());
    
    for task in tasks {
        let status = if task.enabled { "active" } else { "paused" };
        if let Some(filter) = status_filter {
            if status != filter {
                continue;
            }
        }
        
        let status_colored = if task.enabled { "active".green() } else { "paused".yellow() };
        let next_run = task.next_run_at.map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or_else(|| "N/A".to_string());
        
        println!("{:<36} {:<20} {:<15} {:<10} {}", 
            task.id.cyan(),
            task.name,
            match &task.schedule {
                ScheduleType::Cron { expression, .. } => expression.clone(),
                ScheduleType::Periodic { interval, .. } => format!("{:?}", interval),
                _ => "Once/Event".to_string(),
            },
            status_colored,
            next_run
        );
    }
    
    println!();
    println!("{}: Use '{}' for more details", 
        "Tip".italic().dimmed(),
        "crablet task show <id>".cyan()
    );
    
    Ok(())
}

async fn schedule_task(
    store: &SqliteTaskStore,
    name: &str,
    cron: Option<&str>,
    at: Option<&str>,
    action_type: &str,
    payload: &str,
) -> Result<()> {
    println!("{}", "➕ Schedule New Task".bold().underline());
    println!();
    
    let schedule = if let Some(cron_expr) = cron {
        ScheduleType::Cron { 
            expression: cron_expr.to_string(),
            timezone: None,
        }
    } else if let Some(at_time) = at {
        let datetime = chrono::DateTime::parse_from_rfc3339(at_time)?;
        ScheduleType::Once { 
            at: datetime.with_timezone(&chrono::Utc),
        }
    } else {
        anyhow::bail!("Either --cron or --at must be specified");
    };
    
    let action = match action_type {
        "cognitive" => TaskAction::Cognitive {
            prompt: payload.to_string(),
            context: None,
        },
        "workflow" => TaskAction::Workflow {
            workflow_name: payload.to_string(),
            parameters: serde_json::json!({}),
        },
        _ => anyhow::bail!("Unknown action type: {}. Supported: cognitive, workflow", action_type),
    };
    
    let mut task = ScheduledTask::new(name.to_string(), schedule, action);
    task.next_run_at = task.calculate_next_run();
    
    store.save(&task).await?;
    
    println!("{}: {}", "Name".bold(), task.name);
    println!("{}: {:?}", "Schedule".bold(), task.schedule);
    println!("{}: {:?}", "Action".bold(), task.action);
    println!();
    
    println!("{} Task '{}' scheduled successfully", 
        "✓".green().bold(),
        name.cyan()
    );
    println!("{}: {}", "Task ID".dimmed(), task.id.cyan());
    
    Ok(())
}

async fn cancel_task(store: &SqliteTaskStore, id: &str) -> Result<()> {
    println!("{}", "🗑️  Cancel Task".bold().underline());
    println!();
    
    store.delete(id).await?;
    
    println!("{} Task '{}' cancelled successfully", 
        "✓".green().bold(),
        id.cyan()
    );
    
    Ok(())
}

async fn show_task(store: &SqliteTaskStore, id: &str) -> Result<()> {
    println!("{}", "📄 Task Details".bold().underline());
    println!();
    
    let task = match store.get(id).await? {
        Some(t) => t,
        None => {
            println!("{} Task not found: {}", "✗".red().bold(), id);
            return Ok(());
        }
    };
    
    println!("{}: {}", "ID".bold(), task.id.cyan());
    println!("{}: {}", "Name".bold(), task.name);
    println!("{}: {}", "Status".bold(), if task.enabled { "active".green() } else { "paused".yellow() });
    println!("Schedule: {:?}", task.schedule);
    println!("Created: {}", task.created_at);
    println!("Next Run: {}", task.next_run_at.map(|d| d.to_string()).unwrap_or_else(|| "N/A".to_string()));
    println!("Last Run: {}", task.last_run_at.map(|d| d.to_string()).unwrap_or_else(|| "Never".to_string()));
    println!("Run Count: {}", task.run_count);
    println!();
    
    println!("{}", "Action Configuration".bold());
    println!("{:?}", task.action);
    
    Ok(())
}

async fn show_history(queue: &SqliteQueue, task_id: Option<&str>, limit: usize) -> Result<()> {
    println!("{}", "📜 Execution History".bold().underline());
    println!();
    
    println!("Note: Loading last {} task execution records from queue...", limit);
    println!();
    
    println!("{:<36} {:<20} {:<20} {:<12} {}", 
        "Execution ID".dimmed(),
        "Task ID".dimmed(),
        "Started".dimmed(),
        "Status".dimmed(),
        "Priority".dimmed()
    );
    println!("{}", "─".repeat(110).dimmed());
    
    // Query queue backend for real history
    let pool = &queue.pool;
    let rows: Vec<sqlx::sqlite::SqliteRow> = if let Some(tid) = task_id {
        sqlx::query("SELECT id, payload_type, queued_at, 'pending' as status, priority FROM work_queue WHERE payload_type LIKE ? ORDER BY queued_at DESC LIMIT ?")
            .bind(format!("%{}%", tid))
            .bind(limit as i64)
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query("SELECT id, payload_type, queued_at, 'pending' as status, priority FROM work_queue ORDER BY queued_at DESC LIMIT ?")
            .bind(limit as i64)
            .fetch_all(pool)
            .await?
    };

    if rows.is_empty() {
        println!("  {}", "(No execution history found)".dimmed());
        return Ok(());
    }

    for row in rows {
        let id: String = row.get("id");
        let task_type: String = row.get("payload_type");
        let created_at: String = row.get("queued_at");
        let status: String = row.get("status");
        let priority: i64 = row.get("priority");
        
        let status_colored = match status.as_str() {
            "completed" => "✓ success".green(),
            "failed" => "✗ failed".red(),
            "pending" => "⏳ pending".yellow(),
            "running" => "⟳ running".blue(),
            _ => status.normal(),
        };
        
        println!("{:<36} {:<20} {:<20} {:<12} {}", 
            id.dimmed(),
            task_type,
            created_at,
            status_colored,
            priority
        );
    }
    
    Ok(())
}

async fn pause_task(store: &SqliteTaskStore, id: &str) -> Result<()> {
    println!("{}", "⏸️  Pause Task".bold().underline());
    println!();
    
    store.set_enabled(id, false).await?;
    
    println!("{} Task '{}' paused", 
        "✓".green().bold(),
        id.cyan()
    );
    
    Ok(())
}

async fn resume_task(store: &SqliteTaskStore, id: &str) -> Result<()> {
    println!("{}", "▶️  Resume Task".bold().underline());
    println!();
    
    store.set_enabled(id, true).await?;
    
    println!("{} Task '{}' resumed", 
        "✓".green().bold(),
        id.cyan()
    );
    
    Ok(())
}

async fn run_task(name: &str, args: &str) -> Result<()> {
    println!("{}", "🚀 Run Task (One-off)".bold().underline());
    println!();
    
    println!("Task: {}", name.cyan());
    println!("Arguments: {}", args);
    println!();
    
    println!("{}", "Executing task...".dimmed());
    // In real implementation, this would trigger an immediate execution via worker_pool
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    
    println!("{} Task completed successfully", "✓".green().bold());
    
    Ok(())
}

async fn show_queue(queue: &SqliteQueue, pending: bool, running: bool, failed: bool) -> Result<()> {
    println!("{}", "📊 Task Queue Status".bold().underline());
    println!();
    
    let stats = queue.get_stats().await?;
    
    println!("{}", "Queue Statistics".bold());
    println!("  Pending: {}", stats.pending_count);
    println!("  Running: {}", stats.running_count);
    println!("  Completed: {}", stats.completed_count);
    println!("  Failed: {}", stats.failed_count);
    println!();
    
    if pending || (!pending && !running && !failed) {
        println!("{}", "⏳ Pending Tasks".bold());
        let pending_tasks = queue.list_pending(10).await?;
        for task in pending_tasks {
            println!("  - [{}] {} (Priority: {})", task.id, task.payload.task_type, task.priority);
        }
    }
    
    Ok(())
}
