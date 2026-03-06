-- Add Swarm Graph Table
CREATE TABLE IF NOT EXISTS swarm_graphs (
    id TEXT PRIMARY KEY,
    goal TEXT NOT NULL,
    status TEXT NOT NULL, -- Active, Paused, Completed, Failed
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Add Swarm Tasks Table
CREATE TABLE IF NOT EXISTS swarm_tasks (
    id TEXT PRIMARY KEY,
    graph_id TEXT NOT NULL,
    agent_role TEXT NOT NULL,
    prompt TEXT NOT NULL,
    dependencies TEXT NOT NULL, -- JSON array of task IDs
    status TEXT NOT NULL, -- JSON object: {"Running": ...} or string "Pending"
    result TEXT,
    logs TEXT NOT NULL DEFAULT '[]', -- JSON array of strings
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(graph_id) REFERENCES swarm_graphs(id) ON DELETE CASCADE
);

CREATE INDEX idx_swarm_tasks_graph_id ON swarm_tasks(graph_id);
