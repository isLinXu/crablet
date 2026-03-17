# Crablet + OpenClaw 融合架构方案

> **双向优化** | 让 Crablet 更强，让 OpenClaw 更实  
> **版本**: v2.0  
> **日期**: 2026-03-15

---

## 1. 融合架构愿景

### 1.1 核心理念

**不是选择，而是融合** - 将 Crablet 的工程成熟度与 OpenClaw 的配置优雅性结合，创造 1+1>2 的效果。

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         融合架构愿景                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   Before:                        After:                                      │
│   ┌──────────┐  ┌──────────┐    ┌──────────────────────────────────────┐    │
│   │ Crablet  │  │ OpenClaw │ →  │      Crablet OpenClaw Edition        │    │
│   │  功能强   │  │  配置优   │    │  功能强大 + 配置优雅 + 双向增强       │    │
│   └──────────┘  └──────────┘    └──────────────────────────────────────┘    │
│                                                                              │
│   优势:                                                                     │
│   • Crablet 获得人类友好的配置管理                                          │
│   • OpenClaw 获得生产级的功能实现                                           │
│   • 两者相互增强，形成生态                                                   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 融合架构图

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    Crablet OpenClaw Edition (COE)                           │
│                         融合架构 v2.0                                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                        配置表示层 (OpenClaw 风格)                     │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │   │
│  │  │AGENTS.md │ │ SOUL.md  │ │ USER.md  │ │MEMORY.md │ │TOOLS.md  │  │   │
│  │  │(身份定义) │ │(人格指令)│ │(用户画像)│ │(长期记忆)│ │(技能系统)│  │   │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘  │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐                            │   │
│  │  │HEARTBEAT │ │ memory/  │ │ skills/  │                            │   │
│  │  │(定时任务) │ │(日志目录)│ │(技能目录)│                            │   │
│  │  └──────────┘ └──────────┘ └──────────┘                            │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    ↓                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                        双向适配层 (Bridge)                           │   │
│  │  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐          │   │
│  │  │ Config Loader│ ←→ │  Schema      │ ←→ │ Validator    │          │   │
│  │  │ (配置加载)    │    │  (结构定义)   │    │ (验证器)      │          │   │
│  │  └──────────────┘    └──────────────┘    └──────────────┘          │   │
│  │  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐          │   │
│  │  │ Sync Engine  │ ←→ │  Diff        │ ←→ │ Exporter     │          │   │
│  │  │ (同步引擎)    │    │  (差异检测)   │    │ (导出器)      │          │   │
│  │  └──────────────┘    └──────────────┘    └──────────────┘          │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    ↓                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                        核心引擎层 (Crablet 增强)                      │   │
│  │                                                                      │   │
│  │  ┌─────────────────────────────────────────────────────────────┐   │   │
│  │  │                    四层记忆系统 (增强版)                      │   │   │
│  │  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │   │   │
│  │  │  │L4: SOUL  │ │L3: TOOLS │ │L2: USER  │ │L1: Session│       │   │   │
│  │  │  │(不可变)  │ │(动态)    │ │(持久化)  │ │(实时)     │       │   │   │
│  │  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │   │   │
│  │  │         + Daily Logs (新增) + Memory Weaver (增强)         │   │   │
│  │  └─────────────────────────────────────────────────────────────┘   │   │
│  │                                                                      │   │
│  │  ┌─────────────────────────────────────────────────────────────┐   │   │
│  │  │                    三层认知架构 (增强版)                      │   │   │
│  │  │  ┌──────────┐ ┌──────────┐ ┌──────────┐                     │   │   │
│  │  │  │ System 1 │ │ System 2 │ │ System 3 │                     │   │   │
│  │  │  │(直觉)    │ │(分析)    │ │(协作)    │                     │   │   │
│  │  │  └──────────┘ └──────────┘ └──────────┘                     │   │   │
│  │  │         + OpenClaw Prompt Skills 集成                       │   │   │
│  │  └─────────────────────────────────────────────────────────────┘   │   │
│  │                                                                      │   │
│  │  ┌─────────────────────────────────────────────────────────────┐   │   │
│  │  │                    技能系统 (增强版)                         │   │   │
│  │  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │   │   │
│  │  │  │  Local   │ │   MCP    │ │  Plugin  │ │OpenClaw  │       │   │   │
│  │  │  │(本地)    │ │(远程)    │ │(原生)    │ │(提示词)  │       │   │   │
│  │  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │   │   │
│  │  │         + Skill Chain + Composite + Orchestrator           │   │   │
│  │  └─────────────────────────────────────────────────────────────┘   │   │
│  │                                                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    ↓                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                        基础设施层                                     │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │   │
│  │  │  RAG     │ │Knowledge │ │  Safety  │ │  Vector  │ │  Graph   │  │   │
│  │  │ (检索)   │ │ (知识)   │ │ (安全)   │ │ (向量)   │ │ (图谱)   │  │   │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. 双向优化策略

