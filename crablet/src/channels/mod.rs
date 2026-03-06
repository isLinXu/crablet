pub mod cli;
#[cfg(feature = "discord")]
pub mod discord;
#[cfg(feature = "web")]
pub mod web;
pub mod domestic;
pub mod international;
pub mod universal;
pub mod manager;

use async_trait::async_trait;
use anyhow::Result;

/// Unified Channel Trait for all messaging platforms
#[async_trait]
pub trait Channel: Send + Sync {
    /// Send a message to the specified recipient (user/group ID)
    async fn send(&self, to: &str, content: &str) -> Result<()>;

    /// Start listening for incoming messages (long-polling or webhook server)
    async fn start(&self) -> Result<()>;
    
    /// Get channel name
    fn name(&self) -> &str;
}

// Re-export specific implementations
#[cfg(feature = "telegram")]
pub use international::telegram;
