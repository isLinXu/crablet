# Chat & Canvas 增强设计方案

**项目**: Crablet
**日期**: 2026-03-22
**版本**: v1.0
**状态**: 设计中

---

## 1. 需求概述

### 1.1 目标
增强 Crablet 框架的 Chat 对话和 Canvas 画布功能，实现两条线并行开发，以数据流和状态管理为核心。

### 1.2 用户选择
- **实施策略**: Chat + Canvas 两条线并行
- **技术顺序**: 先完成数据流和状态管理
- **Chat 增强**: 上下文窗口可视化 + RAG 深度检索
- **Canvas 增强**: Chat↔Canvas 双向转换 + 多画布管理
- **存储方案**: SQLite + Redis 混合
- **实时性**: WebSocket 实时同步

---

## 2. 整体架构

```
┌─────────────────────────────────────────────────────────────┐
│                      前端 (React)                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │   Chat UI    │  │  Canvas UI   │  │  协作面板    │      │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘      │
│         │                 │                 │              │
│  ┌──────┴─────────────────┴─────────────────┴───────┐      │
│  │              状态管理层 (Zustand)                  │      │
│  │   • Chat Context Store  • Canvas Store             │      │
│  │   • Collab Store (WebSocket)                      │      │
│  └──────────────────────┬───────────────────────────┘      │
└─────────────────────────┼─────────────────────────────────┘
                          │ WebSocket + REST
┌─────────────────────────┼─────────────────────────────────┐
│                      后端 (Rust)                           │
│  ┌──────────────────────┴───────────────────────────┐    │
│  │              API Gateway                           │    │
│  └──────────────────────┬───────────────────────────┘    │
│         ┌───────────────┴───────────────┐                  │
│  ┌──────┴───────┐              ┌───────┴───────┐         │
│  │ Chat Service │              │Canvas Service │          │
│  └──────┬───────┘              └───────┬───────┘          │
│         │                              │                   │
│  ┌──────┴──────────────────────────────┴───────┐         │
│  │           SQLite + Redis 混合存储             │         │
│  │  热数据(Redis) ←→ 冷数据(SQLite)              │         │
│  └─────────────────────────────────────────────┘         │
└───────────────────────────────────────────────────────────┘
```

### 2.1 核心设计原则
1. **前后端分离**: API 优先设计，数据流驱动
2. **Redis 热数据**: 活跃会话、Canvas 锁、实时协作状态
3. **SQLite 冷数据**: 历史会话、版本快照、模板
4. **WebSocket 实时**: 协作编辑、消息推送

---

## 3. Chat 增强详情

### 3.1 上下文窗口可视化

**功能描述**:
- 实时显示当前会话的 token 使用量
- 可视化展示 System、History、Now 的 token 分布
- 提供多种自动压缩策略

**UI 设计**:
```
┌─────────────────────────────────────────────────────────┐
│ 上下文窗口状态                                           │
│ ┌─────────────────────────────────────────────────────┐ │
│ │░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│ │
│ │ 已用: 32,000 / 128,000 tokens (25%)                  │ │
│ │                                                      │ │
│ │ [System] ──── [History-1] ──── [History-2] ─── [Now] │ │
│ │   2KB           8KB           12KB          10KB     │ │
│ └─────────────────────────────────────────────────────┘ │
│                                                          │
│ 自动压缩策略:                                             │
│ ○ 保留 System + 最近 N 条消息                             │
│ ○ 保留 System + 关键消息（收藏/标记）                      │
│ ○ 手动选择保留范围                                        │
└─────────────────────────────────────────────────────────┘
```

**数据流**:
1. 前端定时计算当前会话 token 使用量（使用 tiktoken）
2. 超阈值前触发压缩建议弹窗
3. 用户选择策略 → 调用压缩 API → 更新会话状态
4. Redis 缓存压缩后的会话摘要

**API**:
- `GET /api/chat/sessions/{id}/token-usage` - 获取 token 使用统计
- `POST /api/chat/sessions/{id}/compress` - 压缩会话上下文

