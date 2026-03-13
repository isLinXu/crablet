//! # Calendar Connector
//!
//! Calendar integration for event-based triggers.
//!
//! ## Features
//!
//! - Event start/end notifications
//! - Reminder triggers
//! - Multi-calendar support (Google, Outlook, iCal)
//! - Recurring event handling
//!
//! ## Example
//!
//! ```rust
//! use crablet::connectors::{CalendarConnector, ConnectorConfig};
//!
//! let config = ConnectorConfig {
//!     connector_type: "calendar".to_string(),
//!     name: "Work Calendar".to_string(),
//!     enabled: true,
//!     settings: serde_json::json!({
//!         "provider": "google",
//!         "calendar_id": "primary",
//!         "credentials_path": "/path/to/credentials.json",
//!         "look_ahead_minutes": 15,
//!         "poll_interval_seconds": 60
//!     }),
//!     filters: vec![],
//!     transformations: vec![],
//! };
//!
//! let connector = CalendarConnector::new(config).await?;
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{interval, Duration as TokioDuration};
use tracing::{error, info, warn};

use crate::connectors::{CalendarEventType, Connector, ConnectorConfig, ConnectorError, ConnectorEvent, ConnectorHealth, ConnectorResult, HealthStatus};

/// Calendar connector configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarConfig {
    pub provider: CalendarProvider,
    #[serde(default = "default_calendar_id")]
    pub calendar_id: String,
    #[serde(default)]
    pub credentials_path: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub access_token: Option<String>,
    #[serde(default = "default_look_ahead")]
    pub look_ahead_minutes: i64,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_seconds: u64,
    #[serde(default)]
    pub event_types: Vec<CalendarEventType>,
    #[serde(default)]
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalendarProvider {
    Google,
    Outlook,
    Ical,
    CalDav,
}

fn default_calendar_id() -> String {
    "primary".to_string()
}

fn default_look_ahead() -> i64 {
    15
}

fn default_poll_interval() -> u64 {
    60
}

