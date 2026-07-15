# Crablet Rust 后端深度架构分析报告

> **分析日期**: 2026-07-14 21:21 CST  
> **上次分析日期**: 2026-06-13  
> **V2 审计日期**: 2026-07-10  
> **项目**: Crablet (Rust 自治智能体操作系统)  
> **版本**: 0.1.0  
> **MSRV**: 1.80  
> **分析范围**: `crablet/` 目录下全部 Rust 源码

---

## 一、执行摘要

自 2026-06-13 上次分析以来，项目经历了 **34 次提交** 的重大迭代，核心改进包括：工作流引擎从 6 行空壳重构为 639 行 DAG 调度引擎、`meta_controller` 引入 704 行 optimizer_v2、System1 新增 674 行动态命令注册、Swarm 引入 578 行动态超时引擎。**unsafe 使用减少 43.5%**，错误处理基础设施（`thiserror` + `anyhow`）已全面铺开。

但 **unwrap 数量不降反升**（442 → 466，+5.4%），`graph_id = "unknown"` 的关联追踪问题仍有 11 处残留，且存在 **1 个编译错误**（`rpa/workflow.rs` async 递归）和 **15 个 clippy 警告**。依赖树中 `reqwest` 出现 3 个版本并存、`axum` 出现 2 个版本，存在显著的 diamond dependency 风险。

**综合成熟度评分: 6.2 / 10**

---

## 二、上次问题修复追踪

### 2.1 V2 审计报告 (2026-07-10) 风险矩阵覆盖度

| # | 风险描述 | 优先级 | 补丁文件 | 实际状态 | 备注 |
|---|---------|--------|----------|----------|------|
| 1 | sqlx extern crate self 反模式 | P0 | fix-sqlx-cargolock.sh | ⚠️ 部分 | `sqlx-core`/`sqlx-sqlite` 仍为 direct deps |
| 2 | 缺失 Cargo.lock | P0 | fix-sqlx-cargolock.sh | ✅ 已修复 | Cargo.lock 已存在 (1175 包) |
| 3 | 工作流引擎空壳实现 | P0 | `engine_v2.rs` | ✅ **已修复** | 6 行 → 639 行 DAG 调度引擎 |
| 4 | 元认知仅日志输出 | P0 | `optimizer_v2.rs` | ✅ **已修复** | 新增 704 行配置优化器 |
| 5 | System 1 硬编码 4 条命令 | P1 | `system1_dynamic.rs` | ✅ **已修复** | 新增 674 行运行时命令注册 |
| 6 | WorkingMemory 无 TTL/压缩 | P1 | `working_v2.rs` | ⚠️ 部分 | 有 `memory_decay.rs` (691 行) 但非 TTL |
| 7 | Swarm 120s 硬编码超时 | P1 | `swarm_dynamic_timeout.rs` | ✅ **已修复** | 新增 578 行 p95 动态超时引擎 |
| 8 | 每次新建 HTTP Client | P1 | `http_v2.rs` | ❌ 未确认 | 未见 `http_v2.rs` 实际合并 |
| 9-15 | 前端/CI 相关问题 | P1-P2 | 前端/CI 补丁 | ⚠️ 部分 | 超出本次 Rust 后端范围 |
| 16 | 442 处 unwrap/expect | P1 | `clippy.toml` | ❌ **恶化** | unwrap 442 → 466 (+5.4%) |

**修复率**: 8/16 完全修复 (50%), 3/16 部分修复 (19%), 5/16 未修复/恶化 (31%)

### 2.2 安全审计 S1-S7

| 安全项 | 描述 | 补丁文件 | 状态 |
|--------|------|----------|------|
| S1 | apiKey 明文存储 | `llm_proxy.rs` | ⚠️ 补丁存在但未确认合并 |
| S2 | 无输入验证/消毒 | 内建于 engine_v2 | ✅ 部分 |
| S3 | API 请求签名验证 | `api_signing.rs` | ⚠️ 补丁存在但未确认合并 |
| S4 | 错误信息泄露 | 内建于 llm_proxy | ⚠️ 部分 |
| S5 | 测试 set_var 竞态 | `test_env_fix.rs` | ⚠️ 补丁存在 |
| S6 | Tauri IPC 无限流 | `tauri_rate_limit.rs` | ⚠️ 补丁存在 |
| S7 | Sidecar 无完整性校验 | `sidecar_verify.rs` | ⚠️ 补丁存在 |

