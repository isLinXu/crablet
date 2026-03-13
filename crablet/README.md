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
- Node.js 18+ (前端开发)

### Installation

```bash
# Clone the repository
git clone https://github.com/yourname/crablet.git
cd crablet

# 配置环境变量
cp .env.example .env
# 编辑 .env 文件，填入必要的 API 密钥

# 配置文件（可选）
cp config/config.toml.example config/config.toml

# Build
cargo build --release
```

### Environment Variables

复制 `.env.example` 到 `.env` 并配置以下变量：

#### 必需变量
- `DATABASE_URL` - 数据库连接字符串 (默认: `sqlite:crablet.db?mode=rwc`)

#### AI API 密钥 (至少配置一个)
- `OPENAI_API_KEY` - OpenAI API 密钥
- `DASHSCOPE_API_KEY` - 阿里云 DashScope API 密钥

#### 消息平台 (可选)
- `TELEGRAM_BOT_TOKEN` - Telegram Bot Token
- `FEISHU_APP_ID` / `FEISHU_APP_SECRET` - 飞书应用凭证
- `WECOM_CORP_ID` / `WECOM_CORP_SECRET` - 企业微信凭证

#### 认证 (可选)
- `OIDC_ISSUER` / `OIDC_CLIENT_ID` / `OIDC_CLIENT_SECRET` - OIDC 配置
- `JWT_SECRET` - JWT 签名密钥

完整的环境变量列表请参考 `.env.example` 文件。

### Running CLI

```bash
# Start interactive chat
cargo run -- chat

# Run a single prompt
cargo run -- run "hello"

# Check status
cargo run -- status
```

### Running Web Server

```bash
# 启动后端服务
cargo run -- serve

# 前端开发 (新终端)
cd frontend
npm install
npm run dev
```

### Running Telegram Bot

1. 在 `.env` 文件中设置 `TELEGRAM_BOT_TOKEN`
2. 运行:
```bash
cargo run -- serve
```

## 🏗️ Project Structure

- `src/`: Rust 后端源代码
  - `cognitive/`: 认知路由系统 (System 1/2/3)
  - `memory/`: 记忆系统 (工作记忆/情景记忆)
  - `tools/`: 工具实现 (Bash, File, etc.)
  - `channels/`: 消息通道 (CLI, Telegram, 飞书, 企业微信)
  - `auth/`: 认证系统 (OIDC, JWT)
  - `knowledge/`: 知识库和 RAG
  - `skills/`: 技能系统
- `frontend/`: React + TypeScript 前端
- `mcp_servers/`: MCP (Model Context Protocol) 服务器
- `skills/`: 技能定义文件
- `migrations/`: 数据库迁移脚本
- `config/`: 配置文件目录

## 📝 License

MIT / Apache 2.0
