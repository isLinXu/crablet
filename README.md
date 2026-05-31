<div align="center">

<h1>🦀 Crablet</h1>

<p>
  <strong>A Production-Ready AI Agent Operating System Built from Scratch in Rust</strong>
</p>

<p>
  <em>"Building the next-generation intelligent assistant infrastructure,<br>
  making AI as ubiquitous as water — fluid, adaptive, and ceaseless."</em>
</p>

<p>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.80%2B-orange?style=flat-square&logo=rust" alt="Rust Version"/></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue?style=flat-square" alt="License"/></a>
  <img src="https://img.shields.io/badge/Tokio-Async%20Runtime-green?style=flat-square" alt="Tokio"/>
  <img src="https://img.shields.io/badge/Cognition-System%201%2F2%2F3-purple?style=flat-square" alt="Architecture"/>
  <img src="https://img.shields.io/badge/RAG-GraphRAG%20%2B%20Qdrant-yellow?style=flat-square" alt="RAG"/>
  <img src="https://img.shields.io/badge/Auth-OIDC%20%2F%20OAuth2-red?style=flat-square" alt="Auth"/>
  <img src="https://img.shields.io/badge/Canvas-Live%20Artifacts-teal?style=flat-square" alt="Canvas"/>
</p>

<p>
  <img src="https://img.shields.io/github/languages/top/isLinXu/crablet?style=flat-square" alt="Top Language"/>
  <img src="https://img.shields.io/github/repo-size/isLinXu/crablet?style=flat-square" alt="Repo Size"/>
  <img src="https://img.shields.io/github/stars/isLinXu/crablet?style=flat-square" alt="Stars"/>
  <img src="https://img.shields.io/github/last-commit/isLinXu/crablet?style=flat-square" alt="Last Commit"/>
</p>

<p>
  <a href="README.md">English</a> | <a href="README_zh.md">中文</a>
</p>

<p>
  <a href="#-why-crablet">Why Crablet</a> •
  <a href="#-three-tier-cognitive-architecture">Architecture</a> •
  <a href="#-memory-system">Memory</a> •
  <a href="#-graphrag-knowledge-engine">GraphRAG</a> •
  <a href="#-canvas--live-artifacts">Canvas</a> •
  <a href="#-multi-agent-swarm">Swarm</a> •
  <a href="#-quick-start">Quick Start</a> •
  <a href="#-documentation">Docs</a>
</p>
</div>

---

## 🌟 Why Crablet?

**Crablet** is not another chatbot wrapper. It is a **complete AI Agent Operating System** written entirely in Rust — a system that provides production-ready cognitive infrastructure for Large Language Models with genuine thinking, planning, memory, tool use, and multi-agent collaboration.

| Traditional LLM Wrappers | Crablet |
|---|---|
| Stateless request/response | **Persistent layered memory** (working → episodic → semantic) |
| Single-step tool calls | **ReAct reasoning loops** with middleware pipeline |
| Flat retrieval (keyword search) | **GraphRAG** combining vector search + knowledge graph traversal |
| No output structuring | **Canvas** — live structured artifact rendering |
| Sequential processing | **Swarm** — 100+ concurrent collaborative agents |
| Python GC pauses | **Zero-GC Rust** with Tokio async runtime |

### Language Composition

| Language | Share | Role |
|---|---|---|
| 🦀 **Rust** | 65.2% | Core engine, cognitive layers, memory, tools |
| 🟦 **TypeScript** | 28.8% | Frontend, Web UI, Dashboard |
| 🌐 **HTML** | 4.7% | Templates, Canvas rendering |
| 🐍 **Python** | 0.8% | MCP servers, skill scripts |
| 🐳 **Dockerfile** | 0.2% | Container definitions |

---

## 🧠 Three-Tier Cognitive Architecture

Crablet's most distinctive feature is its **biologically-inspired three-tier cognitive routing system**. Inspired by Dual Process Theory, every input is automatically classified and routed by the **Cognitive Router** to the optimal processing layer:

```
┌──────────────────────────────────────────────────────────────────┐
│                     INPUT CHANNELS                                │
│    CLI │ Web UI │ Telegram │ DingTalk │ Feishu │ HTTP Webhook     │
└───────────────────────────┬──────────────────────────────────────┘
                            │
┌───────────────────────────▼──────────────────────────────────────┐
│              GATEWAY LAYER  (Axum + WebSocket)                    │
│    JSON-RPC │ REST API │ SSE Streaming │ Auth │ Rate Limiting      │
└───────────────────────────┬──────────────────────────────────────┘
                            │
┌───────────────────────────▼──────────────────────────────────────┐
│                    EVENT BUS                                       │
│              Tokio Broadcast Channel (lock-free)                   │
└──────────┬────────────────┬───────────────────┬───────────────────┘
           │                │                   │
  ┌────────▼───────┐ ┌──────▼──────┐ ┌──────────▼──────┐
  │   SYSTEM  1    │ │  SYSTEM  2  │ │    SYSTEM  3     │
  │  Intuitive     │ │  Analytical │ │  Collaborative   │
  │                │ │             │ │                  │
  │ Trie + Levensh.│ │ ReAct Engine│ │  Swarm Agents    │
  │    < 10 ms     │ │   2 – 10 s  │ │     10 s +       │
  └────────┬───────┘ └──────┬──────┘ └──────────┬───────┘
           │                │                   │
┌──────────▼────────────────▼───────────────────▼───────────────────┐
│                    FOUNDATION LAYER                                 │
│   Memory │ GraphRAG │ Tools │ Safety Oracle │ Canvas │ Skills      │
└───────────────────────────────────────────────────────────────────┘
```

