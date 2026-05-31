//! L1: Session Layer - Real-time Context
//!
//! The Session layer manages real-time conversation context, including:
//! - Current conversation messages
//! - Token usage tracking
//! - Context compression
//! - Temporary state

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::memory::fusion::MemoryError;
use crate::types::Message;

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
        drop(messages);
        self.recalculate_tokens().await?;
        *self.last_activity.write().await = Instant::now();

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

    /// Add a message using a runtime role string.
    pub async fn add_role_message(&self, role: &str, content: String) -> Result<(), MemoryError> {
        match role {
            "system" => self.add_system_message(Message::system(content)).await,
            "user" => self.add_user_message(content).await,
            "assistant" => self.add_assistant_message(content).await,
            other => self.add_message(Message::new(other, &content)).await,
        }
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
                self.compress_context(CompressionStrategy::Moderate).await?;
            }
        }

        // Add message
        {
            let mut messages = self.messages.write().await;
            messages.push(message);
        }

        // Update token usage
        let total_tokens = {
            let mut usage = self.token_usage.write().await;
            usage.total_tokens += tokens;
            usage.message_tokens.push(tokens);
            usage.total_tokens
        };

        // Update metadata
        {
            let mut metadata = self.metadata.write().await;
            metadata.message_count += 1;
            metadata.last_message_at = Utc::now();
        }

        // Update activity
        *self.last_activity.write().await = Instant::now();

        debug!(
            "Added message to session {} (tokens: {}, total: {})",
            self.session_id, tokens, total_tokens
        );

        Ok(())
    }

    /// Get all messages
    pub async fn get_messages(&self) -> Vec<Message> {
        self.messages.read().await.clone()
    }

    /// Get messages for LLM API (with context window management)
    pub async fn get_context_messages(&self) -> Vec<Message> {
        let messages = self.messages.read().await.clone();

        // If within token limit, return all
        let total_tokens = self.token_usage.read().await.total_tokens;
        if total_tokens <= self.max_tokens {
            return messages;
        }

        // Otherwise, need to truncate
        // Keep system messages in order plus the most recent non-system messages.
        let system_messages: Vec<Message> = messages
            .iter()
            .filter(|message| message.role == "system")
            .cloned()
            .collect();
        let mut recent_messages = Vec::new();

        // Add recent messages until we hit the limit
        let mut current_tokens: usize = system_messages
            .iter()
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

            recent_messages.push(message.clone());
            current_tokens += tokens;
        }

        recent_messages.reverse();

        let mut result = system_messages;
        result.extend(recent_messages);
        result
    }

    /// Compress context to free up tokens
    async fn compress_context(&self, strategy: CompressionStrategy) -> Result<(), MemoryError> {
        info!(
            "Compressing session {} with strategy {:?}",
            self.session_id, strategy
        );

        let original_tokens = self.token_usage.read().await.total_tokens;
        let mut messages = self.messages.write().await;
        let snapshot = messages.clone();
        let original_count = snapshot.len();

        if original_count <= 2 {
            return Ok(());
        }

        let primary_system = snapshot
            .iter()
            .find(|message| message.role == "system")
            .cloned();
        let non_system_messages: Vec<Message> = snapshot
            .iter()
            .filter(|message| message.role != "system")
            .cloned()
            .collect();

        if non_system_messages.len() <= 2 {
            return Ok(());
        }

        let (method, keep_recent) = match strategy {
            CompressionStrategy::Light => (
                CompressionMethod::Truncate,
                ((non_system_messages.len() as f64) * 0.75).ceil() as usize,
            ),
            CompressionStrategy::Moderate => (
                CompressionMethod::Summarize,
                non_system_messages.len().min(6),
            ),
            CompressionStrategy::Deep => (
                CompressionMethod::Semantic,
                non_system_messages.len().min(4),
            ),
        };

        let keep_recent = keep_recent.max(1).min(non_system_messages.len());
        let split_index = non_system_messages.len().saturating_sub(keep_recent);
        let older_messages = &non_system_messages[..split_index];
        let recent_messages = &non_system_messages[split_index..];

        let summary_message = match strategy {
            CompressionStrategy::Light => None,
            CompressionStrategy::Moderate | CompressionStrategy::Deep => {
                self.build_summary_message(older_messages, method)
            }
        };

        let mut new_messages = Vec::new();
        if let Some(system_message) = primary_system {
            new_messages.push(system_message);
        }
        if let Some(summary_message) = summary_message {
            new_messages.push(summary_message);
        }
        new_messages.extend(recent_messages.iter().cloned());

        let new_count = new_messages.len();
        *messages = new_messages;
        drop(messages);

        self.recalculate_tokens().await?;
        let tokens_after = self.token_usage.read().await.total_tokens;

        let mut history = self.compression_history.write().await;
        history.push(CompressionRecord {
            timestamp: Utc::now(),
            tokens_before: original_tokens,
            tokens_after,
            method,
            messages_affected: original_count.saturating_sub(new_count),
        });
        drop(history);

        let mut metadata = self.metadata.write().await;
        metadata.compression_count += 1;

        info!(
            "Compressed session {}: {} -> {} messages",
            self.session_id, original_count, new_count
        );

        Ok(())
    }

    /// Recalculate token usage
    async fn recalculate_tokens(&self) -> Result<(), MemoryError> {
        let messages = self.messages.read().await.clone();
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
        {
            let mut state = self.temp_state.write().await;
            state.insert(key, value);
        }
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
        let messages = self.messages.read().await.clone();
        let metadata = self.metadata.read().await.clone();
        let usage = self.token_usage.read().await.clone();

        // Extract key topics (simplified)
        let key_topics = Self::extract_topics(&messages);

        SessionSummary {
            session_id: self.session_id.clone(),
            started_at: metadata.started_at,
            ended_at: Utc::now(),
            message_count: messages.len(),
            total_tokens: usage.total_tokens,
            title: metadata.title.clone(),
            summary: Self::generate_brief_summary(&messages),
            key_topics,
        }
    }

    /// Extract topics from messages
    fn extract_topics(messages: &[Message]) -> Vec<String> {
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
            if content.contains("agent") || content.contains("framework") {
                topics.insert("agents".to_string());
            }
            if content.contains("route") || content.contains("router") {
                topics.insert("routing".to_string());
            }
            if content.contains("rust") {
                topics.insert("rust".to_string());
            }
        }

        topics.into_iter().collect()
    }

    /// Generate brief summary
    fn generate_brief_summary(messages: &[Message]) -> String {
        if messages.is_empty() {
            return "Empty session".to_string();
        }

        let topics = Self::extract_topics(messages);
        let first_user = messages
            .iter()
            .find(|m| m.role == "user")
            .and_then(|m| m.text())
            .map(|text| Self::truncate_text(&text, 96))
            .unwrap_or_else(|| "Session started".to_string());

        if topics.is_empty() {
            first_user
        } else {
            format!("{} Topics: {}.", first_user, topics.join(", "))
        }
    }

    fn build_summary_message(
        &self,
        messages: &[Message],
        method: CompressionMethod,
    ) -> Option<Message> {
        if messages.is_empty() {
            return None;
        }

        let topics = Self::extract_topics(messages);
        let user_signals = Self::collect_recent_lines(messages, "user", 2);
        let assistant_signals = Self::collect_recent_lines(messages, "assistant", 1);

        let mut lines = vec![format!(
            "Conversation summary ({:?}, {} messages compressed).",
            method,
            messages.len()
        )];

        if !topics.is_empty() {
            lines.push(format!("Topics: {}.", topics.join(", ")));
        }

        if !user_signals.is_empty() {
            lines.push(format!("Recent user intent: {}.", user_signals.join(" | ")));
        }

        if !assistant_signals.is_empty() {
            lines.push(format!(
                "Established assistant context: {}.",
                assistant_signals.join(" | ")
            ));
        }

        Some(Message::system(lines.join("\n")))
    }

    fn collect_recent_lines(messages: &[Message], role: &str, limit: usize) -> Vec<String> {
        let mut lines = Vec::new();

        for text in messages
            .iter()
            .filter(|message| message.role == role)
            .filter_map(|message| message.text())
            .rev()
        {
            let normalized = Self::truncate_text(&text, 96);
            if normalized.is_empty() || lines.contains(&normalized) {
                continue;
            }

            lines.push(normalized);
            if lines.len() >= limit {
                break;
            }
        }

        lines.reverse();
        lines
    }

    fn truncate_text(text: &str, max_chars: usize) -> String {
        let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let trimmed = normalized.trim();

        if trimmed.chars().count() <= max_chars {
            return trimmed.to_string();
        }

        let mut result = String::new();
        for ch in trimmed.chars().take(max_chars.saturating_sub(3)) {
            result.push(ch);
        }
        result.push_str("...");
        result
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn compression_inserts_summary_message_and_keeps_recent_context() {
        let session = SessionLayer::new("compression-test".to_string(), 80);
        session
            .add_system_message(Message::system("Base instructions"))
            .await
            .unwrap();

        for idx in 0..6 {
            session
                .add_user_message(format!(
                    "I want to build a Rust agent framework with memory and routing step {}",
                    idx
                ))
                .await
                .unwrap();
            session
                .add_assistant_message(format!(
                    "Acknowledged. We will keep working on the memory and routing design step {}",
                    idx
                ))
                .await
                .unwrap();
        }

        let messages = session.get_messages().await;
        assert!(messages.iter().any(|message| {
            message.role == "system"
                && message
                    .text()
                    .unwrap_or_default()
                    .contains("Conversation summary")
        }));
        assert!(messages
            .iter()
            .any(|message| { message.text().unwrap_or_default().contains("step 5") }));
    }

    #[tokio::test]
    async fn generated_summary_surfaces_topics() {
        let session = SessionLayer::new("summary-test".to_string(), 512);
        session
            .add_user_message("Please help me improve the Rust memory routing layer".to_string())
            .await
            .unwrap();
        session
            .add_assistant_message("We can refine the agent memory and router design.".to_string())
            .await
            .unwrap();

        let summary = session.generate_summary().await;
        assert!(summary.summary.to_lowercase().contains("topics"));
        assert!(summary.key_topics.iter().any(|topic| topic == "rust"));
        assert!(summary.key_topics.iter().any(|topic| topic == "memory"));
    }

    #[tokio::test]
    async fn context_messages_keep_system_prompts_at_the_front() {
        let session = SessionLayer::new("context-order-test".to_string(), 10);
        session
            .add_system_message(Message::system("System rules"))
            .await
            .unwrap();
        session
            .add_user_message("Need terse answers".to_string())
            .await
            .unwrap();
        session
            .add_assistant_message("Understood and noted".to_string())
            .await
            .unwrap();

        let context_messages = session.get_context_messages().await;
        assert_eq!(
            context_messages
                .first()
                .map(|message| message.role.as_str()),
            Some("system")
        );
        assert!(context_messages
            .iter()
            .skip(1)
            .all(|message| message.role != "system"));
    }
}