### 2.1 优化 Crablet（获得 OpenClaw 优势）

| Crablet 现有 | 优化方向 | 预期效果 |
|--------------|----------|----------|
| 代码内嵌配置 | Markdown 配置层 | 人类可读、AI 友好 |
| 分散的配置文件 | Agent 工作区 | 统一管理、版本控制 |
| 隐式人格定义 | 显式 SOUL.md | 可配置的人格 |
| 基础用户偏好 | 完整 USER 画像 | 深度个性化 |
| SQLite 日志 | Daily Logs | 上下文连续性 |

### 2.2 增强 OpenClaw（获得 Crablet 优势）

| OpenClaw 现有 | 增强方向 | 预期效果 |
|---------------|----------|----------|
| 基础技能系统 | Crablet Skills | 企业级功能 |
| 简单记忆分层 | 三层认知架构 | 智能路由 |
| 纯向量存储 | 知识图谱 | 关系推理 |
| 无安全体系 | Safety Oracle | 生产安全 |
| 无多 Agent | Swarm 支持 | 协作能力 |

---

## 3. 核心组件设计

### 3.1 配置表示层

#### 统一配置格式

```yaml
# agent-workspace/AGENTS.md
---
name: Crablet
version: 2.0.0
edition: openclaw
---

# Agent 定义
## 身份
- **名称**: Crablet
- **版本**: 2.0.0
- **角色**: 多模态 AI 助手

## 能力
- [x] 文档问答 (RAG)
- [x] 图片分析 (OCR)
- [x] 知识检索 (Graph + Vector)
- [x] 流式对话 (WebSocket)
- [x] 工具调用 (Skills)
- [x] 多 Agent 协作 (Swarm)

## 引擎配置
```toml
[cognitive]
router = "adaptive"  # 自适应路由
system1_timeout = "10ms"
system2_timeout = "30s"
system3_timeout = "300s"

[memory]
working_capacity = 20
episodic_backend = "sqlite"
semantic_backend = "hybrid"  # neo4j + sqlite
consolidation_interval = "1h"

[skills]
auto_load = true
hot_reload = true
enable_openclaw = true
enable_mcp = true
```
```

#### 增强的 SOUL.md

```markdown
# Crablet SOUL - 灵魂/人格指令

---
personality:
  name: "小螃蟹"
  traits: ["friendly", "professional", "curious"]
  communication_style: "adaptive"  # 根据用户偏好自适应
  
values:
  - name: "user_first"
    priority: 10
    description: "用户至上"
  - name: "honesty"
    priority: 9
    description: "诚实透明"
  - name: "evolution"
    priority: 8
    description: "持续进化"

principles:
  - name: "do_no_harm"
    immutable: true
    description: "绝不伤害"
  - name: "privacy_protection"
    immutable: true
    description: "保护隐私"

cognitive_profile:
  system1:
    enabled: true
    intent_trie: "builtin"
    fuzzy_matching: true
  system2:
    enabled: true
    react_engine: "enhanced"
    middleware_chain:
      - safety
      - cost_guard
      - semantic_cache
      - planning
      - rag
  system3:
    enabled: true
    swarm_coordinator: "default"
    max_agents: 100
---

## 核心身份
我是 Crablet（小螃蟹），一个智能、可靠、有帮助的 AI 助手...
```

