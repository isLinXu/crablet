//! Always-On Memory System - Integrated memory management
//!
//! This module provides a unified interface for all Always-On Memory
//! components, coordinating their operation and providing a simple
//! API for the rest of the system.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │              Always-On Memory System                                │
//! │                                                                      │
//! │   ┌─────────────┐  ┌─────────────┐  ┌──────────────────────────┐   │
//! │   │ Background  │  │   Memory    │  │    Event-Driven          │   │
//! │   │ Thinker     │  │   Gardener  │  │    Updates               │   │
//! │   └─────────────┘  └─────────────┘  └──────────────────────────┘   │
//! │                                                                      │
//! │   ┌─────────────┐  ┌─────────────┐  ┌──────────────────────────┐   │
//! │   │ Knowledge   │  │  Predictive │  │   Cross-Session          │   │
//! │   │ Weaver      │  │   Memory    │  │   Fusion                 │   │
//! │   └─────────────┘  └─────────────┘  └──────────────────────────┘   │
//! │                                                                      │
//! │   ┌────────────────────────────────────────────────────────────┐   │
//! │   │              Self-Improvement Loop                         │   │
//! │   └────────────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────────────┘

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error};

use crate::events::EventBus;
use crate::memory::manager::MemoryManager;
use crate::memory::background_thinker::{BackgroundThinker, BackgroundThinkerConfig};
use crate::memory::memory_gardener::{MemoryGardener, MemoryGardenerConfig};
use crate::memory::event_driven::{EventDrivenMemory, EventDrivenConfig};
use crate::memory::knowledge_weaver::{KnowledgeWeaver, KnowledgeWeaverConfig};
use crate::memory::predictive::{PredictiveMemory, PredictiveMemoryConfig};
use crate::memory::cross_session::{CrossSessionFusion, CrossSessionConfig};
use crate::memory::self_improvement::{SelfImprovementLoop, SelfImprovementConfig};
use crate::knowledge::vector_store::VectorStore;
use crate::cognitive::llm::LlmClient;
use crate::error::Result;

/// Configuration for the entire Always-On Memory system
#[derive(Debug, Clone)]
pub struct AlwaysOnMemoryConfig {
    /// Enable background thinking
    pub enable_background_thinker: bool,
    /// Enable memory gardener
    pub enable_memory_gardener: bool,
    /// Enable event-driven updates
    pub enable_event_driven: bool,
    /// Enable knowledge weaver
    pub enable_knowledge_weaver: bool,
    /// Enable predictive memory
    pub enable_predictive_memory: bool,
    /// Enable cross-session fusion
    pub enable_cross_session: bool,
    /// Enable self-improvement
    pub enable_self_improvement: bool,
    /// Component configurations
    pub background_thinker: BackgroundThinkerConfig,
    pub memory_gardener: MemoryGardenerConfig,
    pub event_driven: EventDrivenConfig,
    pub knowledge_weaver: KnowledgeWeaverConfig,
    pub predictive: PredictiveMemoryConfig,
    pub cross_session: CrossSessionConfig,
    pub self_improvement: SelfImprovementConfig,
}

impl Default for AlwaysOnMemoryConfig {
    fn default() -> Self {
        Self {
            enable_background_thinker: true,
            enable_memory_gardener: true,
            enable_event_driven: true,
            enable_knowledge_weaver: true,
            enable_predictive_memory: true,
            enable_cross_session: true,
            enable_self_improvement: true,
            background_thinker: BackgroundThinkerConfig::default(),
            memory_gardener: MemoryGardenerConfig::default(),
            event_driven: EventDrivenConfig::default(),
            knowledge_weaver: KnowledgeWeaverConfig::default(),
            predictive: PredictiveMemoryConfig::default(),
            cross_session: CrossSessionConfig::default(),
            self_improvement: SelfImprovementConfig::default(),
        }
    }
}

/// Always-On Memory System - Main coordinator
pub struct AlwaysOnMemory {
    config: AlwaysOnMemoryConfig,
    /// Background Thinker
    background_thinker: Option<Arc<BackgroundThinker>>,
    /// Memory Gardener
    memory_gardener: Option<Arc<MemoryGardener>>,
    /// Event-Driven Memory
    event_driven: Option<Arc<EventDrivenMemory>>,
    /// Knowledge Weaver
    knowledge_weaver: Option<Arc<KnowledgeWeaver>>,
    /// Predictive Memory
    predictive_memory: Option<Arc<PredictiveMemory>>,
    /// Cross-Session Fusion
    cross_session: Option<Arc<CrossSessionFusion>>,
    /// Self-Improvement Loop
    self_improvement: Option<Arc<SelfImprovementLoop>>,
    /// Running state
    running: Arc<RwLock<bool>>,
}

