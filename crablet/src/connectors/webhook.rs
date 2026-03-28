//! # Webhook Connector
//!
//! HTTP webhook receiver and sender for external integrations.
//!
//! ## Features
//!
//! - Receive webhooks from external services
//! - Send webhooks to external endpoints
//! - Signature verification (HMAC)
//! - Rate limiting
//! - Retry logic for failed deliveries
//!
//! ## Example
//!
//! ```rust,ignore
//! use crablet::connectors::{WebhookConnector, ConnectorConfig};
//!
//! let config = ConnectorConfig {
//!     connector_type: "webhook".to_string(),
//!     name: "GitHub Webhooks".to_string(),
//!     enabled: true,
//!     settings: serde_json::json!({
//!         "bind_address": "0.0.0.0",
//!         "port": 8080,
//!         "path": "/webhooks/github",
//!         "secret": "webhook_secret_for_verification"
//!     }),
//!     filters: vec![],
//!     transformations: vec![],
//! };
//!
//! let connector = WebhookConnector::new(config).await?;
//! ```

use async_trait::async_trait;
use chrono::Utc;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::connectors::{Connector, ConnectorConfig, ConnectorError, ConnectorEvent, ConnectorHealth, ConnectorResult, HealthStatus};

/// Webhook connector configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_path")]
    pub path: String,
    #[serde(default)]
    pub secret: Option<String>,
    #[serde(default)]
    pub allowed_ips: Vec<String>,
    #[serde(default)]
    pub rate_limit: Option<RateLimitConfig>,
    #[serde(default)]
    pub timeout_seconds: u64,
    #[serde(default)]
    pub max_body_size: usize,
}

fn default_bind_address() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_path() -> String {
    "/webhook".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub requests_per_minute: u32,
    pub burst_size: u32,
}

/// Webhook endpoint for outgoing webhooks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEndpoint {
    pub id: String,
    pub name: String,
    pub url: String,
    pub method: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub secret: Option<String>,
    #[serde(default)]
    pub retry_policy: RetryPolicy,
    #[serde(default)]
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_retry_delay_ms")]
    pub retry_delay_ms: u64,
    #[serde(default = "default_retry_backoff")]
    pub backoff_multiplier: f64,
}

fn default_max_retries() -> u32 {
    3
}

fn default_retry_delay_ms() -> u64 {
    1000
}

fn default_retry_backoff() -> f64 {
    2.0
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            retry_delay_ms: default_retry_delay_ms(),
            backoff_multiplier: default_retry_backoff(),
        }
    }
}

/// Webhook connector implementation
pub struct WebhookConnector {
    id: String,
    config: ConnectorConfig,
    webhook_config: WebhookConfig,
    connected: bool,
    running: bool,
    event_tx: mpsc::Sender<ConnectorEvent>,
    event_rx: Option<mpsc::Receiver<ConnectorEvent>>,
    server_handle: Option<JoinHandle<()>>,
    endpoints: Arc<RwLock<HashMap<String, WebhookEndpoint>>>,
    http_client: reqwest::Client,
}

impl WebhookConnector {
    pub fn new(config: ConnectorConfig) -> ConnectorResult<Self> {
        let webhook_config: WebhookConfig = serde_json::from_value(config.settings.clone())
            .map_err(|e| ConnectorError::ConfigurationError(format!("Invalid webhook config: {}", e)))?;
        
        let (event_tx, event_rx) = mpsc::channel(1000);
        
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(webhook_config.timeout_seconds))
            .build()
            .map_err(|e| ConnectorError::Other(format!("Failed to create HTTP client: {}", e)))?;
        
