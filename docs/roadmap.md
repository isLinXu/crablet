# Crablet v3.0 超越战略：三步走路线图

## 阶段一：补齐短板（1-3 个月）🔴
**目标：达到 OpenClaw/Agent Zero 的核心功能完整度**

### 1.1 网关架构改造 ⭐⭐⭐⭐⭐
**技术栈**: Rust (Axum + Tokio) vs TypeScript (Fastify) -> 性能提升 3-5 倍

**核心模块**: `src/gateway/`
```rust
pub struct CrabletGateway {
    websocket_server: WsServer,      // WebSocket 服务器 (ws://localhost:18789)
    rpc_dispatcher: RpcDispatcher,   // JSON-RPC 2.0 调度器 (兼容 OpenClaw)
    auth_manager: AuthManager,       // 多层认证 (off/local/token/api-key/mTLS)
    session_manager: SessionManager, // 会话管理
    event_bus: EventBus,             // 事件总线 (SSE 实时流)
}
```

### 1.2 消息渠道大扩展 ⭐⭐⭐⭐⭐
**目标**: 3 个月内支持 20+ 平台 (超越 OpenClaw 15+)

**架构**: `src/channels/`
- **国内平台 (优先)**: 飞书 (企业级), 钉钉, 企业微信, QQ (go-cqhttp/Lagrange), 企微群机器人
- **国际平台**: Telegram, Discord, Slack, WhatsApp, Microsoft Teams
- **通用协议**: HTTP Webhook, WebSocket, MCP Channel

**策略**: 统一抽象层 `Channel` trait。国内平台优先官方 SDK/FFI。

### 1.3 多 Agent 协作系统 ⭐⭐⭐⭐⭐
**架构**: 层级架构 (参考 Agent Zero) -> `src/agent/coordinator.rs`

```rust
pub struct AgentCoordinator {
    primary_agent: Arc<Agent>,
    subordinate_agents: HashMap<String, Agent>,
    task_graph: TaskGraph,
    context_isolator: ContextIsolator,
}
```

**角色预设**: `config/agents/`
- Researcher (研究专家)
- Coder (代码专家)
- Analyst (数据分析师)
- Executor (执行专家)
- Reviewer (代码审查员)
- Planner (任务规划师)

**优势**: Rust Tokio 并发模型支持 100+ Agent。

### 1.4 Canvas 可视化系统 ⭐⭐⭐⭐
**参考**: OpenClaw Canvas -> `src/canvas/`

**组件**:
- 流程图 (Mermaid)
- 代码编辑器 (Monaco)
- Markdown 渲染
- 文件浏览器
- 实时日志流
- Agent 思维链可视化 (3D, 类似 LangSmith)
- 任务甘特图

**技术栈**: Axum + SSE (后端), React + TailwindCSS + shadcn/ui (前端), WebSocket + SSE (通信)。

### 1.5 用户与权限管理 ⭐⭐⭐⭐
**机制**: DM 配对 (OpenClaw 模式) -> `src/auth/`

**流程**:
1. 未知用户发消息 -> 生成 6 位配对码
2. 管理员运行 `crablet approve ABC123`
3. 用户加入白名单 (Admin/ReadWrite/ReadOnly/ToolExecution/AgentSpawn/ConfigModify)

### 1.6 配置与部署优化 ⭐⭐⭐⭐
- **一键部署**: `curl -fsSL https://crablet.dev/install-cn.sh` (自动检测 Rust, Docker, 插件)
- **Docker 优化**: 多阶段构建 (Alpine), 镜像大小 < 20MB, 启动 < 500ms
- **配置分层**: `config/crablet.toml` (deploy mode, channels, agents, security)

---

## 阶段二：建立优势（3-6 个月）🟡
**目标：在核心功能上超越竞品**

### 2.1 企业级增强 ⭐⭐⭐⭐⭐
- **多租户架构**: `src/tenant/` (资源隔离, 配额管理)
- **功能**: RBAC, 审计日志, SSO (LDAP/OAuth2), 合规性 (GDPR/等保)

### 2.2 AI 能力升级 ⭐⭐⭐⭐⭐
- **Function Calling V2**: 并行工具调用 (`ParallelToolExecution`)
- **模型路由增强**: 动态选择模型 (根据 token/内容), 成本优化
- **长上下文**: 128K+ 支持, 智能压缩, 分片合并

### 2.3 知识与记忆升级 ⭐⭐⭐⭐⭐
- **向量数据库**: Qdrant (替换 FastEmbed) + Neo4j (图谱) + MeiliSearch (全文)
- **记忆系统 V2**: 工作记忆, 情节记忆, 语义记忆, **技能记忆 (Procedural)**
- **自动摘要**: 定期/定量触发

### 2.4 调度与自动化 ⭐⭐⭐⭐⭐
- **Cron 调度**: `src/scheduler/` (定时任务)
- **工作流引擎**: 可视化节点编排 (Trigger, Agent, Tool, Condition, Loop, Approval)

### 2.5 监控与可观测性 ⭐⭐⭐⭐⭐
- **OpenTelemetry**: 全链路追踪 (`tracing`), Metrics (LLM cost/latency)
- **Dashboard**: 实时监控面板
- **告警系统**: `config/alerts.yaml` (错误率, 成本激增)

---

## 阶段三：建立生态（6-12 个月）🟢
**目标：构建护城河，形成生态壁垒**

### 3.1 技能市场 ⭐⭐⭐⭐⭐
- **Crablet Skill Store**: `crablet skill install/search/publish`
- **标准化**: `skill.yaml` (依赖, 入口, 能力, 定价)
- **生态**: 分类, 评分, 安全审计

### 3.2 云服务与 SaaS ⭐⭐⭐⭐⭐
- **Crablet Cloud**: 托管服务, 多租户, 自动扩容, 全球 CDN, SLA 保障
