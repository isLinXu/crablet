---
title: Configuration
description: Customize Crablet with config files, environment variables, and feature flags
---

# :gear: Configuration

Crablet is highly configurable through TOML config files, environment variables, and compile-time feature flags.

## Config File

Default location: `~/.config/crablet/config.toml`

```toml
# Database connection
database_url = "sqlite:crablet.db?mode=rwc"

# LLM provider
model_name = "gpt-4o-mini"
max_tokens = 4096
temperature = 0.7

# Logging
log_level = "info"  # trace | debug | info | warn | error

# Safety policies
[safety]
level = "Strict"  # Strict | Permissive | Disabled
allowed_commands = ["ls", "cat", "echo", "git", "cargo"]
blocked_commands = ["rm -rf", "mkfs", "dd"]

# MCP servers
[mcp_servers]
math_server = { command = "python3", args = ["mcp_server.py"] }

# Skills directory
skills_dir = "skills"

# Rate limits
[limits]
max_concurrent_requests = 100
request_timeout = 30
```

## Environment Variables

Environment variables override config file settings:

| Variable | Description |
|:---------|:------------|
| `OPENAI_API_KEY` | OpenAI API key |
| `DASHSCOPE_API_KEY` | Alibaba Cloud DashScope API key |
| `OPENAI_API_BASE` | OpenAI-compatible API endpoint URL |
| `OLLAMA_MODEL` | Local Ollama model name |
| `SERPER_API_KEY` | Serper search API key |
| `DATABASE_URL` | Database connection string |
| `RUST_LOG` | Log level override |
| `GRAPH_RAG_ENTITY_MODE` | Entity extraction mode: `rule` \| `phrase` \| `hybrid` (default: `hybrid`) |

## Feature Flags

Compile-time feature flags allow you to trim Crablet to only the capabilities you need:

| Feature | Includes | Default |
|:--------|:---------|:-------:|
| `web` | Web UI, API gateway | :white_check_mark: |
| `knowledge` | RAG, vector storage, Neo4j, Qdrant | :white_check_mark: |
| `audio` | Whisper speech recognition, TTS | :white_check_mark: |
| `scripting` | Lua 5.4 scripting engine | :white_check_mark: |
| `telemetry` | OpenTelemetry tracing | :white_check_mark: |
| `sandbox` | Docker sandbox execution | :white_check_mark: |
| `telegram` | Telegram Bot channel | :white_check_mark: |
| `discord` | Discord Bot channel | :white_check_mark: |
| `browser` | Headless browser automation | :white_check_mark: |

=== "Minimal Build"

    ```bash
    # Only Web UI + scripting
    cargo build --release --no-default-features --features web,scripting
    ```

=== "Full Build"

    ```bash
    # Everything enabled (default)
    cargo build --release
    ```

=== "Production Server"

    ```bash
    # Server-focused: no audio/browser, with telemetry
    cargo build --release --no-default-features \
      --features web,knowledge,telemetry,sandbox
    ```

## Safety Levels

Crablet provides three safety modes for tool execution:

=== "Strict"

    - All shell commands require approval
    - File access restricted to allowed paths
    - No network access from sandbox
    
    ```toml
    [safety]
    level = "Strict"
    allowed_commands = ["ls", "cat", "head", "tail", "grep", "find"]
    ```

=== "Permissive"

    - Known-safe commands auto-approved
    - File access limited to home directory tree
    - Network allowed with rate limiting
    
    ```toml
    [safety]
    level = "Permissive"
    blocked_commands = ["rm -rf /", "mkfs", "dd"]
    ```

=== "Disabled"

    - All commands auto-approved
    - Full filesystem access
    - Unrestricted network
    
    ```toml
    [safety]
    level = "Disabled"
    ```
    
    !!! warning "Use with caution"
        Disabled mode should only be used in trusted, isolated environments.

## Next Steps

- [:zap: Quickstart](quickstart.md) — Put your config to work
- [:shield: Security](../user-guide/security.md) — Deep dive into safety policies
