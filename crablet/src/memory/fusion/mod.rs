//! Crablet + OpenClaw Fusion Memory System
//!
//! This module provides a unified four-layer memory system combining
//! Crablet's implementation with OpenClaw's configuration style.

pub mod layer_soul;
pub mod layer_tools;
pub mod layer_user;
pub mod layer_session;
pub mod daily_logs;
pub mod weaver;
pub mod adapter;

use std::path::PathBuf;
use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, debug};

use crate::types::Message;

/// Fusion configuration (local definition)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionConfig {
    pub soul: layer_soul::SoulConfig,
    pub tools: layer_tools::ToolsConfig,
    pub user: layer_user::UserConfig,
    pub memory: MemoryConfig,
}

/// Memory configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub working: WorkingMemoryConfig,
    pub daily_logs: daily_logs::DailyLogsConfig,
    pub semantic: weaver::SemanticMemoryConfig,
}

/// Working memory configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingMemoryConfig {
    pub max_tokens: usize,
    pub capacity_messages: usize,
}

/// Fusion Memory System - Four-layer architecture with Daily Logs
pub struct FusionMemorySystem {
    /// Configuration
    config: Arc<FusionConfig>,
    
    /// L4: SOUL Layer - Immutable core
    soul: Arc<layer_soul::SoulLayer>,
    
    /// L3: TOOLS Layer - Dynamic tools
    tools: Arc<layer_tools::ToolsLayer>,
    
    /// L2: USER Layer - Semantic long-term memory
    user: Arc<RwLock<layer_user::UserLayer>>,
    
    /// L1: Session Layer - Real-time context (per session)
    sessions: DashMap<String, Arc<layer_session::SessionLayer>>,
    
    /// Daily Logs - OpenClaw style append-only logs
    daily_logs: Arc<daily_logs::DailyLogs>,
    
    /// Memory Weaver - Integrates and consolidates memories
    weaver: Arc<weaver::MemoryWeaver>,
}

impl FusionMemorySystem {
    /// Initialize the fusion memory system
    pub async fn initialize(config: Arc<FusionConfig>) -> Result<Self, MemoryError> {
        info!("Initializing Fusion Memory System...");
        
        // L4: Load SOUL layer (immutable)
        let soul = Arc::new(layer_soul::SoulLayer::from_config(&config.soul)?);
        debug!("SOUL layer loaded: {}", soul.identity().name);
        
        // L3: Initialize TOOLS layer
        let tools = Arc::new(layer_tools::ToolsLayer::from_config(&config.tools).await?);
        debug!("TOOLS layer initialized with {} tools", tools.tool_count());
        
        // L2: Initialize USER layer
        let user = Arc::new(RwLock::new(
            layer_user::UserLayer::from_config(&config.user).await?
        ));
        debug!("USER layer initialized");
        
        // Initialize Daily Logs
        let daily_logs = Arc::new(daily_logs::DailyLogs::from_config(&config.memory.daily_logs).await?);
        debug!("Daily Logs initialized");
        
        // Initialize Memory Weaver
        let weaver = Arc::new(weaver::MemoryWeaver::new(
            config.memory.semantic.clone(),
        ));
        
        info!("Fusion Memory System initialized successfully");
        
        Ok(Self {
            config,
            soul,
            tools,
            user,
            sessions: DashMap::new(),
            daily_logs,
            weaver,
        })
    }
    
    /// Create a new session with context from Daily Logs
    pub async fn create_session(&self, session_id: String) -> Result<Arc<layer_session::SessionLayer>, MemoryError> {
        debug!("Creating new session: {}", session_id);
        
        // Create session layer
        let session = Arc::new(layer_session::SessionLayer::new(
            session_id.clone(),
            self.config.memory.working.max_tokens,
        ));
        
        // Load recent context from Daily Logs
        let recent_logs = self.daily_logs.load_recent().await?;
        
        // Retrieve relevant long-term memories
        let user_guard = self.user.read().await;
        let relevant_memories = user_guard.search_relevant_context(5).await?;
        drop(user_guard);
        
        // Build enriched system message
        let system_message = self.build_context_message(&recent_logs, &relevant_memories).await?;
        session.add_system_message(system_message).await?;
        
        // Store session
        self.sessions.insert(session_id.clone(), session.clone());
        
        // Log session start
        self.daily_logs.log_event(
            session_id.clone(),
            daily_logs::LogEventType::SessionStart,
            "Session created with context from Daily Logs",
        ).await?;
        
        info!("Session {} created successfully", session_id);
        Ok(session)
    }
    
