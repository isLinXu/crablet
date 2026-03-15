//! Daily Logs - OpenClaw Style Append-Only Logs
//!
//! Daily Logs provide a chronological, append-only record of all sessions
//! and events. This enables:
//! - Cross-session context continuity
//! - Memory extraction and consolidation
//! - Audit trail
//! - Pattern recognition

use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, NaiveDate, Local};
use tracing::{info, debug};

use crate::memory::fusion::MemoryError;
use crate::memory::fusion::layer_session::{SessionLayer, SessionSummary};

/// Daily logs configuration (local definition)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyLogsConfig {
    pub enabled: bool,
    pub storage_path: String,
    pub context_window_days: usize,
    pub auto_extract_memories: bool,
}

/// Daily Logs manager
pub struct DailyLogs {
    /// Configuration
    config: DailyLogsConfig,
    
    /// Storage path
    storage_path: PathBuf,
    
    /// Current day's log (cached)
    current_log: RwLock<Option<DailyLog>>,
    
    /// Log index by date
    log_index: RwLock<HashMap<NaiveDate, PathBuf>>,
}

/// Single daily log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyLog {
    /// Date of the log
    pub date: NaiveDate,
    
    /// Log entries
    pub entries: Vec<LogEntry>,
    
    /// Session summaries for this day
    pub sessions: Vec<SessionSummary>,
    
    /// Daily summary
    pub summary: String,
    
    /// Key topics discussed
    pub topics: Vec<String>,
    
    /// Metadata
    pub metadata: DailyLogMetadata,
}

/// Log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Entry timestamp
    pub timestamp: DateTime<Utc>,
    
    /// Entry type
    pub entry_type: LogEntryType,
    
    /// Session ID (if applicable)
    pub session_id: Option<String>,
    
    /// Entry content
    pub content: String,
    
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Log entry type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LogEntryType {
    /// Session started
    SessionStart,
    /// Session ended
    SessionEnd,
    /// User message
    UserMessage,
    /// Assistant message
    AssistantMessage,
    /// Tool invocation
    ToolInvocation,
    /// Memory recorded
    MemoryRecorded,
    /// System event
    SystemEvent,
    /// Error occurred
    Error,
}

/// Daily log metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyLogMetadata {
    /// Created at
    pub created_at: DateTime<Utc>,
    
    /// Last updated
    pub updated_at: DateTime<Utc>,
    
    /// Total entries
    pub entry_count: u64,
    
    /// Total sessions
    pub session_count: u64,
}

/// Log event type (for logging convenience)
#[derive(Debug, Clone)]
pub enum LogEventType {
    SessionStart,
    SessionEnd,
    Message,
    ToolCall,
    Memory,
    System,
    Error,
}

impl DailyLogs {
    /// Initialize Daily Logs from configuration
    pub async fn from_config(config: &DailyLogsConfig) -> Result<Self, MemoryError> {
        info!("Initializing Daily Logs...");
        
        let storage_path = PathBuf::from(&config.storage_path);
        
        // Create storage directory
        if !storage_path.exists() {
            tokio::fs::create_dir_all(&storage_path).await?;
            debug!("Created Daily Logs directory: {:?}", storage_path);
        }
        
        // Build index of existing logs
        let log_index = Self::build_index(&storage_path).await?;
        
        let logs = Self {
            config: config.clone(),
            storage_path,
            current_log: RwLock::new(None),
            log_index: RwLock::new(log_index),
        };
        
        // Load or create today's log
        let today = Local::now().date_naive();
        let today_log = logs.load_or_create_log(today).await?;
        *logs.current_log.write().await = Some(today_log);
        
        info!("Daily Logs initialized");
        Ok(logs)
    }
    
