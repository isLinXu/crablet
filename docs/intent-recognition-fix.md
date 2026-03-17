# 意图识别与路由修复总结

## 问题分析

### 问题1：闲聊对话走到 System 2

**现象**：用户发送"你好"，显示 System 2 负载 25%

**原因**：
1. Classifier 正确识别为 `Greeting` 意图
2. Router 尝试调用 System 1 处理
3. System 1 的 Trie 匹配逻辑没有正确处理中文问候
4. System 1 返回错误，fallback 到 System 2

### 问题2：缺少意图识别过程

**现象**：思考过程中没有显示意图识别步骤

**原因**：思考过程只显示 traceSteps，缺少意图识别和系统选择的前置步骤

## 修复方案

### 1. 修复 System 1 中文问候匹配

**文件**: `crablet/src/cognitive/system1.rs`

**修改**：添加更多中文问候变体到 aliases
```rust
aliases: vec![
    "hi".to_string(), 
    "hey".to_string(), 
    "你好".to_string(), 
    "您好".to_string(),
    "你好!".to_string(),   // 新增
    "你好！".to_string(),   // 新增
],
```

### 2. 添加意图识别步骤到思考过程

**文件**: `frontend/src/components/chat/MessageBubble.tsx`

**修改**：在构建思考过程时添加意图识别和系统选择步骤
```typescript
// 1. 添加意图识别步骤
steps.push({
  id: 'intent-recognition',
  type: 'intent',
  title: '意图识别',
  content: `识别用户意图: ${intentDescription} (${intentType})`,
  // ...
});

// 2. 添加系统选择步骤
steps.push({
  id: 'system-selection',
  type: 'system',
  title: '系统选择',
  content: `路由到: ${layerNames[currentLayer]}`,
  // ...
});
```

### 3. 添加新的步骤类型

**文件**: `frontend/src/components/chat/EnhancedThinkingVisualization.tsx`

**新增**：
- `intent` 步骤类型
- 对应的图标（Target）
- 对应的标签（意图识别）
- 对应的颜色（fuchsia-400）

## 意图识别逻辑

```typescript
if (text.includes('greeting') || text.includes('你好') || text.includes('hello')) {
  intentType = 'Greeting';
  intentDescription = '问候/打招呼';
} else if (text.includes('code') || text.includes('代码')) {
  intentType = 'Coding';
  intentDescription = '代码相关';
} else if (text.includes('search') || text.includes('检索')) {
  intentType = 'Search';
  intentDescription = '知识检索';
} else if (text.includes('analyze') || text.includes('分析')) {
  intentType = 'Analysis';
  intentDescription = '数据分析';
} else if (text.includes('help') || text.includes('帮助')) {
  intentType = 'Help';
  intentDescription = '寻求帮助';
}
```

## 系统选择逻辑

| 意图类型 | 路由目标 | 原因 |
|---------|---------|------|
| Greeting | System 1 | 简单问候，快速响应 |
| Help | System 1 | 标准帮助信息 |
| Coding | System 2 | 需要深度分析 |
| Search | System 2 | 需要知识检索 |
| Analysis | System 2 | 需要推理分析 |

## 思考过程显示

修复后的思考过程将显示：

1. **意图识别** (intent) - 识别到的用户意图类型
2. **系统选择** (system) - 路由到的认知系统
3. **思考步骤** (reasoning/search/code/...) - 具体的思考过程

## 测试结果

```
✅ 前端编译成功
✅ 后端编译成功
✅ 安装脚本通过
✅ 类型检查通过
```

## 运行方式

```bash
cd /Users/gatilin/PycharmProjects/crablet-latest-v260313
./start.sh
```

现在发送"你好"，应该：
1. 显示意图识别步骤："识别用户意图: 问候/打招呼 (Greeting)"
2. 显示系统选择步骤："路由到: System 1 (快速直觉)"
3. System 1 负载应该有值，System 2 负载为 0