### ⚡ System 1 — Intuitive Response (`< 10ms`)

The fastest path, designed for high-frequency, low-complexity interactions:

- **`IntentTrie`** — O(L) prefix-tree intent lookup
- **Levenshtein fuzzy matching** — tolerates typos and spelling variations
- **Semantic cache** — memoized responses for repeated queries
- Ideal for: greetings, FAQs, system commands, cached results

### 🔬 System 2 — Deep Analysis (`2–10s`)

The workhorse for complex reasoning tasks, built on a **full ReAct pipeline** with a pluggable middleware chain:

```
[Safety Oracle]
    → [CostGuard]
    → [SemanticCache]
    → [Planning]
    → [RAG Middleware]
    → [SkillContext]
    → [ReAct Core: Thought → Action → Observation loop]
    → [Canvas Post-processor]
    → [Streaming Pipeline]
```

Additional System 2 capabilities:
- **Tree of Thoughts (ToT)** — parallel branch exploration for open-ended problems
- **MCTS (Monte Carlo Tree Search)** — UCB1-guided thought exploration with LLM simulation
- **Multimodal** — image and audio understanding integrated into reasoning

### 🌐 System 3 — Swarm Collaboration (`10s+`)

For tasks that require decomposition, parallelism, and coordination across multiple specialised agents:

- **`SwarmOrchestrator`** decomposes goals into a `TaskGraph`
- Individual agents run in isolated **Tokio tasks** (100+ concurrent)
- Agents communicate via **typed message channels** (`Task`, `Result`, `StatusUpdate`, `Broadcast`, `Error`)
- **`SharedBlackboard`** for cross-agent state sharing
- Built-in **`DebateModerator`** for structured multi-agent argumentation
- **`VotingAgent`** for democratic consensus on proposals
- **`TaskGraph` templates** — reusable multi-step workflow definitions

---

## 🧩 Memory System

Crablet implements a **three-tier hierarchical memory architecture** that mirrors how humans organize knowledge:

```
┌────────────────────────────────────────────────────────────┐
│                     MEMORY HIERARCHY                        │
│                                                             │
│  ┌──────────────┐  ┌───────────────┐  ┌─────────────────┐ │
│  │   Working    │  │   Episodic    │  │    Semantic     │ │
│  │   Memory     │  │   Memory      │  │    Memory       │ │
│  │              │  │               │  │                 │ │
│  │ VecDeque     │  │   SQLite      │  │  Neo4j / SQLite │ │
│  │ + Tiktoken   │  │   WAL Mode    │  │  Knowledge      │ │
│  │ Token Budget │  │   Sessions    │  │    Graph        │ │
│  │   O(1)       │  │  + Messages   │  │  + D3 Export    │ │
│  └──────┬───────┘  └───────┬───────┘  └────────┬────────┘ │
│         │                  │                   │           │
│         └──────────────────▼───────────────────┘           │
│                   MEMORY CONSOLIDATOR                       │
│         LLM-powered background summarization loop           │
│         Triggers: every 20 messages OR 1 hour TTL           │
│         Output: long-term vector embeddings                 │
└────────────────────────────────────────────────────────────┘
```

### Working Memory

- **Token-budgeted context window** — tracks exact token count using `tiktoken-rs` (cl100k_base tokenizer)
- **Smart compression** — preserves system message + last N exchanges; drops oldest messages when budget exceeded
- **Expiry TTL** — auto-evicts idle sessions (`is_expired(Duration)`)
- **Consolidator hook** — plugs into `MemoryConsolidator` for seamless promotion to long-term memory

### Episodic Memory

- **SQLite-backed persistence** — WAL journal mode, 64MB cache, optimized pragmas for high-throughput writes
- **Transactional writes** — `save_message_transactional` ensures consistency across concurrent sessions
- **Session management** — UUID-based session IDs with per-channel tracking
- **Chronological recall** — `get_history(session_id, limit)` returns messages in conversation order

### Semantic Memory (Knowledge Graph)

- **Dual backend** — `SqliteKnowledgeGraph` (zero-dependency) or `Neo4jKnowledgeGraph` (enterprise scale)
- **Entity + relation modeling** — `add_entity`, `add_relation`, `find_related` with directional traversal
- **D3.js export** — `export_d3_json()` produces graph visualization data for the Web UI dashboard
- **Batch entity lookups** — `find_entities_batch` for efficient bulk graph queries

### Memory Consolidator

The background **`MemoryConsolidator`** runs as a persistent Tokio task and:
1. Triggers every **20 messages** or **every 1 hour** per session
2. Fetches the last 50 messages and generates an LLM summary
3. Stores the summary as a timestamped embedding in the **Vector Store** (`type: "conversation_summary"`)
4. Tags memories with `importance` and `access_count` for future decay/retrieval scoring

