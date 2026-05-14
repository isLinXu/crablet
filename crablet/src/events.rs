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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub session_id: Option<String>,
    pub user_id: Option<String>,
    pub payload: AgentEvent,
    pub timestamp: DateTime<Utc>,
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
        // Only publish real events to broadcast channel, persistence is async
        let evt = Event {
            session_id: None,
            user_id: None,
            payload: event.clone(),
            timestamp: Utc::now(),
        };
        let _ = self.sender.send(evt);

        self.persist(None, None, event);
    }

    pub fn publish_contextual(
        &self,
        session_id: Option<String>,
        user_id: Option<String>,
        payload: AgentEvent,
    ) {
        let event = Event {
            session_id: session_id.clone(),
            user_id: user_id.clone(),
            payload: payload.clone(),
            timestamp: Utc::now(),
        };

        // 1. Broadcast to real-time subscribers
        let _ = self.sender.send(event.clone());

        // 2. Persist asynchronously
        self.persist(session_id, user_id, payload);
    }

    fn persist(&self, session_id: Option<String>, user_id: Option<String>, payload: AgentEvent) {
        if let Some(pool) = &self.pool {
            let pool = pool.clone();

            tokio::spawn(async move {
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
                };

                let payload_json = serde_json::to_string(&payload).unwrap_or_default();
                let session_str = session_id.unwrap_or_else(|| "global".to_string());
                let user_str = user_id.unwrap_or_else(|| "anonymous".to_string());
                let index_fields = event_index_fields(&payload);

                // Use chrono for timestamp
                let now = Utc::now();

                if let Err(e) = sqlx::query("INSERT INTO event_log (session_id, user_id, event_type, payload, graph_id, task_id, event_timestamp_ms, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
                    .bind(session_str)
                    .bind(user_str)
                    .bind(event_type)
                    .bind(payload_json)
                    .bind(index_fields.graph_id)
                    .bind(index_fields.task_id)
                    .bind(index_fields.event_timestamp_ms)
                    .bind(now)
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
            let rows = sqlx::query("SELECT session_id, user_id, payload, created_at FROM event_log WHERE session_id = ? ORDER BY id ASC")
                .bind(session_id)
                .fetch_all(pool)
                .await;

            match rows {
                Ok(rows) => rows
                    .into_iter()
                    .filter_map(|row| {
                        let payload_str: String = row.get("payload");
                        let payload: AgentEvent = serde_json::from_str(&payload_str).ok()?;
                        Some(Event {
                            session_id: row.get("session_id"),
                            user_id: row.get("user_id"),
                            payload,
                            timestamp: row.get("created_at"),
                        })
                    })
                    .collect(),
                Err(e) => {
                    warn!("Replay failed: {}", e);
                    vec![]
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
}
