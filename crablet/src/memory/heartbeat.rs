//! Heartbeat Worker - Background thinking and memory consolidation
//!
//! This module implements the MemGPT-style "Always-On" heartbeat mechanism.
//! When the user is idle, the agent can perform background tasks like:
//! - Reviewing recent conversations
//! - Updating Core Memory
//! - Consolidating episodic memories
//! - Generating proactive suggestions
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Heartbeat Worker                         │
//! │                                                              │
//! │   ┌─────────────┐    ┌─────────────┐    ┌──────────────┐   │
//! │   │ Timer Tick  │───→│ Check Idle  │───→│ Background   │   │
//! │   │ (60s)       │    │ Threshold   │    │ Think        │   │
//! │   └─────────────┘    └─────────────┘    └──────────────┘   │
//! │                                                │             │
//! │                                                ▼             │
//! │   ┌──────────────────────────────────────────────────────┐ │
//! │   │              Background Tasks                        │ │
//! │   │  • Review conversations → Update Core Memory         │ │
//! │   │  • Consolidate episodic memories                     │ │
//! │   │  • Generate proactive suggestions                    │ │
//! │   │  • Clean up expired memories                         │ │
//! │   └──────────────────────────────────────────────────────┘ │
//! └─────────────────────────────────────────────────────────────┘
//! ```

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{info, warn, error};

use crate::events::{AgentEvent, EventBus};
use crate::memory::manager::MemoryManager;
use crate::memory::core::CoreMemoryBlock;
use crate::cognitive::llm::LlmClient;
use crate::types::Message;
use crate::error::Result;

/// Configuration for Heartbeat Worker
#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    /// Interval between heartbeat ticks (default: 60 seconds)
    pub interval: Duration,
    /// Idle threshold - user must be inactive for this duration before background tasks run (default: 300 seconds)
    pub idle_threshold: Duration,
    /// Enable background thinking when idle
    pub enable_background_thinking: bool,
    /// Enable memory consolidation
    pub enable_memory_consolidation: bool,
    /// Maximum messages to review during background thinking
    pub max_review_messages: usize,
    /// Minimum messages before consolidation
    pub min_consolidation_messages: usize,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(60),
            idle_threshold: Duration::from_secs(300), // 5 minutes
            enable_background_thinking: true,
            enable_memory_consolidation: true,
            max_review_messages: 20,
            min_consolidation_messages: 10,
        }
    }
}

/// Statistics for heartbeat worker
#[derive(Debug, Clone, Default)]
pub struct HeartbeatStats {
    /// Total number of heartbeat ticks
    pub ticks: u64,
    /// Number of background thinking sessions triggered
    pub background_thinks: u64,
    /// Number of memory consolidations
    pub consolidations: u64,
    /// Number of core memory updates from background thinking
    pub core_memory_updates: u64,
    /// Last heartbeat timestamp
    pub last_heartbeat: Option<chrono::DateTime<chrono::Utc>>,
    /// Last background thinking timestamp
    pub last_background_think: Option<chrono::DateTime<chrono::Utc>>,
}

/// Heartbeat Worker - Runs background tasks when user is idle
pub struct HeartbeatWorker {
    /// Configuration
    config: HeartbeatConfig,
    /// Event bus for publishing events
    event_bus: Arc<EventBus>,
    /// Memory manager reference
    memory_manager: Arc<MemoryManager>,
    /// LLM client for background thinking
    llm: Arc<Box<dyn LlmClient>>,
    /// Statistics
    stats: Arc<RwLock<HeartbeatStats>>,
    /// Shutdown signal
    shutdown: Arc<RwLock<bool>>,
}

