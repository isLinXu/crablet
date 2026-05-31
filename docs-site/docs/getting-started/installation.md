---
title: Installation
description: Install Crablet from source, Docker, or binary releases
---

# :hammer_and_wrench: Installation

## Prerequisites

| Requirement | Version | Notes |
|:------------|:--------|:------|
| **Rust** | 1.80+ | Install via [rustup](https://rustup.rs) |
| **Docker** | 20+ | Optional — for sandbox and Neo4j |
| **Git** | 2.x | For cloning the repository |

## Option 1: Build from Source

=== "Full Build"

    ```bash
    # Clone the repository
    git clone https://github.com/isLinXu/crablet.git
    cd crablet
    
    # Full build with all features (~15-20 min)
    cargo build --release
    
    # Initialize configuration
    ./target/release/crablet init
    ```

=== "Minimal Build"

    ```bash
    git clone https://github.com/isLinXu/crablet.git
    cd crablet
    
    # Minimal build — CLI + Web only (~5 min)
    cargo build --release --no-default-features --features web
    
    ./target/release/crablet init
    ```

=== "Custom Features"

    ```bash
    # Pick only the features you need
    cargo build --release --no-default-features \
      --features web,knowledge,scripting
    
    # Available features:
    #   web         — Web UI + API gateway
    #   knowledge   — RAG, vector storage, Neo4j, Qdrant
    #   audio       — Whisper speech recognition + TTS
    #   scripting   — Lua 5.4 scripting engine
    #   telemetry   — OpenTelemetry tracing
    #   sandbox     — Docker sandbox execution
    #   telegram    — Telegram Bot channel
    #   discord     — Discord Bot channel
    #   browser     — Headless browser automation
    ```

??? tip "Speed up compilation with sccache"

    Install [sccache](https://github.com/mozilla/sccache) to cache compiled artifacts:

    ```bash
    cargo install sccache
    export RUSTC_WRAPPER=sccache
    ```

    Subsequent builds can be **5-10× faster**.

## Option 2: Docker

=== "Docker Run"

    ```bash
    docker run -d \
      --name crablet \
      -p 18790:18790 \
      -e OPENAI_API_KEY=sk-xxx \
      -v ./data:/data \
      -v ./skills:/skills \
      crablet:latest
    ```

=== "Docker Compose"

    ```yaml
    # docker-compose.yml
    version: '3.8'
    
    services:
      crablet:
        image: crablet:latest
        ports:
          - "18790:18790"
        environment:
          - OPENAI_API_KEY=${OPENAI_API_KEY}
          - DATABASE_URL=sqlite:///data/crablet.db
        volumes:
          - ./data:/data
          - ./skills:/skills
    
      neo4j:  # Optional — for graph RAG
        image: neo4j:5
        ports:
          - "7474:7474"
          - "7687:7687"
        environment:
          - NEO4J_AUTH=neo4j/password
    ```

    ```bash
    docker-compose up -d
    ```

## Option 3: One-Click Install

```bash
# Linux/macOS
curl -fsSL https://crablet.dev/install.sh | bash

# Or use the included script
./install.sh
```

## Verify Installation

```bash
# Check version
crablet --version

# Initialize (first run)
crablet init

# Quick health check
crablet status
```

## Next Steps

- [:zap: Quickstart](quickstart.md) — Have your first conversation
- [:gear: Configuration](configuration.md) — Customize your setup