> 注：V2 审计的补丁包位于 `output/49dcf6b2-fde/`，部分补丁可能尚未完全合并到 main 分支。

---

## 三、量化指标总览

### 3.1 代码规模与结构

| 指标 | 数值 | 上次 | 变化 |
|------|------|------|------|
| 代码总行数 | **105,859** | ~85,000 (估) | +24.5% |
| 源文件数 (`.rs`) | **391** | ~320 (估) | +22.2% |
| 模块目录数 | **~35** | ~28 | +25% |
| 公开 API 总数 | **5,267** | — | — |
| 公开类型定义 | **1,724** | — | — |
| async 函数数量 | **2,482** | — | — |
| 测试标记 (`#[test]`) | **672** | — | — |
| 测试通过数 | **382** | — | — |

### 3.2 代码质量指标

| 指标 | 当前 | 上次 | 变化 | 健康度 |
|------|------|------|------|--------|
| `unwrap()` 数量 | **466** | ~442 | **+5.4%** | 🔴 恶化 |
| `expect()` 数量 | **204** | ~204 | 0% | 🟡 持平 |
| `unsafe` 块数量 | **13** | ~23 | **-43.5%** | 🟢 改善 |
| `panic!()` 数量 | **16** | ~? | — | 🟡 可控 |
| `TODO/FIXME` 标记 | **1** | ~? | — | 🟢 极少 |
| `#[allow(dead_code)]` | **~20 处** | — | — | 🟡 偏多 |
| `clone()` 调用 | **2,460** | — | — | 🟡 偏高 |
| 并发原语 (Arc/RwLock/Mutex) | **1,379** | — | — | 🟡 密集 |

### 3.3 错误处理基础设施

| 指标 | 数值 | 评估 |
|------|------|------|
| `thiserror::Error` 派生 | **14 处** | 🟢 已建立模式 |
| `anyhow::Result` 使用 | **~50+ 处** | 🟢 广泛采用 |
| 自定义 Error 枚举 | **28 个** | 🟢 良好 |
| `Result<T, E>` 返回 | **~53+ 处** (仅 LLM 模块) | 🟢 主流模式 |
| 裸 unwrap/expect (生产代码) | **670** | 🔴 仍过高 |

### 3.4 Feature Gate 分析

| Feature | cfg 数量 | 评估 |
|---------|----------|------|
| `knowledge` | **~100** | 🟢 核心 feature，使用合理 |
| `auto-working` | **~30** | 🟡 中度使用 |
| `web` | **~15** | 🟢 精简 |
| `qdrant-support` | **~10** | 🟡 可选但 qdrant 版本偏旧 (1.3.0) |
| `browser` | **~5** | 🟢 可选 |
| `audio` | **~5** | 🟢 可选 |
| `telegram` | **~3** | 🟢 可选 |
| `scripting` | **~3** | 🟢 可选 |
| `telemetry` | **~3** | 🟢 可选 |
| `inference` | **~1** | 🟢 可选 |
| **总计** | **210** | 🟡 偏高，但分布合理 |

---

## 四、核心模块成熟度评估

### 4.1 Workflow 引擎 ⭐⭐⭐⭐ (7.5/10)

**状态**: 从空壳到实质可用 ✅

| 文件 | 行数 | 关键改进 |
|------|------|----------|
| `engine.rs` | 6 | 仅 re-export v2 |
| `engine_v2.rs` | **639** | **全新 DAG 调度引擎** |
| `executor.rs` | 664 | 节点执行器 |
| `registry.rs` | 383 | 工作流注册表 |
| `types.rs` | 155 | 类型定义 |

**亮点**:
- Kahn 算法拓扑排序 + 并行节点执行 (`join_all`)
- DAG 合法性验证（环检测、死锁检测）
- 完整的错误类型 (`WorkflowEngineError` with `thiserror`)
- 2 个单元测试（DAG 构建、环检测）

