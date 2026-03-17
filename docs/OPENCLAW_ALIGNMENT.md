# Crablet OpenClaw 架构对齐方案

> **目标**: 参考 OpenClaw 设计，重构 Crablet 的记忆系统和 Agent 工作区  
> **版本**: v0.2.0  
> **日期**: 2026-03-15

---

## 1. 架构对比分析

### 1.1 OpenClaw 设计理念

```
┌─────────────────────────────────────────────────────────────┐
│                    OpenClaw 四层记忆架构                       │
├─────────────────────────────────────────────────────────────┤
│  L4: SOUL (不可变内核)                                        │
│     └── 人格定义、核心价值观、不可变原则                        │
├─────────────────────────────────────────────────────────────┤
│  L3: TOOLS (动态工具)                                         │
│     └── 可用技能、扩展插件、API 集成                           │
├─────────────────────────────────────────────────────────────┤
│  L2: USER (语义长期记忆)                                      │
│     └── 用户偏好、历史决策、重要事实                           │
├─────────────────────────────────────────────────────────────┤
│  L1: Session (实时情景)                                       │
│     └── 当前对话上下文、临时状态、短期记忆                      │
└─────────────────────────────────────────────────────────────┘
```

### 1.2 Crablet 现状 vs OpenClaw

| 维度 | Crablet (当前) | OpenClaw (目标) | 差距 |
|------|----------------|-----------------|------|
| **记忆分层** | 三层记忆（工作/情节/语义） | 四层记忆（SOUL/TOOLS/USER/Session） | 需要重构 |
| **配置管理** | 分散在代码和配置文件中 | 集中式 Agent 工作区 | 需要统一 |
| **日志系统** | 传统日志 | Append-only Daily Logs | 需要改进 |
| **用户画像** | 简单的偏好存储 | 完整的 USER.md 记忆体系 | 需要增强 |
| **技能系统** | Skill + MCP | TOOLS 动态加载 | 基本对齐 |

---

## 2. 重构方案

### 2.1 目录结构重构

当前 Crablet 结构：
```
crablet/
├── src/
│   ├── memory/          # 记忆系统
│   ├── skills/          # 技能系统
│   └── ...
└── config/              # 配置文件
```

OpenClaw 对齐结构：
```
crablet/
├── agent-workspace/           # Agent 工作区（新增）
│   ├── AGENTS.md              # Agent 定义（身份、行为规则）
│   ├── SOUL.md                # 灵魂/人格指令（不可变内核）
│   ├── USER.md                # 用户信息与偏好
│   ├── MEMORY.md              # 长期记忆存储
│   ├── HEARTBEAT.md           # 心跳配置（定时任务）
│   ├── memory/                # 日志目录
│   │   └── YYYY-MM-DD.md      # 每日 append-only 日志
│   ├── skills/                # 本地技能目录
│   └── sessions.json          # 会话存储
├── src/
│   ├── memory/                # 记忆系统实现
│   │   ├── layer_soul.rs      # SOUL 层（不可变）
│   │   ├── layer_tools.rs     # TOOLS 层（动态）
│   │   ├── layer_user.rs      # USER 层（持久化）
│   │   └── layer_session.rs   # Session 层（临时）
│   ├── skills/                # 技能系统
│   └── ...
└── config/
```

### 2.2 四层记忆系统实现

#### L4: SOUL 层（不可变内核）

```rust
// src/memory/layer_soul.rs
use std::sync::Arc;
use once_cell::sync::Lazy;

/// SOUL 层 - 不可变内核
/// 包含 Agent 的人格定义、核心价值观、不可变原则
pub struct SoulLayer {
    /// 核心身份定义（只读）
    identity: Arc<AgentIdentity>,
    /// 核心价值观（只读）
    values: Arc<Vec<CoreValue>>,
    /// 不可变原则（只读）
    immutable_principles: Arc<Vec<Principle>>,
}

impl SoulLayer {
    /// 全局单例，运行时不可修改
    pub fn global() -> &'static Self {
        static SOUL: Lazy<SoulLayer> = Lazy::new(|| {
            SoulLayer::load_from_file("agent-workspace/SOUL.md")
        });
        &SOUL
    }
    
    /// 从 SOUL.md 加载（仅在启动时执行一次）
    fn load_from_file(path: &str) -> Self {
        let content = std::fs::read_to_string(path)
            .expect("SOUL.md is required");
        Self::parse(&content)
    }
}
```

#### L3: TOOLS 层（动态工具）

