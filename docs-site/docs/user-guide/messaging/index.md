---
title: Messaging
description: Connect Crablet to messaging platforms
icon: material/message-text
---

# :message-text: Messaging

Crablet uses a unified Channel trait to connect to multiple messaging platforms. All channels share the same core capabilities — tool usage, cognitive processing, and memory.

## Supported Platforms

| Platform | Status | Protocol |
|:---------|:------:|:---------|
| **CLI** | :white_check_mark: | stdin/stdout |
| **Web UI** | :white_check_mark: | HTTP + WebSocket |
| **Telegram** | :white_check_mark: | Telegram Bot API |
| **Discord** | :white_check_mark: | Discord Gateway |
| **Feishu (飞书)** | :construction: | Feishu Open Platform |
| **DingTalk (钉钉)** | :construction: | DingTalk Open Platform |
| **WeCom (企微)** | :construction: | WeCom API |
| **HTTP Webhook** | :white_check_mark: | HTTP POST |

## Channel Trait

All platforms implement the same `Channel` trait:

```rust
#[async_trait]
pub trait Channel: Send + Sync {
    async fn send_message(&self, msg: &str) -> Result<()>;
    async fn receive_message(&self) -> Result<Option<IncomingMessage>>;
    async fn start(self: Box<Self>) -> Result<()>;
    fn name(&self) -> &str;
}
```

## Choosing a Channel

- **Development** → CLI or Web UI for rapid iteration
- **Personal Use** → Telegram or Discord for mobile access
- **Team Collaboration** → Feishu or DingTalk for enterprise
- **Integration** → HTTP Webhook for programmatic access

## Next Steps

- [:simple-telegram: Telegram Setup](telegram.md)
- [:simple-discord: Discord Setup](discord.md)
- [:globe_with_meridians: Web UI](web-ui.md)
