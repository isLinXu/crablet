pub mod cli;
pub mod domestic;
pub mod international;
pub mod manager;
pub mod universal;
#[cfg(feature = "web")]
pub mod web;

use anyhow::Result;
use async_trait::async_trait;

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