### 3.2 双向适配层

#### 配置加载器

```rust
// src/config/loader.rs
pub struct ConfigLoader {
    workspace_path: PathBuf,
    cache: Arc<RwLock<ConfigCache>>,
}

impl ConfigLoader {
    /// 从 Agent 工作区加载完整配置
    pub async fn load(&self) -> Result<CrabletConfig, ConfigError> {
        let agents = self.load_markdown("AGENTS.md").await?;
        let soul = self.load_markdown("SOUL.md").await?;
        let user = self.load_markdown("USER.md").await?;
        let memory = self.load_markdown("MEMORY.md").await?;
        let tools = self.load_markdown("TOOLS.md").await?;
        
        // 解析并合并配置
        let config = CrabletConfig::from_markdowns(
            agents, soul, user, memory, tools
        )?;
        
        // 验证配置
        config.validate()?;
        
        Ok(config)
    }
    
    /// 热重载支持
    pub async fn watch(&self) -> Result<(), ConfigError> {
        let mut watcher = notify::recommended_watcher(
            move |res: Result<notify::Event, notify::Error>| {
                match res {
                    Ok(event) => {
                        if event.kind.is_modify() {
                            // 触发配置重载
                            tokio::spawn(async move {
                                Self::reload_config().await;
                            });
                        }
                    }
                    Err(e) => error!("Watch error: {:?}", e),
                }
            }
        )?;
        
        watcher.watch(&self.workspace_path, RecursiveMode::Recursive)?;
        
        Ok(())
    }
}
```

#### 同步引擎

```rust
// src/config/sync.rs
pub struct ConfigSyncEngine {
    loader: ConfigLoader,
    exporter: ConfigExporter,
    diff_engine: DiffEngine,
}

impl ConfigSyncEngine {
    /// 双向同步：Markdown ↔ 运行时状态
    pub async fn sync_bidirectional(&self) -> Result<SyncReport, SyncError> {
        let markdown_config = self.loader.load().await?;
        let runtime_config = self.export_runtime_state().await?;
        
        // 检测差异
        let diff = self.diff_engine.compare(&markdown_config, &runtime_config);
        
        // 自动合并（冲突时以 Markdown 为准）
        let merged = self.merge_configs(markdown_config, runtime_config, diff)?;
        
        // 应用配置
        self.apply_to_runtime(&merged).await?;
        
        // 导出回 Markdown（如果运行时状态有更新）
        self.exporter.export(&merged).await?;
        
        Ok(SyncReport::new(merged))
    }
    
    /// 将运行时状态导出到 Markdown
    async fn export_runtime_state(&self) -> Result<CrabletConfig, SyncError> {
        // 从现有组件收集状态
        let memory_state = self.collect_memory_state().await?;
        let user_state = self.collect_user_state().await?;
        let tools_state = self.collect_tools_state().await?;
        
        Ok(CrabletConfig::from_runtime(
            memory_state, user_state, tools_state
        ))
    }
}
```

### 3.3 增强的记忆系统

#### 四层记忆 + Daily Logs