---

## 📚 GraphRAG Knowledge Engine

Crablet's RAG system goes far beyond simple vector similarity — it combines dense retrieval with structured knowledge graph reasoning:

```
Query Input
    │
    ▼
Vector Search (fastembed / Qdrant)
    │           ↕  cosine similarity
    ▼
Entity Extraction (Rule + Phrase + Hybrid modes)
    │
    ▼
Knowledge Graph Traversal (Neo4j / SQLite)
    │  → finds related entities and relations
    ▼
Graph-Augmented Reranking
    │  score = vector_score × 0.7 + graph_boost × 0.3
    │  graph_boost = coverage × 0.4 + relation_weight × 0.25
    │              + centrality × 0.15 + graph_signal × 0.2
    ▼
Retrieved Context (documents + KG relations injected)
```

### Key RAG Features

| Feature | Implementation |
|---|---|
| **Embedding backends** | `fastembed` (AllMiniLM-L6-v2, 384-dim, local) or Qdrant cloud |
| **Vector store** | SQLite (default), Qdrant (via `qdrant-support` feature), In-Memory (tests) |
| **Chunking strategies** | `RecursiveCharacterChunker` (500 chars, 50 overlap) + `MarkdownChunker` (heading-aware) |
| **Entity extraction** | `Rule` (tokenization), `Phrase` (bigram windows), `Hybrid` (both) |
| **Graph backends** | `SqliteKnowledgeGraph` or `Neo4jKnowledgeGraph` (feature-gated) |
| **Centrality scoring** | Normalized in-/out-degree centrality for entity relevance ranking |
| **Document types** | Text, Markdown (structure-aware), PDF (`pdf-extract`), Multimodal |
| **Embedder pool** | 2 concurrent fastembed workers (Tokio blocking tasks) |
| **Reranking** | Graph-signal-weighted cosine reranking before final top-k selection |

### Supported Document Formats

```bash
crablet knowledge extract --file document.pdf      # PDF ingestion
crablet knowledge extract --file notes.md          # Markdown-aware chunking
crablet knowledge extract --file code.rs           # Source code indexing
crablet knowledge query "Rust async patterns"      # Semantic search
```

---

## 🎨 Canvas — Live Artifacts

Canvas is Crablet's **real-time structured output rendering system**. Rather than returning plain text, System 2 automatically detects and publishes rich artifacts to an interactive canvas that users can view, edit, and export.

### Canvas Component Types

```rust
enum CanvasComponent {
    Markdown  { content: String },
    Code      { language: String, content: String, filename: Option<String> },
    Mermaid   { chart: String },       // Auto-rendered flow/sequence diagrams
    DataTable { headers, rows, title }, // Structured tabular data
    Html      { content: String },      // Live HTML preview (UI mockups)
}
```

### How Canvas Works

1. **Auto-detection** — `detect_and_publish_canvas()` scans every LLM response for structured blocks
2. **Artifact routing** — detected artifacts are published to the `EventBus` as `AgentEvent::CanvasUpdate`
3. **Session-scoped state** — `CanvasManager` maintains per-session `CanvasState` with ordered sections
4. **Real-time streaming** — artifacts are streamed to the frontend via **SSE** (Server-Sent Events)
5. **CRUD operations** — `add_component`, `update_component`, `remove_component` for interactive editing

### Supported Artifact Triggers

