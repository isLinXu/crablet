ALTER TABLE event_log ADD COLUMN schema_version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE event_log ADD COLUMN event_id TEXT;
ALTER TABLE event_log ADD COLUMN run_id TEXT;
ALTER TABLE event_log ADD COLUMN agent_id TEXT;
ALTER TABLE event_log ADD COLUMN step_id TEXT;
ALTER TABLE event_log ADD COLUMN tool_id TEXT;
ALTER TABLE event_log ADD COLUMN span_id TEXT;

-- Preserve replay identity for rows written before the envelope columns existed.
UPDATE event_log
SET event_id = lower(hex(randomblob(16)))
WHERE event_id IS NULL;

UPDATE event_log
SET run_id = session_id
WHERE run_id IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_event_log_event_id
    ON event_log(event_id)
    WHERE event_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_event_log_run_id
    ON event_log(run_id, id);
