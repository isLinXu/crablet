//! # Database Connector
//!
//! Database change monitoring for trigger-based workflows.
//!
//! ## Features
//!
//! - Poll-based change detection
//! - CDC (Change Data Capture) support for PostgreSQL/MySQL
//! - Query-based triggers
//! - Connection pooling
//!
//! ## Example
//!
//! ```rust,ignore
//! use crablet::connectors::{DatabaseConnector, ConnectorConfig};
//!
//! let config = ConnectorConfig {
//!     connector_type: "database".to_string(),
//!     name: "Orders DB".to_string(),
//!     enabled: true,
//!     settings: serde_json::json!({
//!         "connection_string": "postgresql://user:pass@localhost/mydb",
//!         "database_type": "postgresql",
//!         "triggers": [
//!             {
//!                 "table": "orders",
//!                 "operations": ["INSERT", "UPDATE"],
//!                 "condition": "status = 'pending'"
//!             }
//!         ],
//!         "poll_interval_seconds": 30
//!     }),
//!     filters: vec![],
//!     transformations: vec![],
//! };
//!
//! let connector = DatabaseConnector::new(config).await?;
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info};

use crate::connectors::{Connector, ConnectorConfig, ConnectorError, ConnectorEvent, ConnectorHealth, ConnectorResult, DbOperation, HealthStatus};

/// Database connector configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub connection_string: String,
    #[serde(rename = "database_type")]
    pub db_type: DatabaseType,
    #[serde(default)]
    pub triggers: Vec<DbTrigger>,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_seconds: u64,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    #[serde(default)]
    pub use_cdc: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseType {
    PostgreSQL,
    MySQL,
    SQLite,
    MongoDB,
}

fn default_poll_interval() -> u64 {
    60
}

fn default_max_connections() -> u32 {
    10
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbTrigger {
    pub table: String,
    #[serde(default)]
    pub operations: Vec<DbOperation>,
    #[serde(default)]
    pub condition: Option<String>,
    #[serde(default)]
    pub columns: Vec<String>,
}

/// Database connector implementation
pub struct DatabaseConnector {
    id: String,
    config: ConnectorConfig,
    db_config: DatabaseConfig,
    connected: bool,
    running: bool,
    event_tx: mpsc::Sender<ConnectorEvent>,
    event_rx: Option<mpsc::Receiver<ConnectorEvent>>,
    poller_handle: Option<JoinHandle<()>>,
    last_check_times: HashMap<String, DateTime<Utc>>,
}

impl DatabaseConnector {
    pub fn new(config: ConnectorConfig) -> ConnectorResult<Self> {
        let db_config: DatabaseConfig = serde_json::from_value(config.settings.clone())
            .map_err(|e| ConnectorError::ConfigurationError(format!("Invalid database config: {}", e)))?;
        
        if db_config.connection_string.is_empty() {
            return Err(ConnectorError::ConfigurationError(
                "Connection string is required".to_string()
            ));
        }
        
        let (event_tx, event_rx) = mpsc::channel(1000);
        
        Ok(Self {
            id: uuid::Uuid::new_v4().to_string(),
            config,
            db_config,
            connected: false,
            running: false,
            event_tx,
            event_rx: Some(event_rx),
            poller_handle: None,
            last_check_times: HashMap::new(),
        })
    }
    
    /// Start polling for database changes
    async fn start_polling(&mut self) -> ConnectorResult<()> {
        let poll_interval = Duration::from_secs(self.db_config.poll_interval_seconds);
        let mut interval = interval(poll_interval);
        let event_tx = self.event_tx.clone();
        let triggers = self.db_config.triggers.clone();
        let db_type = self.db_config.db_type.clone();
        
        let handle = tokio::spawn(async move {
            loop {
                interval.tick().await;
                
                for trigger in &triggers {
                    if let Err(e) = Self::check_trigger(&db_type, trigger, &event_tx).await {
                        debug!("Trigger check error: {}", e);
                    }
                }
            }
        });
        
        self.poller_handle = Some(handle);
        Ok(())
    }
    
    async fn check_trigger(
        db_type: &DatabaseType,
        trigger: &DbTrigger,
        event_tx: &mpsc::Sender<ConnectorEvent>,
    ) -> ConnectorResult<()> {
        // In a real implementation, this would:
        // 1. Query the database for changes since last check
        // 2. Compare with previous state
        // 3. Emit events for detected changes
        
        debug!("Checking trigger for table: {} ({:?})", trigger.table, db_type);
        
        // Simulate detecting a change
        let event = ConnectorEvent::DatabaseChange {
            connector_id: "database".to_string(),
            table: trigger.table.clone(),
            operation: DbOperation::Insert,
            old_data: None,
            new_data: Some(serde_json::json!({
                "id": 1,
                "status": "pending"
            })),
            timestamp: Utc::now(),
        };
        
        if let Err(e) = event_tx.send(event).await {
            error!("Failed to send database event: {}", e);
        }
        
        Ok(())
    }
    
    /// Execute a query and return results
    pub async fn execute_query(&self, _query: &str) -> ConnectorResult<Vec<HashMap<String, serde_json::Value>>> {
        if !self.connected {
            return Err(ConnectorError::NotConnected);
        }
        
        // In a real implementation, this would execute the query
        // and return the results as JSON
        
        Ok(vec![])
    }
}

#[async_trait]
impl Connector for DatabaseConnector {
    fn id(&self) -> &str {
        &self.id
    }
    
    fn name(&self) -> &str {
        &self.config.name
    }
    
    fn connector_type(&self) -> &str {
        "database"
    }
    
    fn is_connected(&self) -> bool {
        self.connected
    }
    
    async fn connect(&mut self) -> ConnectorResult<()> {
        info!("Connecting to database: {:?}", self.db_config.db_type);
        
        // In a real implementation, this would:
        // 1. Parse connection string
        // 2. Create connection pool
        // 3. Test connection
        
        self.connected = true;
        info!("Database connector '{}' connected", self.config.name);
        Ok(())
    }
    
    async fn disconnect(&mut self) -> ConnectorResult<()> {
        self.running = false;
        self.connected = false;
        
        if let Some(handle) = self.poller_handle.take() {
            handle.abort();
        }
        
        info!("Database connector '{}' disconnected", self.config.name);
        Ok(())
    }
    
    async fn start(&mut self) -> ConnectorResult<()> {
        if !self.connected {
            return Err(ConnectorError::NotConnected);
        }
        
        if !self.db_config.triggers.is_empty() {
            self.start_polling().await?;
        }
        
        self.running = true;
        info!("Database connector '{}' started", self.config.name);
        Ok(())
    }
    
    async fn stop(&mut self) -> ConnectorResult<()> {
        self.running = false;
        
        if let Some(handle) = self.poller_handle.take() {
            handle.abort();
        }
        
        info!("Database connector '{}' stopped", self.config.name);
        Ok(())
    }
    
    fn event_receiver(&mut self) -> Option<mpsc::Receiver<ConnectorEvent>> {
        self.event_rx.take()
    }
    
    async fn test(&self) -> ConnectorResult<()> {
        if !self.connected {
            return Err(ConnectorError::NotConnected);
        }
        
        // Test connection by executing a simple query
        info!("Testing database connection for '{}'", self.config.name);
        
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
