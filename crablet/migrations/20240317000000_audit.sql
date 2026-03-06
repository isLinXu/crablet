CREATE TABLE IF NOT EXISTS swarm_logs (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    from_agent TEXT NOT NULL,
    to_agent TEXT NOT NULL,
    message_type TEXT NOT NULL,
    content TEXT,
    timestamp INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_swarm_logs_task 
  ON swarm_logs(task_id);

CREATE INDEX IF NOT EXISTS idx_swarm_logs_ts 
  ON swarm_logs(timestamp DESC);
