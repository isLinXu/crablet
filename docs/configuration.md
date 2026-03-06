# 配置参考

## 配置文件

配置文件路径：`~/.config/crablet/config.toml`

```toml
# 数据库连接
database_url = "sqlite:crablet.db?mode=rwc"

# LLM 配置
model_name = "gpt-4o-mini"
max_tokens = 4096
temperature = 0.7

# 日志级别
log_level = "info"  # trace/debug/info/warn/error

# 安全等级
[safety]
level = "Strict"  # Strict/Permissive/Disabled
allowed_commands = ["ls", "cat", "echo"]
blocked_commands = ["rm", "mv"]

# MCP 服务器
[mcp_servers]
math_server = { command = "python3", args = ["mcp_server.py"] }

# 技能目录
skills_dir = "skills"

# 限制配置
[limits]
max_concurrent_requests = 100
request_timeout = 30
```

## 环境变量

环境变量可以覆盖配置文件中的设置：

| 变量名 | 说明 |
|:---|:---|
| `OPENAI_API_KEY` | OpenAI API 密钥 |
| `DASHSCOPE_API_KEY` | 阿里云 DashScope API 密钥 |
| `OPENAI_API_BASE` | OpenAI 兼容 API 地址 |
| `OLLAMA_MODEL` | 本地 Ollama 模型名称 |
| `SERPER_API_KEY` | Serper 搜索 API 密钥 |
| `DATABASE_URL` | 数据库连接字符串 |
| `RUST_LOG` | 日志级别 |
| `GRAPH_RAG_ENTITY_MODE` | GraphRAG 实体抽取模式（`rule`/`phrase`/`hybrid`，默认 `hybrid`） |

## Feature Flags

编译时可通过 Feature Flags 按需裁剪功能：

| Feature | 包含内容 | 默认启用 |
|:---|:---|:---:|
| `web` | Web UI、API 网关 | ✅ |
| `knowledge` | RAG、向量存储、Neo4j、Qdrant | ✅ |
| `audio` | 语音识别（Whisper）、语音合成 | ✅ |
| `scripting` | Lua 5.4 脚本引擎 | ✅ |
| `telemetry` | OpenTelemetry 追踪 | ✅ |
| `sandbox` | Docker 沙箱执行 | ✅ |
| `telegram` | Telegram Bot 接入 | ✅ |
| `discord` | Discord Bot 接入 | ✅ |
| `browser` | 无头浏览器自动化 | ✅ |

示例：仅启用 Web 和脚本引擎
```bash
cargo build --release --no-default-features --features web,scripting
```
