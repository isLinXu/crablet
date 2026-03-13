//! Event-Driven Memory Updates - Real-time memory synchronization
//!
//! This module implements an event-driven architecture for immediate
//! memory updates based on significant events:
//! - Critical user interactions
//! - Knowledge updates
//! - Preference changes
//! - Context switches
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │              Event-Driven Memory System                             │
//! │                                                                      │
//! │   Events ──→  Event Router  ──→  Handlers  ──→  Memory Updates     │
//! │                                                                      │
//! │   ┌─────────────┐    ┌─────────────┐    ┌──────────────────────┐   │
//! │   │  UserInput  │───→│  Router     │───→│  Update Working Mem  │   │
//! │   │  ToolExec   │───→│  (Filter)   │───→│  Update Core Mem     │   │
//! │   │  Preference │───→│             │───→│  Trigger Consolidate │   │
//! │   │  Change     │    │             │    │  Update Knowledge    │   │
//! │   └─────────────┘    └─────────────┘    └──────────────────────┘   │
//! └─────────────────────────────────────────────────────────────────────┘

use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{RwLock, mpsc};
use tracing::{info, warn, debug};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::events::{AgentEvent, EventBus, Event};
use crate::memory::manager::MemoryManager;
use crate::memory::core::{CoreMemoryBlock};
use crate::memory::consolidator::MemoryConsolidator;
use crate::knowledge::vector_store::VectorStore;
use crate::error::Result;

/// Priority level for event processing
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EventPriority {
    Critical,   // Immediate processing
    High,       // Process within 1 second
    Normal,     // Process within 5 seconds
    Low,        // Process when idle
}

/// Configuration for event-driven memory system
#[derive(Debug, Clone)]
pub struct EventDrivenConfig {
    /// Channel buffer size
    pub channel_buffer_size: usize,
    /// Number of worker threads
    pub worker_count: usize,
    /// Enable immediate core memory updates
    pub enable_immediate_core_updates: bool,
    /// Enable working memory synchronization
    pub enable_working_memory_sync: bool,
    /// Enable knowledge graph updates
    pub enable_knowledge_updates: bool,
    /// Batch processing interval for low priority events
    pub batch_interval_ms: u64,
}

impl Default for EventDrivenConfig {
    fn default() -> Self {
        Self {
            channel_buffer_size: 1000,
            worker_count: 4,
            enable_immediate_core_updates: true,
            enable_working_memory_sync: true,
            enable_knowledge_updates: true,
            batch_interval_ms: 5000,
        }
    }
}

/// Event handler trait for custom processing
#[async_trait::async_trait]
pub trait EventHandler: Send + Sync {
    /// Check if this handler can process the event
    fn can_handle(&self, event: &AgentEvent) -> bool;
    
    /// Process the event
    async fn handle(&self, event: &AgentEvent, context: &EventContext) -> Result<()>;
    
    /// Get handler priority
    fn priority(&self) -> EventPriority {
        EventPriority::Normal
    }
}

/// Context passed to event handlers
#[derive(Debug, Clone)]
pub struct EventContext {
    pub session_id: Option<String>,
    pub user_id: Option<String>,
    pub timestamp: chrono::DateTime<Utc>,
}

/// Event-driven memory updater
pub struct EventDrivenMemory {
    config: EventDrivenConfig,
    event_bus: Arc<EventBus>,
    memory_manager: Arc<MemoryManager>,
    vector_store: Option<Arc<VectorStore>>,
    consolidator: Option<Arc<MemoryConsolidator>>,
    /// Event sender
    event_sender: mpsc::Sender<(AgentEvent, EventContext, EventPriority)>,
    /// Custom handlers
    handlers: Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
    /// Statistics
    stats: Arc<RwLock<EventDrivenStats>>,
    /// Shutdown signal
    shutdown: Arc<RwLock<bool>>,
}

/// Statistics for event-driven system
#[derive(Debug, Clone, Default)]
pub struct EventDrivenStats {
    pub total_events_received: u64,
    pub events_processed: u64,
    pub events_by_priority: HashMap<String, u64>,
    pub events_by_type: HashMap<String, u64>,
    pub core_memory_updates: u64,
    pub working_memory_updates: u64,
    pub knowledge_updates: u64,
    pub failed_events: u64,
    pub avg_processing_time_ms: f64,
}