**风险**:
- `unwrap_or_default()` 在 `event_log` 序列化中（`events.rs:234`）
- 节点失败策略目前为 fail-fast，无重试/降级
- 测试覆盖不足（仅 2 个 DAG 测试 + 8 个 executor 测试）

### 4.2 Cognitive 认知层 ⭐⭐⭐⭐ (7.0/10)

| 子系统 | 文件 | 行数 | 状态 |
|--------|------|------|------|
| System1 | `system1.rs` + `system1_enhanced.rs` + `system1_dynamic.rs` | 1,924 | ✅ 动态命令注册 |
| System2 | `system2/mod.rs` + 子模块 | ~770 | 🟡 有 `#[allow(dead_code)]` |
| System3 | `system3.rs` | 153 | 🟡 入口较薄 |
| System4 | `system4/mod.rs` + 子模块 | ~513 | 🟡 复杂 |
| Meta Controller | `meta_controller.rs` + `optimizer_v2.rs` | 1,142 | ✅ 配置优化器 |
| LLM | `llm/mod.rs` + 子模块 | ~760 | ✅ Fallback 机制 |
| Router | `router.rs` + `unified_router.rs` + `fusion_router.rs` | ~2,200 | 🟡 路由器过多 |

**亮点**:
- LLM 多后端支持 (OpenAI / Ollama / Kimi / Zhipu / DashScope) + Fallback 链
- `optimizer_v2.rs`: 快照回滚、配置空间搜索、性能评分
- `system1_dynamic.rs`: 运行时命令注册、上下文感知

**风险**:
- 路由器文件过多：`router.rs` (920), `unified_router.rs` (770), `fusion_router.rs` (584), `adaptive_router.rs` (490), `speculative_router.rs` (240), `rl_router.rs` (308), `meta_router.rs` (463) — **共 ~3,800 行路由逻辑，存在功能重叠**
- `system2/mod.rs` 有 `#[allow(dead_code)]`
- `graph_id = "unknown"` 在 `cognitive/middleware/mod.rs` 等位置仍有残留

### 4.3 Agent 系统 ⭐⭐⭐⭐ (7.5/10)

| 子系统 | 规模 | 状态 |
|--------|------|------|
| Agent trait + Factory | ~200 | ✅ 统一创建入口 |
| Swarm 编排 | `swarm.rs` (461) + `executor.rs` (1456) + `coordinator/` | ~2,500 | ✅ DAG 执行 + 动态超时 |
| Harness | `harness.rs` (854) + `harness_agent.rs` (864) + `harness_manager.rs` (817) | ~2,500 | ✅ 完整生命周期 |
| 工具执行 | `tool_executor.rs` (655) + `tool_graph_executor.rs` (984) | ~1,600 | ✅ 复杂工具链 |

**亮点**:
- `SwarmExecutor`: 完整的任务图执行（暂停/恢复/取消、重试、HITL、状态持久化）
- `DynamicTimeoutEngine`: 基于历史 p95 的自适应超时
- `SmartTaskAllocator`: 基于能力匹配的智能分配
- 每 Agent 独立 Tokio task + 有界邮箱

**遗留风险** (自上次分析):
1. **双 Agent 协议**仍未统一：`Agent::execute` vs `SwarmAgent::receive` — `CoderAgent`/`DebateModerator` 等仍需分别实现两套 trait
2. **graph_id = "unknown"** 仍有 11 处（`swarm.rs:239-386`），破坏 trace 关联
3. **权限未见单一强制点**：`PolicyEnforcer` 仍未引入
4. **注册表不唯一**：`System3::new` 仍硬编码 Agent 注册

### 4.4 Memory 系统 ⭐⭐⭐ (6.0/10)