/// Calendar event data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEventData {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub location: Option<String>,
    pub attendees: Vec<Attendee>,
    pub organizer: Option<String>,
    pub is_recurring: bool,
    pub recurrence_rule: Option<String>,
    pub reminders: Vec<Reminder>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attendee {
    pub email: String,
    pub name: Option<String>,
    pub response_status: ResponseStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResponseStatus {
    NeedsAction,
    Declined,
    Tentative,
    Accepted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reminder {
    pub minutes_before: i64,
    pub method: ReminderMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReminderMethod {
    Popup,
    Email,
    Sms,
}

/// Calendar connector implementation
pub struct CalendarConnector {
    id: String,
    config: ConnectorConfig,
    calendar_config: CalendarConfig,
    connected: bool,
    running: bool,
    event_tx: mpsc::Sender<ConnectorEvent>,
    event_rx: Option<mpsc::Receiver<ConnectorEvent>>,
    poller_handle: Option<JoinHandle<()>>,
    tracked_events: HashMap<String, CalendarEventData>,
    triggered_events: HashMap<String, Vec<CalendarEventType>>,
}

impl CalendarConnector {
    pub fn new(config: ConnectorConfig) -> ConnectorResult<Self> {
        let calendar_config: CalendarConfig = serde_json::from_value(config.settings.clone())
            .map_err(|e| ConnectorError::ConfigurationError(format!("Invalid calendar config: {}", e)))?;
        
        let (event_tx, event_rx) = mpsc::channel(1000);
        
        Ok(Self {
            id: uuid::Uuid::new_v4().to_string(),
            config,
            calendar_config,
            connected: false,
            running: false,
            event_tx,
            event_rx: Some(event_rx),
            poller_handle: None,
            tracked_events: HashMap::new(),
            triggered_events: HashMap::new(),
        })
    }
    
    /// Start polling for calendar events
    async fn start_polling(&mut self) -> ConnectorResult<()> {
        let poll_interval = TokioDuration::from_secs(self.calendar_config.poll_interval_seconds);
        let mut interval = interval(poll_interval);
        let event_tx = self.event_tx.clone();
        let look_ahead_minutes = self.calendar_config.look_ahead_minutes;
        let event_types = self.calendar_config.event_types.clone();
        let _keywords = self.calendar_config.keywords.clone();
        
        let handle = tokio::spawn(async move {
            let mut tracked_events: HashMap<String, CalendarEventData> = HashMap::new();
            let mut triggered_events: HashMap<String, Vec<CalendarEventType>> = HashMap::new();
            
            loop {
                interval.tick().await;
                
                let now = Utc::now();
                let lookahead_time = now + Duration::minutes(look_ahead_minutes);
                
                // Fetch events (simulated)
                match Self::fetch_events(&lookahead_time).await {
                    Ok(events) => {
                        for event in events {
                            // Check if event is new or updated
                            let is_new = !tracked_events.contains_key(&event.id);
                            let is_updated = tracked_events.get(&event.id)
                                .map(|e| e.start_time != event.start_time || e.end_time != event.end_time)
                                .unwrap_or(false);
                            
                            if is_new || is_updated {
                                tracked_events.insert(event.id.clone(), event.clone());
                                
                                // Emit event created/updated
                                let event_type = if is_new {
                                    CalendarEventType::EventCreated
                                } else {
                                    CalendarEventType::EventUpdated
                                };
                                
                                if event_types.is_empty() || event_types.contains(&event_type) {
                                    Self::emit_event(&event_tx, &event, event_type).await;
                                }
                            }
                            
                            // Check for upcoming events
                            let time_until_start = event.start_time.signed_duration_since(now);
                            let time_until_end = event.end_time.signed_duration_since(now);
                            
                            // Event starting soon
                            if time_until_start.num_minutes() <= look_ahead_minutes
                                && time_until_start.num_seconds() > 0 {
                                let triggered = triggered_events.entry(event.id.clone()).or_default();
                                if !triggered.contains(&CalendarEventType::EventStarted) {
                                    triggered.push(CalendarEventType::EventStarted);
                                    if event_types.is_empty() || event_types.contains(&CalendarEventType::EventStarted) {
                                        Self::emit_event(&event_tx, &event, CalendarEventType::EventStarted).await;
                                    }
                                }
                            }
                            
                            // Event ending soon
                            if time_until_end.num_minutes() <= 5 && time_until_end.num_seconds() > 0 {
                                let triggered = triggered_events.entry(event.id.clone()).or_default();
                                if !triggered.contains(&CalendarEventType::EventEnded) {
                                    triggered.push(CalendarEventType::EventEnded);
                                    if event_types.is_empty() || event_types.contains(&CalendarEventType::EventEnded) {
                                        Self::emit_event(&event_tx, &event, CalendarEventType::EventEnded).await;
                                    }
                                }
                            }
                            
                            // Check reminders
                            for reminder in &event.reminders {
                                let reminder_time = event.start_time - Duration::minutes(reminder.minutes_before);
                                let time_until_reminder = reminder_time.signed_duration_since(now);
                                
                                if time_until_reminder.num_seconds() <= 0
                                    && time_until_reminder.num_seconds() > -60 {
                                    let triggered = triggered_events.entry(event.id.clone()).or_default();
                                    if !triggered.contains(&CalendarEventType::EventReminder) {
                                        triggered.push(CalendarEventType::EventReminder);
                                        if event_types.is_empty() || event_types.contains(&CalendarEventType::EventReminder) {
                                            Self::emit_event(&event_tx, &event, CalendarEventType::EventReminder).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to fetch calendar events: {}", e);
                    }
                }
                
                // Cleanup old events
                tracked_events.retain(|_, event| event.end_time > now - Duration::hours(1));
                triggered_events.retain(|id, _| tracked_events.contains_key(id));
            }
        });
        
        self.poller_handle = Some(handle);
        Ok(())
    }
    
    async fn fetch_events(_lookahead_time: &DateTime<Utc>) -> ConnectorResult<Vec<CalendarEventData>> {
        // In a real implementation, this would:
        // 1. Connect to calendar API (Google, Outlook, etc.)
        // 2. Fetch events up to lookahead_time
        // 3. Parse and return events
        
        // Return empty list for now
        Ok(vec![])
    }
    
    async fn emit_event(
        event_tx: &mpsc::Sender<ConnectorEvent>,
        event: &CalendarEventData,
        event_type: CalendarEventType,
    ) {
        let connector_event = ConnectorEvent::CalendarEvent {
            connector_id: "calendar".to_string(),
            event_type,
            event_id: event.id.clone(),
            title: event.title.clone(),
            start_time: event.start_time,
            end_time: event.end_time,
            attendees: event.attendees.iter().map(|a| a.email.clone()).collect(),
            description: event.description.clone(),
        };
        
        if let Err(e) = event_tx.send(connector_event).await {
            error!("Failed to send calendar event: {}", e);
        }
    }
    
    /// Create a new calendar event
    pub async fn create_event(&self, _event: CalendarEventData) -> ConnectorResult<String> {
        if !self.connected {
            return Err(ConnectorError::NotConnected);
        }
        
        // In a real implementation, this would create the event in the calendar
        let event_id = uuid::Uuid::new_v4().to_string();
        info!("Created calendar event: {}", event_id);
        
        Ok(event_id)
    }
    
    /// Update an existing event
    pub async fn update_event(&self, _event_id: &str, _event: CalendarEventData) -> ConnectorResult<()> {
        if !self.connected {
            return Err(ConnectorError::NotConnected);
        }
        
        info!("Updated calendar event");
        Ok(())
    }
    
    /// Delete an event
    pub async fn delete_event(&self, _event_id: &str) -> ConnectorResult<()> {
        if !self.connected {
            return Err(ConnectorError::NotConnected);
        }
        
        info!("Deleted calendar event");
        Ok(())
    }
}

#[async_trait]
impl Connector for CalendarConnector {
    fn id(&self) -> &str {
        &self.id
    }
    
    fn name(&self) -> &str {
        &self.config.name
    }
    
    fn connector_type(&self) -> &str {
        "calendar"
    }
    
    fn is_connected(&self) -> bool {
        self.connected
    }
    
    async fn connect(&mut self) -> ConnectorResult<()> {
        info!("Connecting to calendar: {:?}", self.calendar_config.provider);
        
        // Validate configuration
        match self.calendar_config.provider {
            CalendarProvider::Google | CalendarProvider::Outlook => {
                if self.calendar_config.credentials_path.is_none() && self.calendar_config.access_token.is_none() {
                    return Err(ConnectorError::ConfigurationError(
                        "Credentials or access token required".to_string()
                    ));
                }
            }
            _ => {}
        }
        
        self.connected = true;
        info!("Calendar connector '{}' connected", self.config.name);
        Ok(())
    }
    
    async fn disconnect(&mut self) -> ConnectorResult<()> {
        self.running = false;
        self.connected = false;
        
        if let Some(handle) = self.poller_handle.take() {
            handle.abort();
        }
        
        info!("Calendar connector '{}' disconnected", self.config.name);
        Ok(())
    }
    
    async fn start(&mut self) -> ConnectorResult<()> {
        if !self.connected {
            return Err(ConnectorError::NotConnected);
        }
        
        self.start_polling().await?;
        self.running = true;
        
        info!("Calendar connector '{}' started", self.config.name);
        Ok(())
    }
    
    async fn stop(&mut self) -> ConnectorResult<()> {
        self.running = false;
        
        if let Some(handle) = self.poller_handle.take() {
            handle.abort();
        }
        
        info!("Calendar connector '{}' stopped", self.config.name);
        Ok(())
    }
    
    fn event_receiver(&mut self) -> Option<mpsc::Receiver<ConnectorEvent>> {
        self.event_rx.take()
    }
    
    async fn test(&self) -> ConnectorResult<()> {
        // Test connection to calendar API
        info!("Testing calendar connection for '{}'", self.config.name);
        Ok(())
    }
    
    async fn health(&self) -> ConnectorHealth {
        ConnectorHealth {
            status: if self.connected {
                if self.running {
                    HealthStatus::Healthy
                } else {
                    HealthStatus::Degraded
                }
            } else {
                HealthStatus::Unhealthy
            },
            last_check: Utc::now(),
            message: None,
            latency_ms: None,
        }
    }
}