impl AlwaysOnMemory {
    /// Create a new Always-On Memory system
    pub fn new(
        config: AlwaysOnMemoryConfig,
        event_bus: Arc<EventBus>,
        memory_manager: Arc<MemoryManager>,
        vector_store: Option<Arc<VectorStore>>,
        llm: Arc<Box<dyn LlmClient>>,
    ) -> Result<Self> {
        let mut system = Self {
            config: config.clone(),
            background_thinker: None,
            memory_gardener: None,
            event_driven: None,
            knowledge_weaver: None,
            predictive_memory: None,
            cross_session: None,
            self_improvement: None,
            running: Arc::new(RwLock::new(false)),
        };

        // Initialize Background Thinker
        if config.enable_background_thinker {
            let thinker = Arc::new(BackgroundThinker::new(
                config.background_thinker,
                event_bus.clone(),
                memory_manager.clone(),
                llm.clone(),
            ));
            system.background_thinker = Some(thinker);
        }

        // Initialize Memory Gardener
        if config.enable_memory_gardener {
            let gardener = Arc::new(MemoryGardener::new(
                config.memory_gardener,
                event_bus.clone(),
                memory_manager.clone(),
                vector_store.clone(),
            ));
            system.memory_gardener = Some(gardener);
        }

        // Initialize Event-Driven Memory
        if config.enable_event_driven {
            let (event_system, receiver) = EventDrivenMemory::new(
                config.event_driven,
                event_bus.clone(),
                memory_manager.clone(),
                vector_store.clone(),
                None, // consolidator
            );
            let event_arc = Arc::new(event_system);
            event_arc.start(receiver);
            system.event_driven = Some(event_arc);
        }

        // Initialize Knowledge Weaver
        if config.enable_knowledge_weaver {
            let weaver = Arc::new(KnowledgeWeaver::new(
                config.knowledge_weaver,
                event_bus.clone(),
                None, // knowledge_graph
                vector_store.clone(),
                llm.clone(),
            ));
            system.knowledge_weaver = Some(weaver);
        }

        // Initialize Predictive Memory
        if config.enable_predictive_memory {
            let predictive = Arc::new(PredictiveMemory::new(
                config.predictive,
                event_bus.clone(),
                memory_manager.clone(),
                vector_store.clone(),
                llm.clone(),
            ));
            system.predictive_memory = Some(predictive);
        }

        // Initialize Cross-Session Fusion
        if config.enable_cross_session {
            let fusion = Arc::new(CrossSessionFusion::new(
                config.cross_session,
                event_bus.clone(),
                memory_manager.clone(),
                vector_store.clone(),
                llm.clone(),
            ));
            system.cross_session = Some(fusion);
        }

        // Initialize Self-Improvement Loop
        if config.enable_self_improvement {
            let improvement = Arc::new(SelfImprovementLoop::new(
                config.self_improvement,
                event_bus.clone(),
            ));
            system.self_improvement = Some(improvement);
        }

        Ok(system)
    }

    /// Start all Always-On Memory components
    pub async fn start(&self) -> Result<()> {
        info!("Starting Always-On Memory system");

        *self.running.write().await = true;

        // Start Background Thinker
        if let Some(thinker) = &self.background_thinker {
            thinker.clone().start();
            info!("Background Thinker started");
        }

        // Start Memory Gardener
        if let Some(gardener) = &self.memory_gardener {
            gardener.clone().start();
            info!("Memory Gardener started");
        }

        // Start Knowledge Weaver
        if let Some(weaver) = &self.knowledge_weaver {
            weaver.clone().start();
            info!("Knowledge Weaver started");
        }

        // Start Cross-Session Fusion
        if let Some(fusion) = &self.cross_session {
            fusion.clone().start();
            info!("Cross-Session Fusion started");
        }

        // Start Self-Improvement Loop
        if let Some(improvement) = &self.self_improvement {
            improvement.clone().start();
            info!("Self-Improvement Loop started");
        }

        info!("Always-On Memory system fully started");
        Ok(())
    }

    /// Stop all components
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping Always-On Memory system");

        *self.running.write().await = false;

        if let Some(thinker) = &self.background_thinker {
            thinker.stop().await;
        }

        if let Some(gardener) = &self.memory_gardener {
            gardener.stop().await;
        }

        if let Some(event_driven) = &self.event_driven {
            event_driven.stop().await;
        }

        if let Some(weaver) = &self.knowledge_weaver {
            weaver.stop().await;
        }

        if let Some(fusion) = &self.cross_session {
            fusion.stop().await;
        }

