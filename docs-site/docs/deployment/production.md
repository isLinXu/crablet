---
title: Production
description: Production best practices, databases, and scaling
---

# :factory: Production Deployment

## Database Selection

| Database | Use Case | Performance |
|:---------|:---------|:------------|
| **SQLite** | Development, small deployments | Good for < 100 concurrent users |
| **PostgreSQL** | Production, multi-user | Recommended for production |
| **MySQL** | Enterprise environments | Supported via Diesel ORM |

## Production Configuration

```toml
# config.toml (production)
database_url = "postgresql://user:pass@localhost/crablet"
log_level = "warn"

[safety]
level = "Strict"
allowed_commands = ["ls", "cat", "grep", "find", "git", "cargo"]
blocked_commands = ["rm -rf", "mkfs", "dd", "format"]

[telemetry]
enabled = true
endpoint = "http://tempo:4317"

[limits]
max_concurrent_requests = 100
request_timeout = 30

[cache]
enabled = true
ttl_seconds = 3600
max_entries = 10000
```

## Scaling Considerations

### Vertical Scaling

- Increase `max_concurrent_requests` based on CPU cores
- Tune `tokio` worker threads: `TOKIO_WORKER_THREADS=8`
- Allocate sufficient memory for vector indices

### Horizontal Scaling

- Use PostgreSQL for shared state
- Deploy behind a load balancer (nginx/HAProxy)
- Sticky sessions for WebSocket connections
- Shared storage for skills and knowledge bases

## Backup Strategy

```bash
# Backup database
pg_dump crablet > backup_$(date +%Y%m%d).sql

# Backup Neo4j
neo4j-admin database dump neo4j --to=/backups/neo4j_$(date +%Y%m%d).dump

# Backup configuration
tar czf config_backup.tar.gz ~/.config/crablet/
```
