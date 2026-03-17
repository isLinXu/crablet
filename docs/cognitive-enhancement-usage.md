# Crablet 认知增强功能使用指南

本文档介绍如何使用 Crablet 框架新增的认知增强功能，包括思维链可视化、实时思考流、智能建议、思考干预、多模态展示、分析统计和分享协作等功能。

## 功能概览

### 1. 思维链可视化图谱 (ThoughtGraphViewer)

使用 D3.js 实现的可交互思维图谱，支持：
- 树形/图形布局展示思考路径
- 节点类型区分（推理、工具调用、观察、决策等）
- 置信度可视化
- 分支路径展示
- 迷你地图导航
- Mermaid 导出

**使用示例：**

```tsx
import { ThoughtGraphViewer } from './components/cognitive';

const graph = {
  id: 'graph-1',
  root_id: 'node-1',
  nodes: {
    'node-1': {
      id: 'node-1',
      node_type: 'reasoning',
      status: 'completed',
      content: '分析代码结构',
      parent_ids: [],
      child_ids: ['node-2'],
      depth: 0,
      confidence: 0.95,
    },
    // ... 更多节点
  },
  edges: [
    { id: 'edge-1', source: 'node-1', target: 'node-2', edge_type: 'sequential', weight: 1 },
  ],
  active_node_id: 'node-2',
  created_at: Date.now(),
  updated_at: Date.now(),
};

<ThoughtGraphViewer 
  graph={graph}
  onNodeClick={(node) => console.log('Clicked:', node.id)}
  showMiniMap={true}
  showStats={true}
/>
```

### 2. 实时思考流 (ThinkingStream)

模拟打字机效果的思考过程展示：
- Token 级别的实时显示
- 思考阶段指示器
- 质量指标面板（token/s、置信度、回溯次数等）
- 暂停/继续控制
- 速度调节

**使用示例：**

```tsx
import { ThinkingStream, ThinkingPhase } from './components/cognitive';

const tokens = [
  { id: '1', text: '首先', type: 'thought', timestamp: Date.now(), isComplete: true },
  { id: '2', text: '分析', type: 'thought', timestamp: Date.now(), isComplete: true },
  // ...
];

const metrics = {
  tokensPerSecond: 15.5,
  averageConfidence: 0.85,
  backtrackCount: 2,
  toolHitRate: 0.92,
  reasoningDepth: 4,
  coherenceScore: 0.88,
};

<ThinkingStream
  tokens={tokens}
  phase="reasoning"
  metrics={metrics}
  isPaused={false}
  onPause={() => {}}
  onResume={() => {}}
  onSpeedChange={(speed) => console.log('Speed:', speed)}
/>
```

### 3. 智能建议 (SmartSuggestions)

基于上下文的智能提示系统：
- 分类快捷操作（代码、分析、创意、工具）
- 智能建议生成
- 键盘导航支持
- 置信度指示

**使用示例：**

```tsx
import { SmartSuggestions } from './components/cognitive';

<SmartSuggestions
  context={currentInput}
  conversationHistory={messages}
  onSuggestionClick={(suggestion) => handleSuggestion(suggestion)}
  onQuickActionClick={(action) => handleAction(action)}
  maxSuggestions={4}
/>
```

### 4. 思考干预 (ThinkingIntervention)

允许用户在思考过程中进行干预：
- 纠正错误
- 提供引导方向
- 分支选择
- 工具调用确认
- 回溯历史

**使用示例：**

```tsx
import { ThinkingIntervention } from './components/cognitive';

<ThinkingIntervention
  isActive={true}
  currentNodeId="node-3"
  branchOptions={[
    { id: '1', label: '方案A', description: '使用递归', confidence: 0.8 },
    { id: '2', label: '方案B', description: '使用迭代', confidence: 0.75 },
  ]}
  onIntervene={(intervention) => handleIntervention(intervention)}
  onConfirmTool={(name, params, confirmed) => handleToolConfirm(name, params, confirmed)}
/>
```

### 5. 多模态思考展示 (MultimodalThinking)

支持多种内容格式的思考展示：
- 代码块（支持语法高亮、行号、折叠）
- Diff 视图
- Mermaid 图表
- 数据表格
- Markdown 渲染
- JSON/SHELL 展示

**使用示例：**

```tsx
import { MultimodalThinking } from './components/cognitive';

const steps = [
  {
    id: 'step-1',
    title: '代码分析',
    description: '分析代码结构',
    blocks: [
      {
        id: 'block-1',
        type: 'code',
        content: 'fn main() { println!("Hello"); }',
        language: 'rust',
      },
      {
        id: 'block-2',
        type: 'mermaid',
        content: 'graph TD; A-->B;',
      },
    ],
    timestamp: Date.now(),
    status: 'completed',
  },
];

<MultimodalThinking
  steps={steps}
  onStepClick={(step) => console.log('Step:', step.id)}
  onBlockAction={(action, block) => handleBlockAction(action, block)}
  enableCodeExecution={true}
  enableMermaidRender={true}
/>
```

### 6. 分析统计 (ThinkingAnalytics)

思考过程的统计分析和可视化：
- 概览统计卡片
- 能力雷达图
- 复杂度分布
- 状态分布
- 时间线趋势
- 会话对比

