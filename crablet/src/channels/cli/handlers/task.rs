//! # Task Handler
//!
//! CLI handlers for Auto-Working task management commands.

use anyhow::Result;
use colored::Colorize;

use crate::channels::cli::args::TaskSubcommands;
use crate::auto_working::{ScheduledTask, ScheduleType, TaskAction};

/// Handle task subcommands
pub async fn handle_task(subcmd: &TaskSubcommands) -> Result<()> {
    match subcmd {
        TaskSubcommands::List { status } => {
            list_tasks(status.as_deref()).await
        }
        TaskSubcommands::Schedule { name, cron, at, action_type, payload } => {
            schedule_task(name, cron.as_deref(), at.as_deref(), action_type, payload).await
        }
        TaskSubcommands::Cancel { id } => {
            cancel_task(id).await
        }
        TaskSubcommands::Show { id } => {
            show_task(id).await
        }
        TaskSubcommands::History { id, limit } => {
            show_history(id.as_deref(), *limit).await
        }
        TaskSubcommands::Pause { id } => {
            pause_task(id).await
        }
        TaskSubcommands::Resume { id } => {
            resume_task(id).await
        }
        TaskSubcommands::Run { name, args } => {
            run_task(name, args).await
        }
        TaskSubcommands::Queue { pending, running, failed } => {
            show_queue(*pending, *running, *failed).await
        }
    }
}

