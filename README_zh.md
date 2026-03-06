<div align="center">

<h1>🦀 Crablet</h1>

<p>
  <strong>用 Rust 从零构建的生产级 AI Agent 操作系统</strong>
</p>

<p>
  <em>"构建下一代智能助手基础设施，<br>
  让 AI 像水一样无处不在——流动、自适应、生生不息。"</em>
</p>

<p>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.80%2B-orange?style=flat-square&logo=rust" alt="Rust 版本"/></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue?style=flat-square" alt="开源协议"/></a>
  <img src="https://img.shields.io/badge/Tokio-异步运行时-green?style=flat-square" alt="Tokio"/>
  <img src="https://img.shields.io/badge/认知架构-System%201%2F2%2F3-purple?style=flat-square" alt="Architecture"/>
  <img src="https://img.shields.io/badge/RAG-GraphRAG%20%2B%20Qdrant-yellow?style=flat-square" alt="RAG"/>
  <img src="https://img.shields.io/badge/认证-OIDC%20%2F%20OAuth2-red?style=flat-square" alt="Auth"/>
  <img src="https://img.shields.io/badge/Canvas-实时产物渲染-teal?style=flat-square" alt="Canvas"/>
</p>

<p>
  <img src="https://img.shields.io/github/languages/top/isLinXu/crablet?style=flat-square" alt="主语言"/>
  <img src="https://img.shields.io/github/repo-size/isLinXu/crablet?style=flat-square" alt="仓库大小"/>
  <img src="https://img.shields.io/github/stars/isLinXu/crablet?style=flat-square" alt="Star 数"/>
  <img src="https://img.shields.io/github/last-commit/isLinXu/crablet?style=flat-square" alt="最近提交"/>
</p>

<p>
  <a href="README.md">English</a> | <a href="README_zh.md">中文</a>
</p>

<p>
  <a href="#-为什么选择-crablet">为什么选择 Crablet</a> •
  <a href="#-三层认知架构">架构</a> •
  <a href="#-记忆系统">记忆</a> •
  <a href="#-graphrag-知识引擎">GraphRAG</a> •
  <a href="#-canvas--实时产物渲染">Canvas</a> •
  <a href="#-多-agent-swarm">Swarm</a> •
  <a href="#-快速开始">快速开始</a> •
  <a href="#-文档">文档</a>
</p>

</div>

---

## 🌟 为什么选择 Crablet？

**Crablet** 不是又一个聊天机器人封装库。它是一个完全用 Rust 编写的**完整 AI Agent 操作系统**——为大型语言模型提供生产就绪的认知基础设施，赋予其真正的思考、规划、记忆、工具使用和多 Agent 协作能力。

| 传统 LLM 封装 | Crablet |
|---|---|
| 无状态请求/响应 | **持久化分层记忆**（工作记忆 → 情节记忆 → 语义记忆） |
| 单步工具调用 | **带中间件管道的 ReAct 推理循环** |
| 平坦检索（关键词搜索） | **GraphRAG**：向量检索 + 知识图谱遍历融合 |
| 无输出结构化 | **Canvas**——实时结构化产物渲染 |
| 串行处理 | **Swarm**——100+ 并发协作 Agent |
| Python GC 停顿 | **零 GC 的 Rust** + Tokio 异步运行时 |

### 语言构成

| 语言 | 占比 | 职责 |
|---|---|---|
| 🦀 **Rust** | 65.2% | 核心引擎、认知层、记忆系统、工具 |
| 🟦 **TypeScript** | 28.8% | 前端、Web UI、Dashboard |
| 🌐 **HTML** | 4.7% | 模板、Canvas 渲染 |
| 🐍 **Python** | 0.8% | MCP 服务器、技能脚本 |
| 🐳 **Dockerfile** | 0.2% | 容器定义 |

---

## 🧠 三层认知架构

Crablet 最具特色的设计是其**受生物学启发的三层认知路由系统**。受双过程理论启发，每条输入都由**认知路由器（Cognitive Router）**自动分类并分发至最优处理层：

```
┌──────────────────────────────────────────────────────────────────┐
│                        输入渠道层                                  │
│    CLI │ Web UI │ Telegram │ 钉钉 │ 飞书 │ HTTP Webhook            │
└───────────────────────────┬──────────────────────────────────────┘
                            │
┌───────────────────────────▼──────────────────────────────────────┐
│               网关层（Axum + WebSocket）                           │
│    JSON-RPC │ REST API │ SSE 流式 │ 认证 │ 限流                    │
└───────────────────────────┬──────────────────────────────────────┘
                            │
┌───────────────────────────▼──────────────────────────────────────┐
│                       事件总线                                     │
│              Tokio Broadcast Channel（无锁）                       │
└──────────┬────────────────┬───────────────────┬───────────────────┘
           │                │                   │
  ┌────────▼───────┐ ┌──────▼──────┐ ┌──────────▼──────┐
  │   SYSTEM  1    │ │  SYSTEM  2  │ │    SYSTEM  3     │
  │   直觉响应     │ │  深度分析   │ │    群体协作      │
  │                │ │             │ │                  │
  │ Trie+Levensh.  │ │ ReAct 引擎  │ │  Swarm Agents    │
  │    < 10 ms     │ │   2 – 10 s  │ │     10 s +       │
  └────────┬───────┘ └──────┬──────┘ └──────────┬───────┘
           │                │                   │
┌──────────▼────────────────▼───────────────────▼───────────────────┐
│                         基础能力层                                  │
│   Memory │ GraphRAG │ Tools │ Safety Oracle │ Canvas │ Skills      │
└───────────────────────────────────────────────────────────────────┘
```