| Trigger | Artifact Type | Example |
|---|---|---|
| ` ```mermaid ` | Flow diagram | Architecture diagrams, sequence charts |
| ` ```html ` + `<div/html/body>` | HTML Preview | UI mockups, dashboards |
| ` ```rust/python/ts ` (>5 lines) | Code snippet | Generated code, scripts |
| `DataTable` JSON | Data table | Query results, comparisons |
| Markdown sections | Rich text | Reports, documentation |

---

## 🤖 Multi-Agent Swarm

The **Swarm** system enables true multi-agent collaboration using Rust's ownership model to guarantee race-free concurrent execution:

### Agent Roles (Pre-built)

| Agent | File | Specialization |
|---|---|---|
| `Researcher` | `agent/researcher.rs` | Web search, information gathering |
| `Coder` | `agent/coder.rs` | Code generation and review |
| `Analyst` | `agent/analyst.rs` | Data analysis, pattern recognition |
| `Reviewer` | `agent/reviewer.rs` | Quality assurance, critique |
| `Planner` | `agent/planning.rs` | Task decomposition, goal trees |
| `SecurityAgent` | `agent/security.rs` | Vulnerability scanning, audit |
| `VotingAgent` | `agent/voting.rs` | Democratic consensus on proposals |
| `DebateModerator` | `agent/debate.rs` | Structured multi-round argumentation |
| `HITL` | `agent/hitl.rs` | Human-in-the-loop approval gates |

### Swarm Communication Model

```rust
// Typed message protocol — compile-time safe
enum SwarmMessage {
    Task         { task_id, description, context, payload },
    Result       { task_id, content, payload },
    StatusUpdate { task_id, status },
    Broadcast    { topic, content },     // pub/sub via topic registry
    Error        { task_id, error },
}
```

- **Pub/Sub topics** — agents subscribe to named topics; `Swarm::publish(topic, msg)` fans out
- **Timeout protection** — every agent processing step has a **30-second hard timeout**
- **Blackboard pattern** — `SharedBlackboard` (DashMap) for shared state without message passing
- **Persistence** — `SwarmPersister` saves task graphs and results to SQLite for crash recovery

---

## 🗂️ Project Structure

```
crablet/
├── src/
│   ├── main.rs                       # Entry point + CLI command dispatch
│   ├── cognitive/                    # ★ Cognitive core
│   │   ├── router.rs                 # Meta-router: classifies & dispatches to S1/S2/S3
│   │   ├── system1.rs                # Intuitive layer (Trie + Levenshtein)
│   │   ├── system2/
│   │   │   ├── mod.rs                # Deep reasoning orchestration
│   │   │   ├── canvas.rs             # Canvas artifact auto-detection
│   │   │   ├── multimodal.rs         # Vision + audio integration
│   │   │   └── post_process.rs       # Response post-processing
│   │   ├── system3.rs                # Swarm routing
│   │   ├── react.rs                  # ReAct Thought→Action→Observation loop
│   │   ├── tot.rs                    # Tree of Thoughts
│   │   ├── mcts_tot.rs               # MCTS + UCB1 thought exploration
│   │   ├── streaming_pipeline.rs     # SSE streaming output
│   │   ├── llm/
│   │   │   ├── mod.rs                # LlmClient trait + OpenAI/Ollama/DashScope/Kimi/ZhiPu
│   │   │   └── cache.rs              # Semantic response cache (Moka LRU)
│   │   ├── middleware/
│   │   │   ├── safety.rs             # Safety oracle injection
│   │   │   ├── cost_guard.rs         # Token budget enforcement
│   │   │   ├── semantic_cache.rs     # Query deduplication
│   │   │   ├── planning.rs           # Goal decomposition
│   │   │   ├── rag.rs                # RAG context injection
│   │   │   └── skill_context.rs      # Skill metadata injection
│   │   └── multimodal/               # Image + audio processing
│   ├── memory/                       # ★ Memory system
│   │   ├── working.rs                # Token-budgeted in-memory context
│   │   ├── episodic.rs               # SQLite session & message persistence
│   │   ├── semantic.rs               # Knowledge graph (SQLite + Neo4j)
│   │   ├── consolidator.rs           # Background LLM summarization loop
│   │   ├── manager.rs                # Unified memory access interface
│   │   └── shared.rs                 # SharedBlackboard for swarm agents
│   ├── knowledge/                    # ★ RAG engine
│   │   ├── vector_store.rs           # fastembed + SQLite/Qdrant vector store
│   │   ├── graph_rag.rs              # GraphRAG: vector + KG hybrid retrieval
│   │   ├── graph.rs                  # Knowledge graph CRUD
│   │   ├── chunking.rs               # Recursive + Markdown chunkers
│   │   ├── reranking.rs              # Graph-signal reranker
│   │   ├── extractor.rs              # Document text extraction
│   │   ├── pdf.rs                    # PDF parsing
│   │   ├── ingestion.rs              # Batch document ingestion pipeline
│   │   └── multimodal.rs             # Image/audio knowledge extraction
│   ├── gateway/                      # ★ API Gateway (Axum)
│   │   ├── server.rs                 # HTTP + WebSocket server setup
│   │   ├── websocket.rs              # Real-time WebSocket handler
│   │   ├── canvas.rs                 # Canvas component types
│   │   ├── canvas_manager.rs         # Per-session canvas state management
│   │   ├── rpc.rs                    # JSON-RPC 2.0 dispatcher
│   │   ├── session.rs                # Session lifecycle management
│   │   ├── ratelimit.rs              # Token bucket rate limiting (governor)
│   │   └── events.rs                 # SSE event streaming
│   ├── agent/                        # ★ Agent roles
│   │   ├── swarm.rs                  # Swarm orchestrator + channel mesh
│   │   ├── coordinator.rs            # Multi-agent task coordinator
│   │   ├── factory.rs                # Agent instantiation registry
│   │   ├── voting.rs                 # Consensus voting mechanism
│   │   ├── debate.rs                 # Multi-round debate moderator
│   │   ├── hitl.rs                   # Human-in-the-loop approval
│   │   ├── researcher.rs             # Research specialist
│   │   ├── coder.rs                  # Code generation specialist
│   │   ├── analyst.rs                # Data analysis specialist
│   │   ├── reviewer.rs               # Review & critique specialist
│   │   └── planning.rs               # Planning & decomposition
│   ├── tools/                        # Tool implementations
│   │   ├── bash.rs                   # Shell execution (SafetyOracle filtered)
│   │   ├── file.rs                   # File read/write/list
│   │   ├── search.rs                 # Web search (Serper / DuckDuckGo)
│   │   ├── http.rs                   # HTTP client tool
│   │   ├── vision.rs                 # Image analysis
│   │   ├── browser.rs                # Chromium browser automation
│   │   └── mcp.rs                    # MCP protocol tool bridge
│   ├── skills/                       # Plugin/skill management
│   │   ├── registry.rs               # Skill discovery and registration
│   │   ├── executor.rs               # YAML/Python/Node.js skill runner
│   │   ├── installer.rs              # Git-based skill installation
│   │   ├── openclaw.rs               # SKILL.md prompt-based skills
│   │   └── watcher.rs                # Hot-reload skill file watcher
│   ├── channels/                     # Input channel adapters
│   │   ├── cli/                      # Interactive CLI + subcommands
│   │   ├── domestic/
│   │   │   ├── dingtalk.rs           # DingTalk integration
│   │   │   └── feishu.rs             # Feishu/Lark integration
│   │   ├── international/
│   │   │   └── telegram.rs           # Telegram Bot
│   │   ├── discord.rs                # Discord Gateway
│   │   └── universal/
│   │       └── http_webhook.rs       # Generic HTTP webhook
│   ├── auth/                         # Authentication & authorization
│   │   ├── oidc.rs                   # OpenID Connect / OAuth2 flow
│   │   ├── handlers.rs               # Login/callback HTTP handlers
│   │   └── middleware.rs             # JWT validation middleware
│   ├── safety/                       # Safety layer
│   │   ├── oracle.rs                 # Command allowlist/blocklist
│   │   └── mod.rs                    # Safety level config (Strict/Permissive/Disabled)
│   ├── sandbox/                      # Execution sandboxing
│   │   ├── docker.rs                 # Docker container isolation
│   │   └── local.rs                  # Process-level sandboxing
│   ├── scripting/                    # Lua 5.4 scripting engine
│   │   ├── engine.rs                 # mlua runtime integration
│   │   └── bindings.rs               # Rust → Lua API bindings
│   ├── audit/                        # Security audit agent
│   ├── protocols/
│   │   └── a2a.rs                    # Agent-to-Agent protocol
│   └── telemetry.rs                  # OpenTelemetry tracing + metrics
├── frontend/                         # Web UI (TypeScript + React + TailwindCSS)
├── skills/                           # Built-in skill definitions (SKILL.md)
│   ├── create_skills/
│   ├── find_skills/
│   ├── proactive_agent/
│   └── safe_run/
├── mcp_servers/                      # Example MCP server scripts
│   ├── math_server.py
│   └── mcp_test_server.py
├── migrations/                       # SQLite migration files
├── config/config.toml                # Default configuration
├── templates/index.html              # Web UI HTML template
├── tests/                            # Integration & unit tests (25+ test files)
├── Cargo.toml
├── Dockerfile
├── docker-compose.yml
└── Justfile
```

---

## ⚙️ LLM Provider Support

Crablet supports a wide range of LLM backends through a unified `LlmClient` trait:

| Provider | Authentication | Streaming | Tool Calls | Notes |
|---|---|---|---|---|
| **OpenAI** | `OPENAI_API_KEY` | ✅ SSE | ✅ Function Calling | gpt-5.2, gpt-5.3, etc. |
| **DashScope** (Qwen) | `DASHSCOPE_API_KEY` | ✅ | ✅ | OpenAI-compatible endpoint |
| **Ollama** | Local | ✅ | ✅ | qwen2.5:14b default |
| **Kimi** (Moonshot) | `MOONSHOT_API_KEY` | ✅ | ✅ | Long-context specialist |
| **ZhiPu** (GLM) | `ZHIPU_API_KEY` | ✅ | ✅ | Chinese-optimized |
| **Any OpenAI-compat** | `OPENAI_API_BASE` | ✅ | ✅ | Custom base URL |

---

## 🚀 Quick Start

### Prerequisites

- **Rust 1.80+** — [install via rustup](https://rustup.rs/)
- **Docker** (optional — for Neo4j and sandbox execution)
- **Git**

### Option 1: One-Click Scripts (Recommended)

```bash
# 1. Clone
git clone https://github.com/isLinXu/crablet.git
cd crablet

