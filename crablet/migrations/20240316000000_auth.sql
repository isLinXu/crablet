CREATE TABLE IF NOT EXISTS api_keys (
    id TEXT PRIMARY KEY,
    key_hash TEXT NOT NULL,  -- Storing hashed key (argon2)
    key_prefix TEXT NOT NULL, -- Storing first 8 chars for display
    user_id TEXT NOT NULL,
    name TEXT,
    created_at INTEGER NOT NULL,
    last_used_at INTEGER,
    status TEXT DEFAULT 'active'
);

CREATE INDEX IF NOT EXISTS idx_api_keys_user 
  ON api_keys(user_id);
