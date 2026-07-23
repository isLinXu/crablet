use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use tokio::sync::broadcast;
use tracing::warn;
// use std::fmt;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum AgentEvent {
    UserInput(String),
    SystemLog(String),
    ThoughtGenerated(String),
    ToolExecutionStarted {
        tool: String,
        args: String,
    },
    ToolExecutionFinished {
        tool: String,
        output: String,
    },
    CanvasUpdate {
        title: String,
        content: String,
        kind: String,
    }, // kind: markdown, mermaid, code, html
    SwarmActivity {
        task_id: String,
        graph_id: String,
        from: String,
        to: String,
        message_type: String,
        content: String,
        timestamp: i64,
    },
    SwarmGraphUpdate {
        graph_id: String,
        status: String, // "Active", "Paused", "Completed", "Failed"
        timestamp: i64,
    },
    SwarmTaskUpdate {
        graph_id: String,
        task_id: String,
        status: String, // "Pending", "Running", "Completed", "Failed", "Paused"
        result: Option<String>,
        timestamp: i64,
    },
    SwarmLog {
        graph_id: String,
        task_id: String,
        content: String,
        timestamp: i64,
    },
    GraphRagEntityModeChanged {
        from_mode: String,
        to_mode: String,
    },
    ResponseGenerated(String),
    CognitiveLayerChanged {
        layer: String,
    },
    Error(String),
    // Heartbeat events for Always-On Memory Agent
    Heartbeat {
        timestamp: DateTime<Utc>,
        active_sessions: usize,
    },
    BackgroundThinkingTriggered {
        reason: String,
        context_summary: String,
    },
    BackgroundThinkingResult {
        insights: String,
        suggested_actions: Vec<String>,
        memories_updated: Vec<String>,
    },
    CoreMemoryUpdated {
        block: String,
        operation: String,
        timestamp: DateTime<Utc>,
    },
    /// High-confidence insight from BackgroundThinker published as a learning signal.
    /// OnlineLearner and other adaptive subsystems can subscribe to this event to
    /// incorporate the insight as a training experience (closing the reflection→learning loop).
    InsightLearningSignal {
        /// Unique insight identifier
        insight_id: String,
        /// Human-readable insight type (e.g. "UserPreference", "BehaviorPattern")
        insight_type: String,
        /// Textual content of the insight
        content: String,
        /// Confidence score [0.0, 1.0]
        confidence: f32,
        /// Source session IDs that contributed to this insight
        source_sessions: Vec<String>,
        /// When the insight was generated
        generated_at: DateTime<Utc>,
    },
}

pub const RUNTIME_EVENT_SCHEMA_VERSION: u16 = 1;

/// Stable, serializable envelope shared by live delivery and persistence.
/// Legacy `session_id`/`user_id` fields remain available for API compatibility.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RuntimeEventEnvelope {
    #[serde(default = "runtime_event_schema_version")]
    pub schema_version: u16,
    #[serde(default = "new_event_id")]
    pub event_id: String,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub step_id: Option<String>,
    #[serde(default)]
    pub tool_id: Option<String>,
    #[serde(default)]
    pub span_id: Option<String>,
    pub session_id: Option<String>,
    pub user_id: Option<String>,
    pub payload: AgentEvent,
    pub timestamp: DateTime<Utc>,
}

pub type Event = RuntimeEventEnvelope;

fn runtime_event_schema_version() -> u16 {
    RUNTIME_EVENT_SCHEMA_VERSION
}

fn new_event_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn legacy_event_id(row_id: i64) -> String {
    format!("legacy-{row_id}")
}

/// Correlation coordinates supplied by a concrete execution chain.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeEventContext {
    pub run_id: Option<String>,
    pub agent_id: Option<String>,
    pub step_id: Option<String>,
    pub tool_id: Option<String>,
    pub span_id: Option<String>,
    pub session_id: Option<String>,
    pub user_id: Option<String>,
}

