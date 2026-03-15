//! L1: Session Layer - Real-time Context
//!
//! The Session layer manages real-time conversation context, including:
//! - Current conversation messages
//! - Token usage tracking
//! - Context compression
//! - Temporary state

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use tracing::{info, debug};

use crate::types::Message;
use crate::memory::fusion::MemoryError;

/// L1 Session Layer - Real-time conversation context
pub struct SessionLayer {
    /// Session ID
    session_id: String,
    
    /// Conversation messages
    messages: RwLock<Vec<Message>>,
    
    /// Token usage tracking
    token_usage: RwLock<TokenUsage>,
    
    /// Maximum tokens allowed
    max_tokens: usize,
    
    /// Compression threshold (percentage of max_tokens)
    compression_threshold: f64,
    
    /// Session metadata
    metadata: RwLock<SessionMetadata>,
    
    /// Temporary state
    temp_state: RwLock<HashMap<String, serde_json::Value>>,
    
    /// Last activity timestamp
    last_activity: RwLock<Instant>,
    
    /// Compression history
    compression_history: RwLock<Vec<CompressionRecord>>,
}

/// Token usage tracking
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Total tokens used
    pub total_tokens: usize,
    
    /// Prompt tokens
    pub prompt_tokens: usize,
    
    /// Completion tokens
    pub completion_tokens: usize,
    
    /// Tokens by message index
    pub message_tokens: Vec<usize>,
}

/// Session metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Session start time
    pub started_at: DateTime<Utc>,
    
    /// Last message time
    pub last_message_at: DateTime<Utc>,
    
    /// Total message count
    pub message_count: u64,
    
    /// Compression count
    pub compression_count: u64,
    
    /// Session title (auto-generated or user-set)
    pub title: Option<String>,
    
    /// Session tags
    pub tags: Vec<String>,
}

/// Compression record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionRecord {
    /// When compression occurred
    pub timestamp: DateTime<Utc>,
    
    /// Tokens before compression
    pub tokens_before: usize,
    
    /// Tokens after compression
    pub tokens_after: usize,
    
    /// Compression method used
    pub method: CompressionMethod,
    
    /// Messages removed/summarized
    pub messages_affected: usize,
}

/// Compression method
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CompressionMethod {
    /// Remove oldest messages
    Truncate,
    /// Summarize old messages
    Summarize,
    /// Extract key points
    Extract,
    /// Semantic compression
    Semantic,
}

/// Compression strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionStrategy {
    /// Light compression - only remove oldest messages
    Light,
    /// Moderate compression - summarize old messages
    Moderate,
    /// Deep compression - aggressive summarization
    Deep,
}

/// Session summary for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub message_count: usize,
    pub total_tokens: usize,
    pub title: Option<String>,
    pub summary: String,
    pub key_topics: Vec<String>,
}

impl SessionLayer {
    /// Create a new session
    pub fn new(session_id: String, max_tokens: usize) -> Self {
        let now = Utc::now();
        
        Self {
            session_id,
            messages: RwLock::new(Vec::new()),
            token_usage: RwLock::new(TokenUsage::default()),
            max_tokens,
            compression_threshold: 0.8, // Compress at 80% capacity
            metadata: RwLock::new(SessionMetadata {
                started_at: now,
                last_message_at: now,
                message_count: 0,
                compression_count: 0,
                title: None,
                tags: Vec::new(),
            }),
            temp_state: RwLock::new(HashMap::new()),
            last_activity: RwLock::new(Instant::now()),
            compression_history: RwLock::new(Vec::new()),
        }
    }
    
    /// Add a system message
    pub async fn add_system_message(&self, message: Message) -> Result<(), MemoryError> {
        // System message should always be first
        let mut messages = self.messages.write().await;
        
        // Remove existing system message if any
        messages.retain(|m| m.role != "system");
        
        // Insert at beginning
        messages.insert(0, message);
        
        // Update token count
        let mut usage = self.token_usage.write().await;
        let content_text = messages[0].text().unwrap_or_default();
        usage.total_tokens += self.estimate_tokens(&content_text);
        
        debug!("Added system message to session {}", self.session_id);
        Ok(())
    }
    
