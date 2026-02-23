# 🦀 Crablet (MVP v0.1.0)

> "Building the next-generation intelligent assistant infrastructure, making AI as ubiquitous as water—fluid, adaptive, and ceaseless."

## 🚀 Features

- **Dual-System Cognitive Architecture**:
  - **System 1**: Fast, intuitive responses (<50ms) using in-memory cache.
  - **System 2**: Deep reasoning (mocked for MVP) with tool usage capabilities.
- **Memory System**:
  - **Working Memory**: In-memory context window.
  - **Episodic Memory**: SQLite-based chat history storage.
- **Tools**:
  - `BashTool`: Execute shell commands.
  - `FileTool`: Read/Write/List files.
- **Channels**:
  - CLI: Interactive chat and single command execution.
  - Telegram: Bot integration.

## 🛠️ Usage

### Prerequisites

- Rust (latest stable)
- SQLite (bundled via sqlx)

### Installation

```bash
# Clone the repository
git clone https://github.com/yourname/crablet.git
cd crablet

# Build
cargo build --release
```

### Running CLI

```bash
# Start interactive chat
cargo run -- chat

# Run a single prompt
cargo run -- run "hello"

# Check status
cargo run -- status
```

### Running Telegram Bot

1. Set `TELEGRAM_BOT_TOKEN` environment variable.
2. Run:
```bash
export TELEGRAM_BOT_TOKEN=your_token
cargo run -- serve
```

## 🏗️ Project Structure

- `src/cognitive`: Router, System 1, System 2 implementation.
- `src/memory`: Working and Episodic memory systems.
- `src/tools`: Tool implementations (Bash, File).
- `src/channels`: CLI and Telegram interfaces.

## 📝 License

MIT / Apache 2.0
