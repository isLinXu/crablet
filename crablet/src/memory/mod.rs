#[cfg(feature = "knowledge")]
pub mod consolidator;
pub mod core;
pub mod distributed;
pub mod episodic;
pub mod heartbeat;
pub mod hot_reload;
pub mod manager;
pub mod priority;
pub mod semantic;
pub mod shared;
pub mod working;

// Knowledge Weaver — continuous knowledge relationship discovery
// (ROI-optimized: hash pre-filter, batch LLM, verification cache, inverted index)
// Requires `knowledge` feature for VectorStore and KnowledgeGraph types.
#[cfg(feature = "knowledge")]
pub mod knowledge_weaver;

// Always-on memory system components (require `knowledge` feature)
#[cfg(feature = "knowledge")]
pub mod background_thinker;
#[cfg(feature = "knowledge")]
pub mod cross_session;
#[cfg(feature = "knowledge")]
pub mod event_driven;
#[cfg(feature = "knowledge")]
pub mod memory_gardener;
#[cfg(feature = "knowledge")]
pub mod predictive;
#[cfg(feature = "knowledge")]
pub mod self_improvement;

// Always-on memory system (integrates all above components)
#[cfg(feature = "knowledge")]
pub mod always_on;

// Fusion Memory System (OpenClaw-style four-layer architecture)
pub mod fusion;

// Re-export fusion types for convenience
pub use fusion::{
    adapter::{AdapterConfig, FusionAdapter},
    daily_logs::DailyLogs,
    layer_session::SessionLayer,
    layer_soul::SoulLayer,
    layer_tools::ToolsLayer,
    layer_user::UserLayer,
    weaver::MemoryWeaver,
    FusionMemorySystem, MemoryError, MemoryStats,
};
