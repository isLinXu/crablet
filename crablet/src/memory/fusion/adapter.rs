//! Fusion Memory Adapter
//!
//! This adapter provides a unified interface for the Fusion Memory System.
//! It simplifies memory management by providing a single entry point.

use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::memory::fusion::{
    layer_session::SessionLayer,
    layer_user::{create_memory_from_session, Memory},
    FusionConfig, FusionMemorySystem, MemoryError,
};
use crate::types::Message;

/// Adapter result type
pub type Result<T> = std::result::Result<T, MemoryError>;

/// Fusion Memory Adapter
///
/// Provides unified access to the Fusion Memory System.
pub struct FusionAdapter {
    /// The new Fusion Memory System
    fusion: Arc<FusionMemorySystem>,

    /// Configuration
    config: AdapterConfig,

    /// Session mapping (session_id -> fusion session)
    session_map: RwLock<std::collections::HashMap<String, Arc<SessionLayer>>>,
}

/// Adapter configuration
#[derive(Debug, Clone)]
pub struct AdapterConfig {
    /// Whether to use Fusion system as primary
    pub fusion_primary: bool,

    /// Whether to extract memories from conversations
    pub enable_memory_extraction: bool,

    /// Maximum tool calls per request
    pub max_tool_calls: usize,
}

impl Default for AdapterConfig {
    fn default() -> Self {
        Self {
            fusion_primary: true,
            enable_memory_extraction: true,
            max_tool_calls: 5,
        }
    }
}

impl FusionAdapter {
    /// Create a new Fusion Adapter
    pub async fn new(
        fusion_config: Arc<FusionConfig>,
        adapter_config: AdapterConfig,
    ) -> Result<Self> {
        info!("Initializing Fusion Memory Adapter...");

        // Initialize Fusion Memory System
        let fusion = Arc::new(FusionMemorySystem::initialize(fusion_config).await?);

        let adapter = Self {
            fusion,
            config: adapter_config,
            session_map: RwLock::new(std::collections::HashMap::new()),
        };

        info!("Fusion Memory Adapter initialized");
        Ok(adapter)
    }

    /// Create adapter with default configuration
    pub async fn new_default(fusion_config: Arc<FusionConfig>) -> Result<Self> {
        Self::new(fusion_config, AdapterConfig::default()).await
    }

    // ==================== Session Management ====================

    /// Create or get a session
    pub async fn get_or_create_session(&self, session_id: &str) -> Result<Arc<SessionLayer>> {
        // Check if session exists in map
        {
            let map = self.session_map.read().await;
            if let Some(session) = map.get(session_id) {
                return Ok(session.clone());
            }
        }

        // Create new session in Fusion system
        let session = self.fusion.create_session(session_id.to_string()).await?;

        // Store in map
        {
            let mut map = self.session_map.write().await;
            map.insert(session_id.to_string(), session.clone());
        }

        Ok(session)
    }

    /// End a session
    pub async fn end_session(&self, session_id: &str) -> Result<()> {
        // End in Fusion system
        self.fusion.end_session(session_id).await?;

        // Remove from map
        {
            let mut map = self.session_map.write().await;
            map.remove(session_id);
        }

        Ok(())
    }

    // ==================== Message Handling ====================

    /// Add a user message
    pub async fn add_user_message(&self, session_id: &str, content: &str) -> Result<()> {
        // Get or create session
        let session = self.get_or_create_session(session_id).await?;

        // Add to Fusion system
        session.add_user_message(content.to_string()).await?;

        // Log to Daily Logs
        self.fusion
            .daily_logs()
            .log_event(
                session_id.to_string(),
                crate::memory::fusion::daily_logs::LogEventType::Message,
                &format!("User: {}", content.chars().take(100).collect::<String>()),
            )
            .await?;

        Ok(())
    }

    /// Add an assistant message
    pub async fn add_assistant_message(&self, session_id: &str, content: &str) -> Result<()> {
        let session = self.get_or_create_session(session_id).await?;

        // Add to Fusion system
        session.add_assistant_message(content.to_string()).await?;

        Ok(())
    }