```rust
// src/memory/fusion.rs
pub struct FusionMemorySystem {
    // L4: SOUL - 从 SOUL.md 加载
    soul: Arc<SoulLayer>,
    
    // L3: TOOLS - 动态技能管理
    tools: Arc<ToolsLayer>,
    
    // L2: USER - 增强的用户画像
    user: Arc<UserLayer>,
    
    // L1: Session - 实时会话
    sessions: DashMap<String, Arc<SessionLayer>>,
    
    // Daily Logs - OpenClaw 风格日志
    daily_logs: Arc<DailyLogs>,
    
    // Memory Weaver - 记忆整合器（Crablet 原有）
    weaver: Arc<MemoryWeaver>,
}

impl FusionMemorySystem {
    /// 初始化时加载所有层次
    pub async fn initialize(&self) -> Result<(), MemoryError> {
        // 1. 加载 SOUL（不可变）
        self.soul.load_from_file("agent-workspace/SOUL.md").await?;
        
        // 2. 扫描并加载 TOOLS
        self.tools.scan_directory("agent-workspace/skills").await?;
        
        // 3. 加载 USER 画像
        self.user.load_from_file("agent-workspace/USER.md").await?;
        
        // 4. 初始化 Daily Logs
        self.daily_logs.initialize().await?;
        
        Ok(())
    }
    
    /// 创建新会话（加载上下文）
    pub async fn create_session(&self, session_id: String) -> Result<Arc<SessionLayer>, MemoryError> {
        let session = Arc::new(SessionLayer::new(session_id.clone()));
        
        // 加载今日和昨日日志作为上下文
        let recent_logs = self.daily_logs.load_recent().await?;
        
        // 检索相关长期记忆
        let relevant_memories = self.user.search_relevant(&session_id).await?;
        
        // 构建系统消息
        let system_msg = self.build_context_message(recent_logs, relevant_memories);
        session.add_message(system_msg).await?;
        
        // 保存会话
        self.sessions.insert(session_id, session.clone());
        
        Ok(session)
    }
    
    /// 会话结束时的处理
    pub async fn end_session(&self, session_id: &str) -> Result<(), MemoryError> {
        let session = self.sessions.remove(session_id)
            .ok_or(MemoryError::SessionNotFound)?;
        
        // 1. 保存到 Daily Logs
        self.daily_logs.append_session(&session.1).await?;
        
        // 2. 提取长期记忆
        let extracted = self.weaver.extract_memories(&session.1).await?;
        for memory in extracted {
            self.user.record_memory(memory).await?;
        }
        
        // 3. 更新用户画像
        self.user.update_profile_from_session(&session.1).await?;
        
        // 4. 保存会话状态
        session.1.save_to_file().await?;
        
        Ok(())
    }
}
```

### 3.4 增强的认知架构

#### System 1 + OpenClaw Prompt

```rust
// src/cognitive/system1_enhanced.rs
pub struct EnhancedSystem1 {
    // 原有的 Trie 意图匹配
    intent_trie: IntentTrie,
    
    // 新增的 OpenClaw Prompt 匹配
    prompt_matcher: OpenClawPromptMatcher,
    
    // 语义匹配器
    semantic_matcher: SemanticMatcher,
}

impl EnhancedSystem1 {
    /// 处理输入，支持多种匹配方式
    pub async fn process(&self, input: &str) -> Option<System1Response> {
        // 1. 尝试 Trie 精确匹配
        if let Some(response) = self.intent_trie.match_exact(input) {
            return Some(response);
        }
        
        // 2. 尝试 OpenClaw Prompt 匹配
        if let Some(response) = self.prompt_matcher.match_prompt(input).await {
            return Some(response);
        }
        
        // 3. 尝试语义匹配
        if let Some(response) = self.semantic_matcher.match_semantic(input).await {
            return Some(response);
        }
        
        // 4. 尝试模糊匹配
        self.intent_trie.match_fuzzy(input, 0.8)
    }
}

/// OpenClaw Prompt 匹配器
pub struct OpenClawPromptMatcher {
    prompts: Vec<OpenClawPrompt>,
}

impl OpenClawPromptMatcher {
    /// 从 agent-workspace/skills/*.md 加载 Prompt
    pub async fn load_prompts(&mut self, skills_dir: &Path) -> Result<(), Error> {
        for entry in fs::read_dir(skills_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension() == Some("md".as_ref()) {
                let prompt = OpenClawPrompt::from_file(&path).await?;
                self.prompts.push(prompt);
            }
        }
        
        Ok(())
    }
    
    /// 匹配输入到 Prompt
    pub async fn match_prompt(&self, input: &str) -> Option<System1Response> {
        for prompt in &self.prompts {
            if prompt.matches(input) {
                return Some(System1Response::OpenClaw {
                    skill: prompt.name.clone(),
                    instructions: prompt.instructions.clone(),
                });
            }
        }
        
        None
    }
}
```