    /// Get an existing session
    pub fn get_session(&self, session_id: &str) -> Option<Arc<layer_session::SessionLayer>> {
        self.sessions.get(session_id).map(|s| s.clone())
    }
    
    /// End a session and persist memories
    pub async fn end_session(&self, session_id: &str) -> Result<(), MemoryError> {
        info!("Ending session: {}", session_id);
        
        let session = self.sessions.remove(session_id)
            .ok_or_else(|| MemoryError::SessionNotFound(session_id.to_string()))?;
        
        // 1. Save to Daily Logs
        self.daily_logs.append_session(&session.1).await?;
        
        // 2. Extract and consolidate memories
        let extracted_memories = self.weaver.extract_from_session(&session.1).await?;
        
        // 3. Store in USER layer (L2)
        let user_guard = self.user.write().await;
        for memory in extracted_memories {
            user_guard.record_memory(memory).await?;
        }
        drop(user_guard);
        
        // 4. Update user profile based on session
        let user_guard = self.user.write().await;
        user_guard.update_from_session(&session.1).await?;
        drop(user_guard);
        
        // 5. Persist session state
        session.1.persist().await?;
        
        // 6. Log session end
        let message_count = session.1.message_count().await;
        self.daily_logs.log_event(
            session_id.to_string(),
            daily_logs::LogEventType::SessionEnd,
            &format!("Session ended, {} memories extracted", message_count),
        ).await?;
        
        info!("Session {} ended successfully", session_id);
        Ok(())
    }
    
    /// Build enriched context message combining all layers
    async fn build_context_message(
        &self,
        recent_logs: &[daily_logs::DailyLog],
        relevant_memories: &[layer_user::Memory],
    ) -> Result<Message, MemoryError> {
        let soul = self.soul.identity();
        
        let mut context_parts = vec![
            format!("You are {}, {}", soul.name, soul.description),
            format!("Role: {}", soul.role),
        ];
        
        // Add SOUL values
        context_parts.push("\nCore Values:".to_string());
        for value in self.soul.core_values() {
            context_parts.push(format!("- {} (priority: {}): {}", 
                value.name, value.priority, value.description));
        }
        
        // Add recent context from Daily Logs
        if !recent_logs.is_empty() {
            context_parts.push("\nRecent Context:".to_string());
            for log in recent_logs.iter().take(2) {
                context_parts.push(format!("Date: {}", log.date));
                context_parts.push(log.summary.clone());
            }
        }
        
        // Add relevant memories
        if !relevant_memories.is_empty() {
            context_parts.push("\nRelevant Memories:".to_string());
            for memory in relevant_memories.iter().take(3) {
                context_parts.push(format!("- {}", memory.content));
            }
        }
        
        let content = context_parts.join("\n");
        
        Ok(Message::system(content))
    }
    
    /// Get SOUL layer (L4)
    pub fn soul(&self) -> Arc<layer_soul::SoulLayer> {
        self.soul.clone()
    }
    
    /// Get TOOLS layer (L3)
    pub fn tools(&self) -> Arc<layer_tools::ToolsLayer> {
        self.tools.clone()
    }
    
