# Crablet 可观测性 API 文档

## 概述

Crablet 可观测性系统提供完整的 Agent 执行追踪、调试和监控能力。通过 ObservableReActEngine 替换标准的 ReActEngine，您可以获得详细的执行轨迹、设置智能断点、实时监控 Agent 行为。

## 核心组件

### 1. ObservableReActEngine

增强版 ReAct 引擎，集成完整的追踪和断点功能。

```rust
use crablet::cognitive::react_observable::ObservableReActEngine;
use crablet::observability::{ObservabilityManager, InMemoryStorage};

// 创建可观测性管理器
let storage = Arc::new(InMemoryStorage::new());
let observability = ObservabilityManager::new(storage);

// 创建 ObservableReActEngine
let engine = ObservableReActEngine::new(
    llm,
    skills,
    event_bus,
    observability.tracer(),
    observability.breakpoint_manager(),
);

// 执行并追踪
let execution_id = "exec-001".to_string();
observability.start_session(execution_id.clone(), "workflow-001".to_string()).await;
let (response, traces) = engine.execute(&execution_id, &context, max_steps).await?;
```

### 2. 断点系统

支持多种断点条件，允许在关键执行点暂停并介入。

#### 断点类型

```rust
use crablet::observability::{Breakpoint, BreakpointCondition, BreakpointAction};

// 1. 迭代次数断点
let bp = Breakpoint::new(BreakpointCondition::AfterIteration { count: 5 })
    .with_name("After 5 steps")
    .with_action(BreakpointAction::Pause);

// 2. 思考内容断点
let bp = Breakpoint::new(BreakpointCondition::ThoughtContains { 
    text: "error".to_string() 
})
.with_name("Error detected")
.with_action(BreakpointAction::InjectHint { 
    hint: "请检查错误".to_string() 
});

// 3. 工具调用断点
let bp = Breakpoint::new(BreakpointCondition::BeforeToolCall { 
    tool_pattern: Some("search.*".to_string()) 
})
.with_name("Before search")
.with_action(BreakpointAction::Continue);

// 4. 低置信度断点
let bp = Breakpoint::new(BreakpointCondition::LowConfidence { threshold: 0.5 })
    .with_name("Low confidence")
    .with_action(BreakpointAction::Pause);

// 5. 循环检测断点
let bp = Breakpoint::new(BreakpointCondition::LoopDetected)
    .with_name("Loop detected")
    .with_action(BreakpointAction::Abort { 
        reason: "Detected infinite loop".to_string() 
    });

// 6. 复合条件断点
let bp = Breakpoint::new(BreakpointCondition::All(vec![
    BreakpointCondition::AfterIteration { count: 3 },
    BreakpointCondition::ThoughtContains { text: "important".to_string() },
]))
.with_name("Compound condition")
.with_action(BreakpointAction::ModifyContext { 
    variable_updates: HashMap::new() 
});
```

#### 断点操作

```rust
// 暂停等待人工介入
BreakpointAction::Pause

// 自动继续
BreakpointAction::Continue

// 注入提示信息
BreakpointAction::InjectHint { hint: String }

// 修改变量上下文
BreakpointAction::ModifyContext { variable_updates: HashMap }

// 跳过当前步骤
BreakpointAction::Skip

// 中止执行
BreakpointAction::Abort { reason: String }

// 使用修改后的参数重试
BreakpointAction::RetryWithParams { params: serde_json::Value }
```

### 3. WebSocket 实时事件

通过 WebSocket 接收实时执行事件。

#### 连接

```javascript
const ws = new WebSocket('ws://localhost:8080/ws/observability');

// 或指定特定执行
const ws = new WebSocket('ws://localhost:8080/ws/observability?execution_id=exec-001');
```

#### 事件类型

```typescript
interface ObservabilityEvent {
  event_type: 'session_started' | 'session_completed' | 'span_recorded' | 
              'breakpoint_hit' | 'execution_paused' | 'execution_resumed' | 'error';
  execution_id: string;
  timestamp: number;
  // ... 其他字段根据事件类型变化
}
```

#### 客户端示例

```typescript
import { useWebSocket } from './hooks/useWebSocket';

function TraceViewer() {
  const { lastMessage, sendMessage, connectionStatus } = useWebSocket(
    'ws://localhost:8080/ws/observability'
  );

  useEffect(() => {
    if (lastMessage) {
      const event = JSON.parse(lastMessage.data);
      
      switch (event.event_type) {
        case 'session_started':
          console.log('Session started:', event.execution_id);
          break;
        case 'span_recorded':
          console.log('New span:', event.span);
          break;
        case 'execution_paused':
          // 显示暂停 UI，等待用户操作
          showPauseDialog(event.execution_id);
          break;
        case 'execution_resumed':
          console.log('Execution resumed with action:', event.action);
          break;
      }
    }
  }, [lastMessage]);

  // 恢复执行
  const handleResume = (action: string) => {
    sendMessage(JSON.stringify({
      type: 'resume_execution',
      execution_id: executionId,
      action
    }));
  };
}
```