### 3.2 RAG + 历史会话双重检索

**功能描述**:
- 同时检索知识库和历史会话
- 融合评分，结果可点击跳转
- 显示相关性评分

**架构图**:
```
查询流程:
┌──────────┐    ┌──────────────┐    ┌────────────────────┐
│  用户    │───▶│  查询路由     │───▶│  并行检索           │
│  输入    │    │  Router      │    │  • 知识库向量检索   │
└──────────┘    └──────────────┘    │  • 历史会话检索     │
                                     └────────┬─────────┘
                                              │ 融合评分
                                     ┌────────▼─────────┐
                                     │  结果合并        │
                                     │  Score = α·KB +  │
                                     │          β·Hist  │
                                     └────────┬─────────┘
                                              │
                                     ┌────────▼─────────┐
                                     │ 来源展示         │
                                     │ [📄 Doc #23] 0.92│
                                     │ [💬 会话 #5] 0.78│
                                     └──────────────────┘
```

**评分公式**:
```
FinalScore = α × KB_Score + β × History_Score

默认 α = 0.6, β = 0.4，可通过 API 参数调整
```

**API**:
- `GET /api/rag/search?q=&mode=dual&alpha=0.6` - 双重检索

### 3.3 消息收藏与标记

**功能描述**:
- 收藏重要消息便于后续查找
- 支持在会话中快速定位收藏消息
- 收藏消息可导出

---

## 4. Canvas 增强详情

### 4.1 Chat ↔ Canvas 双向转换

**Chat → Canvas**:
- AI 分析自然语言描述的流程
- 自动生成对应的 Canvas 节点和连接
- 支持条件分支的识别和生成

**Canvas → Chat**:
- 选择节点/流程后生成自然语言描述
- 可嵌入 Chat 对话进行进一步分析

**架构图**:
```
Chat → Canvas:
┌─────────────────────────────────────────────────────────┐
│ 用户: "帮我创建一个处理订单的流程"                          │
│                                                          │
│ AI 分析后生成:                                            │
│ ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌───────┐ │
│ │接收订单  │───▶│验证库存  │───▶│处理支付  │───▶│发货   │ │
│ └─────────┘    └────┬────┘    └─────────┘    └───────┘ │
│                     │                                    │
│              ┌──────▼──────┐                             │
│              │ 库存不足?   │                             │
│              └──┬────────┬─┘                             │
│           Yes/ └────────┘\ No                           │
│                ▼             ▼                          │
│          [通知客户]      [继续流程]                       │
└─────────────────────────────────────────────────────────┘

Canvas → Chat:
┌─────────────────────────────────────────────────────────┐
│ 用户选择 Canvas 节点/流程                                 │
│ 系统生成描述: "当前流程: 订单处理 → 包含 4 个节点，       │
│               1 个条件分支"                               │
│                                                          │
│ 可嵌入 Chat 对话:                                         │
│ "请分析这个流程的瓶颈节点并优化"                           │
└─────────────────────────────────────────────────────────┘
```

**API**:
- `POST /api/canvas/chat2canvas` - Chat 转 Canvas
- `POST /api/canvas/canvas2chat` - Canvas 转 Chat 描述

### 4.2 多画布管理

**功能描述**:
- 画布文件夹组织
- 画布搜索（节点内容、连接线、元数据）
- 画布比较和合并

**UI 设计**:
```
┌─────────────────────────────────────────────────────────┐
│ 画布管理面板                                              │
│ ┌─────────────────────────────────────────────────────┐ │
│ │ 🔍 搜索画布: [订单流程]                    [+ 新建] │ │
│ ├─────────────────────────────────────────────────────┤ │
│ │ 📁 我的画布                    │ 📁 共享给我          │ │
│ │  ├ 📄 订单处理流程     v3     │  ├ 📄 周会模板       │ │
│ │  ├ 📄 用户注册流程     v1     │  └ 📄 审批流程       │ │
│ │  └ 📄 客服工单流程     v2     │                      │ │
│ └─────────────────────────────────────────────────────┘ │
│                                                          │
│ 画布搜索能力:                                             │
│ • 节点内容搜索: "库存不足" → 定位到条件节点               │
│ • 连接线搜索: "退货" → 定位到退货处理分支                  │
│ • 元数据搜索: 创建者、创建时间、标签                       │
└─────────────────────────────────────────────────────────┘
```

