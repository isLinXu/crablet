//! Model Fallback System
//!
//! Provides automatic failover between multiple LLM providers.
//! When the primary model fails, automatically switches to fallback models.

use std::sync::Arc;
use std::time::{Duration, Instant};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{info, warn, error};

use crate::cognitive::llm::{LlmClient, KimiClient, ZhipuClient};
use crate::types::Message;

/// LLM Error types
#[derive(Debug, Clone)]
pub enum LlmError {
    ConfigError(String),
    ApiError(String),
    Timeout,
    AllModelsFailed,
    NotImplemented(String),
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmError::ConfigError(msg) => write!(f, "Config error: {}", msg),
            LlmError::ApiError(msg) => write!(f, "API error: {}", msg),
            LlmError::Timeout => write!(f, "Request timeout"),
            LlmError::AllModelsFailed => write!(f, "All models failed"),
            LlmError::NotImplemented(msg) => write!(f, "Not implemented: {}", msg),
        }
    }
}

impl std::error::Error for LlmError {}

/// Fallback configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackConfig {
    /// Primary model configuration
    pub primary: ModelConfig,
    /// Fallback chain (in order of preference)
    pub fallbacks: Vec<ModelConfig>,
    /// Health check interval
    pub health_check_interval_secs: u64,
    /// Max retry attempts per model
    pub max_retries: u32,
    /// Timeout for each request
    pub timeout_secs: u64,
    /// Enable circuit breaker pattern
    pub enable_circuit_breaker: bool,
    /// Circuit breaker failure threshold
    pub circuit_breaker_threshold: u32,
    /// Circuit breaker reset timeout
    pub circuit_breaker_reset_secs: u64,
}

impl Default for FallbackConfig {
    fn default() -> Self {
        Self {
            primary: ModelConfig::default(),
            fallbacks: vec![],
            health_check_interval_secs: 60,
            max_retries: 2,
            timeout_secs: 30,
            enable_circuit_breaker: true,
            circuit_breaker_threshold: 5,
            circuit_breaker_reset_secs: 300,
        }
    }
}

/// Model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Provider name
    pub provider: String,
    /// Model name
    pub model: String,
    /// API key (optional, can use env var)
    pub api_key: Option<String>,
    /// API base URL
    pub api_base: Option<String>,
    /// Request timeout
    pub timeout_secs: u64,
    /// Max tokens
    pub max_tokens: u32,
    /// Temperature
    pub temperature: f32,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            provider: "openai".to_string(),
            model: "gpt-4".to_string(),
            api_key: None,
            api_base: None,
            timeout_secs: 30,
            max_tokens: 4096,
            temperature: 0.7,
        }
    }
}

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,     // Normal operation
    Open,       // Failing, reject requests
    HalfOpen,   // Testing if recovered
}

/// Model health status
#[derive(Debug, Clone)]
pub struct ModelHealth {
    pub state: CircuitState,
    pub consecutive_failures: u32,
    pub last_failure: Option<Instant>,
    pub last_success: Option<Instant>,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub average_latency_ms: f64,
}

impl Default for ModelHealth {
    fn default() -> Self {
        Self {
            state: CircuitState::Closed,
            consecutive_failures: 0,
            last_failure: None,
            last_success: None,
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            average_latency_ms: 0.0,
        }
    }
}

/// Fallback LLM Client
pub struct FallbackLlmClient {
    config: FallbackConfig,
    /// Primary client
    primary: Arc<Box<dyn LlmClient>>,
    /// Fallback clients
    fallbacks: Vec<Arc<Box<dyn LlmClient>>>,
    /// Health status for each model
    health: Arc<RwLock<Vec<ModelHealth>>>,
}

