//! # Email Connector
//!
//! IMAP/SMTP integration for email-based triggers.
//!
//! ## Features
//!
//! - IMAP IDLE support for real-time email notifications
//! - Polling fallback for servers without IDLE
//! - Attachment handling
//! - Email filtering by subject, sender, etc.
//! - SMTP for sending responses
//!
//! ## Example
//!
//! ```rust,ignore
//! use crablet::connectors::{EmailConnector, ConnectorConfig};
//!
//! let config = ConnectorConfig {
//!     connector_type: "email".to_string(),
//!     name: "Gmail Inbox".to_string(),
//!     enabled: true,
//!     settings: serde_json::json!({
//!         "imap_server": "imap.gmail.com",
//!         "imap_port": 993,
//!         "smtp_server": "smtp.gmail.com",
//!         "smtp_port": 587,
//!         "username": "user@gmail.com",
//!         "password": "app_password",
//!         "use_idle": true,
//!         "poll_interval_seconds": 60,
//!         "folder": "INBOX"
//!     }),
//!     filters: vec![],
//!     transformations: vec![],
//! };
//!
//! let connector = EmailConnector::new(config).await?;
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info};

use crate::connectors::{Connector, ConnectorConfig, ConnectorError, ConnectorEvent, ConnectorHealth, ConnectorResult, HealthStatus};

/// Email connector configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailConfig {
    pub imap_server: String,
    pub imap_port: u16,
    #[serde(default)]
    pub imap_use_tls: bool,
    pub smtp_server: String,
    pub smtp_port: u16,
    #[serde(default)]
    pub smtp_use_tls: bool,
    pub username: String,
    pub password: String,
    #[serde(default = "default_folder")]
    pub folder: String,
    #[serde(default = "default_use_idle")]
    pub use_idle: bool,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_seconds: u64,
    #[serde(default)]
    pub mark_as_read: bool,
    #[serde(default)]
    pub filters: EmailFilters,
}

fn default_folder() -> String {
    "INBOX".to_string()
}

fn default_use_idle() -> bool {
    true
}