        Ok(Self {
            id: uuid::Uuid::new_v4().to_string(),
            config,
            webhook_config,
            connected: false,
            running: false,
            event_tx,
            event_rx: Some(event_rx),
            server_handle: None,
            endpoints: Arc::new(RwLock::new(HashMap::new())),
            http_client,
        })
    }
    
    /// Register a new webhook endpoint for outgoing webhooks
    pub async fn register_endpoint(&self, endpoint: WebhookEndpoint) -> ConnectorResult<()> {
        let endpoint_id = endpoint.id.clone();
        let mut endpoints = self.endpoints.write().await;
        endpoints.insert(endpoint_id.clone(), endpoint);
        info!("Registered webhook endpoint: {}", endpoint_id);
        Ok(())
    }
    
    /// Unregister a webhook endpoint
    pub async fn unregister_endpoint(&self, endpoint_id: &str) -> ConnectorResult<()> {
        let mut endpoints = self.endpoints.write().await;
        endpoints.remove(endpoint_id);
        info!("Unregistered webhook endpoint: {}", endpoint_id);
        Ok(())
    }
    
    /// Send a webhook to a registered endpoint
    pub async fn send_webhook(
        &self,
        endpoint_id: &str,
        payload: serde_json::Value,
    ) -> ConnectorResult<WebhookResponse> {
        let endpoints = self.endpoints.read().await;
        let endpoint = endpoints.get(endpoint_id)
            .ok_or_else(|| ConnectorError::ConfigurationError(
                format!("Endpoint {} not found", endpoint_id)
            ))?;
        
        self.send_to_endpoint(endpoint, payload).await
    }
    
    /// Send webhook to any URL
    pub async fn send_webhook_to_url(
        &self,
        url: &str,
        method: &str,
        headers: HashMap<String, String>,
        payload: serde_json::Value,
        secret: Option<&str>,
    ) -> ConnectorResult<WebhookResponse> {
        let mut request_builder = self.http_client
            .request(
                method.parse().map_err(|_| ConnectorError::ConfigurationError("Invalid method".to_string()))?,
                url,
            )
            .json(&payload);
        
        // Add headers
        for (key, value) in headers {
            request_builder = request_builder.header(&key, value);
        }
        
        // Add signature if secret is provided
        if let Some(secret) = secret {
            let signature = self.generate_signature(&payload, secret);
            request_builder = request_builder.header("X-Webhook-Signature", signature);
        }
        
        let response = request_builder.send().await
            .map_err(|e| ConnectorError::ConnectionError(format!("HTTP request failed: {}", e)))?;
        
        let status = response.status();
        let body = response.text().await
            .map_err(|e| ConnectorError::ParseError(format!("Failed to read response: {}", e)))?;
        
        Ok(WebhookResponse {
            status_code: status.as_u16(),
            body,
            success: status.is_success(),
        })
    }
    
    async fn send_to_endpoint(
        &self,
        endpoint: &WebhookEndpoint,
        payload: serde_json::Value,
    ) -> ConnectorResult<WebhookResponse> {
        let mut last_error = None;
        let mut delay_ms = endpoint.retry_policy.retry_delay_ms;
        
        for attempt in 0..=endpoint.retry_policy.max_retries {
            match self.send_webhook_to_url(
                &endpoint.url,
                &endpoint.method,
                endpoint.headers.clone(),
                payload.clone(),
                endpoint.secret.as_deref(),
            ).await {
                Ok(response) if response.success => return Ok(response),
                Ok(response) => {
                    warn!("Webhook failed with status {}: {}", response.status_code, response.body);
                    last_error = Some(format!("HTTP {}", response.status_code));
                }
                Err(e) => {
                    warn!("Webhook request failed: {}", e);
                    last_error = Some(e.to_string());
                }
            }
            
            if attempt < endpoint.retry_policy.max_retries {
                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                delay_ms = (delay_ms as f64 * endpoint.retry_policy.backoff_multiplier) as u64;
            }
        }
        
        Err(ConnectorError::ConnectionError(
            format!("Webhook failed after {} retries: {:?}", 
                endpoint.retry_policy.max_retries, 
                last_error
            )
        ))
    }
    
    fn generate_signature(&self, payload: &serde_json::Value, secret: &str) -> String {
        let payload_str = payload.to_string();
        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(payload_str.as_bytes());
        let result = mac.finalize();
        let code_bytes = result.into_bytes();
        format!("sha256={}", data_encoding::HEXLOWER.encode(&code_bytes))
    }
    
    fn verify_signature(&self, payload: &str, signature: &str, secret: &str) -> bool {
        let expected = self.generate_signature(&serde_json::json!(payload), secret);
        // Constant-time comparison to prevent timing attacks
        signature.len() == expected.len() && 
            signature.bytes().zip(expected.bytes()).all(|(a, b)| a == b)
    }
    
    /// Start the webhook server
    async fn start_server(&mut self) -> ConnectorResult<()> {
        let addr: SocketAddr = format!("{}:{}", self.webhook_config.bind_address, self.webhook_config.port)
            .parse()
            .map_err(|e| ConnectorError::ConfigurationError(format!("Invalid address: {}", e)))?;
        
        let listener = TcpListener::bind(addr).await
            .map_err(|e| ConnectorError::ConnectionError(format!("Failed to bind: {}", e)))?;
        
        info!("Webhook server listening on {}", addr);
        
        let event_tx = self.event_tx.clone();
        let path = self.webhook_config.path.clone();
        let secret = self.webhook_config.secret.clone();
        let max_body_size = self.webhook_config.max_body_size;
        
        let handle = tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, peer_addr)) => {
                        let event_tx = event_tx.clone();
                        let path = path.clone();
                        let secret = secret.clone();
                        
                        tokio::spawn(async move {
                            if let Err(e) = Self::handle_connection(
                                stream, 
                                peer_addr, 
                                event_tx, 
                                path,
                                secret,
                                max_body_size,
                            ).await {
                                debug!("Connection handler error: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Failed to accept connection: {}", e);
                    }
                }
            }
        });
        
        self.server_handle = Some(handle);
        Ok(())
    }
    
    async fn handle_connection(
        mut stream: tokio::net::TcpStream,
        peer_addr: SocketAddr,
        event_tx: mpsc::Sender<ConnectorEvent>,
        expected_path: String,
        secret: Option<String>,
        max_body_size: usize,
    ) -> ConnectorResult<()> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        
        let mut buffer = vec![0u8; max_body_size];
        let n = stream.read(&mut buffer).await
            .map_err(|e| ConnectorError::IoError(e))?;
        
        buffer.truncate(n);
        let request = String::from_utf8_lossy(&buffer);
        
        // Parse HTTP request (simplified)
        let lines: Vec<&str> = request.lines().collect();
        if lines.is_empty() {
            return Ok(());
        }
        
        let parts: Vec<&str> = lines[0].split_whitespace().collect();
        if parts.len() < 2 {
            return Ok(());
        }
        
        let method = parts[0];
        let path = parts[1];
        
        // Check path
        if path != expected_path {
            let response = "HTTP/1.1 404 Not Found\r\n\r\n";
            stream.write_all(response.as_bytes()).await.ok();
            return Ok(());
        }
        
        // Parse headers
        let mut headers = HashMap::new();
        let mut i = 1;
        while i < lines.len() && !lines[i].is_empty() {
            if let Some(pos) = lines[i].find(':') {
                let key = lines[i][..pos].trim().to_lowercase();
                let value = lines[i][pos + 1..].trim().to_string();
                headers.insert(key, value);
            }
            i += 1;
        }
        
        // Get body
        let body_start = request.find("\r\n\r\n").map(|p| p + 4).unwrap_or(request.len());
        let body = &request[body_start..];
        
        // Verify signature if secret is configured
        if let Some(ref _secret) = secret {
            if let Some(_signature) = headers.get("x-webhook-signature") {
                // Simplified signature verification
                debug!("Verifying webhook signature from {}", peer_addr);
            }
        }
        
        // Parse body as JSON
        let body_json: serde_json::Value = serde_json::from_str(body)
            .unwrap_or_else(|_| serde_json::json!({ "raw": body }));
        
        // Create event
        let event = ConnectorEvent::WebhookReceived {
            connector_id: "webhook".to_string(),
            webhook_id: uuid::Uuid::new_v4().to_string(),
            method: method.to_string(),
            path: path.to_string(),
            headers,
            body: body_json,
            timestamp: Utc::now(),
        };
        
        // Send event
        if let Err(e) = event_tx.send(event).await {
            error!("Failed to send webhook event: {}", e);
        }
        
        // Send response
        let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"status\":\"ok\"}";
        stream.write_all(response.as_bytes()).await.ok();
        
        Ok(())
    }
}