# 2. Configure model settings anytime
./settings.sh
# Optional: non-interactive or skip model connectivity check
./settings.sh --non-interactive
./settings.sh --skip-verify

# 3. Install and start in one command
./one-click.sh
```

Common maintenance commands:

```bash
# Check service status
crablet status

# Deep clean build artifacts and cache (free up space)
./clean.sh

# Manual build and install CLI
./install.sh

# Show current configuration
crablet config

# Manage scheduled tasks
crablet task list

# Manage workflows
crablet workflow list

# Start services in debug mode (detailed logs)
./debug.sh

# Full uninstall
./uninstall.sh --full
```

### Option 2: Local Build

```bash
# 1. Clone
git clone https://github.com/isLinXu/crablet.git
cd crablet

# 2. Fast build — CLI + Web only (~5 min)
cargo build --release --no-default-features --features web

# 3. Full build — all features (~15-20 min)
cargo build --release

# 4. Initialize config & database
./target/release/crablet init

# 5. Set your LLM API key
export OPENAI_API_KEY=sk-xxx
# or for DashScope/Qwen:
export DASHSCOPE_API_KEY=sk-xxx
export OPENAI_API_BASE=https://dashscope.aliyuncs.com/compatible-mode/v1

# 6. Start chatting
./target/release/crablet chat
```

> 💡 **Speed tip**: Install [sccache](https://github.com/mozilla/sccache) to cache Rust compilation artifacts across builds:
> ```bash
> cargo install sccache
> export RUSTC_WRAPPER=sccache
> ```

### Option 3: Docker Compose (Recommended for full stack)

```bash
# Set your API key
export OPENAI_API_KEY=sk-xxx