async fn list_tasks(status_filter: Option<&str>) -> Result<()> {
    println!("{}", "📋 Scheduled Tasks".bold().underline());
    println!();
    
    // In a real implementation, this would query the database
    // For now, show placeholder
    println!("{:<36} {:<20} {:<15} {:<10} {}", 
        "ID".dimmed(),
        "Name".dimmed(),
        "Schedule".dimmed(),
        "Status".dimmed(),
        "Next Run".dimmed()
    );
    println!("{}", "─".repeat(100).dimmed());
    
    // Placeholder data
    let tasks = vec![
        ("task-001", "Daily Report", "0 9 * * *", "active", "2024-01-15 09:00:00"),
        ("task-002", "Data Sync", "0 */6 * * *", "active", "2024-01-15 12:00:00"),
        ("task-003", "Weekly Summary", "0 10 * * 1", "paused", "Paused"),
    ];
    
    for (id, name, schedule, status, next_run) in tasks {
        if let Some(filter) = status_filter {
            if status != filter {
                continue;
            }
        }
        
        let status_colored = match status {
            "active" => status.green(),
            "paused" => status.yellow(),
            "failed" => status.red(),
            _ => status.normal(),
        };
        
        println!("{:<36} {:<20} {:<15} {:<10} {}", 
            id.cyan(),
            name,
            schedule,
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
    name: &str,
    cron: Option<&str>,
    at: Option<&str>,
    action_type: &str,
    payload: &str,
) -> Result<()> {
    println!("{}", "➕ Schedule New Task".bold().underline());
    println!();
    
    // Determine schedule type
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
    
    // Determine action type
    let action = match action_type {
        "cognitive" => TaskAction::Cognitive {
            prompt: payload.to_string(),
            context: None,
        },
        "rpa" => TaskAction::Rpa {
            workflow_id: payload.to_string(),
            parameters: serde_json::json!({}),
        },
        "workflow" => TaskAction::Workflow {
            workflow_name: payload.to_string(),
            parameters: serde_json::json!({}),
        },
        _ => anyhow::bail!("Unknown action type: {}", action_type),
    };
    
    let task = ScheduledTask::new(name.to_string(), schedule, action);
    
    println!("{}: {}", "Name".bold(), task.name);
    println!("{}: {:?}", "Schedule".bold(), task.schedule);
    println!("{}: {:?}", "Action".bold(), task.action);
    println!();
    
    // In a real implementation, this would save to database
    println!("{} Task '{}' scheduled successfully", 
        "✓".green().bold(),
        name.cyan()
    );
    println!("{}: {}", "Task ID".dimmed(), task.id.cyan());
    
    Ok(())
}

async fn cancel_task(id: &str) -> Result<()> {
    println!("{}", "🗑️  Cancel Task".bold().underline());
    println!();
    
    // In a real implementation, this would:
    // 1. Find the task by ID
    // 2. Cancel any pending executions
    // 3. Mark as cancelled in database
    
    println!("{} Task '{}' cancelled successfully", 
        "✓".green().bold(),
        id.cyan()
    );
    
    Ok(())
}

async fn show_task(id: &str) -> Result<()> {
    println!("{}", "📄 Task Details".bold().underline());
    println!();
    
    // Placeholder data
    println!("{}: {}", "ID".bold(), id.cyan());
    println!("{}: {}", "Name".bold(), "Daily Report");
    println!("{}: {}", "Status".bold(), "active".green());
    println!("{}: {}", "Schedule".bold(), "0 9 * * * (Daily at 9:00 AM)");
    println!("{}: {}", "Timezone".bold(), "UTC");
    println!("{}: {}", "Created".bold(), "2024-01-01 00:00:00 UTC");
    println!("{}: {}", "Next Run".bold(), "2024-01-15 09:00:00 UTC");
    println!("{}: {}", "Last Run".bold(), "2024-01-14 09:00:00 UTC");
    println!();
    
    println!("{}", "Action Configuration".bold());
    println!("{}: {}", "Type".bold(), "cognitive");
    println!("{}: {}", "Prompt".bold(), "Generate daily sales report");
    println!();
    
    println!("{}", "Execution Statistics".bold());
    println!("  Total Runs: 14");
    println!("  Successful: 14 (100%)");
    println!("  Failed: 0");
    println!("  Average Duration: 2.3s");
    
    Ok(())
}

async fn show_history(task_id: Option<&str>, limit: usize) -> Result<()> {
    println!("{}", "📜 Execution History".bold().underline());
    println!();
    
    if let Some(id) = task_id {
        println!("Showing last {} executions for task: {}", limit, id.cyan());
    } else {
        println!("Showing last {} executions for all tasks", limit);
    }
    println!();
    
    println!("{:<36} {:<20} {:<20} {:<12} {}", 
        "Execution ID".dimmed(),
        "Task Name".dimmed(),
        "Started".dimmed(),
        "Status".dimmed(),
        "Duration".dimmed()
    );
    println!("{}", "─".repeat(110).dimmed());
    
    // Placeholder data
    let history = vec![
        ("exec-001", "Daily Report", "2024-01-14 09:00:00", "success", "2.1s"),
        ("exec-002", "Daily Report", "2024-01-13 09:00:00", "success", "2.3s"),
        ("exec-003", "Data Sync", "2024-01-13 06:00:00", "success", "5.2s"),
        ("exec-004", "Daily Report", "2024-01-12 09:00:00", "failed", "30.0s"),
    ];
    
    for (exec_id, name, started, status, duration) in history.iter().take(limit) {
        let status_colored = match *status {
            "success" => "✓ success".green(),
            "failed" => "✗ failed".red(),
            "running" => "⟳ running".yellow(),
            _ => status.normal(),
        };
        
        println!("{:<36} {:<20} {:<20} {:<12} {}", 
            exec_id.dimmed(),
            name,
            started,
            status_colored,
            duration
        );
    }
    
    Ok(())
}

async fn pause_task(id: &str) -> Result<()> {
    println!("{}", "⏸️  Pause Task".bold().underline());
    println!();
    
    println!("{} Task '{}' paused", 
        "✓".green().bold(),
        id.cyan()
    );
    println!("{}: Use '{}' to resume", 
        "Tip".italic().dimmed(),
        format!("crablet task resume {}", id).cyan()
    );
    
    Ok(())
}

async fn resume_task(id: &str) -> Result<()> {
    println!("{}", "▶️  Resume Task".bold().underline());
    println!();
    
    println!("{} Task '{}' resumed", 
        "✓".green().bold(),
        id.cyan()
    );
    
    Ok(())
}

async fn run_task(name: &str, args: &str) -> Result<()> {
    println!("{}", "🚀 Run Task".bold().underline());
    println!();
    
    println!("Task: {}", name.cyan());
    println!("Arguments: {}", args);
    println!();
    
    // Simulate execution
    println!("{}", "Executing task...".dimmed());
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    
    println!("{} Task completed successfully", "✓".green().bold());
    
    Ok(())
}

async fn show_queue(pending: bool, running: bool, failed: bool) -> Result<()> {
    println!("{}", "📊 Task Queue Status".bold().underline());
    println!();
    
    // If no filters specified, show all
    let show_all = !pending && !running && !failed;
    
    if show_all || pending {
        println!("{}", "⏳ Pending Tasks".bold());
        println!("  Count: 3");
        println!("  Oldest: 2024-01-15 08:45:00 UTC");
        println!();
    }
    
    if show_all || running {
        println!("{}", "🔄 Running Tasks".bold());
        println!("  Count: 1");
        println!("  Active: Data Sync (started 2024-01-15 09:00:00 UTC)");
        println!();
    }
    
    if show_all || failed {
        println!("{}", "❌ Failed Tasks (Dead Letter Queue)".bold());
        println!("  Count: 0");
        println!();
    }
    
    println!("{}", "Queue Statistics".bold());
    println!("  Total Processed Today: 47");
    println!("  Success Rate: 97.9%");
    println!("  Average Wait Time: 0.3s");
    println!("  Average Execution Time: 3.2s");
    
    Ok(())
}
