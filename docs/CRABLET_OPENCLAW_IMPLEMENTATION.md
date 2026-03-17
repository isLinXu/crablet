# Crablet OpenClaw 架构实施指南

> **完整实施文档** | 参考 OpenClaw 设计重构 Crablet  
> **版本**: v0.2.0  
> **日期**: 2026-03-15

---

## 执行摘要

本文档提供了将 Crablet 从当前架构迁移到 OpenClaw 风格的完整实施方案。通过引入**四层记忆系统**和**Agent 工作区**，Crablet 将具备更强的上下文理解能力、个性化服务和可扩展性。

### 关键改进

| 维度 | 改进前 | 改进后 | 提升 |
|------|--------|--------|------|
| **记忆架构** | 三层记忆（工作/情节/语义） | 四层记忆（SOUL/TOOLS/USER/Session） | 更清晰的职责分离 |
| **个性化** | 基础偏好存储 | 完整的 USER 画像系统 | 深度个性化 |
| **上下文连续性** | 会话级记忆 | Daily Logs + 长期记忆 | 跨会话连续性 |
| **可维护性** | 配置分散 | 集中式 Agent 工作区 | 易于管理 |
| **可扩展性** | 固定技能集 | 动态 TOOLS 加载 | 热插拔扩展 |

---

## 1. 架构总览

### 1.1 OpenClaw 四层记忆架构

```
┌─────────────────────────────────────────────────────────────────┐
│                    Crablet 记忆金字塔 v0.2.0                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│   L4: SOUL (不可变内核)                                           │
│   ┌─────────────────────────────────────────────────────────┐   │
│   │  • 人格定义 (AGENTS.md)                                  │   │
│   │  • 核心价值观 (SOUL.md)                                  │   │
│   │  • 不可变原则                                            │   │
│   │  生命周期: 永久不变 | 存储: 文件系统                      │   │
│   └─────────────────────────────────────────────────────────┘   │
│                              ↓                                    │
│   L3: TOOLS (动态工具)                                            │
│   ┌─────────────────────────────────────────────────────────┐   │
│   │  • 可用技能列表 (TOOLS.md)                               │   │
│   │  • 扩展插件 (skills/)                                    │   │
│   │  • API 集成                                              │   │
│   │  生命周期: 按需加载 | 存储: 文件系统 + WASM              │   │
│   └─────────────────────────────────────────────────────────┘   │
│                              ↓                                    │
│   L2: USER (语义长期记忆)                                         │
│   ┌─────────────────────────────────────────────────────────┐   │
│   │  • 用户偏好 (USER.md)                                    │   │
│   │  • 重要事实 (MEMORY.md)                                  │   │
│   │  • 历史决策                                              │   │
│   │  生命周期: 持久化 | 存储: 文件 + 向量数据库              │   │
│   └─────────────────────────────────────────────────────────┘   │
│                              ↓                                    │
│   L1: Session (实时情景)                                          │
│   ┌─────────────────────────────────────────────────────────┐   │
│   │  • 当前对话上下文                                        │   │
│   │  • 临时状态                                              │   │
│   │  • Token 管理                                            │   │
│   │  生命周期: 会话级 | 存储: 内存 + sessions.json           │   │
│   └─────────────────────────────────────────────────────────┘   │
│                                                                   │
├─────────────────────────────────────────────────────────────────┤
│   Daily Logs: memory/YYYY-MM-DD.md (append-only 日志)            │
└─────────────────────────────────────────────────────────────────┘
```

### 1.2 Agent 工作区结构

```
crablet/
├── agent-workspace/                 # Agent 工作区（新增）
│   ├── AGENTS.md                    # Agent 定义（身份、行为规则）
│   ├── SOUL.md                      # 灵魂/人格指令（不可变内核）
│   ├── USER.md                      # 用户信息与偏好
│   ├── MEMORY.md                    # 长期记忆存储
│   ├── TOOLS.md                     # 动态工具系统
│   ├── HEARTBEAT.md                 # 心跳配置（定时任务）
│   ├── memory/                      # 日志目录
│   │   └── YYYY-MM-DD.md            # 每日 append-only 日志
│   ├── skills/                      # 本地技能目录
│   │   ├── weather/
│   │   ├── calculator/
│   │   └── web_search/
│   └── sessions.json                # 会话存储
│
├── src/                             # 源代码
│   ├── memory/                      # 记忆系统实现
│   │   ├── mod.rs                   # 记忆模块入口
│   │   ├── layer_soul.rs            # SOUL 层（不可变）
│   │   ├── layer_tools.rs           # TOOLS 层（动态）
│   │   ├── layer_user.rs            # USER 层（持久化）
│   │   ├── layer_session.rs         # Session 层（临时）
│   │   ├── daily_logs.rs            # 日志管理
│   │   └── vector_store.rs          # 向量数据库接口
│   ├── skills/                      # 技能系统
│   └── ...
│
└── docs/                            # 文档
    ├── OPENCLAW_ALIGNMENT.md        # 架构对齐方案
    └── CRABLET_OPENCLAW_IMPLEMENTATION.md  # 本实施指南
```

