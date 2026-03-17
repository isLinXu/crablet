# Crablet Fusion 迁移指南

> **版本**: 2.0.0  
> **日期**: 2026-03-15  
> **状态**: 生产就绪

---

## 目录

1. [概述](#概述)
2. [迁移前准备](#迁移前准备)
3. [迁移步骤](#迁移步骤)
4. [配置转换](#配置转换)
5. [API 变更](#api-变更)
6. [故障排除](#故障排除)
7. [回滚方案](#回滚方案)

---

## 概述

### 什么是 Fusion Memory System？

Fusion Memory System 是 Crablet 2.0 引入的全新记忆架构，结合了：

- **OpenClaw 风格配置**: Markdown 文件驱动，人类可读
- **四层记忆架构**: SOUL/TOOLS/USER/Session 分层管理
- **Daily Logs**: 跨会话连续性
- **Memory Weaver**: 自动记忆提取和整合

### 为什么要迁移？

| 特性 | 旧系统 | Fusion 系统 |
|------|--------|-------------|
| 配置管理 | 代码内嵌 | Markdown 文件 |
| 个性化 | 基础 | 深度用户画像 |
| 上下文连续性 | 会话级 | 跨会话 + Daily Logs |
| 工具系统 | 静态 | 动态热加载 |
| 可扩展性 | 有限 | 模块化设计 |

### 迁移方式

我们提供 **四种迁移模式** 适应不同场景：

1. **LegacyOnly**: 保持现状，不迁移
2. **DualWrite**: 并行运行，逐步切换（推荐）
3. **FusionOnly**: 全新部署
4. **ReadLegacyWriteBoth**: 渐进迁移

---

## 迁移前准备

### 系统要求

- Rust 1.75+
- 磁盘空间: 至少 500MB 可用空间
- 内存: 至少 2GB RAM

### 备份数据

```bash
# 创建完整备份
cp -r /path/to/crablet/data /path/to/crablet/data.backup.$(date +%Y%m%d)

# 或使用脚本
python scripts/migrate_to_fusion.py --source ./data --workspace ./agent-workspace --backup
```

### 检查现有数据

```bash
# 查看 Core Memory
ls -la data/core_memory.json

# 查看 Episodic Memory
ls -la data/episodic/

# 查看 Working Memory
ls -la data/working/
```

---

## 迁移步骤

### 步骤 1: 安装 Crablet 2.0

```bash
# 克隆新版本
git clone https://github.com/crablet/crablet.git
cd crablet
git checkout v2.0.0

# 构建
cargo build --release --features fusion
```

### 步骤 2: 运行自动迁移脚本

```bash
# 预览迁移（干运行）
python scripts/migrate_to_fusion.py \
    --source ./data \
    --workspace ./agent-workspace \
    --dry-run

# 执行实际迁移
python scripts/migrate_to_fusion.py \
    --source ./data \
    --workspace ./agent-workspace \
    --backup
```

### 步骤 3: 验证迁移结果

```bash
# 检查生成的文件
ls -la agent-workspace/
ls -la agent-workspace/memory/

# 查看 SOUL.md
cat agent-workspace/SOUL.md

# 查看 USER.md
cat agent-workspace/USER.md
```

### 步骤 4: 更新应用代码

#### 旧代码 (v1.x)

```rust
use crablet::memory::MemoryManager;
use crablet::cognitive::Router;

let memory_manager = MemoryManager::new(episodic, 100, Duration::from_secs(3600));
let router = Router::new(memory_manager, config);
```

#### 新代码 (v2.0)

```rust
use crablet::memory::fusion::{FusionAdapter, AdapterConfig, MigrationMode};
use crablet::cognitive::{FusionRouter, RouterConfig};

// 创建适配器（DualWrite 模式）
let adapter = FusionAdapter::new(
    fusion_config,
    Some(legacy_manager),  // 保留旧系统
    AdapterConfig {
        migration_mode: MigrationMode::DualWrite,
        fusion_primary: true,
        sync_to_legacy: true,
        ..Default::default()
    }
).await?;

// 创建路由器
let router = FusionRouter::new(
    adapter,
    system1,
    RouterConfig::default()
);
```

### 步骤 5: 启动应用

```bash
# 设置工作区路径
export CRABLET_WORKSPACE=./agent-workspace

# 启动应用
cargo run --release
```

### 步骤 6: 验证功能

```bash
# 检查日志
tail -f logs/crablet.log

# 测试基本功能
curl -X POST http://localhost:8080/chat \
  -H "Content-Type: application/json" \
  -d '{"message": "Hello!"}'
```

---

## 配置转换

### Core Memory → SOUL.md

#### 旧格式 (core_memory.json)

```json
{
  "blocks": {
    "Personality": "Friendly and helpful",
    "Expertise": "Programming and AI"
  }
}
```

#### 新格式 (SOUL.md)

```markdown
---
version: "2.0.0"
---

# Identity

**Name**: Crablet
**Description**: An intelligent AI assistant
**Role**: helpful assistant

## Core Values

- **Friendliness** (Priority: 9)
  - Description: Be friendly and approachable
  - Category: personality

- **Expertise** (Priority: 8)
  - Description: Maintain expertise in programming and AI
  - Category: knowledge

## Immutable Rules

- **Safety**: Never harm humans
  - Reason: Safety is paramount
```

### 会话存储 → Daily Logs

#### 旧格式 (session.json)

```json
{
  "session_id": "abc123",
  "messages": [...],
  "timestamp": "2024-01-01T00:00:00Z"
}
```

#### 新格式 (2024-01-01.md)

```markdown
---
date: 2024-01-01
entry_count: 5
session_count: 2
---

# Daily Log: 2024-01-01

## Sessions

### abc123
- **Started**: 09:00:00
- **Messages**: 10
- **Summary**: User asked about Rust programming

## Events

### 2024-01-01T09:00:00Z
- **Type**: SessionStart
- **Session**: abc123

User: Hello, I want to learn Rust
```

---

## API 变更

### 主要变更

| 旧 API | 新 API | 说明 |
|--------|--------|------|
| `MemoryManager::new(...)` | `FusionAdapter::new(...)` | 使用适配器模式 |
| `memory_manager.get_context(id)` | `adapter.get_context(id).await` | 异步 API |
| `memory_manager.save_message(...)` | `adapter.add_user_message(...).await` | 更明确的命名 |
| `Router::new(...)` | `FusionRouter::new(...)` | 增强型路由器 |

### 代码示例

#### 会话管理

```rust
// 旧代码
let wm = memory_manager.get_or_create_working_memory("session-1", None).await;
wm.add_message("user", "Hello");

// 新代码
let session = adapter.get_or_create_session("session-1").await?;
adapter.add_user_message("session-1", "Hello").await?;
```

#### 上下文获取

```rust
// 旧代码
let context = memory_manager.get_context("session-1").await;

// 新代码
let context = adapter.get_context("session-1").await?;
let enriched_prompt = adapter.get_enriched_system_prompt("session-1").await?;
```

#### 工具调用

```rust
// 新功能 - 旧系统不支持
let tools = adapter.tools().list_tools();
let result = adapter.invoke_tool("web_search", json!({"query": "Rust"})).await?;
```

---

## 故障排除

### 问题 1: 配置文件加载失败

**症状**:
```
Error: Failed to load SOUL.md: missing required field
```

**解决方案**:
```bash
# 检查配置文件格式
cat agent-workspace/SOUL.md

# 重新生成默认配置
python scripts/init_fusion.sh agent-workspace
```

### 问题 2: 会话无法创建

**症状**:
```
Error: Session creation failed: IO error
```

**解决方案**:
```bash
# 检查目录权限
ls -la agent-workspace/
chmod 755 agent-workspace/memory

# 检查磁盘空间
df -h
```

### 问题 3: 记忆无法提取

**症状**:
记忆没有被记录到 USER 层

**解决方案**:
```rust
// 确保启用了记忆提取
let config = RouterConfig {
    enable_memory_extraction: true,
    ..Default::default()
};

// 手动检查记忆
let memories = adapter.search_memories(10).await?;
println!("Found {} memories", memories.len());
```

### 问题 4: 性能下降

**症状**:
响应时间明显变慢

**解决方案**:
```rust
// 启用缓存
let config = AdapterConfig {
    enable_caching: true,
    cache_size: 1000,
    ..Default::default()
};

// 调整压缩阈值
let session_config = SessionConfig {
    compression_threshold: 0.9,  // 提高阈值
    ..Default::default()
};
```

### 问题 5: 数据不一致

**症状**:
DualWrite 模式下数据不一致

**解决方案**:
```bash
# 强制同步
python scripts/migrate_to_fusion.py \
    --source ./data \
    --workspace ./agent-workspace \
    --sync-only

# 或者切换到 FusionOnly 模式
```

---

## 回滚方案

如果需要回滚到旧版本：

### 步骤 1: 停止应用

```bash
# 停止 Crablet
pkill crablet
```

### 步骤 2: 恢复数据

```bash
# 从备份恢复
cp -r data.backup.20240315/* data/

# 或者使用 git
git checkout v1.x
```

### 步骤 3: 回滚代码

```bash
# 切换回旧版本
git checkout v1.x

# 重新构建
cargo build --release
```

### 步骤 4: 启动旧版本

```bash
# 启动旧版本
./target/release/crablet
```

### 数据恢复

如果使用了 DualWrite 模式，数据会自动同步回旧系统。如果使用了 FusionOnly 模式，需要手动导出：

```bash
# 导出 Fusion 数据
python scripts/export_from_fusion.py \
    --workspace ./agent-workspace \
    --output ./data-recovery

# 转换回旧格式
python scripts/convert_to_legacy.py \
    --input ./data-recovery \
    --output ./data
```

---

## 最佳实践

### 1. 渐进迁移

```rust
// 第 1 周: DualWrite 模式
let config = AdapterConfig {
    migration_mode: MigrationMode::DualWrite,
    fusion_primary: false,  // 旧系统为主
    ..Default::default()
};

// 第 2 周: 切换到 Fusion 为主
let config = AdapterConfig {
    migration_mode: MigrationMode::DualWrite,
    fusion_primary: true,   // Fusion 为主
    ..Default::default()
};

// 第 3 周: FusionOnly 模式
let config = AdapterConfig {
    migration_mode: MigrationMode::FusionOnly,
    ..Default::default()
};
```

### 2. 监控指标

```rust
// 定期检查系统健康
let stats = adapter.stats().await;
println!("Active sessions: {}", stats.mapped_sessions);

// 运行维护
let report = adapter.maintenance().await?;
println!("Consolidated {} memories", report.consolidated_memories);
```

### 3. 配置管理

```bash
# 使用版本控制管理配置
git add agent-workspace/*.md
git commit -m "Update agent configuration"

# 不同环境使用不同配置
cp agent-workspace/SOUL.md agent-workspace/SOUL.md.production
```

---

## 获取帮助

### 文档

- [API 文档](https://docs.crablet.io/api)
- [架构设计](ARCHITECTURE.md)
- [示例代码](examples/)

### 社区

- GitHub Issues: https://github.com/crablet/crablet/issues
- Discord: https://discord.gg/crablet
- 邮件: support@crablet.io

### 商业支持

如需商业支持，请联系: enterprise@crablet.io

---

## 总结

迁移到 Crablet Fusion 带来：

✅ **更好的配置管理**: Markdown 文件，版本控制友好  
✅ **更强的个性化**: 深度用户画像，跨会话记忆  
✅ **更灵活的工具系统**: 动态加载，权限管理  
✅ **更好的可观测性**: Daily Logs，完整审计追踪  

按照本指南，您可以平滑地完成迁移。如有问题，请参考故障排除章节或联系社区支持。

**祝迁移顺利！** 🦀