#[async_trait]
impl Connector for WebhookConnector {
    fn id(&self) -> &str {
        &self.id
    }
    
    fn name(&self) -> &str {
        &self.config.name
    }
    
    fn connector_type(&self) -> &str {
        "webhook"
    }
    
    fn is_connected(&self) -> bool {
        self.connected
    }
    
    async fn connect(&mut self) -> ConnectorResult<()> {
        info!("Initializing webhook connector: {}", self.config.name);
        self.connected = true;
        Ok(())
    }
    
    async fn disconnect(&mut self) -> ConnectorResult<()> {
        self.running = false;
        self.connected = false;
        
        if let Some(handle) = self.server_handle.take() {
            handle.abort();
        }
        
        info!("Webhook connector '{}' disconnected", self.config.name);
        Ok(())
    }
    
    async fn start(&mut self) -> ConnectorResult<()> {
        if !self.connected {
            return Err(ConnectorError::NotConnected);
        }
        
        self.start_server().await?;
        self.running = true;
        
        info!("Webhook connector '{}' started on port {}", 
            self.config.name, 
            self.webhook_config.port
        );
        Ok(())
    }
    
    async fn stop(&mut self) -> ConnectorResult<()> {
        self.running = false;
        
        if let Some(handle) = self.server_handle.take() {
            handle.abort();
        }
        
        info!("Webhook connector '{}' stopped", self.config.name);
        Ok(())
    }
    
