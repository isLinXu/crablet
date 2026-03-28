//! # Connectors Module
//!
//! Event-driven connectors for external systems.
//!
//! ## Connectors
//!
//! - **Email Connector**: IMAP/SMTP integration for email triggers
//! - **Webhook Connector**: HTTP webhook receiver and sender
//! - **FileSystem Connector**: File system monitoring
//! - **Database Connector**: Database change monitoring
//! - **Calendar Connector**: Calendar event triggers
//!
//! ## Example
//!
//! ```rust,ignore
//! use crablet::connectors::{EmailConnector, ConnectorConfig};
//!
//! let config = ConnectorConfig {
//!     connector_type: "email".to_string(),
//!     settings: serde_json::json!({
//!         "imap_server": "imap.gmail.com",
//!         "username": "user@gmail.com"
//!     }),
//! };
//!
//! let connector = EmailConnector::new(config).await?;
//! connector.start().await?;
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{info, warn};

pub mod email;
pub mod webhook;
pub mod filesystem;
pub mod database;
pub mod calendar;

pub use email::EmailConnector;
pub use webhook::WebhookConnector;
pub use filesystem::FileSystemConnector;
pub use database::DatabaseConnector;
pub use calendar::CalendarConnector;

/// Connector error types
#[derive(Debug, thiserror::Error)]
pub enum ConnectorError {
    #[error("Connection failed: {0}")]
    ConnectionError(String),
    
    #[error("Authentication failed: {0}")]
    AuthenticationError(String),
    
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Timeout")]
    Timeout,
    
    #[error("Not connected")]
    NotConnected,
    
    #[error("Other: {0}")]
    Other(String),
}

pub type ConnectorResult<T> = Result<T, ConnectorError>;