    /// Add a user message
    pub async fn add_user_message(&self, content: String) -> Result<(), MemoryError> {
        let message = Message::user(content);
        self.add_message(message).await
    }
    
    /// Add an assistant message
    pub async fn add_assistant_message(&self, content: String) -> Result<(), MemoryError> {
        let message = Message::assistant(content);
        self.add_message(message).await
    }
    
    /// Add a message
    async fn add_message(&self, message: Message) -> Result<(), MemoryError> {
        let content_text = message.text().unwrap_or_default();
        let tokens = self.estimate_tokens(&content_text);
        
        // Check if compression is needed
        {
            let usage = self.token_usage.read().await;
            let threshold = (self.max_tokens as f64 * self.compression_threshold) as usize;
            
            if usage.total_tokens + tokens > threshold {
                drop(usage);
                self.compress_context(CompressionStrategy::Light).await?;
            }
        }
        
        // Add message
        let mut messages = self.messages.write().await;
        messages.push(message);
        
        // Update token usage
        let mut usage = self.token_usage.write().await;
        usage.total_tokens += tokens;
        usage.message_tokens.push(tokens);
        
        // Update metadata
        let mut metadata = self.metadata.write().await;
        metadata.message_count += 1;
        metadata.last_message_at = Utc::now();
        
        // Update activity
        *self.last_activity.write().await = Instant::now();
        
        debug!(
            "Added message to session {} (tokens: {}, total: {})",
            self.session_id, tokens, usage.total_tokens
        );
        
        Ok(())
    }
    
    /// Get all messages
    pub async fn get_messages(&self) -> Vec<Message> {
        self.messages.read().await.clone()
    }
    
    /// Get messages for LLM API (with context window management)
    pub async fn get_context_messages(&self) -> Vec<Message> {
        let messages = self.messages.read().await;
        
        // If within token limit, return all
        let usage = self.token_usage.read().await;
        if usage.total_tokens <= self.max_tokens {
            return messages.clone();
        }
        
        // Otherwise, need to truncate
        // Keep system message and most recent messages
        let mut result = Vec::new();
        
        // Always include system message if present
        if let Some(first) = messages.first() {
            if first.role == "system" {
                result.push(first.clone());
            }
        }
        
        // Add recent messages until we hit the limit
        let mut current_tokens: usize = result.iter()
            .map(|m| self.estimate_tokens(&m.text().unwrap_or_default()))
            .sum();
        
        for message in messages.iter().rev() {
            let content_text = message.text().unwrap_or_default();
            let tokens = self.estimate_tokens(&content_text);
            
            if current_tokens + tokens > self.max_tokens {
                break;
            }
            
            // Skip system message (already added)
            if message.role == "system" {
                continue;
            }
            
            result.push(message.clone());
            current_tokens += tokens;
        }
        
        // Reverse to maintain chronological order
        result.reverse();
        
        result
    }
    
    /// Compress context to free up tokens
    async fn compress_context(&self, strategy: CompressionStrategy) -> Result<(), MemoryError> {
        info!("Compressing session {} with strategy {:?}", self.session_id, strategy);
        
        let mut messages = self.messages.write().await;
        let original_count = messages.len();
        
        if original_count <= 2 {
            // Can't compress further
            return Ok(());
        }
        
        let (method, messages_to_keep) = match strategy {
            CompressionStrategy::Light => {
                // Remove oldest 20% of non-system messages
                let non_system = messages.iter()
                    .enumerate()
                    .filter(|(_, m)| m.role != "system")
                    .map(|(i, _)| i)
                    .collect::<Vec<_>>();
                
                let remove_count = (non_system.len() as f64 * 0.2) as usize;
                let keep_indices: std::collections::HashSet<_> = non_system
                    .into_iter()
                    .skip(remove_count)
                    .collect();
                
                (CompressionMethod::Truncate, keep_indices)
            }
            CompressionStrategy::Moderate => {
                // Summarize old messages
                (CompressionMethod::Summarize, self.summarize_old_messages(&messages).await?)
            }
            CompressionStrategy::Deep => {
                // Aggressive compression
                (CompressionMethod::Semantic, self.semantic_compression(&messages).await?)
            }
        };
        
        // Apply compression
        let mut new_messages = Vec::new();
        let mut removed_count = 0;
        
        for (idx, message) in messages.iter().enumerate() {
            if message.role == "system" || messages_to_keep.contains(&idx) {
                new_messages.push(message.clone());
            } else {
                removed_count += 1;
            }
        }
        
        // Update messages
        *messages = new_messages;
        drop(messages);
        
        // Recalculate token usage
        self.recalculate_tokens().await?;
        
        // Record compression
        let usage = self.token_usage.read().await;
        let mut history = self.compression_history.write().await;
        history.push(CompressionRecord {
            timestamp: Utc::now(),
            tokens_before: usage.total_tokens + (removed_count * 100), // Estimate
            tokens_after: usage.total_tokens,
            method,
            messages_affected: removed_count,
        });
        drop(history);
        drop(usage);
        
        // Update metadata
        let mut metadata = self.metadata.write().await;
        metadata.compression_count += 1;
        
        info!(
            "Compressed session {}: removed {} messages",
            self.session_id, removed_count
        );
        
        Ok(())
    }
    