    /// Add a system message
    pub async fn add_system_message(&self, session_id: &str, content: &str) -> Result<()> {
        let session = self.get_or_create_session(session_id).await?;

        let message = Message::system(content.to_string());
        session.add_system_message(message).await?;

        Ok(())
    }

    // ==================== Context Retrieval ====================

    /// Get context messages for LLM
    pub async fn get_context(&self, session_id: &str) -> Result<Vec<Message>> {
        let session = self.get_or_create_session(session_id).await?;
        Ok(session.get_context_messages().await)
    }

    /// Get enriched system prompt (combining SOUL + context)
    pub async fn get_enriched_system_prompt(&self, _session_id: &str) -> Result<String> {
        let soul_prompt = self.fusion.soul().to_system_prompt();

        // Get recent context from Daily Logs
        let recent_logs = self.fusion.daily_logs().load_recent().await?;
        let context_summary = if !recent_logs.is_empty() {
            format!(
                "\n\nRecent context: {} previous conversations today",
                recent_logs.len()
            )
        } else {
            String::new()
        };

        // Get relevant memories
        let user_guard = self.fusion.user().await;
        let relevant_memories = user_guard.search_relevant_context(3).await?;
        let profile_summary = user_guard.prompt_summary().await;
        drop(user_guard);

        let memory_context = if !relevant_memories.is_empty() {
            let memories_text: Vec<String> = relevant_memories
                .iter()
                .map(|m| format!("- {}", m.content))
                .collect();
            format!("\n\nRelevant memories:\n{}", memories_text.join("\n"))
        } else {
            String::new()
        };

        let profile_context = if profile_summary.is_empty() {
            String::new()
        } else {
            format!("\n\nUser profile:\n{}", profile_summary)
        };

        Ok(format!(
            "{}{}{}{}",
            soul_prompt, context_summary, memory_context, profile_context
        ))
    }

    // ==================== Memory Management ====================

    /// Record a memory
    pub async fn record_memory(
        &self,
        content: String,
        category: String,
        session_id: &str,
    ) -> Result<()> {
        let memory = create_memory_from_session(content, category, session_id.to_string());

        let user_guard = self.fusion.user().await;
        user_guard.record_memory(memory).await?;
        drop(user_guard);

        Ok(())
    }

    /// Search relevant memories
    pub async fn search_memories(&self, limit: usize) -> Result<Vec<Memory>> {
        let user_guard = self.fusion.user().await;
        let memories = user_guard.search_relevant_context(limit).await?;
        drop(user_guard);

        Ok(memories)
    }

    // ==================== Tool Access ====================

    /// Get the TOOLS layer
    pub fn tools(&self) -> Arc<crate::memory::fusion::layer_tools::ToolsLayer> {
        self.fusion.tools()
    }

    /// Invoke a tool
    pub async fn invoke_tool(
        &self,
        tool_name: &str,
        params: serde_json::Value,
    ) -> std::result::Result<
        crate::memory::fusion::layer_tools::ToolResult,
        crate::memory::fusion::layer_tools::ToolError,
    > {
        self.fusion.tools().invoke(tool_name, params).await
    }

    // ==================== Maintenance ====================

    /// Run maintenance tasks
    pub async fn maintenance(&self) -> Result<crate::memory::fusion::MaintenanceReport> {
        self.fusion.maintenance().await
    }

    /// Get statistics
    pub async fn stats(&self) -> AdapterStats {
        let fusion_stats = crate::memory::fusion::MemoryStats {
            layer_name: "Fusion".to_string(),
            item_count: self.fusion.sessions.len(),
            size_bytes: 0,
            last_accessed: chrono::Utc::now(),
        };

        AdapterStats {
            fusion: fusion_stats,
            mapped_sessions: self.session_map.read().await.len(),
        }
    }

    /// Export to Markdown
    pub async fn export_to_markdown(&self, workspace_path: &Path) -> Result<()> {
        self.fusion.export_to_markdown(workspace_path).await
    }