    /// Build index of existing logs
    async fn build_index(storage_path: &PathBuf) -> Result<HashMap<NaiveDate, PathBuf>, MemoryError> {
        let mut index = HashMap::new();
        
        let mut entries = tokio::fs::read_dir(storage_path).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            if path.extension().map_or(false, |ext| ext == "md") {
                // Parse filename to get date
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(date) = NaiveDate::parse_from_str(stem, "%Y-%m-%d") {
                        index.insert(date, path);
                    }
                }
            }
        }
        
        debug!("Indexed {} daily logs", index.len());
        Ok(index)
    }
    
    /// Load or create log for a specific date
    async fn load_or_create_log(&self, date: NaiveDate) -> Result<DailyLog, MemoryError> {
        let path = self.get_log_path(date);
        
        if path.exists() {
            // Load existing log
            let content = tokio::fs::read_to_string(&path).await?;
            let log = Self::parse_markdown(&content)?;
            debug!("Loaded log for {}", date);
            Ok(log)
        } else {
            // Create new log
            let now = Utc::now();
            let log = DailyLog {
                date,
                entries: Vec::new(),
                sessions: Vec::new(),
                summary: String::new(),
                topics: Vec::new(),
                metadata: DailyLogMetadata {
                    created_at: now,
                    updated_at: now,
                    entry_count: 0,
                    session_count: 0,
                },
            };
            debug!("Created new log for {}", date);
            Ok(log)
        }
    }
    
    /// Get log file path for a date
    fn get_log_path(&self, date: NaiveDate) -> PathBuf {
        self.storage_path.join(format!("{}.md", date.format("%Y-%m-%d")))
    }
    
    /// Parse Markdown log file
    fn parse_markdown(content: &str) -> Result<DailyLog, MemoryError> {
        // Parse frontmatter
        let mut lines = content.lines();
        
        // Skip opening ---
        lines.next();
        
        let mut frontmatter = String::new();
        for line in lines.by_ref() {
            if line == "---" {
                break;
            }
            frontmatter.push_str(line);
            frontmatter.push('\n');
        }
        
        let metadata: serde_yaml::Value = serde_yaml::from_str(&frontmatter)
            .map_err(|e| MemoryError::PersistenceError(format!("YAML parse error: {}", e)))?;
        
        let date_str = metadata["date"].as_str().unwrap_or("");
        let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .map_err(|e| MemoryError::PersistenceError(format!("Date parse error: {}", e)))?;
        
        // Parse entries (simplified)
        let mut entries = Vec::new();
        let mut current_entry: Option<LogEntry> = None;
        
        for line in lines {
            if line.starts_with("## ") {
                // Save previous entry
                if let Some(entry) = current_entry.take() {
                    entries.push(entry);
                }
                
                // Parse timestamp from header
                let timestamp_str = line.trim_start_matches("## ");
                if let Ok(timestamp) = DateTime::parse_from_rfc3339(timestamp_str) {
                    current_entry = Some(LogEntry {
                        timestamp: timestamp.with_timezone(&Utc),
                        entry_type: LogEntryType::SystemEvent,
                        session_id: None,
                        content: String::new(),
                        metadata: HashMap::new(),
                    });
                }
            } else if let Some(ref mut entry) = current_entry {
                if line.starts_with("- **Type**:") {
                    let type_str = line.split(':').nth(1).unwrap_or("").trim();
                    entry.entry_type = match type_str {
                        "SessionStart" => LogEntryType::SessionStart,
                        "SessionEnd" => LogEntryType::SessionEnd,
                        "UserMessage" => LogEntryType::UserMessage,
                        "AssistantMessage" => LogEntryType::AssistantMessage,
                        "ToolInvocation" => LogEntryType::ToolInvocation,
                        "MemoryRecorded" => LogEntryType::MemoryRecorded,
                        "Error" => LogEntryType::Error,
                        _ => LogEntryType::SystemEvent,
                    };
                } else if line.starts_with("- **Session**:") {
                    entry.session_id = line.split(':').nth(1).map(|s| s.trim().to_string());
                } else if !line.is_empty() && !line.starts_with('-') {
                    entry.content.push_str(line);
                    entry.content.push('\n');
                }
            }
        }
        
        // Save last entry
        if let Some(entry) = current_entry {
            entries.push(entry);
        }
        
        let entry_count = entries.len() as u64;
        Ok(DailyLog {
            date,
            entries,
            sessions: Vec::new(),
            summary: String::new(),
            topics: Vec::new(),
            metadata: DailyLogMetadata {
                created_at: Utc::now(),
                updated_at: Utc::now(),
                entry_count,
                session_count: 0,
            },
        })
    }
    
    /// Log an event
    pub async fn log_event(
        &self,
        session_id: String,
        event_type: LogEventType,
        content: &str,
    ) -> Result<(), MemoryError> {
        let entry_type = match event_type {
            LogEventType::SessionStart => LogEntryType::SessionStart,
            LogEventType::SessionEnd => LogEntryType::SessionEnd,
            LogEventType::Message => LogEntryType::UserMessage,
            LogEventType::ToolCall => LogEntryType::ToolInvocation,
            LogEventType::Memory => LogEntryType::MemoryRecorded,
            LogEventType::System => LogEntryType::SystemEvent,
            LogEventType::Error => LogEntryType::Error,
        };
        
        let entry = LogEntry {
            timestamp: Utc::now(),
            entry_type,
            session_id: Some(session_id),
            content: content.to_string(),
            metadata: HashMap::new(),
        };
        
        // Add to current log
        let mut current = self.current_log.write().await;
        
        if let Some(ref mut log) = *current {
            log.entries.push(entry);
            log.metadata.entry_count += 1;
            log.metadata.updated_at = Utc::now();
            
            // Persist immediately
            drop(current);
            self.persist_current().await?;
        }
        
        debug!("Logged event: {:?}", event_type);
        Ok(())
    }
    
    /// Append session to daily log
    pub async fn append_session(&self, session: &SessionLayer) -> Result<(), MemoryError> {
        let summary = session.generate_summary().await;
        
        let mut current = self.current_log.write().await;
        
        if let Some(ref mut log) = *current {
            log.sessions.push(summary);
            log.metadata.session_count += 1;
            log.metadata.updated_at = Utc::now();
            
            // Update topics
            // (simplified - would use NLP in real implementation)
            
            // Persist
            drop(current);
            self.persist_current().await?;
        }
        
        debug!("Appended session {} to daily log", session.session_id());
        Ok(())
    }
    
    /// Load recent logs for context
    pub async fn load_recent(&self) -> Result<Vec<DailyLog>, MemoryError> {
        let days = self.config.context_window_days;
        let mut logs = Vec::new();
        
        let today = Local::now().date_naive();
        
        for i in 0..days {
            let date = today - chrono::Duration::days(i as i64);
            
            // Check if it's the current log
            let current = self.current_log.read().await;
            if let Some(ref log) = *current {
                if log.date == date {
                    logs.push(log.clone());
                    continue;
                }
            }
            drop(current);
            
            // Load from storage
            let log = self.load_or_create_log(date).await?;
            if !log.entries.is_empty() {
                logs.push(log);
            }
        }
        
        Ok(logs)
    }
    
    /// Get log for a specific date
    pub async fn get_log(&self, date: NaiveDate) -> Result<Option<DailyLog>, MemoryError> {
        // Check current log first
        let current = self.current_log.read().await;
        if let Some(ref log) = *current {
            if log.date == date {
                return Ok(Some(log.clone()));
            }
        }
        drop(current);
        
        // Load from storage
        let log = self.load_or_create_log(date).await?;
        if log.entries.is_empty() {
            Ok(None)
        } else {
            Ok(Some(log))
        }
    }
    
    /// Persist current log
    async fn persist_current(&self) -> Result<(), MemoryError> {
        let current = self.current_log.read().await;
        
        if let Some(ref log) = *current {
            let path = self.get_log_path(log.date);
            let content = self.format_as_markdown(log);
            
            tokio::fs::write(&path, content).await?;
            debug!("Persisted daily log to {:?}", path);
        }
        
        Ok(())
    }
    
    /// Format log as Markdown
    fn format_as_markdown(&self, log: &DailyLog) -> String {
        let mut content = String::new();
        
        // Frontmatter
        content.push_str("---\n");
        content.push_str(&format!("date: {}\n", log.date.format("%Y-%m-%d")));
        content.push_str(&format!("entry_count: {}\n", log.metadata.entry_count));
        content.push_str(&format!("session_count: {}\n", log.metadata.session_count));
        content.push_str(&format!("created_at: {}\n", log.metadata.created_at.to_rfc3339()));
        content.push_str(&format!("updated_at: {}\n", log.metadata.updated_at.to_rfc3339()));
        content.push_str("---\n\n");
        
        // Title
        content.push_str(&format!("# Daily Log: {}\n\n", log.date.format("%Y-%m-%d")));
        
        // Summary
        if !log.summary.is_empty() {
            content.push_str("## Summary\n\n");
            content.push_str(&log.summary);
            content.push_str("\n\n");
        }
        
        // Topics
        if !log.topics.is_empty() {
            content.push_str("## Topics\n\n");
            for topic in &log.topics {
                content.push_str(&format!("- {}\n", topic));
            }
            content.push('\n');
        }
        
        // Sessions
        if !log.sessions.is_empty() {
            content.push_str("## Sessions\n\n");
            for session in &log.sessions {
                content.push_str(&format!("### {}\n", session.session_id));
                content.push_str(&format!("- **Started**: {}\n", session.started_at.format("%H:%M:%S")));
                content.push_str(&format!("- **Messages**: {}\n", session.message_count));
                content.push_str(&format!("- **Tokens**: {}\n", session.total_tokens));
                if let Some(ref title) = session.title {
                    content.push_str(&format!("- **Title**: {}\n", title));
                }
                content.push_str(&format!("\n{}\n\n", session.summary));
            }
        }
        
        // Entries
        if !log.entries.is_empty() {
            content.push_str("## Events\n\n");
            for entry in &log.entries {
                content.push_str(&format!("### {}\n", entry.timestamp.to_rfc3339()));
                content.push_str(&format!("- **Type**: {:?}\n", entry.entry_type));
                if let Some(ref session_id) = entry.session_id {
                    content.push_str(&format!("- **Session**: {}\n", session_id));
                }
                content.push('\n');
                content.push_str(&entry.content);
                content.push_str("\n\n");
            }
        }
        
        content
    }
    
    /// Archive old logs
    pub async fn archive_old(&self, days_to_keep: u64) -> Result<usize, MemoryError> {
        let cutoff = Local::now().date_naive() - chrono::Duration::days(days_to_keep as i64);
        let mut archived = 0;
        
        let index = self.log_index.read().await;
        
        for (date, path) in index.iter() {
            if *date < cutoff {
                // Archive this log
                let archive_path = self.storage_path.join("archive").join(path.file_name().unwrap());
                
                if let Some(parent) = archive_path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                
                tokio::fs::rename(path, &archive_path).await?;
                archived += 1;
                
                info!("Archived log for {} to {:?}", date, archive_path);
            }
        }
        
        Ok(archived)
    }
    
    /// Generate daily summary
    pub async fn generate_summary(&self, date: NaiveDate) -> Result<String, MemoryError> {
        let log = self.get_log(date).await?;
        
        if let Some(log) = log {
            // In a real implementation, this would use an LLM to generate a summary
            let summary = format!(
                "On {}, there were {} sessions with {} total events. Topics included: {}",
                date.format("%Y-%m-%d"),
                log.metadata.session_count,
                log.metadata.entry_count,
                log.topics.join(", ")
            );
            
            Ok(summary)
        } else {
            Ok("No activity recorded".to_string())
        }
    }
    
    /// Search logs
    pub async fn search(&self, query: &str) -> Result<Vec<LogEntry>, MemoryError> {
        let mut results = Vec::new();
        
        // Search in current log
        let current = self.current_log.read().await;
        if let Some(ref log) = *current {
            for entry in &log.entries {
                if entry.content.to_lowercase().contains(&query.to_lowercase()) {
                    results.push(entry.clone());
                }
            }
        }
        drop(current);
        
        // Search in recent logs
        let recent = self.load_recent().await?;
        for log in recent {
            for entry in &log.entries {
                if entry.content.to_lowercase().contains(&query.to_lowercase()) {
                    results.push(entry.clone());
                }
            }
        }
        
        Ok(results)
    }
    
    /// Get statistics
    pub async fn stats(&self) -> DailyLogStats {
        let current = self.current_log.read().await;
        let index = self.log_index.read().await;
        
        DailyLogStats {
            total_logs: index.len() + 1, // +1 for current
            total_entries: current.as_ref().map_or(0, |l| l.metadata.entry_count),
            total_sessions: current.as_ref().map_or(0, |l| l.metadata.session_count),
            storage_path: self.storage_path.clone(),
        }
    }
}

/// Daily log statistics
#[derive(Debug, Clone)]
pub struct DailyLogStats {
    pub total_logs: usize,
    pub total_entries: u64,
    pub total_sessions: u64,
    pub storage_path: PathBuf,
}