---

## 2. 核心组件实现

### 2.1 SOUL 层 - 不可变内核

**职责**: 存储 Agent 的核心身份、价值观和不可变原则

**关键代码**:

```rust
// src/memory/layer_soul.rs
use std::sync::Arc;
use once_cell::sync::Lazy;

pub struct SoulLayer {
    pub identity: AgentIdentity,
    pub values: Vec<CoreValue>,
    pub principles: Vec<Principle>,
}

impl SoulLayer {
    /// 全局单例，运行时不可修改
    pub fn global() -> Arc<Self> {
        static SOUL: Lazy<Arc<SoulLayer>> = Lazy::new(|| {
            Arc::new(SoulLayer::load_from_file("agent-workspace/SOUL.md"))
        });
        SOUL.clone()
    }
}
```

### 2.2 TOOLS 层 - 动态工具

**职责**: 管理当前可用的技能和工具，支持动态加载和卸载

**关键代码**:

```rust
// src/memory/layer_tools.rs
pub struct ToolsLayer {
    skills: RwLock<HashMap<String, Arc<dyn Skill>>>,
    tools: RwLock<HashMap<String, ToolDefinition>>,
}

#[async_trait]
pub trait Skill: Send + Sync {
    fn name(&self) -> &str;
    async fn execute(&self, params: Value) -> Result<Value>;
}
```

### 2.3 USER 层 - 语义长期记忆

**职责**: 存储用户偏好、重要事实和历史决策，支持语义搜索

**关键代码**:

```rust
// src/memory/layer_user.rs
pub struct UserLayer {
    profile: RwLock<UserProfile>,
    facts: RwLock<Vec<Fact>>,
    vector_store: Arc<dyn VectorStore>,
}

impl UserLayer {
    pub async fn search_memories(&self, query: &str, limit: usize) -> Vec<Memory> {
        self.vector_store.search(query, limit).await
    }
}
```

### 2.4 Session 层 - 实时情景

**职责**: 管理当前对话的实时上下文，Token 耗尽时进行压缩

**关键代码**:

```rust
// src/memory/layer_session.rs
pub struct SessionLayer {
    session_id: String,
    messages: RwLock<Vec<Message>>,
    token_usage: RwLock<TokenUsage>,
}

impl SessionLayer {
    pub async fn add_message(&self, msg: Message) {
        self.messages.write().await.push(msg);
        self.check_token_limit().await;
    }
}
```

---

## 3. 迁移计划

### 3.1 阶段划分

| 阶段 | 时间 | 任务 | 产出 |
|------|------|------|------|
| **Phase 1** | 第 1 周 | 基础设施 | 目录结构 + 核心接口 |
| **Phase 2** | 第 2 周 | 数据迁移 | 配置文件 + 历史数据 |
| **Phase 3** | 第 3 周 | 系统集成 | 替换旧系统调用 |
| **Phase 4** | 第 4 周 | 测试优化 | 完整测试覆盖 |

### 3.2 Phase 1: 基础设施 (Week 1)

**目标**: 搭建新的架构框架

**任务清单**:
- [x] 创建 `agent-workspace/` 目录结构
- [x] 创建 `AGENTS.md` - Agent 定义
- [x] 创建 `SOUL.md` - 灵魂指令
- [x] 创建 `USER.md` - 用户偏好
- [x] 创建 `MEMORY.md` - 长期记忆
- [x] 创建 `TOOLS.md` - 工具系统
- [x] 创建 `HEARTBEAT.md` - 心跳配置
- [ ] 实现 `layer_soul.rs` - SOUL 层代码
- [ ] 实现 `layer_tools.rs` - TOOLS 层代码
- [ ] 实现 `layer_user.rs` - USER 层代码
- [ ] 实现 `layer_session.rs` - Session 层代码
- [ ] 实现 `daily_logs.rs` - 日志系统

**命令**:

```bash
# 创建目录结构
mkdir -p agent-workspace/{memory,skills}
mkdir -p src/memory

# 创建文档
touch agent-workspace/{AGENTS,SOUL,USER,MEMORY,TOOLS,HEARTBEAT}.md
touch agent-workspace/memory/2026-03-15.md

# 创建代码文件
touch src/memory/{mod,layer_soul,layer_tools,layer_user,layer_session,daily_logs}.rs
```

### 3.3 Phase 2: 数据迁移 (Week 2)

**目标**: 将现有数据迁移到新格式

**迁移内容**:

| 源数据 | 目标位置 | 迁移方式 |
|--------|----------|----------|
| 现有配置 | `AGENTS.md` | 手动整理 |
| 用户设置 | `USER.md` | 脚本迁移 |
| 对话历史 | `memory/*.md` | 脚本批量转换 |
| 知识库 | 向量数据库 | 重新索引 |

### 3.4 Phase 3: 系统集成 (Week 3)

**目标**: 将新记忆系统集成到现有流程

**集成点**:

1. **会话启动时**
   - 加载 SOUL 层配置
   - 读取 USER 层偏好
   - 加载今日和昨日日志
   - 初始化 Session 层