    // ==================== Accessors ====================

    /// Get the underlying Fusion Memory System
    pub fn fusion_system(&self) -> Arc<FusionMemorySystem> {
        self.fusion.clone()
    }

    // ==================== Health Check ====================

    /// Perform a lightweight health probe of all four memory layers.
    ///
    /// Returns a [`FusionHealthReport`] that the gateway `/api/health/fusion`
    /// endpoint can serialize directly. No writes are performed.
    pub async fn health_check(&self) -> FusionHealthReport {
        // L1 Session layer — verify the session map is accessible
        let l1_session_count = self.session_map.read().await.len();

        // L2 User layer — verify read access
        let l2_ok = self
            .fusion
            .user()
            .await
            .search_relevant_context(1)
            .await
            .is_ok();

        // L3 Daily logs — verify recent entries load
        let l3_ok = self.fusion.daily_logs().load_recent().await.is_ok();

        // L4 SOUL — always available (in-memory struct)
        let l4_soul_loaded = !self.fusion.soul().to_system_prompt().is_empty();

        // Config sanity
        let fusion_primary = self.config.fusion_primary;
        let memory_extraction = self.config.enable_memory_extraction;

        FusionHealthReport {
            healthy: l2_ok && l3_ok && l4_soul_loaded,
            fusion_primary,
            memory_extraction_enabled: memory_extraction,
            l1_active_sessions: l1_session_count,
            l2_user_layer_ok: l2_ok,
            l3_daily_logs_ok: l3_ok,
            l4_soul_loaded,
        }
    }
}

/// Health report for all four Fusion Memory layers.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FusionHealthReport {
    /// True if all layers are operational.
    pub healthy: bool,
    /// Whether Fusion is the primary memory backend.
    pub fusion_primary: bool,
    /// Whether automatic memory extraction is active.
    pub memory_extraction_enabled: bool,
    /// Number of active L1 session contexts.
    pub l1_active_sessions: usize,
    /// L2 user/long-term memory layer status.
    pub l2_user_layer_ok: bool,
    /// L3 daily logs layer status.
    pub l3_daily_logs_ok: bool,
    /// L4 SOUL layer loaded (has non-empty system prompt).
    pub l4_soul_loaded: bool,
}

/// Adapter statistics
#[derive(Debug, Clone)]
pub struct AdapterStats {
    pub fusion: crate::memory::fusion::MemoryStats,
    pub mapped_sessions: usize,
}

/// Convenience methods for cognitive systems
impl FusionAdapter {
    /// Process a message through the full pipeline
    pub async fn process_message(
        &self,
        session_id: &str,
        user_input: &str,
    ) -> Result<(Vec<Message>, String)> {
        // 1. Add user message
        self.add_user_message(session_id, user_input).await?;

        // 2. Get enriched context
        let system_prompt = self.get_enriched_system_prompt(session_id).await?;
        let context = self.get_context(session_id).await?;

        // 3. Build messages for LLM
        let mut messages = vec![Message::system(system_prompt)];
        messages.extend(context);

        // 4. Extract potential memories from input
        let session = self.get_or_create_session(session_id).await?;
        let extracted = self.fusion.weaver().extract_from_session(&session).await?;

        // 5. Queue memories for consolidation
        for memory in extracted {
            self.fusion.weaver().queue_for_consolidation(memory).await;
        }

        // Return messages and session info
        let session_info = format!(
            "Session: {}, Messages: {}",
            session_id,
            session.message_count().await
        );

        Ok((messages, session_info))
    }
}

#[cfg(test)]
mod tests {
    // Note: Tests temporarily disabled due to FusionConfig requirements
    // use super::*;

    #[tokio::test]
    async fn test_adapter_creation() {
        // This would need a proper FusionConfig
        // let config = Arc::new(FusionConfig::default());
        // let adapter = FusionAdapter::new_default(config).await.unwrap();
        // assert!(adapter.config.fusion_primary);
    }
}
