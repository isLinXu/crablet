# Crablet 测试覆盖率提升方案

## 当前状态分析

### P0 创新模块测试状态
| 模块 | 测试数量 | 状态 |
|------|---------|------|
| self_cot_verification | 5 | ✅ 通过 |
| multi_agent | 4 | ✅ 通过 |
| knowledge_graph | 6 | ✅ 通过 |
| verifiable_reasoning | 7 | ✅ 通过 |
| online_learning | 5 | ✅ 通过 |
| **总计** | **27** | **100% 通过** |

### 现有测试文件
- 31 个测试文件
- 分布在 `tests/` 目录

### 当前覆盖率目标
- 目标覆盖率: 80%+
- 关键模块: cognitive, agent, memory, storage

---

## 测试覆盖率提升策略

### 1. 单元测试扩展 (Priority: HIGH)

#### Cognitive 模块测试扩展
```
src/cognitive/
├── system1.rs          → 新增 15+ 测试用例
├── system2.rs         → 新增 20+ 测试用例
├── react.rs           → 新增 10+ 测试用例
├── router.rs          → 新增 12+ 测试用例
├── fusion_router.rs   → 新增 8+ 测试用例
└── meta_controller.rs → 新增 10+ 测试用例
```

#### Agent 模块测试扩展
```
src/agent/
├── memory_pipeline.rs  → 新增 10+ 测试用例
├── dynamic_topology.rs → 新增 8+ 测试用例
└── task_executor.rs    → 新增 12+ 测试用例
```

#### Memory 模块测试扩展
```
src/memory/
├── abstraction.rs      → 新增 15+ 测试用例
├── active_forgetting.rs → 新增 10+ 测试用例
└── storage/           → 新增 12+ 测试用例
```

### 2. 集成测试 (Priority: HIGH)
- API 端点集成测试
- 数据库集成测试
- Redis 缓存集成测试
- LLM 调用集成测试 (mock)

### 3. E2E 测试 (Priority: MEDIUM)
- 完整对话流程测试
- Canvas 编辑流程测试
- 多 Agent 协作流程测试

### 4. CI/CD 覆盖增强 (Priority: HIGH)
- Backend 采用测试矩阵，分别覆盖 `web` 配置、doctest、all-features 编译
- Frontend 在 CI 中固定执行 lint / type-check / build / vitest
- 覆盖率工作流使用 `cargo llvm-cov` + LCOV 80% gate
- 使用统一脚本解析 LCOV，避免 workflow 内重复 shell 逻辑

---

## 测试命名规范

```rust
#[cfg(test)]
mod tests {
    // 单元测试
    #[test]
    fn test_unit_description() { }

    // 集成测试
    #[tokio::test]
    async fn test_integration_description() { }

    // 测试工具函数
    #[test]
    fn test_helper_function() { }
}
```

### 覆盖率计算公式
```
Coverage = (Covered Lines / Total Lines) * 100%
```

---

## 测试数据管理

### Mock 数据
- 使用 `mockall` 库模拟外部依赖
- 提供标准化的测试 fixtures

### 测试数据库
- 使用 SQLite 内存数据库进行测试
- 每个测试独立数据库实例

---

## 执行计划

### Phase 1: 修复现有测试 (已完成 ✅)
- [x] 修复 WorkingMemoryEntry Default
- [x] 修复 MemoryEntry Default
- [x] 修复 MemoryType Default
- [x] 修复测试中的 await 问题
- [x] 修复 metrics 移动问题

### Phase 2: 扩展单元测试 (计划中)
- [ ] cognitive 模块: 50+ 新测试用例
- [ ] agent 模块: 30+ 新测试用例
- [ ] memory 模块: 40+ 新测试用例

### Phase 3: 建立 CI/CD (已完成，并已增强 ✅)
- [x] GitHub Actions 配置
- [x] 覆盖率门禁 (80% gate)
- [x] 自动测试报告
- [x] Backend 测试矩阵
- [x] Frontend 基础质量门禁
- [x] 本地 `Justfile` 复现入口

### Phase 4: 集成/E2E 测试 (计划中)
- [ ] API 集成测试
- [ ] 端到端测试