**API**:
- `GET /api/canvas/search?q=` - 搜索画布
- `GET /api/canvas?folder=` - 按文件夹获取画布列表

### 4.3 版本控制与比较

**功能描述**:
- 完整的版本历史记录
- 版本 diff 可视化对比
- 一键回滚到指定版本

**UI 设计**:
```
┌─────────────────────────────────────────────────────────┐
│ 版本历史                                                  │
│ ┌─────────┬──────────┬────────────────────────────┐   │
│ │ Version │ Changed  │ Summary                     │   │
│ ├─────────┼──────────┼────────────────────────────┤   │
│ │ v3 ★    │ +2 -1    │ 添加库存不足通知节点         │   │
│ │ v2      │ +1 -0    │ 添加支付节点                │   │
│ │ v1      │ initial  │ 初始版本                   │   │
│ └─────────┴──────────┴────────────────────────────┘   │
│                                                          │
│ [对比 v2 vs v3]  [回滚到 v2]  [分享此版本]               │
└─────────────────────────────────────────────────────────┘
```

**API**:
- `GET /api/canvas/{id}/versions` - 版本列表
- `GET /api/canvas/{id}/versions/{v}/diff` - 版本对比
- `POST /api/canvas/{id}/rollback/{v}` - 回滚版本

### 4.4 Canvas 模板市场

**功能描述**:
- 模板发布和分享
- 模板分类和搜索
- 模板使用统计

**API**:
- `GET /api/canvas/templates` - 模板列表
- `POST /api/canvas/templates` - 发布模板
- `POST /api/canvas/templates/{id}/use` - 使用模板

---

## 5. 数据模型

### 5.1 SQLite 表结构

```sql
-- 画布版本表
CREATE TABLE canvas_versions (
    id TEXT PRIMARY KEY,
    canvas_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    snapshot JSON NOT NULL,
    diff JSON,
    summary TEXT,
    created_by TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (canvas_id) REFERENCES canvases(id)
);

-- 画布全文搜索索引
CREATE VIRTUAL TABLE canvas_fts USING fts5(
    canvas_id,
    node_content,
    edge_label,
    metadata
);

-- 消息收藏表
CREATE TABLE message_stars (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    message_id TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (session_id) REFERENCES chat_sessions(id),
    FOREIGN KEY (message_id) REFERENCES chat_messages(id)
);

-- RAG 检索历史
CREATE TABLE rag_search_history (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    query TEXT NOT NULL,
    results JSON NOT NULL,
    scores JSON,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (session_id) REFERENCES chat_sessions(id)
);

-- 画布文件夹
CREATE TABLE canvas_folders (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    parent_id TEXT,
    owner_id TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (parent_id) REFERENCES canvas_folders(id)
);
```

### 5.2 Redis 数据结构

```redis
# 活跃会话上下文 (Hash)
session:{id}:context → {
    token_count: 32000,
    messages: [...],
    compressed: false,
    last_updated: timestamp
}

# Canvas 实时锁 (String with TTL)
canvas:{id}:lock:{user_id} → "locked" EX 30

# 协作状态 (Hash)
canvas:{id}:collab → {
    users: ["user1", "user2"],
    cursors: {...},
    selections: {...}
}

# 模板市场热数据 (Sorted Set)
templates:popular → score: usage_count
templates:recent → score: timestamp

# 画布版本缓存 (String with TTL)
canvas:{id}:version:{v} → JSON snapshot EX 3600
```

---

## 6. API 设计

### 6.1 Chat 增强 API

