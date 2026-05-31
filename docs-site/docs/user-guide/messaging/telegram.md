---
title: Telegram
description: Set up Crablet as a Telegram bot
---

# :simple-telegram: Telegram Setup

Deploy Crablet as a Telegram bot for mobile-first AI assistance.

## Prerequisites

1. Create a bot via [@BotFather](https://t.me/BotFather) on Telegram
2. Obtain your bot token

## Configuration

```toml
# ~/.config/crablet/config.toml
[channels.telegram]
enabled = true
bot_token = "${TELEGRAM_BOT_TOKEN}"  # Or set env var
allowed_users = []  # Empty = DM pairing mode
admin_users = ["your_telegram_id"]
```

## Starting the Bot

```bash
# Set your token
export TELEGRAM_BOT_TOKEN=123456:ABC-DEF

# Start Crablet with Telegram channel
crablet serve-web --features telegram
```

## DM Pairing

When an unknown user messages the bot:

1. Bot responds with a 6-digit pairing code
2. Admin approves: `crablet approve ABC123`
3. User gains access with assigned role

## Features

- :white_check_mark: Text conversations
- :white_check_mark: Image analysis (send photos)
- :white_check_mark: File handling (documents, code)
- :white_check_mark: Inline tool execution
- :white_check_mark: Group chat support
- :white_check_mark: Voice messages (with `audio` feature)
