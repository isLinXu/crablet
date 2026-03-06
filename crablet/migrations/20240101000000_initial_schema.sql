-- Sessions
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    channel TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    last_active INTEGER NOT NULL,
    message_count INTEGER DEFAULT 0
);

-- Messages
CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    tokens INTEGER,
    latency_ms INTEGER,
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);

CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id);
CREATE INDEX IF NOT EXISTS idx_messages_timestamp ON messages(timestamp);

-- Episodic Memory
CREATE TABLE IF NOT EXISTS episodic_events (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    content TEXT NOT NULL,
    embedding BLOB,
    timestamp INTEGER NOT NULL,
    importance REAL DEFAULT 0.5
);

CREATE INDEX IF NOT EXISTS idx_episodic_session ON episodic_events(session_id);

-- Swarm Core
CREATE TABLE IF NOT EXISTS swarm_graphs (
    id TEXT PRIMARY KEY,
    goal TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_swarm_graphs_status ON swarm_graphs(status);

CREATE TABLE IF NOT EXISTS swarm_tasks (
    id TEXT PRIMARY KEY,
    graph_id TEXT NOT NULL,
    agent_role TEXT NOT NULL,
    prompt TEXT NOT NULL,
    dependencies TEXT NOT NULL, -- JSON array of task_ids
    status TEXT NOT NULL, -- JSON object
    result TEXT,
    logs TEXT, -- JSON array
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (graph_id) REFERENCES swarm_graphs(id)
);

CREATE INDEX IF NOT EXISTS idx_swarm_tasks_graph ON swarm_tasks(graph_id);

CREATE TABLE IF NOT EXISTS swarm_templates (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    graph_json TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

-- Event Log (Unified from events.rs)
CREATE TABLE IF NOT EXISTS event_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT,
    user_id TEXT,
    event_type TEXT,
    payload JSON,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_event_log_session ON event_log(session_id);
CREATE INDEX IF NOT EXISTS idx_event_log_type ON event_log(event_type);
CREATE INDEX IF NOT EXISTS idx_event_log_created_at ON event_log(created_at);

-- API Keys
CREATE TABLE IF NOT EXISTS api_keys (
    id TEXT PRIMARY KEY,
    key_hash TEXT NOT NULL,
    key_prefix TEXT NOT NULL,
    user_id TEXT NOT NULL,
    name TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    status TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_api_keys_prefix ON api_keys(key_prefix);
CREATE INDEX IF NOT EXISTS idx_api_keys_user ON api_keys(user_id);

-- Swarm Audit Logs
CREATE TABLE IF NOT EXISTS swarm_logs (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    from_agent TEXT NOT NULL,
    to_agent TEXT NOT NULL,
    message_type TEXT NOT NULL,
    content TEXT NOT NULL,
    timestamp INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_swarm_logs_timestamp ON swarm_logs(timestamp);
CREATE INDEX IF NOT EXISTS idx_swarm_logs_task ON swarm_logs(task_id);
