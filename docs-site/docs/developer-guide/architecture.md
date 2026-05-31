---
title: Architecture
description: Deep dive into Crablet's modular architecture
---

# :building_construction: Architecture

Crablet follows a modular, layered architecture built entirely in Rust on top of the Tokio async runtime.

## System Panorama

```mermaid
graph TB
    subgraph Application Layer
        CH[Channels<br/>CLI/Web/Telegram/Discord]
        GW[Gateway<br/>REST/WebSocket/JSON-RPC]
    end
    
    subgraph Cognitive Layer
        CR[Cognitive Router]
        S1[System 1<br/>Fast Intuition]
        S2[System 2<br/>Slow Reasoning]
        S3[System 3<br/>Meta-Cognition]
    end
    
    subgraph Memory Layer
        WM[Working Memory]
        EM[Episodic Memory]
        SM[Semantic Memory]
        PM[Procedural Memory]
        MF[Memory Fusion]
    end
    
    subgraph Infrastructure Layer
        LL[LLM Adapters<br/>OpenAI/DashScope/Ollama]
        TL[Tool Registry<br/>Bash/File/Web/Browser]
        DB[Storage<br/>SQLite/Neo4j/Qdrant]
        SF[Safety Oracle]
        OB[Observability<br/>OpenTelemetry]
    end
    
    CH --> CR
    GW --> CR
    CR --> S1
    CR --> S2
    CR --> S3
    S2 --> TL
    TL --> SF
    S3 --> MF
    MF --> WM
    MF --> EM
    MF --> SM
    MF --> PM
    S1 --> LL
    S2 --> LL
    TL --> DB
```

## Module Map

| Module | Path | Responsibility |
|:-------|:-----|:---------------|
| `channels` | `src/channels/` | Platform adapters (CLI, Web, Telegram, Discord) |
| `gateway` | `src/gateway/` | API gateway (REST, WebSocket, JSON-RPC) |
| `cognitive` | `src/cognitive/` | Three-layer cognition + routing + meta-controller |
| `memory` | `src/memory/` | Four-layer memory + fusion retrieval |
| `tools` | `src/tools/` | Built-in tool implementations |
| `skills` | `src/skills/` | Skill loader and executor |
| `knowledge` | `src/knowledge/` | RAG pipeline, vector/graph stores |
| `agent` | `src/agent/` | Multi-agent swarm + distributed harness |
| `config` | `src/config/` | Configuration management |
| `auth` | `src/auth/` | Authentication and authorization |
| `safety` | `src/safety/` | Safety oracle and policy enforcement |
| `observability` | `src/observability/` | Telemetry and metrics |
| `storage` | `src/storage/` | Database abstraction layer |
| `canvas` | `src/canvas/` | Visual workspace components |
| `scripting` | `src/scripting/` | Lua 5.4 scripting engine |
| `sandbox` | `src/sandbox/` | Docker sandbox execution |
| `workflow` | `src/workflow/` | Workflow engine |
| `rules` | `src/rules/` | Business rule engine |
| `audit` | `src/audit/` | Audit logging |
| `heartbeat` | `src/heartbeat/` | Health monitoring |
| `testing` | `src/testing/` | Testing utilities |
| `rpa` | `src/rpa/` | Robotic Process Automation |
| `gui` | `src/gui/` | Native GUI (experimental) |
| `evaluation` | `src/evaluation/` | Model evaluation framework |
| `telemetry` | `src/telemetry/` | Telemetry collection |
| `protocols` | `src/protocols/` | Communication protocols |
| `utils` | `src/utils/` | Shared utilities |

## Key Design Decisions

### Why Rust?

- **Zero-cost abstractions** — High-level ergonomics without runtime overhead
- **Fearless concurrency** — Tokio async runtime with compile-time safety
- **Small binaries** — Docker images < 20 MB
- **Fast startup** — < 500ms cold start

### Why Tokio?

- Industry-standard async runtime for Rust
- Supports 100+ concurrent agents efficiently
- Mature ecosystem with tracing, tower, hyper

### Why anyhow + thiserror?

- `anyhow` for application-level error propagation
- `thiserror` for library-level typed errors
- Consistent pattern across the entire codebase

## Data Flow

```mermaid
sequenceDiagram
    participant U as User
    participant CH as Channel
    participant CR as Cognitive Router
    participant S1 as System 1/2
    participant TL as Tools
    participant SF as Safety Oracle
    participant MF as Memory Fusion
    participant LL as LLM
    
    U->>CH: Message
    CH->>CR: Route
    CR->>MF: Retrieve context
    MF-->>CR: Relevant memories
    CR->>LL: Generate response
    LL-->>CR: Raw response
    CR->>TL: Execute tools (if needed)
    TL->>SF: Validate
    SF-->>TL: Approved
    TL-->>CR: Tool results
    CR->>LL: Continue with results
    LL-->>CH: Final response
    CH->>U: Reply
```