impl FallbackLlmClient {
    /// Create a new fallback client
    pub async fn new(config: FallbackConfig) -> Result<Self, LlmError> {
        let primary = Self::create_client(&config.primary).await?;
        
        let mut fallbacks = Vec::new();
        for fallback_config in &config.fallbacks {
            match Self::create_client(fallback_config).await {
                Ok(client) => fallbacks.push(client),
                Err(e) => {
                    warn!("Failed to create fallback client for {}: {}", 
                          fallback_config.model, e);
                }
            }
        }
        
        let health_count = 1 + fallbacks.len();
        let health = Arc::new(RwLock::new(
            vec![ModelHealth::default(); health_count]
        ));
        
        info!("Fallback LLM client initialized with {} fallback models", 
              fallbacks.len());
        
        Ok(Self {
            config,
            primary,
            fallbacks,
            health,
        })
    }
    
    /// Create a client from configuration
    async fn create_client(config: &ModelConfig) -> Result<Arc<Box<dyn LlmClient>>, LlmError> {
        use crate::cognitive::llm::{OpenAiClient, OllamaClient};
        
        let client: Box<dyn LlmClient> = match config.provider.as_str() {
            "openai" | "anthropic" | "deepseek" => {
                Box::new(OpenAiClient::new(&config.model).map_err(|e| LlmError::ConfigError(e.to_string()))?)
            }
            "kimi" => {
                Box::new(KimiClient::new(&config.model).map_err(|e| LlmError::ConfigError(e.to_string()))?)
            }
            "zhipu" | "glm" => {
                Box::new(ZhipuClient::new(&config.model).map_err(|e| LlmError::ConfigError(e.to_string()))?)
            }
            "ollama" | "local" => {
                Box::new(OllamaClient::new(&config.model))
            }
            _ => {
                return Err(LlmError::ConfigError(
                    format!("Unknown provider: {}", config.provider)
                ));
            }
        };
        
        Ok(Arc::new(client))
    }
    
    /// Complete with fallback
    pub async fn complete(&self, prompt: &str) -> Result<String, LlmError> {
        let messages = vec![Message::user(prompt.to_string())];
        self.complete_with_messages(&messages).await
    }