**使用示例：**

```tsx
import { ThinkingAnalytics } from './components/cognitive';

const sessions = [
  {
    id: 'session-1',
    timestamp: Date.now(),
    duration: 5000,
    tokenCount: 150,
    stepCount: 5,
    toolCalls: 2,
    backtracks: 1,
    confidence: 0.9,
    coherence: 0.85,
    efficiency: 0.88,
    complexity: 'medium',
    status: 'success',
    tags: ['code-review'],
  },
  // ... 更多会话
];

<ThinkingAnalytics
  sessions={sessions}
  onSessionSelect={(id) => console.log('Selected:', id)}
  onExport={(format) => exportData(format)}
/>
```

### 7. 分享协作 (ThinkingShare)

思考过程的分享和协作功能：
- 生成分享链接
- 权限控制（公开/私有）
- 过期时间设置
- 批注系统
- 多种格式导出
- 分享历史管理

**使用示例：**

```tsx
import { ThinkingShare } from './components/cognitive';

<ThinkingShare
  thinkingId="thinking-1"
  thinkingTitle="代码审查分析"
  annotations={annotations}
  shareRecords={shareRecords}
  onShare={async (options) => createShareLink(options)}
  onAddAnnotation={(annotation) => addAnnotation(annotation)}
  onReplyAnnotation={(parentId, reply) => addReply(parentId, reply)}
  onExport={async (format) => exportThinking(format)}
  currentUser="user@example.com"
/>
```

### 8. 综合面板 (CognitiveEnhancementPanel)

整合所有功能的综合面板：

```tsx
import { CognitiveEnhancementPanel } from './components/cognitive';

<CognitiveEnhancementPanel
  thoughtGraph={graph}
  thinkingTokens={tokens}
  thinkingPhase="reasoning"
  thinkingMetrics={metrics}
  suggestions={suggestions}
  thinkingSteps={steps}
  sessions={sessions}
  annotations={annotations}
  shareRecords={shareRecords}
  cognitiveLoad={{ system1: 80, system2: 60, system3: 20 }}
  // ... 回调函数
/>
```

## 后端数据结构

### Rust 思维图谱结构

```rust
use crablet::cognitive::thought_graph::*;

// 创建思维图谱
let mut graph = ThoughtGraph::new("开始任务");

// 添加节点
let node1 = ThoughtNode::new(ThoughtNodeType::Reasoning, "分析需求")
    .with_confidence(0.9);
let id1 = graph.add_node(node1);

// 添加工具调用节点
let node2 = ThoughtNode::new(ThoughtNodeType::ToolCall, "搜索文档")
    .with_parent(&id1);
let id2 = graph.add_node(node2);

// 连接节点
graph.connect_sequential(&id1, &id2);

// 导出 Mermaid
let mermaid = graph.to_mermaid();
```

## 集成到现有系统

### 1. 在 Chat 组件中集成

```tsx
// Chat.tsx
import { CognitiveEnhancementPanel } from '../components/cognitive';

function Chat() {
  const [thinkingData, setThinkingData] = useState(/* ... */);
  
  return (
    <div className="chat-container">
      <div className="chat-messages">
        {/* 现有消息列表 */}
      </div>
      
      {/* 新增：认知增强面板 */}
      <div className="cognitive-panel">
        <CognitiveEnhancementPanel
          thoughtGraph={thinkingData.graph}
          thinkingTokens={thinkingData.tokens}
          // ...
        />
      </div>
    </div>
  );
}
```

### 2. 与后端 WebSocket 集成

```typescript
// 接收思考过程更新
ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  
  switch (data.type) {
    case 'thought_node':
      updateGraph(data.node);
      break;
    case 'thinking_token':
      addToken(data.token);
      break;
    case 'intervention_request':
      showIntervention(data);
      break;
  }
};
```

## 自定义主题

所有组件支持通过 CSS 变量自定义主题：

```css
:root {
  --cognitive-bg-primary: #0f172a;
  --cognitive-bg-secondary: #1e293b;
  --cognitive-border: #334155;
  --cognitive-text-primary: #f8fafc;
  --cognitive-text-secondary: #94a3b8;
  --cognitive-accent: #3b82f6;
}
```

## 性能优化建议

1. **大数据量处理**：使用虚拟滚动处理大量思考步骤
2. **图表渲染**：使用 Canvas 替代 SVG 处理复杂图谱
3. **实时更新**：使用 requestAnimationFrame 节流高频更新
4. **内存管理**：及时清理不再使用的图谱数据

## 故障排除

### 常见问题

1. **图表不显示**：检查 D3.js 是否正确安装
2. **类型错误**：确保使用类型导入 `import type`
3. **样式丢失**：确认 CSS 文件已正确导入

### 调试技巧

```typescript
// 启用调试日志
localStorage.setItem('cognitive-debug', 'true');

// 检查数据结构
console.log('Graph:', JSON.stringify(graph, null, 2));
```

## 贡献指南

欢迎为认知增强功能贡献代码！请遵循以下规范：

1. 所有组件需包含 TypeScript 类型定义
2. 添加完整的 JSDoc 注释
3. 提供使用示例
4. 确保通过所有测试
5. 更新本文档