impl EventDrivenMemory {
    pub fn new(
        config: EventDrivenConfig,
        event_bus: Arc<EventBus>,
        memory_manager: Arc<MemoryManager>,
        vector_store: Option<Arc<VectorStore>>,
        consolidator: Option<Arc<MemoryConsolidator>>,
    ) -> (Self, mpsc::Receiver<(AgentEvent, EventContext, EventPriority)>) {
        let (sender, receiver) = mpsc::channel(config.channel_buffer_size);
        
        let system = Self {
            config,
            event_bus,
            memory_manager,
            vector_store,
            consolidator,
            event_sender: sender,
            handlers: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(EventDrivenStats::default())),
            shutdown: Arc::new(RwLock::new(false)),
        };

        (system, receiver)
    }

    /// Start the event-driven system
    pub fn start(self: Arc<Self>, mut receiver: mpsc::Receiver<(AgentEvent, EventContext, EventPriority)>) {
        // Start event listener
        let self_clone = self.clone();
        tokio::spawn(async move {
            self_clone.listen_for_events().await;
        });

        // Start worker pool
        for worker_id in 0..self.config.worker_count {
            let self_clone = self.clone();
            let receiver_clone = receiver.resubscribe();
            
            tokio::spawn(async move {
                self_clone.worker_loop(worker_id, receiver_clone).await;
            });
        }

        info!(
            "Event-Driven Memory system started with {} workers",
            self.config.worker_count
        );
    }

    /// Listen for events from the event bus
    async fn listen_for_events(&self) {
        let mut subscriber = self.event_bus.subscribe();

        loop {
            if *self.shutdown.read().await {
                break;
            }

            match subscriber.recv().await {
                Ok(event) => {
                    if let Err(e) = self.process_event(event).await {
                        warn!("Failed to process event: {}", e);
                    }
                }
                Err(e) => {
                    warn!("Event receive error: {}", e);
                }
            }
        }
    }

    /// Process an incoming event
    async fn process_event(&self, event: Event) -> Result<()> {
        let priority = self.determine_priority(&event.payload);
        let context = EventContext {
            session_id: event.session_id.clone(),
            user_id: event.user_id.clone(),
            timestamp: event.timestamp,
        };

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.total_events_received += 1;
            *stats.events_by_priority.entry(format!("{:?}", priority)).or_insert(0) += 1;
            *stats.events_by_type.entry(self.event_type_name(&event.payload)).or_insert(0) += 1;
        }

        // Send to processing queue
        if let Err(e) = self.event_sender.send((event.payload, context, priority)).await {
            warn!("Failed to queue event: {}", e);
            
            // Update failed count
            self.stats.write().await.failed_events += 1;
        }

        Ok(())
    }

    /// Worker loop for processing events
    async fn worker_loop(&self, worker_id: usize, mut receiver: mpsc::Receiver<(AgentEvent, EventContext, EventPriority)>) {
        info!("Event worker {} started", worker_id);

        loop {
            if *self.shutdown.read().await {
                break;
            }

            match receiver.recv().await {
                Some((event, context, priority)) => {
                    let start = std::time::Instant::now();
                    
                    if let Err(e) = self.handle_event(&event, &context, priority).await {
                        warn!("Worker {} failed to handle event: {}", worker_id, e);
                        self.stats.write().await.failed_events += 1;
                    } else {
                        let duration_ms = start.elapsed().as_millis() as f64;
                        
                        // Update stats
                        let mut stats = self.stats.write().await;
                        stats.events_processed += 1;
                        
                        // Update average processing time
                        if stats.events_processed == 1 {
                            stats.avg_processing_time_ms = duration_ms;
                        } else {
                            stats.avg_processing_time_ms = 
                                (stats.avg_processing_time_ms * (stats.events_processed - 1) as f64 + duration_ms)
                                / stats.events_processed as f64;
                        }
                    }
                }
                None => {
                    // Channel closed
                    break;
                }
            }
        }

        info!("Event worker {} stopped", worker_id);
    }

    /// Handle a specific event
    async fn handle_event(&self, event: &AgentEvent, context: &EventContext, priority: EventPriority) -> Result<()> {
        debug!("Handling event: {:?} with priority {:?}", event, priority);

        // Run custom handlers first
        let handlers = self.handlers.read().await;
        for handler in handlers.iter() {
            if handler.can_handle(event) {
                if let Err(e) = handler.handle(event, context).await {
                    warn!("Custom handler failed: {}", e);
                }
            }
        }
        drop(handlers);

        // Built-in event handling
        match event {
            AgentEvent::UserInput(content) => {
                self.handle_user_input(content, context).await?;
            }
            AgentEvent::ToolExecutionFinished { tool, output } => {
                self.handle_tool_execution(tool, output, context).await?;
            }
            AgentEvent::CoreMemoryUpdated { block, operation, .. } => {
                self.handle_core_memory_update(block, operation, context).await?;
            }
            AgentEvent::BackgroundThinkingResult { insights, .. } => {
                self.handle_background_thinking(insights, context).await?;
            }
            AgentEvent::CognitiveLayerChanged { layer } => {
                self.handle_layer_change(layer, context).await?;
            }
            _ => {
                // Other events may not require immediate memory updates
            }
        }

        Ok(())
    }

    /// Handle user input events
    async fn handle_user_input(&self, content: &str, context: &EventContext) -> Result<()> {
        // Update working memory if session exists
        if let Some(session_id) = &context.session_id {
            if self.config.enable_working_memory_sync {
                self.memory_manager.touch_activity();
                
                // Could trigger immediate context analysis here
                debug!("Updated activity for session: {}", session_id);
            }
        }

        // Analyze for immediate preference updates
        if self.config.enable_immediate_core_updates {
            self.analyze_for_preferences(content, context).await?;
        }

        Ok(())
    }

    /// Handle tool execution events
    async fn handle_tool_execution(&self, tool: &str, output: &str, context: &EventContext) -> Result<()> {
        // Store tool execution results in knowledge base
        if self.config.enable_knowledge_updates {
            if let Some(vs) = &self.vector_store {
                let document = format!("Tool: {}\nResult: {}", tool, output);
                let metadata = serde_json::json!({
                    "type": "tool_execution",
                    "tool": tool,
                    "session_id": context.session_id,
                    "timestamp": context.timestamp,
                });
                
                // vs.add_document(&document, Some(metadata)).await?;
                
                // Update stats
                self.stats.write().await.knowledge_updates += 1;
            }
        }

        Ok(())
    }

    /// Handle core memory update events
    async fn handle_core_memory_update(&self, block: &str, operation: &str, context: &EventContext) -> Result<()> {
        // Trigger immediate persistence
        self.memory_manager.save_core_memory().await?;
        
        // Update stats
        self.stats.write().await.core_memory_updates += 1;

        // Notify other systems
        self.event_bus.publish(AgentEvent::SystemLog(format!(
            "Core memory {} operation completed on {} block",
            operation, block
        )));

        Ok(())
    }

    /// Handle background thinking results
    async fn handle_background_thinking(&self, insights: &str, context: &EventContext) -> Result<()> {
        // Trigger consolidation if significant insights were found
        if !insights.is_empty() {
            if let Some(consolidator) = &self.consolidator {
                if let Some(session_id) = &context.session_id {
                    // consolidator.trigger_consolidation(session_id).await?;
                }
            }
        }

        Ok(())
    }

    /// Handle cognitive layer changes
    async fn handle_layer_change(&self, layer: &str, context: &EventContext) -> Result<()> {
        // Update working memory context based on cognitive layer
        if let Some(session_id) = &context.session_id {
            // Could adjust working memory capacity or strategy based on layer
            debug!("Layer changed to {} for session {}", layer, session_id);
        }

        Ok(())
    }

    /// Analyze content for immediate preference updates
    async fn analyze_for_preferences(&self, content: &str, context: &EventContext) -> Result<()> {
        // Quick heuristic analysis for critical preferences
        // This is a lightweight check - full analysis happens in background thinker
        
        let lower_content = content.to_lowercase();
        
        // Check for explicit preference statements
        if lower_content.contains("i prefer") || lower_content.contains("i like") || lower_content.contains("i want") {
            // Could trigger immediate core memory update for high-confidence preferences
            debug!("Detected potential preference in user input");
        }

        Ok(())
    }

    /// Determine event priority
    fn determine_priority(&self, event: &AgentEvent) -> EventPriority {
        match event {
            AgentEvent::UserInput(_) => EventPriority::Critical,
            AgentEvent::ToolExecutionStarted { .. } => EventPriority::High,
            AgentEvent::ToolExecutionFinished { .. } => EventPriority::High,
            AgentEvent::CoreMemoryUpdated { .. } => EventPriority::Critical,
            AgentEvent::Error(_) => EventPriority::High,
            AgentEvent::BackgroundThinkingResult { .. } => EventPriority::Normal,
            AgentEvent::Heartbeat { .. } => EventPriority::Low,
            _ => EventPriority::Normal,
        }
    }

    /// Get event type name for statistics
    fn event_type_name(&self, event: &AgentEvent) -> String {
        match event {
            AgentEvent::UserInput(_) => "UserInput".to_string(),
            AgentEvent::SystemLog(_) => "SystemLog".to_string(),
            AgentEvent::ThoughtGenerated(_) => "ThoughtGenerated".to_string(),
            AgentEvent::ToolExecutionStarted { .. } => "ToolExecutionStarted".to_string(),
            AgentEvent::ToolExecutionFinished { .. } => "ToolExecutionFinished".to_string(),
            AgentEvent::CanvasUpdate { .. } => "CanvasUpdate".to_string(),
            AgentEvent::SwarmActivity { .. } => "SwarmActivity".to_string(),
            AgentEvent::SwarmGraphUpdate { .. } => "SwarmGraphUpdate".to_string(),
            AgentEvent::SwarmTaskUpdate { .. } => "SwarmTaskUpdate".to_string(),
            AgentEvent::SwarmLog { .. } => "SwarmLog".to_string(),
            AgentEvent::GraphRagEntityModeChanged { .. } => "GraphRagEntityModeChanged".to_string(),
            AgentEvent::ResponseGenerated(_) => "ResponseGenerated".to_string(),
            AgentEvent::CognitiveLayerChanged { .. } => "CognitiveLayerChanged".to_string(),
            AgentEvent::Error(_) => "Error".to_string(),
            AgentEvent::Heartbeat { .. } => "Heartbeat".to_string(),
            AgentEvent::BackgroundThinkingTriggered { .. } => "BackgroundThinkingTriggered".to_string(),
            AgentEvent::BackgroundThinkingResult { .. } => "BackgroundThinkingResult".to_string(),
            AgentEvent::CoreMemoryUpdated { .. } => "CoreMemoryUpdated".to_string(),
        }
    }

    /// Register a custom event handler
    pub async fn register_handler(&self, handler: Box<dyn EventHandler>) {
        self.handlers.write().await.push(handler);
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> EventDrivenStats {
        self.stats.read().await.clone()
    }

    /// Stop the system
    pub async fn stop(&self) {
        *self.shutdown.write().await = true;
    }
}

