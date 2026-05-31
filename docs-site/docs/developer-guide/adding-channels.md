---
title: Adding Channels
description: Connect new messaging platforms via the Channel trait
---

# :satellite: Adding Channels

Connect Crablet to new messaging platforms by implementing the `Channel` trait.

## Channel Trait

```rust
#[async_trait]
pub trait Channel: Send + Sync {
    /// Send a message through this channel
    async fn send_message(&self, msg: &str) -> Result<()>;
    
    /// Receive incoming messages (non-blocking)
    async fn receive_message(&self) -> Result<Option<IncomingMessage>>;
    
    /// Start the channel event loop
    async fn start(self: Box<Self>) -> Result<()>;
    
    /// Human-readable channel name
    fn name(&self) -> &str;
}
```

## Example: Slack Channel

```rust
// src/channels/slack/mod.rs
use crate::channels::Channel;
use async_trait::async_trait;
use anyhow::Result;

pub struct SlackChannel {
    client: slack::Client,
    config: SlackConfig,
}

#[async_trait]
impl Channel for SlackChannel {
    fn name(&self) -> &str { "slack" }
    
    async fn send_message(&self, msg: &str) -> Result<()> {
        self.client.post_message(&self.config.channel_id, msg).await?;
        Ok(())
    }
    
    async fn receive_message(&self) -> Result<Option<IncomingMessage>> {
        // Poll or listen for incoming Slack messages
        let msg = self.client.poll_message().await?;
        Ok(msg.map(|m| IncomingMessage {
            content: m.text,
            sender: m.user,
            channel: self.name().to_string(),
        }))
    }
    
    async fn start(self: Box<Self>) -> Result<()> {
        // Start Slack WebSocket connection
        self.client.connect().await?;
        Ok(())
    }
}
```

## Registering Your Channel

Add to `src/channels/mod.rs`:

```rust
mod slack;

pub fn register_channels(registry: &mut ChannelRegistry, config: &Config) {
    if config.channels.slack.enabled {
        registry.register(slack::SlackChannel::new(config));
    }
}
```

## Testing

Test your channel with mock implementations:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_slack_send() {
        let channel = SlackChannel::new(test_config());
        let result = channel.send_message("Test message").await;
        assert!(result.is_ok());
    }
}
```