### ⚡ System 1 — 直觉响应层（`< 10ms`）

最快响应路径，专为高频、低复杂度交互设计：

- **`IntentTrie`** — O(L) 前缀树意图查找
- **Levenshtein 模糊匹配** — 容忍拼写错误和变体
- **语义缓存** — 对重复查询记忆化响应
- 适用于：问候语、FAQ、系统命令、缓存命中

### 🔬 System 2 — 深度分析层（`2–10s`）

处理复杂推理任务的主力引擎，基于**完整的 ReAct 管道**和可插拔中间件链：

```
[Safety Oracle 安全检查]
    → [CostGuard Token 预算守卫]
    → [SemanticCache 语义缓存]
    → [Planning 规划分解]
    → [RAG Middleware RAG 上下文注入]
    → [SkillContext 技能上下文]
    → [ReAct 核心：思考 → 行动 → 观察 循环]
    → [Canvas 后处理器]
    → [Streaming Pipeline 流式管道]
```

System 2 附加能力：
- **思维树（Tree of Thoughts, ToT）** — 对开放性问题并行探索多个推理分支
- **蒙特卡洛树搜索（MCTS）** — UCB1 引导的思维探索 + LLM 价值评估
- **多模态** — 图像与音频理解无缝集成到推理过程中

### 🌐 System 3 — 群体协作层（`10s+`）

用于需要任务拆解、并行执行和多 Agent 协调的复杂任务：

- **`SwarmOrchestrator`** 将目标分解为 `TaskGraph` 任务图
- 每个 Agent 在独立的 **Tokio 任务**中运行（支持 100+ 并发）
- Agent 间通过**类型安全的消息通道**通信（`Task`、`Result`、`StatusUpdate`、`Broadcast`、`Error`）
- **`SharedBlackboard`** 实现跨 Agent 状态共享
- 内置 **`DebateModerator`** 用于结构化多 Agent 辩论
- **`VotingAgent`** 支持民主投票达成提案共识
- **`TaskGraph` 模板** — 可复用的多步骤工作流定义

---

## 🧩 记忆系统

Crablet 实现了一套**三层分层记忆架构**，与人类组织知识的方式高度对应：

```
┌────────────────────────────────────────────────────────────┐
│                       记忆层次结构                           │
│                                                             │
│  ┌──────────────┐  ┌───────────────┐  ┌─────────────────┐ │
│  │   工作记忆   │  │   情节记忆    │  │    语义记忆     │ │
│  │  Working     │  │   Episodic    │  │    Semantic     │ │
│  │              │  │               │  │                 │ │
│  │ VecDeque     │  │   SQLite      │  │  Neo4j / SQLite │ │
│  │ + Tiktoken   │  │  WAL 模式     │  │   知识图谱      │ │
│  │ Token 预算   │  │   会话管理    │  │  + D3 可视化    │ │
│  │   O(1)       │  │  + 消息存储   │  │                 │ │
│  └──────┬───────┘  └───────┬───────┘  └────────┬────────┘ │
│         │                  │                   │           │
│         └──────────────────▼───────────────────┘           │
│                      记忆固化器                              │
│              LLM 驱动的后台摘要循环                          │
│         触发条件：每 20 条消息 或 每 1 小时 TTL              │
│         输出：长期向量嵌入存入向量数据库                     │
└────────────────────────────────────────────────────────────┘
```

### 工作记忆（Working Memory）

- **Token 预算上下文窗口** — 使用 `tiktoken-rs`（cl100k_base 分词器）精确追踪 Token 数量
- **智能压缩** — 保留系统消息 + 最近 N 轮对话；超出预算时丢弃最旧消息
- **TTL 过期** — 空闲会话自动驱逐（`is_expired(Duration)`）
- **固化器钩子** — 与 `MemoryConsolidator` 集成，无缝将短期记忆升级为长期记忆

### 情节记忆（Episodic Memory）

- **SQLite 持久化** — WAL 日志模式，64MB 缓存，针对高吞吐写入优化的 PRAGMA 配置
- **事务写入** — `save_message_transactional` 保证并发会话间的一致性
- **会话管理** — UUID 会话 ID，按渠道分类追踪
- **按时间序回溯** — `get_history(session_id, limit)` 按对话顺序返回消息

### 语义记忆（Knowledge Graph）

- **双后端** — `SqliteKnowledgeGraph`（零外部依赖）或 `Neo4jKnowledgeGraph`（企业级规模）
- **实体 + 关系建模** — `add_entity`、`add_relation`、`find_related`，支持有向图遍历
- **D3.js 导出** — `export_d3_json()` 生成 Web UI 图谱可视化数据
- **批量实体查询** — `find_entities_batch` 支持高效批量图谱检索