---

## 4. 增强的 OpenClaw 功能

### 4.1 增强版 AGENTS.md

```markdown
---
# Crablet OpenClaw Edition - Agent 配置

metadata:
  name: Crablet
  version: 2.0.0
  edition: openclaw
  
engine:
  type: crablet
  version: ">=0.2.0"
  
capabilities:
  rag:
    enabled: true
    backend: hybrid
    vector_store: qdrant
    graph_store: neo4j
    
  memory:
    layers: 4
    daily_logs: true
    consolidation: true
    
  cognitive:
    router: adaptive
    system1: true
    system2: true
    system3: true
    
  skills:
    local: true
    mcp: true
    openclaw: true
    plugin: true
    
  channels:
    - web
    - telegram
    - discord
    - wechat
    
safety:
  oracle: true
  sandbox: docker
  rate_limiting: true
---

# Agent 定义

## 身份
**Crablet** (小螃蟹) - 你的智能助手伙伴

## 行为准则
1. **用户优先**: 主动理解需求
2. **诚实透明**: 明确能力边界
3. **持续学习**: 记住用户偏好
4. **安全可靠**: 保护用户数据

## 认知配置
- **System 1**: 直觉响应 (< 10ms)
- **System 2**: 深度分析 (ReAct 引擎)
- **System 3**: 群体协作 (Swarm)

## 记忆配置
- **SOUL**: 不可变内核
- **TOOLS**: 动态技能
- **USER**: 语义长期记忆
- **Session**: 实时情景
- **Daily Logs**: 上下文连续性
```

### 4.2 增强版技能格式

```markdown
# agent-workspace/skills/research_assistant.md

---
name: research_assistant
description: 深度研究助手，支持多步骤调研
type: openclaw-enhanced
version: 2.0.0

# Crablet 引擎配置
engine:
  cognitive_level: system3  # 使用 System 3 深度分析
  max_iterations: 10
  enable_swarm: true
  
# 记忆集成
memory:
  use_episodic: true
  use_semantic: true
  consolidate_after: true
  
# 工具链
tools:
  - web_search
  - document_reader
  - knowledge_graph
  - summarizer
  
# 安全设置
safety:
  sandbox: true
  max_network_calls: 50
  allowed_domains:
    - "*.edu"
    - "*.gov"
    - "wikipedia.org"
    
# 工作流定义
workflow:
  steps:
    - name: "理解需求"
      action: "analyze_intent"
      
    - name: "制定计划"
      action: "create_research_plan"
      
    - name: "并行搜索"
      action: "parallel_search"
      parallel: true
      tools: ["web_search", "academic_search", "news_search"]
      
    - name: "综合分析"
      action: "synthesize_findings"
      
    - name: "生成报告"
      action: "generate_report"
      output_format: markdown
---

# 研究助手

你是一个专业的研究助手，擅长深度调研和综合分析。

## 工作流程
1. 首先理解用户的研究需求
2. 制定详细的研究计划
3. 并行搜索多个信息源
4. 综合分析收集到的信息
5. 生成结构化的研究报告

## 输出格式
- 执行摘要
- 关键发现
- 详细分析
- 参考来源
- 建议行动

## 记忆集成
- 记住用户的研究偏好
- 记录重要的研究发现
- 关联相关的历史研究
```

---

## 5. 实施路线图

### Phase 1: 基础架构 (Week 1-2)

**目标**: 建立融合架构的基础

**任务**:
1. 创建 `agent-workspace/` 目录结构
2. 实现 Markdown 配置解析器
3. 创建配置加载器
4. 实现基础同步引擎

