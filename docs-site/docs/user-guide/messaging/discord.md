---
title: Discord
description: Set up Crablet as a Discord bot
---

# :simple-discord: Discord Setup

Deploy Crablet as a Discord bot for community or team AI assistance.

## Prerequisites

1. Create an application in the [Discord Developer Portal](https://discord.com/developers/applications)
2. Create a bot user and obtain the token
3. Invite the bot to your server with appropriate permissions

## Configuration

```toml
# ~/.config/crablet/config.toml
[channels.discord]
enabled = true
bot_token = "${DISCORD_BOT_TOKEN}"
prefix = "!"  # Command prefix
allowed_guilds = []  # Empty = all guilds
admin_users = ["your_discord_id"]
```

## Starting the Bot

```bash
export DISCORD_BOT_TOKEN=your-token-here
crablet serve-web --features discord
```

## Slash Commands

| Command | Description |
|:--------|:------------|
| `/chat <message>` | Start a conversation |
| `/model <name>` | Switch LLM model |
| `/tools` | List available tools |
| `/reset` | Clear conversation history |
| `/status` | Show system status |