    /// Summarize old messages
    async fn summarize_old_messages(&self, messages: &[Message]) -> Result<std::collections::HashSet<usize>, MemoryError> {
        // In a real implementation, this would use an LLM to summarize
        // For now, just keep the most recent 50% and system message
        let mut keep = std::collections::HashSet::new();
        
        for (idx, message) in messages.iter().enumerate() {
            if message.role == "system" {
                keep.insert(idx);
            } else if idx >= messages.len() / 2 {
                keep.insert(idx);
            }
        }
        
        Ok(keep)
    }
    
    /// Semantic compression
    async fn semantic_compression(&self, messages: &[Message]) -> Result<std::collections::HashSet<usize>, MemoryError> {
        // Keep system message and last 3 exchanges
        let mut keep = std::collections::HashSet::new();
        
        for (idx, message) in messages.iter().enumerate() {
            if message.role == "system" {
                keep.insert(idx);
            } else if idx >= messages.len().saturating_sub(6) {
                keep.insert(idx);
            }
        }
        
        Ok(keep)
    }
    
    /// Recalculate token usage
    async fn recalculate_tokens(&self) -> Result<(), MemoryError> {
        let messages = self.messages.read().await;
        let mut usage = self.token_usage.write().await;
        
        usage.total_tokens = 0;
        usage.message_tokens.clear();
        
        for message in messages.iter() {
            let content_text = message.text().unwrap_or_default();
            let tokens = self.estimate_tokens(&content_text);
            usage.total_tokens += tokens;
            usage.message_tokens.push(tokens);
        }
        
        Ok(())
    }
    
    /// Estimate tokens for content
    fn estimate_tokens(&self, content: &str) -> usize {
        // Rough estimate: 1 token ≈ 4 characters for English
        // More sophisticated estimation would use tiktoken or similar
        (content.len() / 4).max(1)
    }
    
    /// Set temporary state
    pub async fn set_temp_state(&self, key: String, value: serde_json::Value) {
        let mut state = self.temp_state.write().await;
        state.insert(key, value);
        *self.last_activity.write().await = Instant::now();
    }
    
    /// Get temporary state
    pub async fn get_temp_state(&self, key: &str) -> Option<serde_json::Value> {
        self.temp_state.read().await.get(key).cloned()
    }
    
    /// Get session ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
    
    /// Get message count
    pub async fn message_count(&self) -> usize {
        self.messages.read().await.len()
    }
    
    /// Get token usage
    pub async fn token_usage(&self) -> TokenUsage {
        self.token_usage.read().await.clone()
    }
    
    /// Get last activity
    pub async fn last_activity(&self) -> Instant {
        *self.last_activity.read().await
    }
    
    /// Get metadata
    pub async fn metadata(&self) -> SessionMetadata {
        self.metadata.read().await.clone()
    }
    
    /// Set session title
    pub async fn set_title(&self, title: String) {
        let mut metadata = self.metadata.write().await;
        metadata.title = Some(title);
    }
    
    /// Add tag
    pub async fn add_tag(&self, tag: String) {
        let mut metadata = self.metadata.write().await;
        if !metadata.tags.contains(&tag) {
            metadata.tags.push(tag);
        }
    }
    