**产出**:
- 可读取 Markdown 配置的基础框架
- 配置验证机制
- 向后兼容的适配层

### Phase 2: 记忆融合 (Week 3-4)

**目标**: 实现四层记忆 + Daily Logs

**任务**:
1. 实现 SOUL 层（从 SOUL.md 加载）
2. 增强 TOOLS 层（整合现有 Skills）
3. 实现 USER 层（用户画像系统）
4. 适配 Session 层（现有 Working/Episodic）
5. 实现 Daily Logs 系统

**产出**:
- 完整的四层记忆系统
- Daily Logs 自动记录
- 记忆提取和整合

### Phase 3: 认知增强 (Week 5-6)

**目标**: 增强三层认知架构

**任务**:
1. 增强 System 1（添加 OpenClaw Prompt 匹配）
2. 优化 System 2（整合配置参数）
3. 增强 System 3（Swarm + OpenClaw 技能）
4. 实现自适应认知路由

**产出**:
- 更智能的认知路由
- OpenClaw 技能深度集成
- 自适应响应策略

### Phase 4: 双向增强 (Week 7-8)

**目标**: 让两者相互增强

**任务**:
1. 增强 OpenClaw 技能格式（支持 Crablet 特性）
2. 实现配置双向同步
3. 添加可视化界面
4. 性能优化

**产出**:
- 增强版 OpenClaw 格式
- 实时配置同步
- 管理界面

### Phase 5: 生态建设 (Week 9-10)

**目标**: 建立完整生态

**任务**:
1. 创建技能模板生成器
2. 实现配置迁移工具
3. 编写完整文档
4. 发布 v2.0

**产出**:
- 开发者工具
- 迁移指南
- 完整文档
- 稳定版本

---

## 6. 预期效果

### 6.1 对 Crablet 的提升

| 维度 | 提升前 | 提升后 | 提升幅度 |
|------|--------|--------|----------|
| **配置管理** | 代码内嵌 | Markdown + 热加载 | +200% |
| **可读性** | 一般 | 优秀 | +150% |
| **个性化** | 基础 | 深度 | +100% |
| **上下文连续性** | 会话级 | 跨会话 | +80% |
| **多 Agent 支持** | 代码级 | 配置级 | +120% |

### 6.2 对 OpenClaw 的增强

| 维度 | 增强前 | 增强后 | 增强幅度 |
|------|--------|--------|----------|
| **功能丰富度** | 基础 | 企业级 | +300% |
| **性能** | 一般 | 高性能 | +200% |
| **安全** | 缺失 | 完整 | +∞ |
| **多 Agent** | 缺失 | 支持 | +∞ |
| **认知路由** | 缺失 | 三层 | +∞ |

### 6.3 融合优势

```
融合后的 Crablet OpenClaw Edition 将具备:

✅ OpenClaw 的优雅配置
✅ Crablet 的强大功能
✅ 双向同步能力
✅ 热加载支持
✅ 深度个性化
✅ 企业级安全
✅ 多 Agent 协作
✅ 智能认知路由
```

---

## 7. 下一步行动

### 立即开始 (今天)

1. **创建基础目录结构**
   ```bash
   mkdir -p agent-workspace/{memory,skills,config}
   touch agent-workspace/{AGENTS,SOUL,USER,MEMORY,TOOLS,HEARTBEAT}.md
   ```

2. **初始化配置文件**
   - 从现有代码生成初始 AGENTS.md
   - 创建 SOUL.md 模板
   - 创建 USER.md 模板

3. **启动 Phase 1 开发**
   - 实现 Markdown 解析器
   - 创建配置加载器

### 本周目标

- [ ] 完成基础架构搭建
- [ ] 实现配置读取
- [ ] 建立测试框架
- [ ] 编写开发文档

---

*本文档定义了 Crablet 和 OpenClaw 的融合架构方案，旨在创造 1+1>2 的效果。*  
*最后更新: 2026-03-15*
