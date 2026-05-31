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