# Launch Crablet + Neo4j
docker-compose up -d

# Open Web UI
open http://localhost:3000
```

### Option 4: Docker Single Container

```bash
docker run -d \
  --name crablet \
  -p 3000:3000 \
  -p 18789:18789 \
  -e OPENAI_API_KEY=sk-xxx \
  -v ./data:/data \
  -v ./skills:/skills \
  crablet:latest
```

---

## 💬 CLI Reference

```bash
# Interactive multi-turn chat
crablet chat

# Single task execution
crablet run "Analyze this Rust code for performance bottlenecks"

# Unified Web UI + API gateway
crablet serve-web --port 18790

# Explicit gateway launch (same unified control plane)
crablet gateway --port 18790

# Vision / multimodal
crablet vision --image ./screenshot.png --query "Describe this diagram"

# Audio transcription
crablet audio --file meeting.wav

# Skill management
crablet skill list
crablet skill install https://github.com/user/my-skill.git
crablet skill create my-new-skill

# Knowledge base
crablet knowledge extract --file document.pdf
crablet knowledge extract --file notes.md
crablet knowledge query "How does GraphRAG work?"

# Security audit
crablet audit .
crablet audit ./src --format json    # CI/CD-friendly JSON output

# System status
crablet status

# Lua scripting
crablet script run examples/scripts/summarize_paper.lua

# Debug / introspection
crablet debug --show-memory
crablet debug --show-graph
```

---

## 🔌 Plugin & Skill Ecosystem

Crablet provides **four complementary extension mechanisms**:

### 1. `skill.yaml` — Cross-Language Skills (Python / Node.js / Shell)

```yaml
name: weather
description: Get current weather for a city using OpenMeteo API
version: 1.0.0
parameters:
  type: object
  properties:
    city:
      type: string
      description: The city to get weather for
  required: [city]
entrypoint: python3 weather.py
timeout: 10
env:
  API_KEY: ${OPENMETEO_API_KEY}
```

### 2. `SKILL.md` — Prompt-Driven Skills (OpenClaw Compatible)

```markdown
---
name: python-expert
description: Expert Python coding assistant
version: 1.0.0
---

You are a Python expert. Always use type hints and docstrings.
When writing code, follow PEP 8 conventions.
```

### 3. MCP Protocol — Model Context Protocol

```toml
# config.toml
[mcp_servers]
math_server  = { command = "python3", args = ["mcp_servers/math_server.py"] }
custom_tools = { command = "node",    args = ["./my-mcp-server/index.js"] }
```

### 4. Built-in Tool Library

| Tool | Description | Safety |
|---|---|---|
| `bash` | Shell command execution | SafetyOracle allowlist |
| `file` | Read / Write / List files | Path sandboxing |
| `web_search` | Web search (Serper / DuckDuckGo) | Safe |
| `http` | Arbitrary HTTP requests | Safe |
| `vision` | Image understanding (multimodal LLM) | Safe |
| `browser` | Chromium browser automation | Docker sandbox |
| `calculator` | Math expression evaluation | Safe |
| `weather` | Weather API | Safe |

---

## ⚙️ Configuration Reference

**Config file location**: `~/.config/crablet/config.toml`

```toml
# Database
database_url = "sqlite:crablet.db?mode=rwc"

# LLM settings
model_name  = "gpt-4o-mini"
max_tokens  = 4096
temperature = 0.7

# Logging
log_level = "info"   # trace | debug | info | warn | error

# Safety
[safety]
level             = "Strict"   # Strict | Permissive | Disabled
allowed_commands  = ["ls", "cat", "echo", "pwd"]
blocked_commands  = ["rm", "mv", "sudo", "chmod"]

# MCP servers
[mcp_servers]
math_server = { command = "python3", args = ["mcp_servers/math_server.py"] }

# GraphRAG entity extraction mode
# GRAPH_RAG_ENTITY_MODE = "rule" | "phrase" | "hybrid" (default)

# Concurrency limits
[limits]
max_concurrent_requests = 100
request_timeout         = 30

# OIDC / OAuth2 (optional)
oidc_issuer        = "https://your-tenant.auth0.com/"
oidc_client_id     = "your-client-id"
oidc_client_secret = "your-client-secret"
jwt_secret         = "your-app-secret"