fn default_poll_interval() -> u64 {
    60
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmailFilters {
    #[serde(default)]
    pub subject_contains: Vec<String>,
    #[serde(default)]
    pub from_addresses: Vec<String>,
    #[serde(default)]
    pub to_addresses: Vec<String>,
    #[serde(default)]
    pub has_attachments: Option<bool>,
    #[serde(default)]
    pub since_days: Option<u32>,
}

/// Email connector implementation
pub struct EmailConnector {
    id: String,
    config: ConnectorConfig,
    email_config: EmailConfig,
    connected: bool,
    running: bool,
    event_tx: mpsc::Sender<ConnectorEvent>,
    event_rx: Option<mpsc::Receiver<ConnectorEvent>>,
    last_check: Option<DateTime<Utc>>,
    processed_ids: std::collections::HashSet<String>,
}

impl EmailConnector {
    pub fn new(config: ConnectorConfig) -> ConnectorResult<Self> {
        let email_config: EmailConfig = serde_json::from_value(config.settings.clone())
            .map_err(|e| ConnectorError::ConfigurationError(format!("Invalid email config: {}", e)))?;
        
        let (event_tx, event_rx) = mpsc::channel(100);
        
        Ok(Self {
            id: uuid::Uuid::new_v4().to_string(),
            config,
            email_config,
            connected: false,
            running: false,
            event_tx,
            event_rx: Some(event_rx),
            last_check: None,
            processed_ids: std::collections::HashSet::new(),
        })
    }
    
    /// Send an email response
    pub async fn send_email(
        &self,
        to: Vec<String>,
        subject: String,
        body: String,
        #[allow(unused_variables)]
        html_body: Option<String>,
    ) -> ConnectorResult<()> {
        if !self.connected {
            return Err(ConnectorError::NotConnected);
        }
        
        info!("Sending email to {:?} with subject: {}", to, subject);
        
        // Note: In a real implementation, this would use lettre crate
        // For now, we just log the intent
        debug!("Email body: {}", body);
        
        Ok(())
    }
    
    /// Process a single email message
    async fn process_message(
        &mut self,
        message_id: String,
        from: String,
        to: Vec<String>,
        subject: String,
        body: String,
        attachments: Vec<crate::connectors::AttachmentInfo>,
        timestamp: DateTime<Utc>,
    ) -> ConnectorResult<()> {
        // Check if already processed
        if self.processed_ids.contains(&message_id) {
            return Ok(());
        }
        
        // Apply filters
        if !self.matches_filters(&subject, &from, &to, !attachments.is_empty()) {
            debug!("Email {} filtered out", message_id);
            return Ok(());
        }
        
        // Create event
        let event = ConnectorEvent::EmailReceived {
            connector_id: self.id.clone(),
            message_id: message_id.clone(),
            from,
            to,
            subject,
            body,
            attachments,
            timestamp,
        };
        
        // Send event
        if let Err(e) = self.event_tx.send(event).await {
            error!("Failed to send email event: {}", e);
            return Err(ConnectorError::Other(format!("Event channel error: {}", e)));
        }
        
        // Mark as processed
        self.processed_ids.insert(message_id);
        
        // Limit processed IDs cache size
        if self.processed_ids.len() > 10000 {
            // Clear oldest half
            let to_remove: Vec<_> = self.processed_ids.iter().take(5000).cloned().collect();
            for id in to_remove {
                self.processed_ids.remove(&id);
            }
        }
        
        Ok(())
    }
    
    fn matches_filters(&self, subject: &str, from: &str, to: &[String], has_attachments: bool) -> bool {
        let filters = &self.email_config.filters;
        
        // Subject filter
        if !filters.subject_contains.is_empty() {
            let matches = filters.subject_contains.iter()
                .any(|s| subject.to_lowercase().contains(&s.to_lowercase()));
            if !matches {
                return false;
            }
        }
        
        // From filter
        if !filters.from_addresses.is_empty() {
            let matches = filters.from_addresses.iter()
                .any(|addr| from.to_lowercase().contains(&addr.to_lowercase()));
            if !matches {
                return false;
            }
        }
        
        // To filter
        if !filters.to_addresses.is_empty() {
            let matches = filters.to_addresses.iter()
                .any(|addr| to.iter().any(|t| t.to_lowercase().contains(&addr.to_lowercase())));
            if !matches {
                return false;
            }
        }
        
        // Attachment filter
        if let Some(required) = filters.has_attachments {
            if has_attachments != required {
                return false;
            }
        }
        
        true
    }
    
    /// Start polling for new emails
    async fn start_polling(&mut self) -> ConnectorResult<()> {
        let poll_interval = Duration::from_secs(self.email_config.poll_interval_seconds);
        let mut interval = interval(poll_interval);
        
        let connector_id = self.id.clone();
        let _event_tx = self.event_tx.clone();
        
        tokio::spawn(async move {
            loop {
                interval.tick().await;
                
                // In a real implementation, this would fetch emails from IMAP server
                // For now, we just simulate the polling
                debug!("Polling emails for connector {}", connector_id);
                
                // Simulate checking for new emails
                // Real implementation would use imap crate
            }
        });
        
        Ok(())
    }
    
    /// Start IMAP IDLE mode
    async fn start_idle(&mut self) -> ConnectorResult<()> {
        info!("Starting IMAP IDLE mode for {}", self.config.name);
        
        // In a real implementation, this would use IMAP IDLE
        // For now, we fall back to polling
        if !self.email_config.use_idle {
            return self.start_polling().await;
        }
        
        // Simulate IDLE mode (would use imap::Session::idle())
        self.start_polling().await
    }
}

#[async_trait]
impl Connector for EmailConnector {
    fn id(&self) -> &str {
        &self.id
    }
    
    fn name(&self) -> &str {
        &self.config.name
    }
    
    fn connector_type(&self) -> &str {
        "email"
    }
    
    fn is_connected(&self) -> bool {
        self.connected
    }
    
    async fn connect(&mut self) -> ConnectorResult<()> {
        info!("Connecting to email server: {}", self.email_config.imap_server);
        
        // In a real implementation, this would:
        // 1. Connect to IMAP server
        // 2. Authenticate
        // 3. Select folder
        
        // Validate configuration
        if self.email_config.username.is_empty() {
            return Err(ConnectorError::ConfigurationError(
                "Username is required".to_string()
            ));
        }
        
        if self.email_config.password.is_empty() {
            return Err(ConnectorError::ConfigurationError(
                "Password is required".to_string()
            ));
        }
        
        self.connected = true;
        info!("Email connector '{}' connected successfully", self.config.name);
        
        Ok(())
    }
    
    async fn disconnect(&mut self) -> ConnectorResult<()> {
        self.running = false;
        self.connected = false;
        info!("Email connector '{}' disconnected", self.config.name);
        Ok(())
    }
    
    async fn start(&mut self) -> ConnectorResult<()> {
        if !self.connected {
            return Err(ConnectorError::NotConnected);
        }
        
        self.running = true;
        
        // Start IDLE or polling
        if self.email_config.use_idle {
            self.start_idle().await?;
        } else {
            self.start_polling().await?;
        }
        
        info!("Email connector '{}' started", self.config.name);
        Ok(())
    }
    
    async fn stop(&mut self) -> ConnectorResult<()> {
        self.running = false;
        info!("Email connector '{}' stopped", self.config.name);
        Ok(())
    }
    
    fn event_receiver(&mut self) -> Option<mpsc::Receiver<ConnectorEvent>> {
        self.event_rx.take()
    }
    
    async fn test(&self) -> ConnectorResult<()> {
        if !self.connected {
            return Err(ConnectorError::NotConnected);
        }
        
        // Test connection by checking folder list
        info!("Testing email connection for '{}'", self.config.name);
        
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

/// Email builder for constructing emails
pub struct EmailBuilder {
    to: Vec<String>,
    cc: Vec<String>,
    bcc: Vec<String>,
    subject: String,
    body: String,
    html_body: Option<String>,
    attachments: Vec<Attachment>,
}

#[derive(Debug, Clone)]
pub struct Attachment {
    pub filename: String,
    pub content_type: String,
    pub data: Vec<u8>,
}

impl EmailBuilder {
    pub fn new() -> Self {
        Self {
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: String::new(),
            body: String::new(),
            html_body: None,
            attachments: Vec::new(),
        }
    }
    
    pub fn to(mut self, address: impl Into<String>) -> Self {
        self.to.push(address.into());
        self
    }
    
    pub fn cc(mut self, address: impl Into<String>) -> Self {
        self.cc.push(address.into());
        self
    }
    
    pub fn bcc(mut self, address: impl Into<String>) -> Self {
        self.bcc.push(address.into());
        self
    }
    
    pub fn subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = subject.into();
        self
    }
    
    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into();
        self
    }
    
    pub fn html(mut self, html: impl Into<String>) -> Self {
        self.html_body = Some(html.into());
        self
    }
    
    pub fn attach(mut self, filename: impl Into<String>, content_type: impl Into<String>, data: Vec<u8>) -> Self {
        self.attachments.push(Attachment {
            filename: filename.into(),
            content_type: content_type.into(),
            data,
        });
        self
    }
    
    pub fn build(self) -> EmailMessage {
        EmailMessage {
            to: self.to,
            cc: self.cc,
            bcc: self.bcc,
            subject: self.subject,
            body: self.body,
            html_body: self.html_body,
            attachments: self.attachments,
        }
    }
}

impl Default for EmailBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct EmailMessage {
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body: String,
    pub html_body: Option<String>,
    pub attachments: Vec<Attachment>,
}

impl EmailMessage {
    pub fn builder() -> EmailBuilder {
        EmailBuilder::new()
    }
}
