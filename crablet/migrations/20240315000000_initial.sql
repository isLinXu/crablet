CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    channel TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    last_active INTEGER NOT NULL,
    message_count INTEGER DEFAULT 0
);

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

CREATE INDEX IF NOT EXISTS idx_messages_session_ts 
  ON messages(session_id, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_sessions_user 
  ON sessions(user_id);

CREATE TABLE IF NOT EXISTS document_embeddings (
    id TEXT PRIMARY KEY,
    content TEXT NOT NULL,
    embedding JSON NOT NULL,
    metadata JSON
);