impl HeartbeatWorker {
    /// Create a new HeartbeatWorker
    pub fn new(
        config: HeartbeatConfig,
        event_bus: Arc<EventBus>,
        memory_manager: Arc<MemoryManager>,
        llm: Arc<Box<dyn LlmClient>>,
    ) -> Self {
        Self {
            config,
            event_bus,
            memory_manager,
            llm,
            stats: Arc::new(RwLock::new(HeartbeatStats::default())),
            shutdown: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the heartbeat loop (runs in background)
    pub fn start(self: Arc<Self>) {
        tokio::spawn(async move {
            info!(
                "Heartbeat Worker started (interval: {:?}, idle_threshold: {:?})",
                self.config.interval,
                self.config.idle_threshold
            );

            let mut interval = tokio::time::interval(self.config.interval);

            loop {
                interval.tick().await;

                // Check for shutdown
                if *self.shutdown.read().await {
                    info!("Heartbeat Worker shutting down");
                    break;
                }

                // Perform heartbeat tick
                if let Err(e) = self.tick().await {
                    error!("Heartbeat tick failed: {}", e);
                }
            }
        });
    }

    /// Stop the heartbeat worker
    pub async fn stop(&self) {
        *self.shutdown.write().await = true;
    }

    /// Perform a single heartbeat tick
    async fn tick(&self) -> Result<()> {
        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.ticks += 1;
            stats.last_heartbeat = Some(chrono::Utc::now());
        }

        // Check if user is idle
        let is_idle = self.memory_manager.is_idle(self.config.idle_threshold).await;
        
        // Publish heartbeat event
        self.event_bus.publish(AgentEvent::Heartbeat {
            timestamp: chrono::Utc::now(),
            active_sessions: self.memory_manager.active_session_count(),
        });

        if !is_idle {
            // User is active, skip background tasks
            return Ok(());
        }

        info!("User is idle, triggering background tasks");

        // Run background tasks
        if self.config.enable_background_thinking {
            self.background_think().await?;
        }

        if self.config.enable_memory_consolidation {
            self.consolidate_memories().await?;
        }

        Ok(())
    }

    /// Background thinking - Review recent conversations and update Core Memory
    async fn background_think(&self) -> Result<()> {
        info!("Starting background thinking...");

        // Publish event
        self.event_bus.publish(AgentEvent::BackgroundThinkingTriggered {
            reason: "User idle threshold reached".to_string(),
            context_summary: "Reviewing recent conversations".to_string(),
        });

        // Get recent conversations (simplified - in real implementation, would aggregate across sessions)
        // For MVP, we just update stats and potentially the core memory
        
        // Generate insights using LLM
        let thinking_prompt = r#"You are in background thinking mode. The user has been idle.
Your task is to reflect on any recent interactions and consider if there are important insights to remember.

Consider:
1. Are there any user preferences that should be remembered?
2. Were there any important decisions or facts discussed?
3. Is there anything that should be added to Core Memory?

If you have insights, respond with a JSON object:
{
  "insights": ["insight 1", "insight 2"],
  "suggested_core_memory_updates": [
    {"block": "human", "content": "User preference..."},
    {"block": "memory", "content": "Important fact..."}
  ]
}

If there are no significant insights, respond with: {"insights": [], "suggested_core_memory_updates": []}
"#;

        let messages = vec![Message::new("system", thinking_prompt)];
        
        match self.llm.chat_complete(&messages).await {
            Ok(response) => {
                // Parse the response
                let insights = self.parse_thinking_response(&response).await;
                
                // Apply suggested updates to Core Memory
                let mut updates_made = Vec::new();
                for update in insights.suggested_core_memory_updates {
                    if let Some(block) = CoreMemoryBlock::from_str(&update.block) {
                        match self.memory_manager.core_memory_append(block, &update.content).await {
                            Ok(_) => {
                                updates_made.push(format!("{}: {}", update.block, update.content));
                            }
                            Err(e) => {
                                warn!("Failed to update Core Memory: {}", e);
                            }
                        }
                    }
                }

                // Update stats
                {
                    let mut stats = self.stats.write().await;
                    stats.background_thinks += 1;
                    stats.core_memory_updates += updates_made.len() as u64;
                    stats.last_background_think = Some(chrono::Utc::now());
                }

                // Publish result event
                self.event_bus.publish(AgentEvent::BackgroundThinkingResult {
                    insights: insights.insights.join("; "),
                    suggested_actions: vec![],
                    memories_updated: updates_made,
                });

                info!("Background thinking completed");
            }
            Err(e) => {
                warn!("Background thinking LLM call failed: {}", e);
            }
        }

        Ok(())
    }

    /// Parse the thinking response from LLM
    async fn parse_thinking_response(&self, response: &str) -> ThinkingInsights {
        // Try to extract JSON from response
        let json_str = if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                &response[start..=end]
            } else {
                response
            }
        } else {
            response
        };

        serde_json::from_str(json_str).unwrap_or_else(|_| ThinkingInsights {
            insights: vec![],
            suggested_core_memory_updates: vec![],
        })
    }

    /// Consolidate memories - Trigger memory consolidation process
    async fn consolidate_memories(&self) -> Result<()> {
        info!("Running memory consolidation check...");

        // In the full implementation, this would:
        // 1. Check for sessions with enough messages to consolidate
        // 2. Trigger the MemoryConsolidator
        // 3. Update memory priority scores

        // For MVP, we just update stats
        {
            let mut stats = self.stats.write().await;
            stats.consolidations += 1;
        }

        Ok(())
    }

    /// Get current heartbeat statistics
    pub async fn get_stats(&self) -> HeartbeatStats {
        self.stats.read().await.clone()
    }

    /// Force trigger a heartbeat (for testing or manual triggers)
    pub async fn force_heartbeat(&self) -> Result<()> {
        self.tick().await
    }
}

/// Struct for parsing thinking insights
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct ThinkingInsights {
    insights: Vec<String>,
    suggested_core_memory_updates: Vec<CoreMemoryUpdate>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct CoreMemoryUpdate {
    block: String,
    content: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_heartbeat_config_default() {
        let config = HeartbeatConfig::default();
        assert_eq!(config.interval, Duration::from_secs(60));
        assert_eq!(config.idle_threshold, Duration::from_secs(300));
        assert!(config.enable_background_thinking);
        assert!(config.enable_memory_consolidation);
    }

    #[test]
    fn test_thinking_insights_parse() {
        let json = r#"{
            "insights": ["User prefers Python", "Likes concise answers"],
            "suggested_core_memory_updates": [
                {"block": "human", "content": "Prefers Python programming"}
            ]
        }"#;

        let insights: ThinkingInsights = serde_json::from_str(json).unwrap();
        assert_eq!(insights.insights.len(), 2);
        assert_eq!(insights.suggested_core_memory_updates.len(), 1);
    }

    #[test]
    fn test_heartbeat_stats_default() {
        let stats = HeartbeatStats::default();
        assert_eq!(stats.ticks, 0);
        assert_eq!(stats.background_thinks, 0);
        assert!(stats.last_heartbeat.is_none());
    }
}