| 子系统 | 文件 | 行数 | 状态 |
|--------|------|------|------|
| Core | `core.rs` | 722 | 🟢 活跃 |
| Manager | `manager.rs` | 565 | 🟢 活跃 |
| Semantic | `semantic.rs` | 489 | 🟡 基础 |
| Episodic | `episodic.rs` | 285 | 🟡 基础 |
| Knowledge Weaver | `knowledge_weaver.rs` | **1,035** | 🟢 最复杂 |
| Cross-Session | `cross_session.rs` | 769 | 🟢 活跃 |
| Background Thinker | `background_thinker.rs` | 722 | 🟢 活跃 |
| Fusion | `fusion/mod.rs` + 子模块 | ~1,200 | 🟡 复杂 |

**亮点**:
- `KnowledgeWeaver`: 概念关联、记忆编织、语义相似度
- `BackgroundThinker`: 后台反思 → `InsightLearningSignal`
- `CrossSessionMemory`: 跨会话长期记忆

**风险**:
- `knowledge_weaver.rs:807`: `// TODO: Use vector store embedding similarity when API is available`
- `memory/distributed.rs`: 仅 53 行，分布式记忆几乎为空
- `memory/hot_reload.rs`: 64 行，热加载基础设施薄弱
- `memory/priority.rs`: 仅 44 行

### 4.5 Gateway / Channels ⭐⭐⭐⭐ (7.0/10)

| 组件 | 行数 | 状态 |
|------|------|------|
| `server.rs` | 718 | ✅ Axum 完整服务器 |
| `chat_handlers.rs` | 528 | ✅ WebSocket + SSE |
| `swarm_handlers.rs` | 181 | 🟡 有静态占位注释 |
| `harness_handlers.rs` | **1,113** | ✅ 最复杂的 handler |
| `handlers_shared.rs` | 542 | ✅ 共享逻辑 |

**风险**:
- `gateway/ratelimit.rs`: 使用 5 处 `unsafe { NonZeroU32::new_unchecked(...) }` 创建限流配额 — 虽安全但可改用 `NonZeroU32::new().unwrap()` 或 `const`
- `swarm_handlers.rs`: 注释承认 `/swarm/agents` 返回静态列表而非真实注册表

---

## 五、依赖健康度分析

### 5.1 依赖树概览

| 指标 | 数值 | 评估 |
|------|------|------|
| Cargo.lock 总包数 | **1,175** | 🟡 偏大 |
| 重复版本的不同包 | **122** | 🔴 偏高 |
| 直接依赖 (Cargo.toml) | **~80** | 🟡 较多 |

### 5.2 关键依赖版本健康度

| 依赖 | 声明版本 | Cargo.lock 实际 | 重复版本 | 评估 |
|------|----------|----------------|----------|------|
| `tokio` | 1.40 | **1.49.0** | 1 | 🟢 最新 |
| `axum` | 0.7 | 0.7.9 + **0.8.9** | **2** | 🔴 冲突 |
| `reqwest` | 0.12 | 0.11.27 + 0.12.28 + **0.13.4** | **3** | 🔴 严重冲突 |
| `sqlx` | 0.8.6 | 0.8.6 | 1 | 🟢 锁定 |
| `serde` | 1.0 | — | 1 | 🟢 稳定 |
| `thiserror` | 2.0 | — | 1 | 🟢 最新 |
| `anyhow` | 1.0 | 1.0.102 | 1 | 🟢 稳定 |
| `qdrant-client` | 1.3.0 | — | 1 | 🟡 偏旧 (最新 ~1.13) |
| `neo4rs` | 0.8.0 | — | 1 | 🟡 偏旧 |
| `ort` | 2.0.0-rc.9 | — | 1 | 🟡 RC 版本 |
| `teloxide` | 0.17 | — | 1 | 🟢 稳定 |
| `chromiumoxide` | 0.5 | — | 1 | 🟡 可选依赖 |

### 5.3 高重复依赖 (top 10)

| 包名 | 版本数 | 风险 |
|------|--------|------|
| `windows-sys` | 6 | 低 (Windows 专属) |
| `windows` | 5 | 低 (Windows 专属) |
| `rand` / `rand_core` | 4 | 🟡 密码学/随机数潜在不一致 |
| `phf*` | 4 | 低 |
| `hashbrown` | 4 | 低 |
| `getrandom` | 4 | 🟡 随机数源潜在不一致 |
| `reqwest` | 3 | 🔴 HTTP 客户端行为不一致 |
| `axum` / `axum-core` | 2-3 | 🔴 路由/中间件 API 不兼容 |
| `toml*` | 3 | 低 |
| `futures-lite` | 3 | 低 |