/// Connector event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectorEvent {
    EmailReceived {
        connector_id: String,
        message_id: String,
        from: String,
        to: Vec<String>,
        subject: String,
        body: String,
        attachments: Vec<AttachmentInfo>,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    
    WebhookReceived {
        connector_id: String,
        webhook_id: String,
        method: String,
        path: String,
        headers: HashMap<String, String>,
        body: serde_json::Value,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    
    FileChanged {
        connector_id: String,
        watch_id: String,
        path: String,
        change_type: FileChangeType,
        metadata: Option<FileMetadata>,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    
    DatabaseChange {
        connector_id: String,
        table: String,
        operation: DbOperation,
        old_data: Option<serde_json::Value>,
        new_data: Option<serde_json::Value>,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    
    CalendarEvent {
        connector_id: String,
        event_type: CalendarEventType,
        event_id: String,
        title: String,
        start_time: chrono::DateTime<chrono::Utc>,
        end_time: chrono::DateTime<chrono::Utc>,
        attendees: Vec<String>,
        description: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentInfo {
    pub filename: String,
    pub content_type: String,
    pub size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileChangeType {
    Created,
    Modified,
    Deleted,
    Renamed { old_path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub size: u64,
    pub modified: Option<chrono::DateTime<chrono::Utc>>,
    pub created: Option<chrono::DateTime<chrono::Utc>>,
    pub permissions: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DbOperation {
    Insert,
    Update,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CalendarEventType {
    EventStarted,
    EventEnded,
    EventReminder,
    EventCreated,
    EventUpdated,
    EventDeleted,
}

/// Connector configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorConfig {
    pub connector_type: String,
    pub name: String,
    pub enabled: bool,
    #[serde(default)]
    pub settings: serde_json::Value,
    #[serde(default)]
    pub filters: Vec<EventFilter>,
    #[serde(default)]
    pub transformations: Vec<Transformation>,
}

/// Event filter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventFilter {
    pub field: String,
    pub operator: FilterOperator,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FilterOperator {
    Equals,
    NotEquals,
    Contains,
    StartsWith,
    EndsWith,
    Regex,
    GreaterThan,
    LessThan,
    In,
    NotIn,
}

/// Event transformation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transformation {
    pub transform_type: TransformType,
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransformType {
    MapField,
    AddField,
    RemoveField,
    Template,
    JsonPath,
    RegexExtract,
}

/// Connector trait
#[async_trait]
pub trait Connector: Send + Sync {
    /// Get connector ID
    fn id(&self) -> &str;
    
    /// Get connector name
    fn name(&self) -> &str;
    
    /// Get connector type
    fn connector_type(&self) -> &str;
    
    /// Check if connector is connected
    fn is_connected(&self) -> bool;
    
    /// Connect to the external system
    async fn connect(&mut self) -> ConnectorResult<()>;
    
    /// Disconnect from the external system
    async fn disconnect(&mut self) -> ConnectorResult<()>;
    
    /// Start listening for events
    async fn start(&mut self) -> ConnectorResult<()>;
    
    /// Stop listening for events
    async fn stop(&mut self) -> ConnectorResult<()>;
    
    /// Get event receiver
    fn event_receiver(&mut self) -> Option<mpsc::Receiver<ConnectorEvent>>;
    
    /// Test connection
    async fn test(&self) -> ConnectorResult<()>;
    
    /// Get health status
    async fn health(&self) -> ConnectorHealth;
}

/// Connector health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorHealth {
    pub status: HealthStatus,
    pub last_check: chrono::DateTime<chrono::Utc>,
    pub message: Option<String>,
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

/// Connector manager for managing multiple connectors
pub struct ConnectorManager {
    connectors: HashMap<String, Box<dyn Connector>>,
    event_tx: mpsc::Sender<ConnectorEvent>,
    event_rx: Option<mpsc::Receiver<ConnectorEvent>>,
}

impl ConnectorManager {
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel(1000);
        Self {
            connectors: HashMap::new(),
            event_tx,
            event_rx: Some(event_rx),
        }
    }
    
    pub async fn add_connector(&mut self, mut connector: Box<dyn Connector>) -> ConnectorResult<()> {
        let id = connector.id().to_string();
        
        // Connect and test
        connector.connect().await?;
        connector.test().await?;
        
        // Start the connector
        connector.start().await?;
        
        info!("Connector '{}' ({}) started successfully", connector.name(), id);
        
        self.connectors.insert(id, connector);
        Ok(())
    }
    
    pub async fn remove_connector(&mut self, id: &str) -> ConnectorResult<()> {
        if let Some(mut connector) = self.connectors.remove(id) {
            connector.stop().await?;
            connector.disconnect().await?;
            info!("Connector '{}' removed", id);
        }
        Ok(())
    }
    
    pub fn get_connector(&self, id: &str) -> Option<&dyn Connector> {
        self.connectors.get(id).map(|c| c.as_ref())
    }
    
    pub fn get_connector_mut(&mut self, id: &str) -> Option<&mut Box<dyn Connector>> {
        self.connectors.get_mut(id)
    }
    
    pub fn list_connectors(&self) -> Vec<&dyn Connector> {
        self.connectors.values().map(|c| c.as_ref()).collect()
    }
    
    pub fn take_event_receiver(&mut self) -> Option<mpsc::Receiver<ConnectorEvent>> {
        self.event_rx.take()
    }
    
    pub async fn shutdown(&mut self) -> ConnectorResult<()> {
        for (id, connector) in self.connectors.iter_mut() {
            if let Err(e) = connector.stop().await {
                warn!("Failed to stop connector {}: {}", id, e);
            }
            if let Err(e) = connector.disconnect().await {
                warn!("Failed to disconnect connector {}: {}", id, e);
            }
        }
        self.connectors.clear();
        Ok(())
    }
}

impl Default for ConnectorManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Apply filters to an event
pub fn apply_filters(event: &ConnectorEvent, filters: &[EventFilter]) -> bool {
    for filter in filters {
        if !apply_filter(event, filter) {
            return false;
        }
    }
    true
}

fn apply_filter(event: &ConnectorEvent, filter: &EventFilter) -> bool {
    // Extract field value from event
    let field_value = match extract_field(event, &filter.field) {
        Some(v) => v,
        None => return false,
    };
    
    match filter.operator {
        FilterOperator::Equals => field_value == filter.value,
        FilterOperator::NotEquals => field_value != filter.value,
        FilterOperator::Contains => {
            if let (Some(field_str), Some(filter_str)) = (
                field_value.as_str(),
                filter.value.as_str()
            ) {
                field_str.contains(filter_str)
            } else {
                false
            }
        }
        FilterOperator::StartsWith => {
            if let (Some(field_str), Some(filter_str)) = (
                field_value.as_str(),
                filter.value.as_str()
            ) {
                field_str.starts_with(filter_str)
            } else {
                false
            }
        }
        FilterOperator::EndsWith => {
            if let (Some(field_str), Some(filter_str)) = (
                field_value.as_str(),
                filter.value.as_str()
            ) {
                field_str.ends_with(filter_str)
            } else {
                false
            }
        }
        _ => true, // TODO: Implement other operators
    }
}

fn extract_field(event: &ConnectorEvent, field: &str) -> Option<serde_json::Value> {
    match event {
        ConnectorEvent::EmailReceived { subject, from, .. } => {
            match field {
                "subject" => Some(serde_json::Value::String(subject.clone())),
                "from" => Some(serde_json::Value::String(from.clone())),
                _ => None,
            }
        }
        ConnectorEvent::WebhookReceived { path, method, .. } => {
            match field {
                "path" => Some(serde_json::Value::String(path.clone())),
                "method" => Some(serde_json::Value::String(method.clone())),
                _ => None,
            }
        }
        ConnectorEvent::FileChanged { path, change_type, .. } => {
            match field {
                "path" => Some(serde_json::Value::String(path.clone())),
                "change_type" => Some(serde_json::json!(format!("{:?}", change_type))),
                _ => None,
            }
        }
        _ => None,
    }
}