    /// Complete with messages and fallback
    pub async fn complete_with_messages(&self, messages: &[Message]) -> Result<String, LlmError> {
        // Try primary first
        if self.is_model_available(0).await {
            match self.try_complete(0, self.primary.clone(), messages).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    warn!("Primary model failed: {}, trying fallbacks...", e);
                    self.record_failure(0).await;
                }
            }
        } else {
            warn!("Primary model circuit breaker is open, skipping...");
        }
        
        // Try fallbacks in order
        for (idx, fallback) in self.fallbacks.iter().enumerate() {
            let health_idx = idx + 1;
            
            if !self.is_model_available(health_idx).await {
                warn!("Fallback model {} circuit breaker is open, skipping...", idx);
                continue;
            }
            
            match self.try_complete(health_idx, fallback.clone(), messages).await {
                Ok(response) => {
                    info!("Fallback model {} succeeded", idx);
                    return Ok(response);
                }
                Err(e) => {
                    warn!("Fallback model {} failed: {}", idx, e);
                    self.record_failure(health_idx).await;
                }
            }
        }
        
        error!("All models failed, no fallback available");
        Err(LlmError::AllModelsFailed)
    }
    
    /// Try to complete with a specific model
    async fn try_complete(
        &self,
        health_idx: usize,
        client: Arc<Box<dyn LlmClient>>,
        messages: &[Message],
    ) -> Result<String, LlmError> {
        let start = Instant::now();
        
        let timeout = Duration::from_secs(self.config.timeout_secs);
        let result = tokio::time::timeout(
            timeout,
            client.chat_complete(messages)
        ).await;
        
        let latency = start.elapsed();
        
        match result {
            Ok(Ok(response)) => {
                self.record_success(health_idx, latency).await;
                Ok(response)
            }
            Ok(Err(_e)) => {
                self.record_failure(health_idx).await;
                Err(LlmError::Timeout)
            }
            Err(_) => {
                self.record_failure(health_idx).await;
                Err(LlmError::Timeout)
            }
        }
    }
    
    /// Check if a model is available (circuit breaker)
    async fn is_model_available(&self, health_idx: usize) -> bool {
        let health = self.health.read().await;
        let model_health = match health.get(health_idx) {
            Some(h) => h,
            None => return false,
        };
        
        match model_health.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if enough time has passed to try half-open
                if let Some(last_failure) = model_health.last_failure {
                    let reset_duration = Duration::from_secs(
                        self.config.circuit_breaker_reset_secs
                    );
                    if last_failure.elapsed() > reset_duration {
                        drop(health);
                        self.transition_to_half_open(health_idx).await;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }
    
    /// Record a successful request
    async fn record_success(&self, health_idx: usize, latency: Duration) {
        let mut health = self.health.write().await;
        if let Some(model_health) = health.get_mut(health_idx) {
            model_health.consecutive_failures = 0;
            model_health.last_success = Some(Instant::now());
            model_health.total_requests += 1;
            model_health.successful_requests += 1;
            
            // Update average latency
            let latency_ms = latency.as_millis() as f64;
            model_health.average_latency_ms = 
                (model_health.average_latency_ms * 0.9) + (latency_ms * 0.1);
            
            // If half-open and success, close the circuit
            if model_health.state == CircuitState::HalfOpen {
                model_health.state = CircuitState::Closed;
                info!("Model {} circuit breaker closed (recovered)", health_idx);
            }
        }
    }
    
    /// Record a failed request
    async fn record_failure(&self, health_idx: usize) {
        let mut health = self.health.write().await;
        if let Some(model_health) = health.get_mut(health_idx) {
            model_health.consecutive_failures += 1;
            model_health.last_failure = Some(Instant::now());
            model_health.total_requests += 1;
            model_health.failed_requests += 1;
            
            // Check if we should open the circuit
            if self.config.enable_circuit_breaker &&
               model_health.consecutive_failures >= self.config.circuit_breaker_threshold {
                model_health.state = CircuitState::Open;
                error!("Model {} circuit breaker opened after {} failures", 
                       health_idx, model_health.consecutive_failures);
            }
        }
    }
    
    /// Transition to half-open state
    async fn transition_to_half_open(&self, health_idx: usize) {
        let mut health = self.health.write().await;
        if let Some(model_health) = health.get_mut(health_idx) {
            model_health.state = CircuitState::HalfOpen;
            info!("Model {} circuit breaker half-open (testing recovery)", health_idx);
        }
    }
    
    /// Get health status for all models
    pub async fn get_health_status(&self) -> Vec<ModelHealth> {
        self.health.read().await.clone()
    }
    
    /// Force reset circuit breaker for a model
    pub async fn reset_circuit_breaker(&self, health_idx: usize) {
        let mut health = self.health.write().await;
        if let Some(model_health) = health.get_mut(health_idx) {
            model_health.state = CircuitState::Closed;
            model_health.consecutive_failures = 0;
            info!("Model {} circuit breaker manually reset", health_idx);
        }
    }
}

#[async_trait]
impl LlmClient for FallbackLlmClient {
    async fn chat_complete(&self, messages: &[Message]) -> anyhow::Result<String> {
        match self.complete_with_messages(messages).await {
            Ok(response) => Ok(response),
            Err(e) => Err(anyhow::anyhow!("Fallback LLM error: {}", e)),
        }
    }
    
    async fn chat_complete_with_tools(&self, messages: &[Message], _tools: &[serde_json::Value]) -> anyhow::Result<Message> {
        let text = self.chat_complete(messages).await?;
        Ok(Message::new("assistant", &text))
    }
    
    fn model_name(&self) -> &str {
        "fallback-llm"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_fallback_config_default() {
        let config = FallbackConfig::default();
        assert_eq!(config.max_retries, 2);
        assert_eq!(config.timeout_secs, 30);
        assert!(config.enable_circuit_breaker);
    }
    
    #[tokio::test]
    async fn test_model_health_default() {
        let health = ModelHealth::default();
        assert_eq!(health.state, CircuitState::Closed);
        assert_eq!(health.consecutive_failures, 0);
    }
}