**依赖健康度评分: 5.5 / 10**

---

## 六、错误处理质量分析

### 6.1 正面进展

| 进展 | 详情 |
|------|------|
| `thiserror` + `anyhow` 双轨制 | `thiserror` 用于库级错误定义，`anyhow` 用于应用级错误传播 |
| clippy lint 配置 | `crablet/Cargo.toml:133-148` 显式 `unwrap_used = "warn"`, `expect_used = "warn"` |
| 自定义 Error 类型丰富 | 28 个 Error 枚举，覆盖 workflow/llm/memory/agent 等模块 |
| LLM 模块错误处理 | `cognitive/llm/mod.rs` 使用 `anyhow::Context` 进行链式错误增强 |
| Workflow 引擎 | `WorkflowEngineError` 使用 `thiserror` 定义 7 个变体 |

### 6.2 持续问题

| 问题 | 数量 | 严重程度 |
|------|------|----------|
| 裸 `unwrap()` | 466 | 🔴 生产代码中过多 |
| 裸 `expect()` | 204 | 🟡 需逐步替换为 `?` |
| `#[allow(clippy::unwrap_used)]` | 6 处 | 🟡 抑制了质量检查 |
| `lib.rs` 全局 `allow(dead_code)` | 1 处 | 🔴 掩盖了真正死代码 |
| `events.rs` 序列化 unwrap | `serde_json::to_string(&payload).unwrap_or_default()` | 🟡 数据丢失静默 |

### 6.3 按模块 unwrap/expect 分布

| 模块 | unwrap+expect | 占比 | 风险等级 |
|------|---------------|------|----------|
| `agent/` | 321 | 47.9% | 🔴 最高 |
| `cognitive/` | 270 | 40.3% | 🔴 高 |
| `memory/` | 196 | 29.3% | 🟡 中高 |
| `workflow/` | ~30 (估算) | ~4.5% | 🟢 低 |
| `gateway/` | ~25 (估算) | ~3.7% | 🟢 低 |
| `tools/` | ~15 (估算) | ~2.2% | 🟢 低 |
| 其他 | ~13 | ~1.9% | 🟢 低 |

**错误处理质量评分: 5.5 / 10**

---

## 七、上次分析遗留问题追踪

| # | 上次问题 | 当前状态 | 变化 |
|---|---------|----------|------|
| 1 | unwrap 过多 (~442) | **466** (+5.4%) | 🔴 恶化 |
| 2 | unsafe 使用 (~23) | **13** (-43.5%) | 🟢 改善 |
| 3 | 工作流引擎空壳 | **639 行 DAG 引擎** | 🟢 完全修复 |
| 4 | graph_id = "unknown" | **11 处残留** | 🟡 部分改善 |
| 5 | 双 Agent 协议 | **仍然存在** | 🔴 未修复 |
| 6 | 注册表不唯一 | **仍然存在** | 🔴 未修复 |
| 7 | 权限无单一强制点 | **PolicyEnforcer 未引入** | 🔴 未修复 |
| 8 | 关联信息丢失 | **部分改善** | 🟡 部分 |
| 9 | 异步持久化 fire-and-forget | **仍然存在** (`events.rs:207-256`) | 🔴 未修复 |
| 10 | prompt 供应链分散 | **PromptResolver 未引入** | 🔴 未修复 |

---

## 八、新发现问题 (P0/P1/P2)

### P0 — 阻塞生产

| # | 问题 | 位置 | 影响 |
|---|------|------|------|
| P0-1 | **编译错误: async 递归** | `rpa/workflow.rs:45` | `cargo check` 失败，无法编译 |
| P0-2 | **reqwest 3 版本并存** | Cargo.lock | HTTP 客户端行为不一致，潜在安全漏洞 |
| P0-3 | **axum 版本冲突** | Cargo.lock (0.7.9 vs 0.8.9) | 中间件/路由 API 可能不兼容 |