    /// Get USER layer (L2)
    pub async fn user(&self) -> tokio::sync::RwLockReadGuard<'_, layer_user::UserLayer> {
        self.user.read().await
    }
    
    /// Get Daily Logs
    pub fn daily_logs(&self) -> Arc<daily_logs::DailyLogs> {
        self.daily_logs.clone()
    }
    
    /// Get Memory Weaver
    pub fn weaver(&self) -> Arc<weaver::MemoryWeaver> {
        self.weaver.clone()
    }
    
    /// Perform memory maintenance (called by heartbeat)
    pub async fn maintenance(&self) -> Result<MaintenanceReport, MemoryError> {
        info!("Running memory maintenance...");
        
        let mut report = MaintenanceReport::default();
        
        // 1. Archive old Daily Logs
        report.archived_logs = self.daily_logs.archive_old(90).await?;
        
        // 2. Consolidate USER memories
        let user_guard = self.user.read().await;
        report.consolidated_memories = user_guard.consolidate().await?;
        drop(user_guard);
        
        // 3. Clean up expired sessions
        report.expired_sessions = self.cleanup_expired_sessions().await?;
        
        // 4. Optimize semantic memory
        report.optimized_indices = self.weaver.optimize().await?;
        
        info!("Memory maintenance completed: {:?}", report);
        Ok(report)
    }
    
    /// Clean up expired sessions
    async fn cleanup_expired_sessions(&self) -> Result<usize, MemoryError> {
        let mut expired = 0;
        let timeout = std::time::Duration::from_secs(3600); // 1 hour
        
        // Collect expired session IDs
        let expired_ids: Vec<String> = {
            let mut ids = Vec::new();
            for entry in self.sessions.iter() {
                let session = entry.value();
                let last_activity = session.last_activity().await;
                if last_activity.elapsed() > timeout {
                    ids.push(entry.key().clone());
                }
            }
            ids
        };
        
        // Remove expired sessions
        for id in expired_ids {
            self.sessions.remove(&id);
            expired += 1;
        }
        
        Ok(expired)
    }
    
    /// Export current state to Markdown configs (bidirectional sync)
    pub async fn export_to_markdown(&self, workspace_path: &PathBuf) -> Result<(), MemoryError> {
        info!("Exporting memory state to Markdown...");
        
        // Export USER state
        let user_guard = self.user.read().await;
        user_guard.export_to_markdown(&workspace_path.join("USER.md")).await?;
        drop(user_guard);
        
        // Export MEMORY state
        let user_guard = self.user.read().await;
        user_guard.export_memories_to_markdown(&workspace_path.join("MEMORY.md")).await?;
        drop(user_guard);
        
        info!("Memory state exported successfully");
        Ok(())
    }
}

/// Maintenance report
#[derive(Debug, Default)]
pub struct MaintenanceReport {
    pub archived_logs: usize,
    pub consolidated_memories: usize,
    pub expired_sessions: usize,
    pub optimized_indices: usize,
}

/// Memory errors
#[derive(Debug)]
pub enum MemoryError {
    SessionNotFound(String),
    LayerError(String),
    PersistenceError(String),
    ConfigError(String),
    IoError(std::io::Error),
}

impl std::fmt::Display for MemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryError::SessionNotFound(id) => write!(f, "Session not found: {}", id),
            MemoryError::LayerError(msg) => write!(f, "Layer error: {}", msg),
            MemoryError::PersistenceError(msg) => write!(f, "Persistence error: {}", msg),
            MemoryError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            MemoryError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for MemoryError {}

impl From<std::io::Error> for MemoryError {
    fn from(e: std::io::Error) -> Self {
        MemoryError::IoError(e)
    }
}

/// Memory trait for unified interface
#[async_trait::async_trait]
pub trait MemoryLayer: Send + Sync {
    /// Initialize the layer
    async fn initialize(&self) -> Result<(), MemoryError>;
    
    /// Persist layer state
    async fn persist(&self) -> Result<(), MemoryError>;
    
    /// Get layer statistics
    async fn stats(&self) -> MemoryStats;
}

/// Memory statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub layer_name: String,
    pub item_count: usize,
    pub size_bytes: usize,
    pub last_accessed: DateTime<Utc>,
}