### 记忆固化器（Memory Consolidator）

后台持久运行的 **`MemoryConsolidator`** Tokio 任务：
1. 每会话每 **20 条消息**或**每 1 小时**触发一次
2. 获取最近 50 条消息，调用 LLM 生成摘要
3. 将摘要以带时间戳的嵌入向量存入**向量存储**（类型标记为 `"conversation_summary"`）
4. 为记忆条目打上 `importance`（重要性）和 `access_count`（访问次数）标签，用于未来的衰减/检索评分

---

## 📚 GraphRAG 知识引擎

Crablet 的 RAG 系统远超普通向量相似度检索——它将稠密向量检索与结构化知识图谱推理深度融合：

```
查询输入
    │
    ▼
向量检索（fastembed / Qdrant）
    │           ↕  余弦相似度
    ▼
实体抽取（Rule 规则 + Phrase 短语 + Hybrid 混合模式）
    │
    ▼
知识图谱遍历（Neo4j / SQLite）
    │  → 查找相关实体及关系
    ▼
图增强重排序
    │  最终得分 = 向量得分 × 0.7 + 图谱增益 × 0.3
    │  图谱增益 = 实体覆盖 × 0.4 + 关系权重 × 0.25
    │           + 中心度 × 0.15 + 图信号 × 0.2
    ▼
检索结果（文档 + 知识图谱关系注入）
```

### RAG 核心特性

| 特性 | 实现方式 |
|---|---|
| **嵌入后端** | `fastembed`（AllMiniLM-L6-v2，384 维，本地运行）或 Qdrant 云端 |
| **向量存储** | SQLite（默认）、Qdrant（需 `qdrant-support` feature）、内存（测试用） |
| **分块策略** | `RecursiveCharacterChunker`（500 字符，50 重叠）+ `MarkdownChunker`（标题感知） |
| **实体抽取** | `Rule`（分词）、`Phrase`（二元组窗口）、`Hybrid`（两者结合） |
| **图谱后端** | `SqliteKnowledgeGraph` 或 `Neo4jKnowledgeGraph`（feature 门控） |
| **中心度评分** | 归一化出入度中心度，用于实体相关性排序 |
| **文档类型** | 文本、Markdown（结构感知）、PDF（`pdf-extract`）、多模态 |
| **嵌入器池** | 2 个并发 fastembed worker（Tokio blocking 任务） |
| **重排序** | 图信号加权余弦重排序，在最终 top-k 筛选前执行 |

### 支持的文档格式

```bash
crablet knowledge extract --file document.pdf      # PDF 文档摄入
crablet knowledge extract --file notes.md          # Markdown 结构感知分块
crablet knowledge extract --file code.rs           # 源代码索引
crablet knowledge query "Rust 异步编程模式"         # 语义搜索
```

---

## 🎨 Canvas — 实时产物渲染

Canvas 是 Crablet 的**实时结构化输出渲染系统**。System 2 不再返回纯文本，而是自动检测并发布丰富的结构化产物到交互式画布，用户可实时查看、编辑和导出。

### Canvas 组件类型

```rust
enum CanvasComponent {
    Markdown  { content: String },
    Code      { language: String, content: String, filename: Option<String> },
    Mermaid   { chart: String },       // 自动渲染流程图/时序图
    DataTable { headers, rows, title }, // 结构化表格数据
    Html      { content: String },      // HTML 实时预览（UI 原型）
}
```

### Canvas 工作原理

1. **自动检测** — `detect_and_publish_canvas()` 扫描每条 LLM 响应中的结构化代码块
2. **产物路由** — 检测到的产物以 `AgentEvent::CanvasUpdate` 事件发布到 `EventBus`
3. **会话级状态** — `CanvasManager` 维护每个会话的 `CanvasState`，包含有序的 Section 列表
4. **实时流式推送** — 产物通过 **SSE（Server-Sent Events）**流式推送到前端
5. **CRUD 操作** — 支持 `add_component`、`update_component`、`remove_component` 交互编辑

### 支持的产物触发规则