# Observability
[telemetry]
enabled  = true
endpoint = "http://tempo:4317"
```

### Environment Variables

| Variable | Description |
|---|---|
| `OPENAI_API_KEY` | OpenAI API key |
| `DASHSCOPE_API_KEY` | Alibaba DashScope (Qwen) API key |
| `OPENAI_API_BASE` | Custom OpenAI-compatible base URL |
| `OLLAMA_MODEL` | Local Ollama model name (default: `qwen2.5:14b`) |
| `OLLAMA_API_BASE` | Ollama server URL (default: `http://localhost:11434`) |
| `MOONSHOT_API_KEY` | Kimi (Moonshot AI) API key |
| `ZHIPU_API_KEY` | ZhiPu GLM API key |
| `SERPER_API_KEY` | Serper web search API key |
| `DATABASE_URL` | SQLite or PostgreSQL connection string |
| `QDRANT_URL` | Qdrant vector database URL |
| `RUST_LOG` | Log level (`info`, `debug`, `trace`) |
| `GRAPH_RAG_ENTITY_MODE` | Entity extraction mode: `rule` / `phrase` / `hybrid` |

### Feature Flags

```bash
cargo build --release --no-default-features --features <flags>
```

| Feature Flag | Description | Default |
|---|---|---|
| `web` | Axum HTTP server + Web UI + REST API | ✅ |
| `qdrant-support` | Qdrant vector database backend | ✅ |
| `knowledge` | Full RAG: fastembed + PDF + Neo4j + Qdrant | ✅ (with `knowledge`) |
| `audio` | Whisper speech-to-text | ❌ |
| `scripting` | Lua 5.4 scripting engine (mlua) | ❌ |
| `telemetry` | OpenTelemetry distributed tracing | ❌ |
| `sandbox` | Docker container isolation (bollard) | ❌ |
| `telegram` | Telegram Bot integration | ❌ |
| `discord` | Discord Gateway integration | ❌ |
| `browser` | Chromium browser automation | ❌ |
| `inference` | ONNX Runtime (local model inference) | ❌ |
| `full` | All features enabled | ❌ |

---

## 🌐 API Reference

### WebSocket Gateway

**Endpoint**: `ws://localhost:18789/ws`

**Send a message:**
```json
{
  "type": "UserInput",
  "content": "Write a Rust async function that reads a file"
}
```

**Receive streaming events:**

| Event Type | Description |
|---|---|
| `ThoughtGenerated` | ReAct reasoning step (System 2 thinking) |
| `ToolExecutionStarted` | Tool invocation initiated |
| `ToolExecutionFinished` | Tool result received |
| `ResponseGenerated` | Final agent response |
| `CanvasUpdate` | New canvas artifact (type: mermaid / code / html / markdown) |
| `SwarmActivity` | Inter-agent message in the swarm |
| `MemoryConsolidated` | Background memory consolidation completed |

### REST API

| Method | Endpoint | Description |
|---|---|---|
| `POST` | `/api/chat` | Single-turn chat completion |
| `GET` | `/api/status` | System health & stats |
| `GET` | `/api/dashboard` | Monitoring dashboard data |
| `GET` | `/api/knowledge` | List knowledge base documents |
| `DELETE` | `/api/knowledge?source=...` | Delete knowledge document |
| `GET` | `/api/canvas/:session_id` | Get canvas state for session |
| `GET` | `/auth/login` | Initiate OIDC login flow |
| `GET` | `/auth/callback` | OIDC callback handler |
| `GET` | `/api/me` | Current authenticated user |

---

## 🚢 Deployment

### Docker Compose (Full Stack with Neo4j)

```yaml
version: '3.8'

services:
  crablet:
    image: crablet:latest
    ports:
      - "3000:3000"      # Web UI
      - "18789:18789"    # WebSocket Gateway
    environment:
      - OPENAI_API_KEY=${OPENAI_API_KEY}
      - DATABASE_URL=sqlite:///data/crablet.db
      - RUST_LOG=info
    volumes:
      - ./data:/data
      - ./skills:/skills
    depends_on:
      - neo4j

  neo4j:
    image: neo4j:5
    ports:
      - "7474:7474"      # Neo4j Web UI
      - "7687:7687"      # Bolt protocol
    environment:
      - NEO4J_AUTH=neo4j/password
    volumes:
      - neo4j_data:/data

volumes:
  neo4j_data:
```

### Production Configuration

```toml
# config.toml (production)
database_url = "postgresql://user:pass@localhost/crablet"
log_level    = "warn"

[safety]
level = "Strict"

[telemetry]
enabled  = true
endpoint = "http://otel-collector:4317"

[limits]
max_concurrent_requests = 100
request_timeout         = 30
```

### Observability Stack

Crablet exports **OpenTelemetry** telemetry compatible with the full CNCF observability stack:

| Component | Integration |
|---|---|
| **Distributed Tracing** | Jaeger / Tempo |
| **Metrics** | Prometheus (`crablet.request.duration`, `crablet.llm.tokens`) |
| **Dashboards** | Grafana |
| **Log aggregation** | Loki / any OTLP-compatible backend |

---

## 🧪 Testing

The project includes **25+ test files** covering:

```bash
# Unit & integration tests
cargo test

# Specific test suites
cargo test memory_test
cargo test vector_store_integration_test
cargo test react_chain_test
cargo test system1_verify
cargo test safety_test
cargo test graph_rag_returns_augmented_context

# End-to-end tests
cargo test e2e_full
cargo test e2e_auth_audit

# Demo tests (run specific agent scenarios)
cargo test demo_debate
cargo test demo_voting
cargo test demo_rag
```

---

## 📚 Documentation

| Document | Description |
|---|---|
| [Getting Started](https://github.com/isLinXu/crablet/blob/main/docs/getting-started.md) | Installation, first run, and basic usage |
| [Architecture](https://github.com/isLinXu/crablet/blob/main/docs/architecture.md) | Deep dive into the cognitive architecture |
| [API Reference](https://github.com/isLinXu/crablet/blob/main/docs/api-reference.md) | CLI, REST API, WebSocket, and JSON-RPC |
| [Configuration](https://github.com/isLinXu/crablet/blob/main/docs/configuration.md) | All config options and environment variables |
| [Deployment](https://github.com/isLinXu/crablet/blob/main/docs/deployment.md) | Docker, production setup, and observability |
| [Skill Development](https://github.com/isLinXu/crablet/blob/main/docs/skill-development.md) | Writing custom skills and MCP servers |
| [Contributing](https://github.com/isLinXu/crablet/blob/main/docs/contributing.md) | How to contribute to the project |
| [Roadmap](https://github.com/isLinXu/crablet/blob/main/docs/roadmap.md) | Upcoming features and milestones |

---

## 🗺️ Roadmap

### ✅ Implemented

- [x] Three-tier cognitive architecture (System 1 / 2 / 3)
- [x] Layered memory: Working → Episodic → Semantic + LLM Consolidation
- [x] GraphRAG: vector store + knowledge graph hybrid retrieval
- [x] Canvas: live artifact rendering (Mermaid, Code, HTML, DataTable)
- [x] Multi-agent Swarm with VotingAgent, DebateModerator, HITL
- [x] MCTS-based Tree of Thoughts reasoning
- [x] OIDC/OAuth2 authentication (Auth0, Google, Keycloak)
- [x] REST + WebSocket + JSON-RPC gateway
- [x] Plugin ecosystem (YAML skills, SKILL.md, MCP protocol)
- [x] Security Audit Agent
- [x] Multi-channel: CLI, Telegram, DingTalk, Feishu, Discord, HTTP Webhook
- [x] OpenTelemetry tracing + Prometheus metrics
- [x] Docker + Docker Compose deployment

### 🔄 Near-term (1–3 months)

- [ ] **High-performance gateway rewrite** — Axum-native, 3-5x throughput improvement
- [ ] **20+ messaging channels** — WeChat Work, Slack, WhatsApp, Microsoft Teams, QQ
- [ ] **Agent Coordinator V2** — Dynamic role assignment, context isolation, sub-agent spawning
- [ ] **Canvas Editor** — Interactive canvas with Monaco code editor + Mermaid live preview
- [ ] **Approval workflow** — `crablet approve <code>` human-in-the-loop API
- [ ] **One-line installer** — `curl -fsSL https://crablet.dev/install.sh | sh`

### 🔮 Mid-term

- [ ] **Multi-tenancy** — RBAC, org isolation, SSO (LDAP/OAuth2), GDPR compliance
- [ ] **Function Calling V2** — Parallel tool execution (`ParallelToolExecution`)
- [ ] **Long context management** — 128K+ token compression + summarization
- [ ] **Procedural Memory** — Skill learning from successful task execution
- [ ] **Cron scheduler** — Time-based and event-triggered task automation
- [ ] **Cost analytics dashboard** — Per-model, per-user token cost tracking

### 🌟 Long-term (6–12 months)

- [ ] **Crablet Skill Store** — `crablet skill install/search/publish` marketplace
- [ ] **Crablet Cloud** — Multi-tenant SaaS hosting with autoscaling

---

## 🤝 Contributing

Contributions of all kinds are welcome! Please read [CONTRIBUTING.md](https://github.com/isLinXu/crablet/blob/main/docs/contributing.md) first.

```bash
# Fork & clone
git clone https://github.com/your-username/crablet.git
cd crablet

# Create a feature branch
git checkout -b feature/amazing-feature

# Make changes, add tests
cargo test

# Commit with conventional commits
git commit -m "feat(memory): add memory decay scoring for old summaries"

# Push and open a PR
git push origin feature/amazing-feature
```

### Development Guidelines

- **Rust style**: `cargo fmt` + `cargo clippy` before committing
- **Tests required**: All new features must include integration tests
- **Feature flags**: New optional dependencies must be feature-gated
- **Safety first**: New tools must go through `SafetyOracle`
- **No blocking**: All I/O must use `tokio::spawn_blocking` or async APIs

---

## 📄 License

This project is licensed under the **MIT License** — see [LICENSE](LICENSE) for details.

---

<div align="center">

**If Crablet helps you build something awesome, please give it a ⭐ Star!**

<br>

Built with 🦀 **Rust** and ❤️ by [isLinXu](https://github.com/isLinXu) and contributors

<br>

*Crablet — because good AI infrastructure should be as reliable as a crab's shell,*  
*and as fast as it scuttles sideways.*

</div>
