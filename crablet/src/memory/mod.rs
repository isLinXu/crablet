pub mod working;
pub mod episodic;
pub mod semantic;
pub mod core;
pub mod heartbeat;
pub mod hot_reload;
pub mod priority;
pub mod distributed;
#[cfg(feature = "knowledge")]
pub mod consolidator;
pub mod manager;
pub mod shared;

// Fusion Memory System (OpenClaw-style four-layer architecture)
pub mod fusion;

// Re-export fusion types for convenience
pub use fusion::{
    FusionMemorySystem, MemoryError, MemoryStats,
    layer_soul::SoulLayer,
    layer_tools::ToolsLayer,
    layer_user::UserLayer,
    layer_session::SessionLayer,
    daily_logs::DailyLogs,
    weaver::MemoryWeaver,
    adapter::{FusionAdapter, AdapterConfig},
};