```rust
// src/memory/layer_tools.rs
use std::collections::HashMap;

/// TOOLS 层 - 动态工具管理
/// 当前可用的工具和技能列表，随安装和加载动态变化
pub struct ToolsLayer {
    /// 已加载的技能
    skills: HashMap<String, Box<dyn Skill>>,
    /// 可用的工具
    tools: HashMap<String, ToolDefinition>,
    /// 扩展插件
    extensions: Vec<Extension>,
}

impl ToolsLayer {
    /// 动态加载技能
    pub fn load_skill(&mut self, skill_path: &str) -> Result<(), Error> {
        // 从 agent-workspace/skills/ 加载
    }
    
    /// 卸载技能
    pub fn unload_skill(&mut self, skill_name: &str) {
        // 动态卸载
    }
    
    /// 获取可用工具列表
    pub fn available_tools(&self) -> Vec<&ToolDefinition> {
        self.tools.values().collect()
    }
}
```

#### L2: USER 层（语义长期记忆）

```rust
// src/memory/layer_user.rs
use vector_db::VectorStore;

/// USER 层 - 语义长期记忆
/// 关于用户的偏好、决策、历史事实，支持语义搜索
pub struct UserLayer {
    /// 用户画像（从 USER.md 加载）
    profile: UserProfile,
    /// 重要事实（从 MEMORY.md 加载）
    facts: Vec<Fact>,
    /// 向量数据库（语义记忆）
    vector_store: Arc<VectorStore>,
    /// 历史决策记录
    decisions: Vec<Decision>,
}

impl UserLayer {
    /// 语义检索记忆
    pub async fn search_memories(
        &self,
        query: &str,
        limit: usize,
    ) -> Vec<Memory> {
        // 向量相似度搜索
        self.vector_store.search(query, limit).await
    }
    
    /// 记录新的事实
    pub async fn record_fact(&mut self, fact: Fact) {
        // 写入 MEMORY.md
        // 同步到向量数据库
    }
    
    /// 加载今日和昨日日志
    pub fn load_recent_logs(&self) -> Vec<DailyLog> {
        // 读取 memory/YYYY-MM-DD.md
    }
}
```

#### L1: Session 层（实时情景）

```rust
// src/memory/layer_session.rs

/// Session 层 - 实时情景
/// 当前对话的实时上下文，Token 耗尽时被压缩
pub struct SessionLayer {
    /// 会话 ID
    session_id: String,
    /// 消息历史
    messages: Vec<Message>,
    /// 当前状态
    state: SessionState,
    /// Token 使用量
    token_usage: TokenUsage,
    /// 临时上下文
    temp_context: HashMap<String, Value>,
}

impl SessionLayer {
    /// 添加消息
    pub fn add_message(&mut self, msg: Message) {
        self.messages.push(msg);
        self.check_token_limit();
    }
    
    /// 检查 Token 限制，必要时压缩
    fn check_token_limit(&mut self) {
        if self.token_usage.current > self.token_usage.max {
            self.compress_history();
        }
    }
    
    /// 压缩历史消息
    fn compress_history(&mut self) {
        // 摘要化早期消息
        // 保留关键信息
    }
    
    /// 保存到 sessions.json
    pub fn save(&self) -> Result<(), Error> {
        // 持久化会话状态
    }
}
```

### 2.3 Daily Logs 系统

#### 日志格式规范

```markdown
# memory/2026-03-15.md

## 会话摘要
- **会话数**: 5
- **总消息数**: 42
- **主要话题**: Rust 开发、架构设计

## 详细记录

### 09:23 - 代码审查任务
用户上传了 `src/memory/mod.rs`，询问代码优化建议。
- **关键决策**: 同意采用四层记忆架构
- **生成的文件**: 
  - `layer_soul.rs`
  - `layer_tools.rs`
  - `layer_user.rs`
  - `layer_session.rs`

### 14:15 - 架构讨论
讨论了 OpenClaw 与 Crablet 的架构差异。
- **用户偏好**: 喜欢表格对比的形式
- **技术选型**: 决定使用向量数据库存储语义记忆

### 16:45 - 性能优化
讨论了流式渲染的性能问题。
- **解决方案**: 使用 React.memo + throttle
- **效果**: 闪烁问题得到解决

## 提取的记忆
- 用户是 Rust 开发者，对架构设计感兴趣
- 用户偏好结构化的输出格式
- 用户关注性能优化
```

#### 日志管理实现