impl RuntimeEventEnvelope {
    pub fn new(payload: AgentEvent, context: RuntimeEventContext) -> Self {
        Self {
            schema_version: RUNTIME_EVENT_SCHEMA_VERSION,
            event_id: new_event_id(),
            run_id: context.run_id.or_else(|| context.session_id.clone()),
            agent_id: context.agent_id,
            step_id: context.step_id,
            tool_id: context.tool_id,
            span_id: context.span_id,
            session_id: context.session_id,
            user_id: context.user_id,
            payload,
            timestamp: Utc::now(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct EventIndexFields {
    graph_id: Option<String>,
    task_id: Option<String>,
    event_timestamp_ms: Option<i64>,
}

fn event_index_fields(payload: &AgentEvent) -> EventIndexFields {
    match payload {
        AgentEvent::SwarmActivity {
            graph_id,
            task_id,
            timestamp,
            ..
        }
        | AgentEvent::SwarmLog {
            graph_id,
            task_id,
            timestamp,
            ..
        }
        | AgentEvent::SwarmTaskUpdate {
            graph_id,
            task_id,
            timestamp,
            ..
        } => EventIndexFields {
            graph_id: Some(graph_id.clone()),
            task_id: Some(task_id.clone()),
            event_timestamp_ms: Some(*timestamp),
        },
        AgentEvent::SwarmGraphUpdate {
            graph_id,
            timestamp,
            ..
        } => EventIndexFields {
            graph_id: Some(graph_id.clone()),
            task_id: None,
            event_timestamp_ms: Some(*timestamp),
        },
        _ => EventIndexFields::default(),
    }
}

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<Event>,
    pool: Option<SqlitePool>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender, pool: None }
    }

    pub fn with_pool(mut self, pool: SqlitePool) -> Self {
        self.pool = Some(pool);
        self
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }

    pub fn publish(&self, event: AgentEvent) {
        self.publish_runtime(RuntimeEventContext::default(), event);
    }

    pub fn publish_contextual(
        &self,
        session_id: Option<String>,
        user_id: Option<String>,
        payload: AgentEvent,
    ) {
        self.publish_runtime(
            RuntimeEventContext {
                session_id,
                user_id,
                ..RuntimeEventContext::default()
            },
            payload,
        );
    }

    /// Publish an event with stable runtime correlation identifiers.
    pub fn publish_runtime(&self, context: RuntimeEventContext, payload: AgentEvent) {
        let event = RuntimeEventEnvelope::new(payload.clone(), context);
        let _ = self.sender.send(event.clone());
        self.persist(event);
    }

    fn persist(&self, event: RuntimeEventEnvelope) {
        if let Some(pool) = &self.pool {
            let pool = pool.clone();

            tokio::spawn(async move {
                let payload = event.payload.clone();
                let event_type = match &payload {
                    AgentEvent::UserInput(_) => "UserInput",
                    AgentEvent::SystemLog(_) => "SystemLog",
                    AgentEvent::ThoughtGenerated(_) => "ThoughtGenerated",
                    AgentEvent::ToolExecutionStarted { .. } => "ToolExecutionStarted",
                    AgentEvent::ToolExecutionFinished { .. } => "ToolExecutionFinished",
                    AgentEvent::CanvasUpdate { .. } => "CanvasUpdate",
                    AgentEvent::SwarmActivity { .. } => "SwarmActivity",
                    AgentEvent::SwarmGraphUpdate { .. } => "SwarmGraphUpdate",
                    AgentEvent::SwarmTaskUpdate { .. } => "SwarmTaskUpdate",
                    AgentEvent::SwarmLog { .. } => "SwarmLog",
                    AgentEvent::GraphRagEntityModeChanged { .. } => "GraphRagEntityModeChanged",
                    AgentEvent::ResponseGenerated(_) => "ResponseGenerated",
                    AgentEvent::CognitiveLayerChanged { .. } => "CognitiveLayerChanged",
                    AgentEvent::Error(_) => "Error",
                    AgentEvent::Heartbeat { .. } => "Heartbeat",
                    AgentEvent::BackgroundThinkingTriggered { .. } => "BackgroundThinkingTriggered",
                    AgentEvent::BackgroundThinkingResult { .. } => "BackgroundThinkingResult",
                    AgentEvent::CoreMemoryUpdated { .. } => "CoreMemoryUpdated",
                    AgentEvent::InsightLearningSignal { .. } => "InsightLearningSignal",
                };

                let payload_json = serde_json::to_string(&payload).unwrap_or_default();
                let session_str = event
                    .session_id
                    .clone()
                    .unwrap_or_else(|| "global".to_string());
                let user_str = event
                    .user_id
                    .clone()
                    .unwrap_or_else(|| "anonymous".to_string());
                let index_fields = event_index_fields(&payload);

                if let Err(e) = sqlx::query("INSERT INTO event_log (session_id, user_id, event_type, payload, graph_id, task_id, event_timestamp_ms, created_at, schema_version, event_id, run_id, agent_id, step_id, tool_id, span_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
                    .bind(session_str)
                    .bind(user_str)
                    .bind(event_type)
                    .bind(payload_json)
                    .bind(index_fields.graph_id)
                    .bind(index_fields.task_id)
                    .bind(index_fields.event_timestamp_ms)
                    .bind(event.timestamp)
                    .bind(event.schema_version as i64)
                    .bind(event.event_id)
                    .bind(event.run_id)
                    .bind(event.agent_id)
                    .bind(event.step_id)
                    .bind(event.tool_id)
                    .bind(event.span_id)
                    .execute(&pool)
                    .await
                {
                    warn!("Failed to persist event: {}", e);
                }
            });
        }
    }

    // Replay capability
    pub async fn replay(&self, session_id: &str) -> Vec<Event> {
        if let Some(pool) = &self.pool {
            let rows = sqlx::query("SELECT id, session_id, user_id, payload, created_at, schema_version, event_id, run_id, agent_id, step_id, tool_id, span_id FROM event_log WHERE session_id = ? ORDER BY id ASC")
                .bind(session_id)
                .fetch_all(pool)
                .await;

            match rows {
                Ok(rows) => rows
                    .into_iter()
                    .filter_map(|row| {
                        let row_id: i64 = row.get("id");
                        let payload_str: String = row.get("payload");
                        let payload: AgentEvent = serde_json::from_str(&payload_str).ok()?;
                        let session_id: Option<String> = row.get("session_id");
                        Some(Event {
                            schema_version: row
                                .try_get::<i64, _>("schema_version")
                                .ok()
                                .map(|version| version as u16)
                                .unwrap_or(RUNTIME_EVENT_SCHEMA_VERSION),
                            event_id: row
                                .try_get::<String, _>("event_id")
                                .ok()
                                .filter(|id| !id.is_empty())
                                .unwrap_or_else(|| legacy_event_id(row_id)),
                            run_id: row.try_get("run_id").ok().or_else(|| session_id.clone()),
                            agent_id: row.try_get("agent_id").ok(),
                            step_id: row.try_get("step_id").ok(),
                            tool_id: row.try_get("tool_id").ok(),
                            span_id: row.try_get("span_id").ok(),
                            session_id,
                            user_id: row.get("user_id"),
                            payload,
                            timestamp: row.get("created_at"),
                        })
                    })
                    .collect(),
                Err(e) => {
                    // A caller may open a database created before the envelope
                    // migration. Keep replay useful until the next migration
                    // pass by reading the original event columns.
                    warn!("Envelope replay query failed, trying legacy schema: {}", e);
                    let legacy_rows = sqlx::query(
                        "SELECT id, session_id, user_id, payload, created_at FROM event_log WHERE session_id = ? ORDER BY id ASC",
                    )
                    .bind(session_id)
                    .fetch_all(pool)
                    .await;
                    match legacy_rows {
                        Ok(rows) => rows
                            .into_iter()
                            .filter_map(|row| {
                                let row_id: i64 = row.get("id");
                                let payload_str: String = row.get("payload");
                                let payload: AgentEvent =
                                    serde_json::from_str(&payload_str).ok()?;
                                let session_id: Option<String> = row.get("session_id");
                                Some(Event {
                                    schema_version: RUNTIME_EVENT_SCHEMA_VERSION,
                                    event_id: legacy_event_id(row_id),
                                    run_id: session_id.clone(),
                                    agent_id: None,
                                    step_id: None,
                                    tool_id: None,
                                    span_id: None,
                                    session_id,
                                    user_id: row.get("user_id"),
                                    payload,
                                    timestamp: row.get("created_at"),
                                })
                            })
                            .collect(),
                        Err(legacy_error) => {
                            warn!("Legacy replay failed: {}", legacy_error);
                            vec![]
                        }
                    }
                }
            }
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;
    #[allow(unused_imports)]
    use std::time::Duration;

    #[tokio::test]
    async fn test_event_bus_broadcast() {
        let bus = EventBus::new(10);
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        let event = AgentEvent::SystemLog("Test Message".to_string());
        bus.publish(event.clone());

        // Both subscribers should receive the event
        let evt1 = rx1.recv().await.unwrap();
        let evt2 = rx2.recv().await.unwrap();

        assert_eq!(evt1.payload, event);
        assert_eq!(evt2.payload, event);
    }

    #[tokio::test]
    async fn test_event_bus_dropped_receiver() {
        let bus = EventBus::new(10);
        let mut rx1 = bus.subscribe();
        {
            let _rx2 = bus.subscribe();
            // rx2 dropped here
        }

        bus.publish(AgentEvent::SystemLog("Msg".to_string()));

        // rx1 should still work
        let evt = rx1.recv().await.unwrap();
        if let AgentEvent::SystemLog(msg) = evt.payload {
            assert_eq!(msg, "Msg");
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_runtime_event_envelope_round_trip_and_ids() {
        let context = RuntimeEventContext {
            run_id: Some("run-1".into()),
            agent_id: Some("agent-1".into()),
            step_id: Some("step-1".into()),
            tool_id: Some("tool-1".into()),
            span_id: Some("span-1".into()),
            session_id: Some("session-1".into()),
            user_id: Some("user-1".into()),
        };
        let envelope = RuntimeEventEnvelope::new(
            AgentEvent::ToolExecutionStarted {
                tool: "search".into(),
                args: "{}".into(),
            },
            context,
        );

        let json = serde_json::to_string(&envelope).unwrap();
        let decoded: RuntimeEventEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, envelope);
        assert_eq!(decoded.schema_version, RUNTIME_EVENT_SCHEMA_VERSION);
        assert_eq!(decoded.run_id.as_deref(), Some("run-1"));
        assert_eq!(decoded.agent_id.as_deref(), Some("agent-1"));
        assert_eq!(decoded.step_id.as_deref(), Some("step-1"));
        assert_eq!(decoded.tool_id.as_deref(), Some("tool-1"));
        assert_eq!(decoded.span_id.as_deref(), Some("span-1"));
        assert!(!decoded.event_id.is_empty());
    }

    #[tokio::test]
    async fn test_publish_contextual_maps_session_to_run_id() {
        let bus = EventBus::new(10);
        let mut receiver = bus.subscribe();
        bus.publish_contextual(
            Some("session-1".into()),
            Some("user-1".into()),
            AgentEvent::SystemLog("ready".into()),
        );

        let event = receiver.recv().await.unwrap();
        assert_eq!(event.session_id.as_deref(), Some("session-1"));
        assert_eq!(event.run_id.as_deref(), Some("session-1"));
        assert_eq!(event.schema_version, RUNTIME_EVENT_SCHEMA_VERSION);
    }

    #[test]
    fn test_event_index_fields_extract_swarm_coordinates() {
        let event = AgentEvent::SwarmTaskUpdate {
            graph_id: "graph-1".to_string(),
            task_id: "task-1".to_string(),
            status: "Running".to_string(),
            result: None,
            timestamp: 42,
        };

        let fields = event_index_fields(&event);
        assert_eq!(fields.graph_id.as_deref(), Some("graph-1"));
        assert_eq!(fields.task_id.as_deref(), Some("task-1"));
        assert_eq!(fields.event_timestamp_ms, Some(42));
    }

    #[test]
    fn test_event_index_fields_ignore_non_swarm_events() {
        let fields = event_index_fields(&AgentEvent::SystemLog("hello".to_string()));
        assert_eq!(fields, EventIndexFields::default());
    }

    #[tokio::test]
    async fn test_sqlite_replay_preserves_envelope_identity_and_coordinates() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE event_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT,
                user_id TEXT,
                event_type TEXT,
                payload JSON,
                created_at DATETIME,
                schema_version INTEGER NOT NULL DEFAULT 1,
                event_id TEXT,
                run_id TEXT,
                agent_id TEXT,
                step_id TEXT,
                tool_id TEXT,
                span_id TEXT,
                graph_id TEXT,
                task_id TEXT,
                event_timestamp_ms INTEGER
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        let bus = EventBus::new(4).with_pool(pool.clone());
        bus.publish_runtime(
            RuntimeEventContext {
                run_id: Some("run-42".into()),
                agent_id: Some("agent-7".into()),
                step_id: Some("step-3".into()),
                tool_id: Some("search".into()),
                span_id: Some("span-9".into()),
                session_id: Some("session-42".into()),
                user_id: Some("user-1".into()),
            },
            AgentEvent::SystemLog("persist me".into()),
        );
        tokio::time::sleep(Duration::from_millis(50)).await;

        let first = bus.replay("session-42").await;
        let second = bus.replay("session-42").await;
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].event_id, second[0].event_id);
        assert_eq!(first[0].run_id.as_deref(), Some("run-42"));
        assert_eq!(first[0].agent_id.as_deref(), Some("agent-7"));
        assert_eq!(first[0].step_id.as_deref(), Some("step-3"));
        assert_eq!(first[0].tool_id.as_deref(), Some("search"));
        assert_eq!(first[0].span_id.as_deref(), Some("span-9"));
    }

    #[tokio::test]
    async fn test_sqlite_replay_legacy_rows_get_deterministic_identity() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE event_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT,
                user_id TEXT,
                event_type TEXT,
                payload JSON,
                created_at DATETIME
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO event_log (session_id, user_id, event_type, payload, created_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind("legacy-session")
        .bind("legacy-user")
        .bind("SystemLog")
        .bind(serde_json::to_string(&AgentEvent::SystemLog("old".into())).unwrap())
        .bind(chrono::Utc::now())
        .execute(&pool)
        .await
        .unwrap();

        let events = EventBus::new(4)
            .with_pool(pool)
            .replay("legacy-session")
            .await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_id, "legacy-1");
        assert_eq!(events[0].run_id.as_deref(), Some("legacy-session"));
    }
}