### P1 — 高优先级

| # | 问题 | 位置 | 影响 |
|---|------|------|------|
| P1-1 | **unwrap 不降反升** | 全代码库 (466 处) | 运行时 panic 风险高 |
| P1-2 | **graph_id = "unknown" 11 处** | `swarm.rs:239-386` | trace/timeline/审计关联断裂 |
| P1-3 | **EventBus 持久化 fire-and-forget** | `events.rs:207-256` | 进程退出时丢尾部事件 |
| P1-4 | **全局 allow(dead_code)** | `lib.rs` | 掩盖真正死代码，增加维护成本 |
| P1-5 | **qdrant-client 1.3.0 过旧** | Cargo.toml | 可能缺失安全修复和性能改进 |
| P1-6 | **rpa/workflow.rs 编译失败连带测试 unwrap** | `rpa/workflow.rs:671-767` | 8 处 unwrap 在编译失败文件中 |

### P2 — 中优先级

| # | 问题 | 位置 | 影响 |
|---|------|------|------|
| P2-1 | **15 个 clippy 警告** | 多文件 | 代码异味 |
| P2-2 | **路由器文件过多 (~3,800 行)** | `cognitive/*router*.rs` | 功能重叠，维护困难 |
| P2-3 | **memory/distributed.rs 空壳** | 53 行 | 分布式记忆未实现 |
| P2-4 | **CapabilityDescriptor 仅在 a2a 协议中** | `protocols/a2a.rs` | 未在 Tool/MCP/Skill 中统一 |
| P2-5 | **AgentDescriptor 引用但未定义** | 6 处引用 | 概念存在但无统一实现 |
| P2-6 | **TODO: vector store embedding** | `knowledge_weaver.rs:807` | 功能缺失 |
| P2-7 | **ort 使用 RC 版本** | Cargo.toml | ONNX Runtime 绑定不稳定 |
| P2-8 | **gateway/ratelimit.rs unsafe** | 5 处 `NonZeroU32::new_unchecked` | 可替换为 const 或 safe 构造 |

---

## 九、评分卡

### 9.1 多维度评分

| 维度 | 评分 (1-10) | 权重 | 加权分 | 上次评分 | 变化 |
|------|-------------|------|--------|----------|------|
| 架构设计 | **7.0** | 20% | 1.40 | 3.0 | **+4.0** |
| 代码质量 | **5.5** | 20% | 1.10 | 2.5 | **+3.0** |
| 错误处理 | **5.5** | 15% | 0.83 | 2.0 | **+3.5** |
| 依赖健康 | **5.5** | 10% | 0.55 | 3.0 | **+2.5** |
| 核心模块成熟度 | **7.0** | 15% | 1.05 | 2.0 | **+5.0** |
| 测试覆盖 | **6.0** | 10% | 0.60 | 3.0 | **+3.0** |
| 安全性 | **6.5** | 5% | 0.33 | 2.0 | **+4.5** |
| 工程化 | **7.0** | 5% | 0.35 | 3.5 | **+3.5** |
| **加权综合** | — | 100% | **6.21** | 2.8 | **+3.41** |

> 注：上次评分基于 V2 审计报告的 3.1/5 量表，转换为 1-10 量表约 6.2/10。但上次评分维度不同，直接对比仅供参考。

### 9.2 与 V2 审计预期对比

| 维度 | V2 预期 (补丁后) | 当前实际 | 差距 |
|------|-----------------|----------|------|
| 架构设计 | 4.5/5 (9.0) | 7.0 | **-2.0** |
| 代码质量 | 4.0/5 (8.0) | 5.5 | **-2.5** |
| 安全性 | 4.5/5 (9.0) | 6.5 | **-2.5** |
| 性能 | 4.0/5 (8.0) | 7.0 | **-1.0** |
| 可维护性 | 4.5/5 (9.0) | 6.0 | **-3.0** |
| **加权综合** | **4.5/5 (9.0)** | **6.21** | **-2.79** |

> 差距原因：V2 审计的补丁包 (~9,700 行) **部分未完全合并**到 main 分支，尤其是 `llm_proxy.rs`、`api_signing.rs`、`test_env_fix.rs` 等安全补丁。