### 4. 追踪数据查询

```rust
use crablet::observability::{AgentTracer, TraceFilter};

let tracer = observability.tracer();
let tracer_read = tracer.read().await;

// 获取所有跨度
let spans = tracer_read.get_spans(&execution_id).await;

// 获取最近 N 个跨度
let recent = tracer_read.get_recent_spans(&execution_id, 10).await;

// 过滤跨度
let filter = TraceFilter {
    span_types: Some(vec!["thought".to_string(), "action".to_string()]),
    start_time: Some(1234567890),
    end_time: Some(1234567999),
    contains_text: Some("search".to_string()),
};
let filtered = tracer_read.filter_spans(&execution_id, filter).await;
```

## 前端组件

### TraceViewer 组件

```tsx
import { TraceViewer } from './components/observability/TraceViewer';

function App() {
  return (
    <div>
      <TraceViewer 
        executionId="exec-001"  // 可选，不指定则监听所有执行
        autoScroll={true}        // 自动滚动到最新内容
      />
    </div>
  );
}
```

### 组件功能

- **实时追踪**: 通过 WebSocket 实时显示执行步骤
- **步骤过滤**: 按类型过滤（思考、动作、观察、反射、错误）
- **断点控制**: 当执行暂停时显示操作按钮（继续、跳过、中止）
- **详细信息**: 点击步骤查看完整详情
- **执行状态**: 显示当前执行状态（运行中、已暂停、已完成、失败）

## 集成指南

### 步骤 1: 替换 ReActEngine

在 `system2/mod.rs` 中：

```rust
// 旧代码
use crate::cognitive::react::ReActEngine;
// ...
let react_engine = Arc::new(ReActEngine::new(llm, skills, event_bus));

// 新代码
use crate::cognitive::react_observable::ObservableReActEngine;
use crate::observability::{ObservabilityManager, InMemoryStorage};
// ...
let storage = Arc::new(InMemoryStorage::new());
let observability = Arc::new(ObservabilityManager::new(storage));
let react_engine = Arc::new(ObservableReActEngine::new(
    llm, skills, event_bus,
    observability.tracer(),
    observability.breakpoint_manager()
));
```

### 步骤 2: 修改执行调用

```rust
// 旧代码
let result = self.react_engine.execute(&context, max_steps).await;

// 新代码
let execution_id = uuid::Uuid::new_v4().to_string();
self.observability.start_session(execution_id.clone(), workflow_id).await;
let result = self.react_engine.execute(&execution_id, &context, max_steps).await;
```

### 步骤 3: 添加前端路由

在 `App.tsx` 中：

```tsx
import { TraceViewer } from './components/observability/TraceViewer';

<Route path="/observability" element={<TraceViewer />} />
```

### 步骤 4: 添加侧边栏菜单

在 `Sidebar.tsx` 中：

```tsx
import { Eye } from 'lucide-react';

const menuItems = [
  // ... 其他菜单项
  { path: '/observability', icon: Eye, label: 'Observability' },
];
```

## 最佳实践

### 1. 性能考虑

- 使用 `InMemoryStorage` 进行开发和测试
- 生产环境使用 `PersistentStorage` 持久化追踪数据
- 定期清理旧的追踪会话
- 对高频执行使用采样策略

### 2. 断点策略

- 避免在生产环境设置 `Pause` 断点
- 使用 `Continue` 或 `InjectHint` 进行自动干预
- 设置合理的超时时间（默认 5 分钟）
- 使用复合条件精确控制断点触发

### 3. 安全考虑

- 追踪数据可能包含敏感信息，确保适当脱敏
- WebSocket 连接需要身份验证
- 限制追踪数据的访问权限
- 定期审计追踪日志

## 故障排除

### 编译错误

```
error: unused import
```
确保所有导入都被使用，或添加 `#[allow(unused_imports)]`。

### WebSocket 连接失败

检查：
1. 后端服务是否启动
2. WebSocket 端点是否正确配置
3. 防火墙是否允许 WebSocket 连接

### 追踪数据缺失

检查：
1. 是否正确启动追踪会话
2. execution_id 是否一致
3. 存储是否正常工作

## 示例代码

参见 `examples/` 目录：

- `test_breakpoints.rs` - 断点系统测试
- `observability_demo.rs` - 完整可观测性演示

## 相关文档

- [架构设计](./ARCHITECTURE.md)
- [WebSocket API](./WEBSOCKET_API.md)
- [前端组件文档](./FRONTEND_COMPONENTS.md)
