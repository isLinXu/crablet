# Crablet Fusion 架构设计文档

> **版本**: 2.0.0  
> **日期**: 2026-03-15  
> **状态**: 已发布

---

## 目录

1. [架构概述](#架构概述)
2. [设计原则](#设计原则)
3. [四层记忆系统](#四层记忆系统)
4. [组件详解](#组件详解)
5. [数据流](#数据流)
6. [扩展性设计](#扩展性设计)
7. [性能优化](#性能优化)
8. [安全设计](#安全设计)

---

## 架构概述

### 系统全景

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Application Layer                               │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Cognitive Systems                               │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────────┐   │   │
│  │  │ System 1 │  │ System 2 │  │ System 3 │  │  Fusion Router   │   │   │
│  │  │  (Fast)  │  │ (Analytical)│ │ (Meta)  │  │   (Unified)      │   │   │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────────────┘   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────────────────────┤
│                              Integration Layer                               │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                        Fusion Adapter                                │   │
│  │              (Bridge between old and new systems)                    │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────────────────────┤
│                           Fusion Memory System                               │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  L4: SOUL Layer          ←  AGENTS.md, SOUL.md                      │   │
│  │  ├─ Identity (Name, Role, Description)                              │   │
│  │  ├─ Core Values (Prioritized principles)                            │   │
│  │  ├─ Immutable Rules (Hard constraints)                              │   │
│  │  └─ Guidelines (Behavioral patterns)                                │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    ↓                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  L3: TOOLS Layer         ←  TOOLS.md, skills/                       │   │
│  │  ├─ Tool Registry (Dynamic registration)                            │   │
│  │  ├─ Permission System (Access control)                              │   │
│  │  ├─ Tool Chains (Orchestrated execution)                            │   │
│  │  └─ Hot Reload (Runtime updates)                                    │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    ↓                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  L2: USER Layer          ←  USER.md, MEMORY.md                      │   │
│  │  ├─ User Profile (Preferences, Facts, Goals)                        │   │
│  │  ├─ Semantic Memory (Vector + Graph storage)                        │   │
│  │  ├─ Memory Index (Fast retrieval)                                   │   │
│  │  └─ Consolidation (Deduplication, merging)                          │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    ↓                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  L1: Session Layer       ←  sessions.json                           │   │
│  │  ├─ Message Queue (Conversation history)                            │   │
│  │  ├─ Token Management (Context window)                               │   │
│  │  ├─ Context Compression (Light/Moderate/Deep)                       │   │
│  │  └─ Temp State (Short-lived data)                                   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    ↓                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Daily Logs              ←  memory/*.md                             │   │
│  │  ├─ Append-only Events (Immutable history)                          │   │
│  │  ├─ Session Summaries (Daily aggregation)                           │   │
│  │  ├─ Cross-session Context (Continuity)                              │   │
│  │  └─ Markdown Format (Human-readable)                                │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    ↓                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Memory Weaver                                                      │   │
│  │  ├─ Extraction Patterns (NLP-based)                                 │   │
│  │  ├─ Memory Consolidation (Similarity detection)                     │   │
│  │  ├─ Connection Building (Related memories)                          │   │
│  │  └─ Storage Optimization (Indexing, archiving)                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────────────────────┤
│                              Storage Layer                                   │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │
│  │   Markdown   │  │    JSON      │  │    SQLite    │  │    Vector    │   │
│  │   Files      │  │   Storage    │  │   Database   │  │    Store     │   │
│  └──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 核心特性

| 特性 | 描述 | 优势 |
|------|------|------|
| **四层架构** | SOUL/TOOLS/USER/Session 分层 | 职责清晰，易于维护 |
| **配置驱动** | Markdown 文件定义行为 | 人类可读，版本控制友好 |
| **双向增强** | Crablet 功能 + OpenClaw 配置 | 1+1 > 2 |
| **渐进迁移** | 支持四种迁移模式 | 风险可控 |
| **异步设计** | 全异步 I/O | 高性能 |

---

## 设计原则

### 1. 分层隔离

```
L4 (SOUL)    →  不可变，启动时加载
L3 (TOOLS)   →  动态，运行时更新
L2 (USER)    →  持久化，持续学习
L1 (Session) →  临时，会话级生命周期
```

### 2. 配置优先

- 行为通过 Markdown 文件定义
- 代码只实现机制，不实现策略
- 支持热重载，无需重启

### 3. 向后兼容

- 保留所有现有 API
- 适配器模式桥接新旧系统
- 支持平滑迁移

### 4. 可观测性

- 详细的 tracing 日志
- Daily Logs 提供审计追踪
- 性能指标收集

---

## 四层记忆系统

### L4: SOUL Layer (不可变内核)

**职责**: 定义 Agent 的本质身份和不可变原则

**数据来源**:
- `AGENTS.md`: Agent 定义
- `SOUL.md`: 灵魂/人格指令

**核心结构**:
```rust
pub struct SoulLayer {
    identity: AgentIdentity,        // 名称、描述、角色
    core_values: Vec<CoreValue>,    // 核心价值观（带优先级）
    immutable_rules: Vec<ImmutableRule>, // 不可变规则
    guidelines: Vec<Guideline>,     // 行为准则
}
```

**使用场景**:
- 生成系统提示词
- 运行时规则检查
- 价值观冲突解决

**不变性保证**:
- 启动时加载
- 运行时只读
- 修改需重启

### L3: TOOLS Layer (动态工具)

**职责**: 管理可用的工具和技能

**数据来源**:
- `TOOLS.md`: 工具定义
- `skills/`: 本地技能目录

**核心结构**:
```rust
pub struct ToolsLayer {
    registry: DashMap<String, Arc<dyn Tool>>,  // 工具注册表
    permissions: RwLock<ToolPermissions>,       // 权限设置
    chains: DashMap<String, ToolChain>,         // 工具链
}
```

**特性**:
- 热加载/热卸载
- 权限管理（5个级别）
- 工具链编排
- 使用统计

### L2: USER Layer (语义长期记忆)

**职责**: 存储关于用户的长期知识

**数据来源**:
- `USER.md`: 用户画像
- `MEMORY.md`: 记忆存储
- 运行时学习

**核心结构**:
```rust
pub struct UserLayer {
    profile: RwLock<UserProfileData>,    // 用户画像
    memories: RwLock<Vec<Memory>>,       // 语义记忆
    memory_index: RwLock<HashMap<String, Vec<usize>>>, // 分类索引
}
```

**记忆类型**:
- ExplicitFact: 明确陈述的事实
- Inferred: 推断的信息
- Preference: 用户偏好
- Decision: 历史决策
- Goal: 用户目标

### L1: Session Layer (实时情景)

**职责**: 管理当前对话的实时上下文

**数据来源**:
- `sessions.json`: 会话存储
- 运行时消息

**核心结构**:
```rust
pub struct SessionLayer {
    messages: RwLock<Vec<Message>>,      // 消息队列
    token_usage: RwLock<TokenUsage>,     // Token 使用跟踪
    max_tokens: usize,                   // 最大 Token 限制
    temp_state: RwLock<HashMap<String, Value>>, // 临时状态
}
```

**上下文压缩策略**:
| 策略 | 触发条件 | 行为 |
|------|----------|------|
| Light | 80% 容量 | 删除最旧 20% |
| Moderate | 90% 容量 | 摘要旧消息 |
| Deep | 95% 容量 | 只保留系统+最后3轮 |

---

## 组件详解

### FusionMemorySystem

中央协调器，管理所有记忆层。

```rust
pub struct FusionMemorySystem {
    config: Arc<FusionConfig>,
    soul: Arc<SoulLayer>,           // L4
    tools: Arc<ToolsLayer>,         // L3
    user: Arc<RwLock<UserLayer>>,   // L2
    sessions: DashMap<String, Arc<SessionLayer>>, // L1
    daily_logs: Arc<DailyLogs>,
    weaver: Arc<MemoryWeaver>,
}
```

**生命周期**:
1. Initialize: 加载所有配置
2. CreateSession: 创建新会话，加载上下文
3. Process: 处理消息，更新状态
4. EndSession: 结束会话，提取记忆
5. Maintenance: 定期维护任务

### FusionAdapter

桥接新旧系统的适配器。

```rust
pub struct FusionAdapter {
    fusion: Arc<FusionMemorySystem>,
    legacy: Option<Arc<MemoryManager>>,
    config: AdapterConfig,
    session_map: RwLock<HashMap<String, Arc<SessionLayer>>>,
}
```

**迁移模式**:
```rust
pub enum MigrationMode {
    LegacyOnly,              // 只使用旧系统
    DualWrite,               // 双向写入
    FusionOnly,              // 只使用新系统
    ReadLegacyWriteBoth,     // 读旧写双
}
```

### FusionRouter

集成四层记忆的认知路由器。

```rust
pub struct FusionRouter {
    memory: Arc<FusionAdapter>,
    system1: Arc<dyn CognitiveSystem>,
    system2: Option<Arc<dyn CognitiveSystem>>,
    system3: Option<Arc<dyn CognitiveSystem>>,
    config: RouterConfig,
}
```

**路由逻辑**:
1. 构建富化上下文（SOUL + USER + Daily Logs）
2. 计算复杂度分数
3. 选择认知系统（S1/S2/S3）
4. 决定是否使用工具
5. 处理并提取记忆

### Memory Weaver

记忆提取和整合引擎。

```rust
pub struct MemoryWeaver {
    extraction_patterns: RwLock<Vec<ExtractionPattern>>,
    consolidation_queue: RwLock<Vec<Memory>>,
}
```

**提取模式**:
- Explicit: "I like...", "I prefer..."
- Goal: "I want to...", "My goal is..."
- Fact: "I am...", "I work..."
- Decision: "I decided...", "I chose..."

---

## 数据流

### 1. 会话生命周期

```
User Input
    ↓
[Create Session]
    ├── Load SOUL → System Prompt
    ├── Load USER → Relevant Memories
    └── Load Daily Logs → Recent Context
    ↓
[Process Message]
    ├── Add to Session (L1)
    ├── Log to Daily Logs
    ├── Extract Memories (Weaver)
    └── Route to Cognitive System
    ↓
[Generate Response]
    ├── Use Tools (if needed)
    ├── Apply Context Compression
    └── Return to User
    ↓
[End Session]
    ├── Persist Session
    ├── Consolidate Memories
    └── Update USER Layer
```

### 2. 记忆整合流程

```
Session Messages
    ↓
[Extract] (Pattern Matching)
    ├── "I like dark mode" → Preference
    ├── "I want to learn Rust" → Goal
    └── "I work at Google" → Fact
    ↓
[Queue] (Consolidation Queue)
    ↓
[Consolidate] (Periodic)
    ├── Group by Category
    ├── Merge Similar (Jaccard > 0.7)
    ├── Archive Old (30+ days, low importance)
    └── Create Connections
    ↓
[Store] (USER Layer)
    ├── Update Profile
    ├── Add to Memories
    └── Update Index
```

### 3. 上下文压缩流程

```
Token Usage Check
    ↓
[Threshold Check]
    ├── < 80% → Continue
    ├── 80-90% → Light Compression
    ├── 90-95% → Moderate Compression
    └── > 95% → Deep Compression
    ↓
[Apply Compression]
    ├── Light: Remove oldest 20%
    ├── Moderate: Summarize old messages
    └── Deep: Keep system + last 3 rounds
    ↓
[Update Token Count]
    ↓
Continue Processing
```

---

## 扩展性设计

### 1. 插件系统

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    async fn execute(&self, params: Value) -> Result<ToolResult, ToolError>;
}

// 注册自定义工具
adapter.tools().register_tool(Arc::new(MyCustomTool)).await?;
```

### 2. 自定义提取模式

```rust
let pattern = ExtractionPattern {
    name: "custom_pattern".to_string(),
    pattern_type: PatternType::Explicit,
    keywords: vec!["my hobby is".to_string()],
    category: "hobbies".to_string(),
    importance_boost: 0.3,
};

weaver.add_pattern(pattern).await;
```

### 3. 存储后端

```rust
pub trait StorageBackend: Send + Sync {
    async fn load(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn save(&self, key: &str, data: &[u8]) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
}

// 实现自定义后端
impl StorageBackend for MyBackend {
    // ...
}
```

---

## 性能优化

### 1. 缓存策略

| 层级 | 缓存类型 | 大小 | TTL |
|------|----------|------|-----|
| L1 | Moka Cache | 100 items | 5 min |
| L2 | LRU Cache | 1000 items | 1 hour |
| L3 | In-memory | All | N/A |
| L4 | Static | All | N/A |

### 2. 异步优化

- 所有 I/O 操作异步
- 批量写入磁盘
- 延迟持久化

### 3. 内存管理

- 会话过期自动清理
- 记忆定期归档
- 向量索引压缩

### 4. 并发控制

```rust
// 使用 DashMap 实现无锁并发
sessions: DashMap<String, Arc<SessionLayer>>

// 使用 RwLock 实现读写分离
user: Arc<RwLock<UserLayer>>
```

---

## 安全设计

### 1. SOUL 层保护

```rust
impl SoulLayer {
    pub fn check_action(&self, action: &str) -> ActionCheckResult {
        for rule in &self.immutable_rules {
            if Self::action_matches_rule(action, &rule.rule) {
                return ActionCheckResult::Violation { ... };
            }
        }
        ActionCheckResult::Allowed
    }
}
```

### 2. 权限系统

```rust
pub enum PermissionLevel {
    Denied,      // 完全禁止
    ReadOnly,    // 只读
    Standard,    // 标准访问
    Elevated,    // 提升权限
    Full,        // 完全访问
}
```

### 3. 数据隔离

- 用户数据隔离存储
- 会话数据加密（可选）
- 审计日志记录

### 4. 输入验证

```rust
impl SessionLayer {
    async fn add_message(&self, message: Message) -> Result<(), MemoryError> {
        // 验证消息长度
        if message.content.len() > MAX_MESSAGE_SIZE {
            return Err(MemoryError::InvalidInput("Message too long".to_string()));
        }
        // ...
    }
}
```

---

## 部署架构

### 单实例部署

```
┌─────────────────────────────────────┐
│           Crablet Instance          │
│  ┌─────────────────────────────┐   │
│  │    Fusion Memory System     │   │
│  │  ┌─────────┐  ┌─────────┐  │   │
│  │  │  SOUL   │  │  TOOLS  │  │   │
│  │  └─────────┘  └─────────┘  │   │
│  │  ┌─────────┐  ┌─────────┐  │   │
│  │  │  USER   │  │ Session │  │   │
│  │  └─────────┘  └─────────┘  │   │
│  └─────────────────────────────┘   │
│  ┌─────────────────────────────┐   │
│  │       Local Storage         │   │
│  │  (Markdown + JSON + SQLite) │   │
│  └─────────────────────────────┘   │
└─────────────────────────────────────┘
```

### 分布式部署

```
┌─────────────────────────────────────────────────────────────┐
│                      Load Balancer                          │
└─────────────────────────────────────────────────────────────┘
                            │
        ┌───────────────────┼───────────────────┐
        ▼                   ▼                   ▼
┌───────────────┐   ┌───────────────┐   ┌───────────────┐
│  Crablet #1   │   │  Crablet #2   │   │  Crablet #3   │
│  (Instance)   │   │  (Instance)   │   │  (Instance)   │
└───────┬───────┘   └───────┬───────┘   └───────┬───────┘
        │                   │                   │
        └───────────────────┼───────────────────┘
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                   Shared Storage Layer                      │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │   Redis     │  │  PostgreSQL │  │  Vector DB          │ │
│  │  (Sessions) │  │  (Metadata) │  │  (Semantic Search)  │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

---

## 监控与运维

### 关键指标

| 指标 | 说明 | 告警阈值 |
|------|------|----------|
| `fusion_sessions_active` | 活跃会话数 | > 1000 |
| `fusion_memory_size_bytes` | 内存使用 | > 1GB |
| `fusion_daily_logs_size` | 日志大小 | > 100MB/day |
| `fusion_consolidation_queue` | 整合队列长度 | > 1000 |

### 健康检查

```bash
# HTTP 健康检查端点
curl http://localhost:8080/health

# 响应
{
  "status": "healthy",
  "layers": {
    "soul": "ok",
    "tools": "ok",
    "user": "ok",
    "session": "ok"
  },
  "stats": {
    "active_sessions": 42,
    "total_memories": 1523
  }
}
```

---

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| 2.0.0 | 2026-03-15 | 初始发布，四层记忆系统 |
| 2.0.1 | - | 性能优化，bug 修复 |
| 2.1.0 | - | 向量数据库集成 |

---

## 参考文档

- [API 文档](API.md)
- [迁移指南](MIGRATION_GUIDE.md)
- [配置参考](CONFIGURATION.md)
- [示例代码](../examples/)

---

*Crablet Fusion - 让 AI 拥有真正的记忆* 🦀