| 方法 | 端点 | 描述 |
|------|------|------|
| GET | `/api/chat/sessions/{id}/token-usage` | 获取 token 使用统计 |
| POST | `/api/chat/sessions/{id}/compress` | 压缩会话上下文 |
| POST | `/api/chat/sessions/{id}/stars` | 收藏消息 |
| DELETE | `/api/chat/sessions/{id}/stars/{message_id}` | 取消收藏 |
| GET | `/api/chat/sessions/search?q=` | 搜索会话 |
| GET | `/api/rag/search?q=&mode=dual` | 双重检索 |

### 6.2 Canvas 增强 API

| 方法 | 端点 | 描述 |
|------|------|------|
| GET | `/api/canvas/search?q=` | 搜索画布 |
| GET | `/api/canvas?folder=` | 按文件夹获取画布列表 |
| GET | `/api/canvas/{id}/versions` | 版本列表 |
| GET | `/api/canvas/{id}/versions/{v}/diff` | 版本对比 |
| POST | `/api/canvas/{id}/rollback/{v}` | 回滚版本 |
| POST | `/api/canvas/chat2canvas` | Chat 转 Canvas |
| GET | `/api/canvas/templates` | 模板列表 |
| POST | `/api/canvas/templates` | 发布模板 |
| POST | `/api/canvas/templates/{id}/use` | 使用模板 |

### 6.3 WebSocket API

| 端点 | 描述 |
|------|------|
| `/ws/canvas/{id}/collab` | Canvas 协作编辑实时同步 |
| `/ws/chat/{id}/stream` | 聊天流式响应 |

---

## 7. 错误处理与降级策略

### 7.1 错误处理策略

| 场景 | 处理策略 |
|------|----------|
| Redis 不可用 | 降级到 SQLite，确保核心功能可用 |
| Token 超限 | 阻止发送，强制引导压缩流程 |
| Canvas 版本冲突 | 显示冲突提示，提供手动合并 |
| WebSocket 断连 | 自动重连，重发未确认消息 |
| RAG 检索超时 | 返回部分结果，标记超时来源 |

### 7.2 降级机制

```
Level 0: 完全可用 (Redis + SQLite 都正常)
   └─ 完整功能

Level 1: 降级运行 (Redis 不可用)
   └─ 会话上下文存 SQLite
   └─ 协作功能暂停，提示"协作暂不可用"

Level 2: 基础运行 (严重故障)
   └─ 消息发送可用
   └─ Canvas 只读，提示"编辑暂不可用"
```

---

## 8. 实施计划

### Phase 1: 数据流与状态管理（1-2 周）
- [ ] Redis + SQLite 混合存储层实现
- [ ] 统一的会话上下文管理服务
- [ ] Canvas 状态管理架构
- [ ] WebSocket 基础连接管理

### Phase 2: Chat 增强（2-3 周）
- [ ] 上下文窗口可视化组件
- [ ] TopK 动态调整 API
- [ ] RAG + 历史双重检索
- [ ] 消息收藏功能

### Phase 3: Canvas 增强（2-3 周）
- [ ] Chat ↔ Canvas 双向转换
- [ ] 多画布管理 UI
- [ ] 画布搜索（节点/连接线）
- [ ] 版本控制与比较

### Phase 4: 协作与实时（2 周）
- [ ] WebSocket 实时同步
- [ ] 协作光标与状态
- [ ] Canvas 模板市场

---

## 9. 技术依赖

### 9.1 前端依赖
- `zustand` - 状态管理
- `@reactflow/reactflow` - Canvas 画布
- `tiktoken` - Token 计算
- `diff` - 版本 diff 计算

### 9.2 后端依赖
- `redis` - Redis 客户端
- `rusqlite` - SQLite 支持
- `tokio-tungstenite` - WebSocket
- `tantivy` - 全文搜索

---

## 10. 风险与缓解

| 风险 | 可能性 | 影响 | 缓解措施 |
|------|--------|------|----------|
| Redis 性能瓶颈 | 中 | 高 | 完善的降级策略，连接池优化 |
| WebSocket 扩展性 | 中 | 中 | 设计支持水平扩展 |
| 版本冲突处理 | 低 | 中 | 提供手动合并工具 |
| Token 计算误差 | 低 | 中 | 使用官方 tiktoken 库 |
