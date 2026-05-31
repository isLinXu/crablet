---
title: CLI Usage
description: Master the Crablet command-line interface
---

# :terminal: CLI Usage

Crablet provides a rich CLI for interactive conversations, single-shot execution, and system management.

## Core Commands

### Interactive Chat

```bash
crablet chat
```

Start an interactive REPL-style conversation. Supports:

- Multi-turn dialogue with context retention
- Inline tool execution (shell, file, web search)
- Cognitive layer switching (System 1/2/3)
- Session persistence

### Single-Shot Execution

```bash
crablet run "Summarize this paper about transformers"
```

Execute a single prompt and return the result. Useful for scripting and pipelines:

```bash
# Pipe input
echo "Translate to French:" | crablet run "$(cat -)"

# Chain with other tools
crablet run "List TODOs in this project" | grep -i urgent
```

### Web Service

```bash
# Unified Web UI + API gateway
crablet serve-web --port 18790

# Explicit gateway mode (REST + WebSocket + JSON-RPC)
crablet gateway --port 18790
```

## Skill Management

```bash
# List installed skills
crablet skill list

# Install a skill from Git
crablet skill install https://github.com/user/weather-skill.git

# Create a new skill scaffold
crablet skill create my-skill

# Remove a skill
crablet skill remove my-skill
```

## Knowledge Management

```bash
# Extract knowledge from a document
crablet knowledge extract --file paper.pdf

# Query the knowledge base
crablet knowledge query "What is the attention mechanism?"

# Index a directory
crablet knowledge index --dir ./docs
```

## System Administration

```bash
# Health check
crablet status

# View active sessions
crablet sessions list

# Reset configuration
crablet init --reset
```

## Command Reference

| Command | Description |
|:--------|:------------|
| `crablet chat` | Interactive conversation mode |
| `crablet run <prompt>` | Single-shot execution |
| `crablet serve-web [--port]` | Start Web UI + API |
| `crablet gateway [--port]` | Start API gateway |
| `crablet skill <subcmd>` | Manage skills |
| `crablet knowledge <subcmd>` | Manage knowledge base |
| `crablet status` | System health check |
| `crablet sessions` | Session management |
| `crablet init [--reset]` | Initialize/reset config |
| `crablet script run <file>` | Execute Lua script |

## Keyboard Shortcuts (Chat Mode)

| Shortcut | Action |
|:---------|:-------|
| `Ctrl+C` | Cancel current generation |
| `Ctrl+D` | Exit chat |
| `↑` / `↓` | Navigate command history |
| `Tab` | Auto-complete commands |
| `/help` | Show in-chat help |
| `/clear` | Clear conversation history |
| `/model <name>` | Switch LLM model |
| `/tools` | Toggle tool usage |
