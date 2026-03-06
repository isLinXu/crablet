<div align="center">

# 🦀 Crablet

<h3>用 Rust 构建的下一代 AI Agent 操作系统</h3>

[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange?logo=rust)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](docs/contributing.md)

[快速开始](docs/getting-started.md) · [架构设计](docs/architecture.md) · [API 参考](docs/api-reference.md) · [文档](docs/)

</div>

---

## 📖 项目简介

**Crablet** 是一个用 Rust 从零构建的 **完整 AI Agent 操作系统**。它不是一个简单的 LLM 聊天机器人，而是提供了一套生产就绪的认知架构，让大型语言模型拥有**思考、规划、记忆、工具使用和多 Agent 协作**的完整能力。

### 核心特性

- **🚀 极致性能**: 基于 Tokio 异步运行时，原生并发，零 GC 停顿。
- **🛡️ 内存安全**: Rust 编译期保证，内置安全沙箱 (Docker) 和命令拦截。
- **🧠 三层认知架构**:
  - **System 1**: 直觉响应 (Trie 查找, <10ms)
  - **System 2**: 深度分析 (ReAct 引擎, 2-10s)，具备语义缓存与循环检测。
  - **System 3**: 群体协作 (Swarm 多 Agent, 10s+)，支持复杂任务分发。
- **📚 RAG 知识引擎**: 内置向量存储 (SQLite/Qdrant)、重排序和知识图谱。
- **🔌 插件生态**: 支持 MCP 协议、Python/Node.js 可执行技能和原生 Rust 插件。
- **🔐 企业级认证**: 支持 OIDC/OAuth2 (OpenID Connect) 登录与多租户隔离。
- **📊 全链路可观测性**: 基于 OpenTelemetry 的分布式追踪与 Dashboard API。

## 🚀 快速开始

```bash
# 1. 克隆项目
git clone https://github.com/yourusername/crablet.git
cd crablet

# 2. 运行初始化 (需 Rust 1.80+)
cargo run --release -- init

# 3. 设置 API Key
export OPENAI_API_KEY=sk-xxx

# 4. 启动聊天
cargo run --release -- chat
```

## ✨ 新特性 (v0.3.0)

### 1. 安全审计 Agent (Security Audit)

内置专注于代码安全的垂直领域 Agent，支持自动扫描代码库漏洞并提供修复建议。

```bash
# 扫描当前目录并输出文本报告
crablet audit .

# 扫描指定路径并输出 JSON 格式（适合 CI/CD）
crablet audit ./src --format json
```

### 2. 管理控制台 API

提供 RESTful API 用于监控系统状态与管理知识库。

- **Dashboard**: `GET /api/dashboard` (系统负载、技能列表)
- **知识库管理**: `GET /api/knowledge`, `DELETE /api/knowledge?source=...`
- **身份认证**: `GET /auth/login` (OIDC), `GET /api/me`

启动 Web 服务：
```bash
cargo run --release -- serve-web --port 3000
```

### 3. 企业级 OIDC 认证

支持对接 Auth0, Google, Keycloak 等标准 OIDC 提供商。

在 `config.toml` 或环境变量中配置：
```toml
oidc_issuer = "https://your-tenant.auth0.com/"
oidc_client_id = "your-client-id"
oidc_client_secret = "your-client-secret"
jwt_secret = "your-app-secret"
```

## 📂 文档索引

- [快速开始](docs/getting-started.md) - 安装、配置与基本使用
- [架构设计](docs/architecture.md) - 系统架构、核心组件与数据流
- [API 参考](docs/api-reference.md) - CLI、Web API 与 WebSocket 协议
- [配置参考](docs/configuration.md) - 配置文件与环境变量
- [部署指南](docs/deployment.md) - Docker 部署与生产环境配置
- [技能开发](docs/skill-development.md) - 如何开发工具与技能
- [贡献指南](docs/contributing.md) - 参与项目开发
- [路线图](docs/roadmap.md) - 项目规划与进度

## 🤝 参与贡献

欢迎所有形式的贡献！请参阅 [贡献指南](docs/contributing.md)。

## 📄 许可证

本项目基于 [MIT License](LICENSE) 开源。