| 触发条件 | 产物类型 | 示例 |
|---|---|---|
| ` ```mermaid ` | 流程图 | 架构图、时序图 |
| ` ```html ` + `<div/html/body>` | HTML 预览 | UI 原型、数据看板 |
| ` ```rust/python/ts ` (>5 行) | 代码片段 | 生成的代码、脚本 |
| `DataTable` JSON | 数据表格 | 查询结果、对比分析 |
| Markdown 段落 | 富文本 | 报告、技术文档 |

---

## 🤖 多 Agent Swarm

**Swarm** 系统基于 Rust 所有权模型，保证无数据竞争的并发执行，实现真正意义上的多 Agent 协作：

### 预置 Agent 角色

| Agent | 源文件 | 专业方向 |
|---|---|---|
| `Researcher` | `agent/researcher.rs` | 网络搜索、信息采集 |
| `Coder` | `agent/coder.rs` | 代码生成与审查 |
| `Analyst` | `agent/analyst.rs` | 数据分析、模式识别 |
| `Reviewer` | `agent/reviewer.rs` | 质量保证、批评性审查 |
| `Planner` | `agent/planning.rs` | 任务分解、目标树规划 |
| `SecurityAgent` | `agent/security.rs` | 漏洞扫描、安全审计 |
| `VotingAgent` | `agent/voting.rs` | 民主投票达成提案共识 |
| `DebateModerator` | `agent/debate.rs` | 结构化多轮辩论主持 |
| `HITL` | `agent/hitl.rs` | 人机协同审批门控 |

### Swarm 通信模型

```rust
// 类型化消息协议 —— 编译期安全保证
enum SwarmMessage {
    Task         { task_id, description, context, payload },
    Result       { task_id, content, payload },
    StatusUpdate { task_id, status },
    Broadcast    { topic, content },     // 通过主题注册表实现发布/订阅
    Error        { task_id, error },
}
```

- **发布/订阅主题** — Agent 订阅命名主题；`Swarm::publish(topic, msg)` 扇出广播
- **超时保护** — 每个 Agent 处理步骤设有 **30 秒硬性超时**
- **黑板模式** — `SharedBlackboard`（DashMap）实现无消息传递的共享状态
- **持久化** — `SwarmPersister` 将任务图和结果保存到 SQLite，支持崩溃恢复

---

## 🗂️ 项目结构

```
crablet/
├── src/
│   ├── main.rs                       # 程序入口 + CLI 命令分发
│   ├── cognitive/                    # ★ 认知核心
│   │   ├── router.rs                 # 元路由器：分类并分发到 S1/S2/S3
│   │   ├── system1.rs                # 直觉层（Trie + Levenshtein）
│   │   ├── system2/
│   │   │   ├── mod.rs                # 深度推理编排
│   │   │   ├── canvas.rs             # Canvas 产物自动检测
│   │   │   ├── multimodal.rs         # 视觉 + 音频集成
│   │   │   └── post_process.rs       # 响应后处理
│   │   ├── system3.rs                # Swarm 路由
│   │   ├── react.rs                  # ReAct 思考→行动→观察 循环
│   │   ├── tot.rs                    # 思维树（Tree of Thoughts）
│   │   ├── mcts_tot.rs               # MCTS + UCB1 思维探索
│   │   ├── streaming_pipeline.rs     # SSE 流式输出
│   │   ├── llm/
│   │   │   ├── mod.rs                # LlmClient trait + OpenAI/Ollama/DashScope/Kimi/智谱
│   │   │   └── cache.rs              # 语义响应缓存（Moka LRU）
│   │   ├── middleware/
│   │   │   ├── safety.rs             # 安全检查注入
│   │   │   ├── cost_guard.rs         # Token 预算守卫
│   │   │   ├── semantic_cache.rs     # 查询去重
│   │   │   ├── planning.rs           # 目标分解
│   │   │   ├── rag.rs                # RAG 上下文注入
│   │   │   └── skill_context.rs      # 技能元数据注入
│   │   └── multimodal/               # 图像 + 音频处理
│   ├── memory/                       # ★ 记忆系统
│   │   ├── working.rs                # Token 预算内存上下文
│   │   ├── episodic.rs               # SQLite 会话与消息持久化
│   │   ├── semantic.rs               # 知识图谱（SQLite + Neo4j）
│   │   ├── consolidator.rs           # 后台 LLM 摘要固化循环
│   │   ├── manager.rs                # 统一记忆访问接口
│   │   └── shared.rs                 # Swarm Agent 共享黑板
│   ├── knowledge/                    # ★ RAG 引擎
│   │   ├── vector_store.rs           # fastembed + SQLite/Qdrant 向量存储
│   │   ├── graph_rag.rs              # GraphRAG：向量 + 知识图谱混合检索
│   │   ├── graph.rs                  # 知识图谱 CRUD
│   │   ├── chunking.rs               # 递归 + Markdown 分块器
│   │   ├── reranking.rs              # 图信号重排序
│   │   ├── extractor.rs              # 文档文本提取
│   │   ├── pdf.rs                    # PDF 解析
│   │   ├── ingestion.rs              # 批量文档摄入管道
│   │   └── multimodal.rs             # 图像/音频知识提取
│   ├── gateway/                      # ★ API 网关（Axum）
│   │   ├── server.rs                 # HTTP + WebSocket 服务器配置
│   │   ├── websocket.rs              # 实时 WebSocket 处理器
│   │   ├── canvas.rs                 # Canvas 组件类型定义
│   │   ├── canvas_manager.rs         # 会话级 Canvas 状态管理
│   │   ├── rpc.rs                    # JSON-RPC 2.0 分发器
│   │   ├── session.rs                # 会话生命周期管理
│   │   ├── ratelimit.rs              # 令牌桶限流（governor）
│   │   └── events.rs                 # SSE 事件流
│   ├── agent/                        # ★ Agent 角色
│   │   ├── swarm.rs                  # Swarm 编排器 + 通道网格
│   │   ├── coordinator.rs            # 多 Agent 任务协调器
│   │   ├── factory.rs                # Agent 实例化注册表
│   │   ├── voting.rs                 # 共识投票机制
│   │   ├── debate.rs                 # 多轮辩论主持人
│   │   ├── hitl.rs                   # 人机协同审批
│   │   ├── researcher.rs             # 研究专家
│   │   ├── coder.rs                  # 代码生成专家
│   │   ├── analyst.rs                # 数据分析专家
│   │   ├── reviewer.rs               # 审查与批评专家
│   │   └── planning.rs               # 规划与分解
│   ├── tools/                        # 工具实现
│   │   ├── bash.rs                   # Shell 执行（SafetyOracle 过滤）
│   │   ├── file.rs                   # 文件读/写/列举
│   │   ├── search.rs                 # 网络搜索（Serper / DuckDuckGo）
│   │   ├── http.rs                   # HTTP 客户端工具
│   │   ├── vision.rs                 # 图像分析
│   │   ├── browser.rs                # Chromium 浏览器自动化
│   │   └── mcp.rs                    # MCP 协议工具桥接
│   ├── skills/                       # 插件/技能管理
│   │   ├── registry.rs               # 技能发现与注册
│   │   ├── executor.rs               # YAML/Python/Node.js 技能运行器
│   │   ├── installer.rs              # Git 技能安装器
│   │   ├── openclaw.rs               # SKILL.md Prompt 驱动技能
│   │   └── watcher.rs                # 技能文件热重载监听
│   ├── channels/                     # 输入渠道适配器
│   │   ├── cli/                      # 交互式 CLI + 子命令
│   │   ├── domestic/
│   │   │   ├── dingtalk.rs           # 钉钉集成
│   │   │   └── feishu.rs             # 飞书/Lark 集成
│   │   ├── international/
│   │   │   └── telegram.rs           # Telegram Bot
│   │   ├── discord.rs                # Discord 网关
│   │   └── universal/
│   │       └── http_webhook.rs       # 通用 HTTP Webhook
│   ├── auth/                         # 认证与授权
│   │   ├── oidc.rs                   # OpenID Connect / OAuth2 流程
│   │   ├── handlers.rs               # 登录/回调 HTTP 处理器
│   │   └── middleware.rs             # JWT 验证中间件
│   ├── safety/                       # 安全层
│   │   ├── oracle.rs                 # 命令白名单/黑名单
│   │   └── mod.rs                    # 安全级别配置（严格/宽松/禁用）
│   ├── sandbox/                      # 执行沙箱
│   │   ├── docker.rs                 # Docker 容器隔离
│   │   └── local.rs                  # 进程级沙箱
│   ├── scripting/                    # Lua 5.4 脚本引擎
│   │   ├── engine.rs                 # mlua 运行时集成
│   │   └── bindings.rs               # Rust → Lua API 绑定
│   ├── audit/                        # 安全审计 Agent
│   ├── protocols/
│   │   └── a2a.rs                    # Agent 间通信协议
│   └── telemetry.rs                  # OpenTelemetry 链路追踪 + 指标
├── frontend/                         # Web UI（TypeScript + React + TailwindCSS）
├── skills/                           # 内置技能定义（SKILL.md）
│   ├── create_skills/
│   ├── find_skills/
│   ├── proactive_agent/
│   └── safe_run/
├── mcp_servers/                      # 示例 MCP 服务器脚本
│   ├── math_server.py
│   └── mcp_test_server.py
├── migrations/                       # SQLite 数据库迁移文件
├── config/config.toml                # 默认配置文件
├── templates/index.html              # Web UI HTML 模板
├── tests/                            # 集成测试与单元测试（25+ 个测试文件）
├── Cargo.toml
├── Dockerfile
├── docker-compose.yml
└── Justfile
```

---

## ⚙️ LLM 提供商支持

Crablet 通过统一的 `LlmClient` trait 支持多种 LLM 后端：

| 提供商 | 认证方式 | 流式输出 | 工具调用 | 备注 |
|---|---|---|---|---|
| **OpenAI** | `OPENAI_API_KEY` | ✅ SSE | ✅ Function Calling | gpt-4o、gpt-4o-mini 等 |
| **DashScope**（通义千问） | `DASHSCOPE_API_KEY` | ✅ | ✅ | OpenAI 兼容端点 |
| **Ollama** | 本地部署 | ❌ | ✅ | 默认 qwen2.5:14b |
| **Kimi**（月之暗面） | `MOONSHOT_API_KEY` | ✅ | ✅ | 长上下文专项 |
| **智谱 GLM** | `ZHIPU_API_KEY` | ✅ | ✅ | 中文优化 |
| **任意 OpenAI 兼容** | `OPENAI_API_BASE` | ✅ | ✅ | 自定义 Base URL |

---

## 🚀 快速开始

### 前置依赖

- **Rust 1.80+** — [通过 rustup 安装](https://rustup.rs/)
- **Docker**（可选——用于 Neo4j 和沙箱执行）
- **Git**

### 方式一：本地构建

```bash
# 1. 克隆仓库
git clone https://github.com/isLinXu/crablet.git
cd crablet

