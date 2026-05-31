---
title: Cron Scheduling
description: Schedule recurring agent tasks
---

# :clock4: Cron Scheduling

Crablet supports cron-style scheduling for recurring tasks — daily summaries, periodic checks, automated reports — without manual intervention.

## Basic Usage

```bash
# Schedule a daily summary
crablet cron add "0 9 * * *" "Summarize yesterday's news about AI"

# Schedule hourly monitoring
crablet cron add "0 * * * *" "Check server health and report anomalies"

# List all scheduled tasks
crablet cron list

# Remove a task
crablet cron remove <task-id>
```

## Cron Expression Format

```
┌───────── minute (0-59)
│ ┌───────── hour (0-23)
│ │ ┌───────── day of month (1-31)
│ │ │ ┌───────── month (1-12)
│ │ │ │ ┌───────── day of week (0-6, Sun=0)
│ │ │ │ │
* * * * *
```

### Common Patterns

| Expression | Meaning |
|:-----------|:--------|
| `*/5 * * * *` | Every 5 minutes |
| `0 * * * *` | Every hour |
| `0 9 * * *` | Daily at 9 AM |
| `0 9 * * 1` | Every Monday at 9 AM |
| `0 0 1 * *` | First day of each month |

## Configuration

```toml
[cron]
enabled = true
timezone = "Asia/Shanghai"
max_concurrent = 5
retry_on_failure = true
retry_delay_seconds = 300
```