        if let Some(improvement) = &self.self_improvement {
            improvement.stop().await;
        }

        info!("Always-On Memory system stopped");
        Ok(())
    }

    /// Record user input for predictive memory
    pub async fn record_input(&self, input: &str, context: &str) {
        if let Some(predictive) = &self.predictive_memory {
            predictive.record_input(input, context).await;
        }
    }

    /// Register a new session for cross-session fusion
    pub async fn register_session(&self, session_id: String, topics: Vec<String>) {
        if let Some(fusion) = &self.cross_session {
            fusion.register_session(session_id, topics).await;
        }
    }

    /// Update session metadata
    pub async fn update_session(&self, session_id: &str, message_count: usize, tools: Vec<String>) {
        if let Some(fusion) = &self.cross_session {
            fusion.update_session(session_id, message_count, tools).await;
        }
    }

    /// Close a session
    pub async fn close_session(&self, session_id: &str) {
        if let Some(fusion) = &self.cross_session {
            fusion.close_session(session_id).await;
        }
    }

    /// Get system status
    pub async fn get_status(&self) -> AlwaysOnMemoryStatus {
        AlwaysOnMemoryStatus {
            running: *self.running.read().await,
            background_thinker: self.background_thinker.is_some(),
            memory_gardener: self.memory_gardener.is_some(),
            event_driven: self.event_driven.is_some(),
            knowledge_weaver: self.knowledge_weaver.is_some(),
            predictive_memory: self.predictive_memory.is_some(),
            cross_session: self.cross_session.is_some(),
            self_improvement: self.self_improvement.is_some(),
        }
    }

    /// Get component statistics
    pub async fn get_statistics(&self) -> AlwaysOnMemoryStatistics {
        let mut stats = AlwaysOnMemoryStatistics::default();

        if let Some(thinker) = &self.background_thinker {
            stats.background_thinker = Some(thinker.get_stats().await);
        }

        if let Some(gardener) = &self.memory_gardener {
            stats.memory_gardener = Some(gardener.get_stats().await);
        }

        if let Some(event_driven) = &self.event_driven {
            stats.event_driven = Some(event_driven.get_stats().await);
        }

        if let Some(weaver) = &self.knowledge_weaver {
            stats.knowledge_weaver = Some(weaver.get_stats().await);
        }

        if let Some(predictive) = &self.predictive_memory {
            stats.predictive_memory = Some(predictive.get_stats().await);
        }

        if let Some(fusion) = &self.cross_session {
            stats.cross_session = Some(fusion.get_stats().await);
        }

        if let Some(improvement) = &self.self_improvement {
            stats.self_improvement = Some(improvement.get_stats().await);
        }

        stats
    }
}

/// System status
#[derive(Debug, Clone)]
pub struct AlwaysOnMemoryStatus {
    pub running: bool,
    pub background_thinker: bool,
    pub memory_gardener: bool,
    pub event_driven: bool,
    pub knowledge_weaver: bool,
    pub predictive_memory: bool,
    pub cross_session: bool,
    pub self_improvement: bool,
}

/// System statistics
#[derive(Debug, Clone, Default)]
pub struct AlwaysOnMemoryStatistics {
    pub background_thinker: Option<crate::memory::background_thinker::BackgroundThinkerStats>,
    pub memory_gardener: Option<crate::memory::memory_gardener::MemoryGardenerStats>,
    pub event_driven: Option<crate::memory::event_driven::EventDrivenStats>,
    pub knowledge_weaver: Option<crate::memory::knowledge_weaver::KnowledgeWeaverStats>,
    pub predictive_memory: Option<crate::memory::predictive::PredictiveMemoryStats>,
    pub cross_session: Option<crate::memory::cross_session::CrossSessionStats>,
    pub self_improvement: Option<crate::memory::self_improvement::SelfImprovementStats>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_always_on_memory_config_default() {
        let config = AlwaysOnMemoryConfig::default();
        assert!(config.enable_background_thinker);
        assert!(config.enable_memory_gardener);
        assert!(config.enable_event_driven);
        assert!(config.enable_knowledge_weaver);
        assert!(config.enable_predictive_memory);
        assert!(config.enable_cross_session);
        assert!(config.enable_self_improvement);
    }

    #[test]
    fn test_always_on_memory_status() {
        let status = AlwaysOnMemoryStatus {
            running: true,
            background_thinker: true,
            memory_gardener: true,
            event_driven: false,
            knowledge_weaver: true,
            predictive_memory: false,
            cross_session: true,
            self_improvement: true,
        };

        assert!(status.running);
        assert!(status.background_thinker);
        assert!(!status.event_driven);
    }
}
