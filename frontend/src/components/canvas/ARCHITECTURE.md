# Canvas 画布架构优化方案

## 一、现有架构分析

### 当前组件
```
Canvas.tsx (主组件 - 900行)
├── NodeTypePanel (节点类型面板)
├── NodeConfigPanel (节点配置面板)
├── ExecutionPanel (执行面板)
├── TemplatePanel (模板面板)
├── ReactFlow 核心
│   ├── nodes (节点状态)
│   ├── edges (连接状态)
│   ├── onNodesChange
│   ├── onEdgesChange
│   └── onConnect
```

### 现有能力
- ✅ 节点拖拽添加
- ✅ 连接线创建
- ✅ 节点配置面板
- ✅ 基础布局算法 (Auto/Hierarchical/Tree)
- ✅ 状态显示 (running/completed/failed)

### 需要增强
- ❌ 复制/粘贴/剪切
- ❌ 撤销/重做
- ❌ 多选和批量操作
- ❌ 连接线自动路由
- ❌ 版本历史
- ❌ 实时状态监控

---

## 二、架构优化设计

### 2.1 新架构

```
src/
├── components/canvas/
│   ├── Canvas.tsx              # 主画布 (简化)
│   ├── CanvasToolbar.tsx       # 工具栏
│   ├── NodeTypePanel.tsx      # 节点面板 (保留)
│   ├── NodeConfigPanel.tsx    # 配置面板 (保留)
│   ├── ExecutionPanel.tsx     # 执行面板 (保留)
│   ├── TemplatePanel.tsx      # 模板面板 (保留)
│   │
│   ├── hooks/                 # 新增 Hooks
│   │   ├── useCanvasState.ts      # 画布状态管理
│   │   ├── useHistory.ts          # 撤销/重做
│   │   ├── useClipboard.ts       # 复制/粘贴
│   │   ├── useSelection.ts        # 多选
│   │   ├── useAutoLayout.ts       # 自动布局
│   │   └── useEdgeRouting.ts      # 连接线路由
│   │
│   ├── components/            # 新增组件
│   │   ├── SelectionBox.tsx       # 框选组件
│   │   ├── ContextMenu.tsx        # 右键菜单
│   │   ├── MiniMap.tsx            # 迷你地图
│   │   ├── VersionHistory.tsx     # 版本历史
│   │   └── EdgeLabel.tsx          # 连接线标签
│   │
│   └── nodes/                 # 节点类型 (保留)
│       └── ...
│
├── store/                     # 状态管理
│   ├── canvasStore.ts         # 基础画布状态
│   ├── canvasEnhanceStore.ts  # 增强功能状态
│   └── versionStore.ts       # 版本历史
│
└── utils/
    ├── canvasLayout.ts        # 布局算法 (保留)
    ├── edgeRouting.ts        # 连接线路由算法
    └── workflowSerializer.ts # 工作流序列化
```

### 2.2 核心 Hook 设计

```typescript
// useHistory.ts - 撤销重做
interface HistoryState {
  past: CanvasSnapshot[];
  future: CanvasSnapshot[];
  maxSize: number;
}
interface CanvasSnapshot {
  nodes: Node[];
  edges: Edge[];
  timestamp: number;
}

// useClipboard.ts - 复制粘贴
interface ClipboardState {
  nodes: Node[];
  edges: Edge[];
  hasContent: boolean;
}

// useSelection.ts - 多选
interface SelectionState {
  selectedIds: Set<string>;
  isMultiSelect: boolean;
}

// useEdgeRouting.ts - 连接线路由
interface EdgeRouter {
  route(edges: Edge[], nodes: Node[]): Edge[];
  getControlPoints(source: Node, target: Node): Point[];
}
```

---

## 三、功能实现计划

### Phase 1: 核心编辑能力 (已完成 ✅)
1. ✅ 撤销/重做系统 - `hooks/useHistory.ts`
2. ✅ 复制/粘贴/剪切 - `hooks/useClipboard.ts`
3. ✅ 多选和批量操作 - `hooks/useSelection.ts`
4. ✅ 框选功能 - `components/SelectionBox.tsx`

### Phase 2: 连接线增强 (已完成 ✅)
1. ✅ 自动路由算法 - `hooks/useEdgeRouting.ts`
2. ✅ 性能优化 - `hooks/useCanvasPerformance.ts`
3. ✅ 批量更新优化 - `useBatchUpdates`

### Phase 3: 版本历史 (已完成 ✅)
1. ✅ 版本保存/恢复 - `hooks/useVersionHistory.ts`
2. ✅ 版本对比 - `getVersionDiff()`
3. ✅ 版本导入/导出
4. ✅ 版本历史面板 - `components/VersionHistoryPanel.tsx`

### Phase 4: 高级功能 (规划中)
1. 迷你地图
2. 实时协作
3. 模板市场

---

## 四、技术要点

### 4.1 撤销重做
- 使用命令模式 (Command Pattern)
- 快照间隔: 500ms (防抖)
- 最大历史: 50 步
- 存储: 内存 + localStorage

### 4.2 连接线路由
- 基于层级布局的自动路由
- 避免交叉和遮挡
- 支持平滑曲线 (贝塞尔)

### 4.3 性能优化
- Virtualization: 只渲染可视区域节点
- Memoization: React.memo + useMemo
- Debounce: 状态变更防抖
- Web Worker: 大规模计算

### 4.4 状态同步
- WebSocket: 实时状态推送
- Optimistic Update: 先更新再确认
- Conflict Resolution: 最后写入胜出