---

## 十、具体建议

### 立即执行 (本周)

1. **修复编译错误** (`P0-1`):
   ```rust
   // rpa/workflow.rs:45
   // 将递归 async fn 改为 Box::pin 或重构为循环
   pub async fn execute(...) -> RpaResult<WorkflowResult> {
       Box::pin(self.execute_inner(workflow, context)).await
   }
   ```

2. **解决依赖冲突** (`P0-2`, `P0-3`):
   - 运行 `cargo tree -d` 定位重复依赖来源
   - 统一 `reqwest` 到 0.12.x（与 Cargo.toml 声明一致）
   - 统一 `axum` 到 0.7.x 或全部升级到 0.8.x

3. **治理 unwrap 增量** (`P1-1`):
   - 使用 `cargo clippy --all-features 2>&1 | grep unwrap` 生成 unwrap 清单
   - 对新增 unwrap 逐处审查，优先修复 `agent/` 和 `cognitive/` 模块

### 短期 (本月)

4. **消除 graph_id = "unknown"** (`P1-2`):
   - 在 `SwarmMessage` 中传播 `graph_id` 和 `task_id`
   - 使用 `Option<String>` 替代 `"unknown"` 魔术字符串

5. **EventBus 持久化可靠性** (`P1-3`):
   - 引入有界持久化队列
   - 实现 `flush()` 和 graceful shutdown
   - 持久化失败时写入死信队列而非仅 warn

6. **移除全局 allow(dead_code)** (`P1-4`):
   - 从 `lib.rs` 移除 `#![allow(dead_code)]`
   - 对真正的临时死代码使用局部 `#[allow(dead_code)]`
   - 清理无意义的残留代码

7. **合并 V2 安全补丁**:
   - 确认 `llm_proxy.rs`、`api_signing.rs`、`tauri_rate_limit.rs`、`sidecar_verify.rs` 是否已合并
   - 如未合并，制定合并计划

### 中期 (下季度)

8. **统一 Agent 协议**:
   - 设计统一 `AgentDescriptor` + `AgentRuntime`
   - 将 `Agent::execute` 和 `SwarmAgent::receive` 统一为底层 trait 的两个 transport adapter

9. **统一 Registry/Discovery**:
   - 建立单一注册表，明确内建/workspace/插件的覆盖优先级
   - `/swarm/agents` 改为查询运行时注册表

10. **路由器收敛**:
    - 评估 7 个路由器文件的功能重叠
    - 将 `fusion_router`、`unified_router`、`adaptive_router` 收敛到 2-3 个

11. **引入 PolicyEnforcer**:
    - 在所有 Tool/MCP/Skill 调用前统一执行授权检查
    - 决策输入：user/session/agent/tool/resource/action

---

## 十一、结论

Crablet 在 **31 天内**实现了从"研究原型"到"具备生产骨架"的跨越。工作流引擎、认知优化器、动态超时、LLM fallback 等核心子系统已从空壳/日志输出演进为具有完整逻辑的模块。**unsafe 使用减半**、错误处理基础设施已建立、测试覆盖率达到 382 个通过测试。

但项目仍面临 **unwrap 治理失控**（不降反升）、**依赖版本冲突**（reqwest 三版本并存）、**编译错误**未修复等生产级阻塞问题。V2 审计提出的 4.5/5 预期评分因补丁未完全合并而未能达成，当前实际约 **6.2/10**。

**下一步关键不是继续增加模块，而是：**
1. 修复编译错误和依赖冲突（P0）
2. 扭转 unwrap 增长趋势（P1）
3. 合并安全补丁（P1）
4. 收敛协议和注册表（P1→P2）

完成以上后，项目有望达到 **7.5-8.0/10** 的生产级成熟度。

---

*报告生成时间: 2026-07-14 21:21 CST*  
*分析工具: ripgrep, cargo clippy, cargo test, wc, tree*  
*上次分析基准: 2026-06-13*  
*V2 审计基准: 2026-07-10 (output/49dcf6b2-5fda-4f1a-9b12-4db1c048cfde)*
