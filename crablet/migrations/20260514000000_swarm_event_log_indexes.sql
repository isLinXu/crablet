ALTER TABLE event_log ADD COLUMN graph_id TEXT;
ALTER TABLE event_log ADD COLUMN task_id TEXT;
ALTER TABLE event_log ADD COLUMN event_timestamp_ms INTEGER;

UPDATE event_log
SET
    graph_id = CASE event_type
        WHEN 'SwarmActivity' THEN json_extract(payload, '$.SwarmActivity.graph_id')
        WHEN 'SwarmGraphUpdate' THEN json_extract(payload, '$.SwarmGraphUpdate.graph_id')
        WHEN 'SwarmTaskUpdate' THEN json_extract(payload, '$.SwarmTaskUpdate.graph_id')
        WHEN 'SwarmLog' THEN json_extract(payload, '$.SwarmLog.graph_id')
        ELSE graph_id
    END,
    task_id = CASE event_type
        WHEN 'SwarmActivity' THEN json_extract(payload, '$.SwarmActivity.task_id')
        WHEN 'SwarmTaskUpdate' THEN json_extract(payload, '$.SwarmTaskUpdate.task_id')
        WHEN 'SwarmLog' THEN json_extract(payload, '$.SwarmLog.task_id')
        ELSE task_id
    END,
    event_timestamp_ms = CASE event_type
        WHEN 'SwarmActivity' THEN CAST(json_extract(payload, '$.SwarmActivity.timestamp') AS INTEGER)
        WHEN 'SwarmGraphUpdate' THEN CAST(json_extract(payload, '$.SwarmGraphUpdate.timestamp') AS INTEGER)
        WHEN 'SwarmTaskUpdate' THEN CAST(json_extract(payload, '$.SwarmTaskUpdate.timestamp') AS INTEGER)
        WHEN 'SwarmLog' THEN CAST(json_extract(payload, '$.SwarmLog.timestamp') AS INTEGER)
        ELSE event_timestamp_ms
    END
WHERE event_type IN ('SwarmActivity', 'SwarmGraphUpdate', 'SwarmTaskUpdate', 'SwarmLog');

CREATE INDEX IF NOT EXISTS idx_event_log_swarm_graph_ts
    ON event_log(graph_id, event_timestamp_ms DESC);

CREATE INDEX IF NOT EXISTS idx_event_log_swarm_graph_task_ts
    ON event_log(graph_id, task_id, event_timestamp_ms DESC);