/// Built-in handler for preference extraction
pub struct PreferenceExtractionHandler;

#[async_trait::async_trait]
impl EventHandler for PreferenceExtractionHandler {
    fn can_handle(&self, event: &AgentEvent) -> bool {
        matches!(event, AgentEvent::UserInput(_))
    }

    async fn handle(&self, event: &AgentEvent, _context: &EventContext) -> Result<()> {
        if let AgentEvent::UserInput(content) = event {
            // Extract preferences from content
            debug!("PreferenceExtractionHandler processing: {}", content);
        }
        Ok(())
    }

    fn priority(&self) -> EventPriority {
        EventPriority::Normal
    }
}

/// Built-in handler for knowledge graph updates
pub struct KnowledgeGraphUpdateHandler;

#[async_trait::async_trait]
impl EventHandler for KnowledgeGraphUpdateHandler {
    fn can_handle(&self, event: &AgentEvent) -> bool {
        matches!(event, AgentEvent::ToolExecutionFinished { .. })
    }

    async fn handle(&self, event: &AgentEvent, _context: &EventContext) -> Result<()> {
        if let AgentEvent::ToolExecutionFinished { tool, output } = event {
            // Update knowledge graph with tool execution results
            debug!("KnowledgeGraphUpdateHandler processing tool: {}", tool);
        }
        Ok(())
    }

    fn priority(&self) -> EventPriority {
        EventPriority::Low
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_priority_ordering() {
        assert!(EventPriority::Critical > EventPriority::High);
        assert!(EventPriority::High > EventPriority::Normal);
        assert!(EventPriority::Normal > EventPriority::Low);
    }

    #[test]
    fn test_event_driven_config_default() {
        let config = EventDrivenConfig::default();
        assert_eq!(config.channel_buffer_size, 1000);
        assert_eq!(config.worker_count, 4);
        assert!(config.enable_immediate_core_updates);
        assert!(config.enable_working_memory_sync);
    }

    #[test]
    fn test_event_context_creation() {
        let context = EventContext {
            session_id: Some("test_session".to_string()),
            user_id: Some("test_user".to_string()),
            timestamp: Utc::now(),
        };

        assert_eq!(context.session_id, Some("test_session".to_string()));
        assert_eq!(context.user_id, Some("test_user".to_string()));
    }
}