```rust
// src/memory/daily_logs.rs
use chrono::{Local, NaiveDate};

/// Daily Logs 管理器
pub struct DailyLogs {
    /// 日志目录
    log_dir: PathBuf,
    /// 当前日期
    current_date: NaiveDate,
}

impl DailyLogs {
    /// 追加记录到当日日志
    pub fn append(&self, entry: LogEntry) -> Result<(), Error> {
        let file_path = self.log_dir.join(format!("{}.md", self.current_date));
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(file_path)?;
        
        writeln!(file, "{}", entry.format())?;
        Ok(())
    }
    
    /// 读取指定日期的日志
    pub fn read_date(&self, date: NaiveDate) -> Result<String, Error> {
        let file_path = self.log_dir.join(format!("{}.md", date));
        fs::read_to_string(file_path)
    }
    
    /// 获取今日和昨日日志（用于会话启动时加载）
    pub fn load_recent(&self) -> Vec<String> {
        let today = Local::now().date_naive();
        let yesterday = today.pred_opt().unwrap();
        
        vec![
            self.read_date(today).unwrap_or_default(),
            self.read_date(yesterday).unwrap_or_default(),
        ]
    }
}
```

---

## 3. 文件内容规范

### 3.1 AGENTS.md 模板

```markdown
# Crablet Agent 定义

## 身份定义
- **名称**: Crablet (小螃蟹)
- **版本**: v0.2.0
- **角色**: 多模态 AI 助手

## 能力边界
### 已具备
- [x] 文档问答
- [x] 图片分析
- [x] 知识检索
- [x] 流式对话

### 规划中
- [ ] 工具调用
- [ ] 代码执行
- [ ] 多 Agent 协作

## 行为准则
1. 用户优先
2. 透明诚实
3. 高效简洁
4. 持续学习
```

### 3.2 SOUL.md 模板

```markdown
# Crablet SOUL - 灵魂/人格指令

## 核心身份
我是 Crablet（小螃蟹），一个智能、可靠、有帮助的 AI 助手。

## 核心价值观
1. 用户至上
2. 诚实透明
3. 持续进化
4. 安全可靠

## 不可变原则
- 绝不伤害
- 保护隐私
- 诚实为本
- 尊重自主

## 人格特质
- 友好、专业、耐心、好奇、谦逊
```

### 3.3 USER.md 模板

```markdown
# Crablet USER - 用户信息与偏好

## 基本信息
- **用户ID**: user_default
- **首次使用**: 2026-03-01

## 语言偏好
- **主要语言**: 简体中文
- **风格**: 友好/自然

## 交互偏好
- **响应长度**: 适中
- **格式**: Markdown

## 学习记录
（将在交互中自动填充）
```

### 3.4 MEMORY.md 模板

```markdown
# Crablet MEMORY - 长期记忆存储

## 重要事实
（将在交互中自动提取）

## 历史决策
| 日期 | 决策 | 说明 |
|------|------|------|

## 用户目标
- 短期:
- 长期:
```

### 3.5 HEARTBEAT.md 模板

```markdown
# Crablet HEARTBEAT - 心跳配置

## 每日任务
- 00:00: 归档日志
- 02:00: 提取记忆
- 04:00: 数据备份

## 每周任务
- Sunday 01:00: 记忆整理
- Sunday 03:00: 用户画像更新

## 健康检查
- 数据库连接: 1m
- 存储空间: 1h
```

---

## 4. 迁移计划

### 阶段 1: 基础设施（1 周）
- [ ] 创建 `agent-workspace/` 目录结构
- [ ] 实现四层记忆系统的核心接口
- [ ] 创建 Daily Logs 管理器
- [ ] 编写文件加载/保存模块

### 阶段 2: 数据迁移（1 周）
- [ ] 从现有配置生成 AGENTS.md
- [ ] 从用户数据生成 USER.md
- [ ] 迁移历史记录到 Daily Logs 格式
- [ ] 建立向量数据库索引

### 阶段 3: 系统集成（1 周）
- [ ] 替换现有的记忆系统调用
- [ ] 集成四层记忆到对话流程
- [ ] 实现自动日志记录
- [ ] 添加记忆检索功能

### 阶段 4: 测试优化（1 周）
- [ ] 单元测试覆盖
- [ ] 集成测试
- [ ] 性能测试
- [ ] 文档更新

---

## 5. 预期收益

| 指标 | 当前 | 目标 | 提升 |
|------|------|------|------|
| 记忆准确性 | 70% | 90% | +28% |
| 上下文连续性 | 中等 | 优秀 | 显著 |
| 个性化程度 | 基础 | 深度 | 显著 |
| 可维护性 | 一般 | 优秀 | 显著 |
| 可扩展性 | 良好 | 优秀 | 中等 |

---

*本文档定义了 Crablet 向 OpenClaw 架构对齐的完整方案。*