# 2. 快速构建——仅 CLI + Web（约 5 分钟）
cargo build --release --no-default-features --features web

# 3. 完整构建——所有 feature（约 15-20 分钟）
cargo build --release

# 4. 初始化配置和数据库
./target/release/crablet init

# 5. 设置 LLM API Key
export OPENAI_API_KEY=sk-xxx
# 或使用 DashScope/通义千问：
export DASHSCOPE_API_KEY=sk-xxx
export OPENAI_API_BASE=https://dashscope.aliyuncs.com/compatible-mode/v1

# 6. 开始对话
./target/release/crablet chat
```

> 💡 **提速技巧**：安装 [sccache](https://github.com/mozilla/sccache) 缓存跨构建的 Rust 编译产物：
> ```bash
> cargo install sccache
> export RUSTC_WRAPPER=sccache
> ```

### 方式二：Docker Compose（推荐，完整技术栈）

```bash
# 设置 API Key
export OPENAI_API_KEY=sk-xxx

# 一键启动 Crablet + Neo4j
docker-compose up -d

# 打开 Web UI
open http://localhost:3000
```

### 方式三：Docker 单容器

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

## 💬 CLI 命令参考

```bash
# 多轮交互式对话
crablet chat

# 单次任务执行
crablet run "分析这段 Rust 代码的性能瓶颈"