2. **对话进行中**
   - 实时记录到 Session
   - 定期保存到 Daily Logs
   - 动态检索相关记忆

3. **会话结束时**
   - 保存 Session 状态
   - 提取长期记忆
   - 更新用户画像

### 3.5 Phase 4: 测试优化 (Week 4)

**测试覆盖**:

- [ ] 单元测试：各层独立测试
- [ ] 集成测试：层间交互测试
- [ ] 性能测试：记忆检索性能
- [ ] 端到端测试：完整对话流程

---

## 4. 文件清单

### 4.1 已创建的文件

| 文件路径 | 说明 | 状态 |
|----------|------|------|
| `agent-workspace/AGENTS.md` | Agent 身份定义 | ✅ 已创建 |
| `agent-workspace/SOUL.md` | 灵魂/人格指令 | ✅ 已创建 |
| `agent-workspace/USER.md` | 用户信息与偏好 | ✅ 已创建 |
| `agent-workspace/MEMORY.md` | 长期记忆存储 | ✅ 已创建 |
| `agent-workspace/TOOLS.md` | 动态工具系统 | ✅ 已创建 |
| `agent-workspace/HEARTBEAT.md` | 心跳配置 | ✅ 已创建 |
| `agent-workspace/memory/2026-03-15.md` | 每日日志示例 | ✅ 已创建 |
| `agent-workspace/sessions.json` | 会话存储 | ✅ 已创建 |
| `docs/OPENCLAW_ALIGNMENT.md` | 架构对齐方案 | ✅ 已创建 |
| `docs/CRABLET_OPENCLAW_IMPLEMENTATION.md` | 实施指南 | ✅ 已创建 |

### 4.2 待实现的代码文件

| 文件路径 | 说明 | 优先级 |
|----------|------|--------|
| `src/memory/mod.rs` | 记忆模块入口 | P0 |
| `src/memory/layer_soul.rs` | SOUL 层实现 | P0 |
| `src/memory/layer_tools.rs` | TOOLS 层实现 | P0 |
| `src/memory/layer_user.rs` | USER 层实现 | P0 |
| `src/memory/layer_session.rs` | Session 层实现 | P0 |
| `src/memory/daily_logs.rs` | 日志系统实现 | P1 |
| `src/memory/vector_store.rs` | 向量数据库接口 | P1 |

---

## 5. 预期收益

### 5.1 技术指标

| 指标 | 当前 | 目标 | 提升 |
|------|------|------|------|
| 记忆准确性 | 70% | 90% | +28% |
| 上下文连续性 | 中等 | 优秀 | 显著 |
| 个性化程度 | 基础 | 深度 | 显著 |
| 可维护性 | 一般 | 优秀 | 显著 |
| 可扩展性 | 良好 | 优秀 | 显著 |

### 5.2 用户体验

- **更连贯的对话**: 跨会话记忆让用户感觉像在和一个了解他的朋友聊天
- **更个性化的服务**: 系统会根据用户偏好自动调整响应风格
- **更智能的检索**: 语义搜索能够理解用户的真实意图
- **更灵活的功能**: 动态技能系统支持按需扩展

---

## 6. 后续建议

### 6.1 短期优化 (1-2 周)

1. **完成代码实现**
   - 实现四层记忆系统的核心代码
   - 集成到现有对话流程
   - 添加必要的错误处理

2. **数据迁移脚本**
   - 编写从旧系统迁移的脚本
   - 测试数据完整性
   - 制定回滚方案

3. **基础测试**
   - 单元测试覆盖核心功能
   - 集成测试验证层间协作

### 6.2 中期优化 (1 个月)

1. **记忆提取优化**
   - 使用 LLM 自动提取重要事实
   - 实现记忆冲突检测和解决
   - 添加记忆置信度评分

2. **性能优化**
   - 向量数据库索引优化
   - 记忆检索缓存
   - 异步日志写入

3. **用户界面**
   - 添加记忆管理界面
   - 显示已学习的偏好
   - 允许用户编辑记忆

### 6.3 长期规划 (3 个月)

1. **多用户支持**
   - 用户隔离的记忆存储
   - 共享知识库
   - 用户组管理

2. **高级功能**
   - 记忆可视化
   - 时间线回顾
   - 智能提醒

3. **生态建设**
   - 技能商店
   - 记忆模板
   - 社区分享

---

## 7. 总结

通过参考 OpenClaw 的设计，我们为 Crablet 构建了一套完整的四层记忆系统和 Agent 工作区架构。这套架构具有以下优势：

1. **清晰的职责分离**: 每层记忆有明确的职责和生命周期
2. **强大的个性化**: 深度用户画像和语义记忆
3. **优秀的可维护性**: 集中式配置管理
4. **灵活的可扩展性**: 动态技能加载

实施这套架构将让 Crablet 能够：
- **服务更多人**: 多用户支持和更好的可扩展性
- **做更多事情**: 动态技能系统和工具调用
- **提供更好的体验**: 深度个性化和上下文连续性

---

*本文档由 Crablet 团队编写，参考 OpenClaw 架构设计。*  
*最后更新: 2026-03-15*