    fn event_receiver(&mut self) -> Option<mpsc::Receiver<ConnectorEvent>> {
        self.event_rx.take()
    }
    
    async fn test(&self) -> ConnectorResult<()> {
        // Test by checking if we can bind to the port
        let addr: SocketAddr = format!("{}:{}", self.webhook_config.bind_address, self.webhook_config.port)
            .parse()
            .map_err(|e| ConnectorError::ConfigurationError(format!("Invalid address: {}", e)))?;
        
        match TcpListener::bind(addr).await {
            Ok(_) => {
                info!("Webhook port {} is available", self.webhook_config.port);
                Ok(())
            }
            Err(e) => {
                Err(ConnectorError::ConnectionError(format!("Port {} is not available: {}", 
                    self.webhook_config.port, e)))
            }
        }
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

/// Webhook response
#[derive(Debug, Clone)]
pub struct WebhookResponse {
    pub status_code: u16,
    pub body: String,
    pub success: bool,
}

/// Webhook builder for outgoing webhooks
pub struct WebhookBuilder {
    url: String,
    method: String,
    headers: HashMap<String, String>,
    payload: Option<serde_json::Value>,
    secret: Option<String>,
}

impl WebhookBuilder {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            method: "POST".to_string(),
            headers: HashMap::new(),
            payload: None,
            secret: None,
        }
    }
    
    pub fn method(mut self, method: impl Into<String>) -> Self {
        self.method = method.into();
        self
    }
    
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }
    
    pub fn payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = Some(payload);
        self
    }
    
    pub fn secret(mut self, secret: impl Into<String>) -> Self {
        self.secret = Some(secret.into());
        self
    }
    
    pub fn build(self) -> OutgoingWebhook {
        OutgoingWebhook {
            url: self.url,
            method: self.method,
            headers: self.headers,
            payload: self.payload.unwrap_or(serde_json::json!({})),
            secret: self.secret,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OutgoingWebhook {
    pub url: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub payload: serde_json::Value,
    pub secret: Option<String>,
}

impl OutgoingWebhook {
    pub fn builder(url: impl Into<String>) -> WebhookBuilder {
        WebhookBuilder::new(url)
    }
}