# 启动 Web UI 服务器
crablet serve-web --port 3000

# 启动 WebSocket + JSON-RPC 网关
crablet gateway --port 18789

# 视觉/多模态
crablet vision --image ./screenshot.png --query "描述这张架构图"

# 音频转录
crablet audio --file meeting.wav

# 技能管理
crablet skill list
crablet skill install https://github.com/user/my-skill.git
crablet skill create my-new-skill

# 知识库管理
crablet knowledge extract --file document.pdf
crablet knowledge extract --file notes.md
crablet knowledge query "GraphRAG 是如何工作的？"

# 安全审计
crablet audit .
crablet audit ./src --format json    # CI/CD 友好的 JSON 格式输出

# 系统状态
crablet status

# Lua 脚本执行
crablet script run examples/scripts/summarize_paper.lua

# 调试/内省
crablet debug --show-memory
crablet debug --show-graph
```

---

## 🔌 插件与技能生态

Crablet 提供**四种互补的扩展机制**：

### 1. `skill.yaml` — 跨语言技能（Python / Node.js / Shell）

```yaml
name: weather
description: 使用 OpenMeteo API 获取城市当前天气
version: 1.0.0
parameters:
  type: object
  properties:
    city:
      type: string
      description: 要查询天气的城市名
  required: [city]
entrypoint: python3 weather.py
timeout: 10
env:
  API_KEY: ${OPENMETEO_API_KEY}
```

### 2. `SKILL.md` — Prompt 驱动技能（兼容 OpenClaw）

```markdown
---
name: python-expert
description: Python 编程专家助手
version: 1.0.0
---

You are a Python expert. Always use type hints and docstrings.
When writing code, follow PEP 8 conventions.
```

### 3. MCP 协议 — Model Context Protocol

```toml
# config.toml
[mcp_servers]
math_server  = { command = "python3", args = ["mcp_servers/math_server.py"] }
custom_tools = { command = "node",    args = ["./my-mcp-server/index.js"] }
```

### 4. 内置工具库

| 工具名 | 功能描述 | 安全机制 |
|---|---|---|
| `bash` | Shell 命令执行 | SafetyOracle 白名单 |
| `file` | 文件读/写/列举 | 路径沙箱限制 |
| `web_search` | 网络搜索（Serper / DuckDuckGo） | 安全 |
| `http` | 任意 HTTP 请求 | 安全 |
| `vision` | 图像理解（多模态 LLM） | 安全 |
| `browser` | Chromium 浏览器自动化 | Docker 沙箱 |
| `calculator` | 数学表达式求值 | 安全 |
| `weather` | 天气 API 查询 | 安全 |

---

## ⚙️ 配置参考

**配置文件路径**：`~/.config/crablet/config.toml`

```toml
# 数据库
database_url = "sqlite:crablet.db?mode=rwc"

# LLM 设置
model_name  = "gpt-4o-mini"
max_tokens  = 4096
temperature = 0.7

# 日志
log_level = "info"   # trace | debug | info | warn | error

# 安全配置
[safety]
level             = "Strict"   # Strict（严格）| Permissive（宽松）| Disabled（禁用）
allowed_commands  = ["ls", "cat", "echo", "pwd"]
blocked_commands  = ["rm", "mv", "sudo", "chmod"]

# MCP 服务器
[mcp_servers]
math_server = { command = "python3", args = ["mcp_servers/math_server.py"] }

# GraphRAG 实体抽取模式
# GRAPH_RAG_ENTITY_MODE = "rule" | "phrase" | "hybrid"（默认）

# 并发限制
[limits]
max_concurrent_requests = 100
request_timeout         = 30

# OIDC / OAuth2（可选）
oidc_issuer        = "https://your-tenant.auth0.com/"
oidc_client_id     = "your-client-id"
oidc_client_secret = "your-client-secret"
jwt_secret         = "your-app-secret"