    /// Generate session summary
    pub async fn generate_summary(&self) -> SessionSummary {
        let messages = self.messages.read().await;
        let metadata = self.metadata.read().await;
        let usage = self.token_usage.read().await;
        
        // Extract key topics (simplified)
        let key_topics = self.extract_topics(&messages).await;
        
        SessionSummary {
            session_id: self.session_id.clone(),
            started_at: metadata.started_at,
            ended_at: Utc::now(),
            message_count: messages.len(),
            total_tokens: usage.total_tokens,
            title: metadata.title.clone(),
            summary: self.generate_brief_summary(&messages).await,
            key_topics,
        }
    }
    
    /// Extract topics from messages
    async fn extract_topics(&self, messages: &[Message]) -> Vec<String> {
        // Simplified topic extraction
        // In a real implementation, this would use NLP
        let mut topics = std::collections::HashSet::new();
        
        for message in messages {
            let content = message.text().unwrap_or_default().to_lowercase();
            
            // Simple keyword matching
            if content.contains("code") || content.contains("programming") {
                topics.insert("coding".to_string());
            }
            if content.contains("memory") || content.contains("remember") {
                topics.insert("memory".to_string());
            }
            if content.contains("tool") || content.contains("function") {
                topics.insert("tools".to_string());
            }
            if content.contains("config") || content.contains("setting") {
                topics.insert("configuration".to_string());
            }
        }
        
        topics.into_iter().collect()
    }
    
    /// Generate brief summary
    async fn generate_brief_summary(&self, messages: &[Message]) -> String {
        if messages.is_empty() {
            return "Empty session".to_string();
        }
        
        // Get first user message as indicator
        let first_user = messages.iter()
            .find(|m| m.role == "user")
            .map(|m| m.text().unwrap_or_default())
            .unwrap_or_else(|| "Session started".to_string());
        
        // Truncate if too long
        if first_user.len() > 100 {
            format!("{}...", &first_user[..100])
        } else {
            first_user
        }
    }
    
    /// Persist session to storage
    pub async fn persist(&self) -> Result<(), MemoryError> {
        let path = PathBuf::from("./sessions").join(format!("{}.json", self.session_id));
        
        // Ensure directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        let data = SessionPersistenceData {
            session_id: self.session_id.clone(),
            messages: self.messages.read().await.clone(),
            token_usage: self.token_usage.read().await.clone(),
            metadata: self.metadata.read().await.clone(),
            temp_state: self.temp_state.read().await.clone(),
            compression_history: self.compression_history.read().await.clone(),
        };
        
        let content = serde_json::to_string_pretty(&data)
            .map_err(|e| MemoryError::PersistenceError(e.to_string()))?;
        
        tokio::fs::write(&path, content).await?;
        
        debug!("Persisted session {} to {:?}", self.session_id, path);
        Ok(())
    }
    
    /// Load session from storage
    pub async fn load(session_id: &str) -> Result<Self, MemoryError> {
        let path = PathBuf::from("./sessions").join(format!("{}.json", session_id));
        
        let content = tokio::fs::read_to_string(&path).await?;
        let data: SessionPersistenceData = serde_json::from_str(&content)
            .map_err(|e| MemoryError::PersistenceError(e.to_string()))?;
        
        let layer = Self {
            session_id: data.session_id,
            messages: RwLock::new(data.messages),
            token_usage: RwLock::new(data.token_usage),
            max_tokens: 8000, // Default
            compression_threshold: 0.8,
            metadata: RwLock::new(data.metadata),
            temp_state: RwLock::new(data.temp_state),
            last_activity: RwLock::new(Instant::now()),
            compression_history: RwLock::new(data.compression_history),
        };
        
        info!("Loaded session {} from storage", session_id);
        Ok(layer)
    }
}

/// Session persistence data
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionPersistenceData {
    pub session_id: String,
    pub messages: Vec<Message>,
    pub token_usage: TokenUsage,
    pub metadata: SessionMetadata,
    pub temp_state: HashMap<String, serde_json::Value>,
    pub compression_history: Vec<CompressionRecord>,
}