# 可观测性
[telemetry]
enabled  = true
endpoint = "http://tempo:4317"
```

### 环境变量

| 变量名 | 说明 |
|---|---|
| `OPENAI_API_KEY` | OpenAI API 密钥 |
| `DASHSCOPE_API_KEY` | 阿里 DashScope（通义千问）API 密钥 |
| `OPENAI_API_BASE` | 自定义 OpenAI 兼容 Base URL |
| `OLLAMA_MODEL` | 本地 Ollama 模型名（默认：`qwen2.5:14b`） |
| `OLLAMA_API_BASE` | Ollama 服务器地址（默认：`http://localhost:11434`） |
| `MOONSHOT_API_KEY` | Kimi（月之暗面）API 密钥 |
| `ZHIPU_API_KEY` | 智谱 GLM API 密钥 |
| `SERPER_API_KEY` | Serper 网络搜索 API 密钥 |
| `DATABASE_URL` | SQLite 或 PostgreSQL 连接字符串 |
| `QDRANT_URL` | Qdrant 向量数据库地址 |
| `RUST_LOG` | 日志级别（`info`、`debug`、`trace`） |
| `GRAPH_RAG_ENTITY_MODE` | 实体抽取模式：`rule` / `phrase` / `hybrid` |

### Feature Flags

```bash
cargo build --release --no-default-features --features <flags>
```

| Feature Flag | 说明 | 默认 |
|---|---|---|
| `web` | Axum HTTP 服务器 + Web UI + REST API | ✅ |
| `qdrant-support` | Qdrant 向量数据库后端 | ✅ |
| `knowledge` | 完整 RAG：fastembed + PDF + Neo4j + Qdrant | ✅（含 `knowledge`） |
| `audio` | Whisper 语音转文字 | ❌ |
| `scripting` | Lua 5.4 脚本引擎（mlua） | ❌ |
| `telemetry` | OpenTelemetry 分布式追踪 | ❌ |
| `sandbox` | Docker 容器隔离（bollard） | ❌ |
| `telegram` | Telegram Bot 集成 | ❌ |
| `discord` | Discord 网关集成 | ❌ |
| `browser` | Chromium 浏览器自动化 | ❌ |
| `inference` | ONNX Runtime（本地模型推理） | ❌ |
| `full` | 启用全部 Feature | ❌ |

---

## 🌐 API 参考

### WebSocket 网关

**端点地址**：`ws://localhost:18789/ws`

**发送消息：**
```json
{
  "type": "UserInput",
  "content": "写一个异步读取文件的 Rust 函数"
}
```

**接收流式事件：**

| 事件类型 | 说明 |
|---|---|
| `ThoughtGenerated` | ReAct 推理步骤（System 2 思考过程） |
| `ToolExecutionStarted` | 工具调用已发起 |
| `ToolExecutionFinished` | 工具执行结果已返回 |
| `ResponseGenerated` | Agent 最终响应 |
| `CanvasUpdate` | 新 Canvas 产物（类型：mermaid / code / html / markdown） |
| `SwarmActivity` | Swarm 内 Agent 间消息 |
| `MemoryConsolidated` | 后台记忆固化完成 |

### REST API

| 方法 | 端点 | 说明 |
|---|---|---|
| `POST` | `/api/chat` | 单轮对话补全 |
| `GET` | `/api/status` | 系统健康状态与统计 |
| `GET` | `/api/dashboard` | 监控看板数据 |
| `GET` | `/api/knowledge` | 列出知识库文档 |
| `DELETE` | `/api/knowledge?source=...` | 删除知识库文档 |
| `GET` | `/api/canvas/:session_id` | 获取会话 Canvas 状态 |
| `GET` | `/auth/login` | 发起 OIDC 登录流程 |
| `GET` | `/auth/callback` | OIDC 回调处理器 |
| `GET` | `/api/me` | 当前已认证用户信息 |

---

## 🚢 部署

### Docker Compose（完整技术栈，含 Neo4j）

```yaml
version: '3.8'

services:
  crablet:
    image: crablet:latest
    ports:
      - "3000:3000"      # Web UI
      - "18789:18789"    # WebSocket 网关
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
      - "7687:7687"      # Bolt 协议
    environment:
      - NEO4J_AUTH=neo4j/password
    volumes:
      - neo4j_data:/data

volumes:
  neo4j_data:
```

### 生产环境配置

```toml
# config.toml（生产环境）
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

### 可观测性技术栈

Crablet 导出 **OpenTelemetry** 遥测数据，完全兼容 CNCF 可观测性全套工具：

| 组件 | 集成方案 |
|---|---|
| **分布式追踪** | Jaeger / Grafana Tempo |
| **指标监控** | Prometheus（`crablet.request.duration`、`crablet.llm.tokens`） |
| **可视化看板** | Grafana |
| **日志聚合** | Loki / 任意 OTLP 兼容后端 |

---

## 🧪 测试

项目包含 **25+ 个测试文件**，覆盖以下场景：

```bash
# 单元测试和集成测试
cargo test

# 指定测试套件
cargo test memory_test
cargo test vector_store_integration_test
cargo test react_chain_test
cargo test system1_verify
cargo test safety_test
cargo test graph_rag_returns_augmented_context

# 端到端测试
cargo test e2e_full
cargo test e2e_auth_audit

# 演示测试（运行特定 Agent 场景）
cargo test demo_debate
cargo test demo_voting
cargo test demo_rag
```

---

## 📚 文档

| 文档 | 说明 |
|---|---|
| [快速开始](https://github.com/isLinXu/crablet/blob/main/docs/getting-started.md) | 安装、首次运行与基础使用 |
| [架构设计](https://github.com/isLinXu/crablet/blob/main/docs/architecture.md) | 认知架构深度解析 |
| [API 参考](https://github.com/isLinXu/crablet/blob/main/docs/api-reference.md) | CLI、REST API、WebSocket、JSON-RPC |
| [配置指南](https://github.com/isLinXu/crablet/blob/main/docs/configuration.md) | 所有配置项与环境变量说明 |
| [部署指南](https://github.com/isLinXu/crablet/blob/main/docs/deployment.md) | Docker、生产环境配置与可观测性 |
| [技能开发](https://github.com/isLinXu/crablet/blob/main/docs/skill-development.md) | 编写自定义技能与 MCP 服务器 |
| [贡献指南](https://github.com/isLinXu/crablet/blob/main/docs/contributing.md) | 如何参与项目贡献 |
| [路线图](https://github.com/isLinXu/crablet/blob/main/docs/roadmap.md) | 未来规划与里程碑 |

---

## 🗺️ 路线图

### ✅ 已实现

- [x] 三层认知架构（System 1 / 2 / 3）
- [x] 分层记忆：工作记忆 → 情节记忆 → 语义记忆 + LLM 自动固化
- [x] GraphRAG：向量存储 + 知识图谱混合检索
- [x] Canvas：实时产物渲染（Mermaid、代码、HTML、DataTable）
- [x] 多 Agent Swarm（VotingAgent、DebateModerator、HITL）
- [x] 基于 MCTS 的思维树推理
- [x] OIDC/OAuth2 认证（Auth0、Google、Keycloak）
- [x] REST + WebSocket + JSON-RPC 网关
- [x] 插件生态（YAML 技能、SKILL.md、MCP 协议）
- [x] 安全审计 Agent
- [x] 多渠道：CLI、Telegram、钉钉、飞书、Discord、HTTP Webhook
- [x] OpenTelemetry 链路追踪 + Prometheus 指标
- [x] Docker + Docker Compose 部署

### 🔄 近期计划（1–3 个月）

- [ ] **高性能网关重写** — Axum 原生实现，吞吐量提升 3-5x
- [ ] **20+ 消息渠道** — 企业微信、Slack、WhatsApp、Microsoft Teams、QQ
- [ ] **Agent 协调器 V2** — 动态角色分配、上下文隔离、子 Agent 动态生成
- [ ] **Canvas 编辑器** — 集成 Monaco 代码编辑器 + Mermaid 实时预览
- [ ] **审批工作流** — `crablet approve <code>` 人机协同 API
- [ ] **一键安装脚本** — `curl -fsSL https://crablet.dev/install.sh | sh`

### 🔮 中期计划（3–6 个月）

- [ ] **多租户** — RBAC、组织隔离、SSO（LDAP/OAuth2）、GDPR 合规
- [ ] **Function Calling V2** — 并行工具执行（`ParallelToolExecution`）
- [ ] **长上下文管理** — 128K+ Token 压缩 + 摘要
- [ ] **程序性记忆** — 从成功任务执行中学习技能
- [ ] **Cron 调度器** — 基于时间和事件触发的任务自动化
- [ ] **成本分析看板** — 按模型、按用户统计 Token 使用成本

### 🌟 长期计划（6–12 个月）

- [ ] **Crablet Skill Store** — `crablet skill install/search/publish` 技能市场
- [ ] **Crablet Cloud** — 多租户 SaaS 云端托管服务（支持自动扩缩容）

---

## 🤝 贡献

欢迎任何形式的贡献！请先阅读 [贡献指南](https://github.com/isLinXu/crablet/blob/main/docs/contributing.md)。

```bash
# Fork 并克隆
git clone https://github.com/your-username/crablet.git
cd crablet

# 创建功能分支
git checkout -b feature/amazing-feature

# 修改代码，添加测试
cargo test

# 使用约定式提交格式
git commit -m "feat(memory): add memory decay scoring for old summaries"

# Push 并发起 PR
git push origin feature/amazing-feature
```

### 开发规范

- **Rust 风格**：提交前执行 `cargo fmt` + `cargo clippy`
- **测试必须**：所有新特性必须包含集成测试
- **Feature 门控**：新增可选依赖必须通过 feature flag 门控
- **安全优先**：所有新工具必须经过 `SafetyOracle` 检查
- **禁止阻塞**：所有 I/O 操作必须使用 `tokio::spawn_blocking` 或异步 API

---

## 📄 许可证

本项目基于 **MIT 许可证** 开源——详情请参阅 [LICENSE](LICENSE)。

---

<div align="center">

**如果 Crablet 对你有所帮助，请给个 ⭐ Star 支持一下！**

<br>

用 🦀 **Rust** 和 ❤️ 由 [isLinXu](https://github.com/isLinXu) 及贡献者们共同构建

<br>

*Crablet——好的 AI 基础设施应该像螃蟹的壳一样坚不可摧，*
*又像它横行时一样迅如闪电。*

</div>
