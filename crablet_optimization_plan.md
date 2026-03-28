# Crablet 深度优化方案

## 现有模块分析

### 1. Canvas 模块 (已有 - ReactFlow Workflow Canvas)
**现状**: 前端已有完整的基于 ReactFlow 的工作流画布
- 拖拽式节点添加
- 节点类型: Start, End, Condition, Loop, LLM, Agent, Knowledge, Code, Template, HTTP, Variable
- 布局算法: Auto, Hierarchical, Tree
- 导入/导出 JSON
- 模板系统
- 执行面板

**增强方向**:
```
Canvas 增强
├── 节点能力扩展
│   ├── Canvas Node - 画布渲染节点
│   ├── ComputerUse Node - 计算机操作节点
│   └── Custom Node - 自定义节点
├── 视觉增强
│   ├── 节点分组 (Group)
│   ├── 注释 (Annotation)
│   ├── 条件分支颜色
│   └── 执行状态可视化
├── 协作功能
│   ├── 多人实时编辑
│   ├── 版本历史
│   └── 评论系统
└── 后端集成
    ├── Canvas API - 画布状态持久化
    └── WebSocket 实时更新
```

---

## 二、Computer Use 模块 (增强)

### 2.1 当前能力
- ✅ 页面元素提取
- ✅ 状态检测
- ✅ 截图能力

### 2.2 增强方案
```
Enhanced Computer Use
├── VisionModule
│   ├── ScreenCapture - 高性能截图
│   ├── ElementDetector - DOM元素检测
│   ├── OCREngine - 文本识别 (Tesseract)
│   └── LayoutAnalyzer - 布局分析
├── ActionModule
│   ├── SmartClick - 智能点击 (元素定位)
│   ├── FormFilling - 表单自动填充
│   ├── ScrollControl - 滚动控制
│   └── DragDrop - 拖拽操作
├── StateModule
│   ├── PageStateTracker - 页面状态跟踪
│   ├── ChangeDetector - 变化检测
│   └── WaitForCondition - 条件等待
└── FeedbackModule
    ├── ActionResult - 执行结果反馈
    ├── ScreenshotDiff - 截图对比
    └── ErrorRecovery - 错误恢复
```

### 2.3 新增能力
1. **DOM 元素智能定位**
   - CSS 选择器、XPath、文本定位
   - 模糊匹配 (partial text, regex)
   - 相对定位 (near, above, below)

2. **表单智能填充**
   - 自动检测字段类型
   - 历史数据复用
   - 验证码识别接口

3. **视觉状态检测**
   - 页面加载完成检测
   - 动画结束检测
   - 弹窗/对话框检测

---

## 三、GUI 模块 (完善)

### 3.1 系统托盘
```
Tray System
├── TrayIcon - 托盘图标
├── TrayMenu - 右键菜单
├── TrayTooltip - 悬停提示
└── TrayAnimation - 动态图标
```

**实现目标**:
- [ ] Windows: 使用 `windows-rs` 创建系统托盘
- [ ] macOS: 使用 `NSStatusItem`
- [ ] Linux: 使用 `libappindicator`

### 3.2 全局快捷键
```
Global Shortcuts
├── Register/Unregister
├── Conflict Detection
├── Key Sequences
└── Action Mapping
```

**实现目标**:
- [ ] 使用 `rdev` 或 `global-hotkey` crate
- [ ] 快捷键冲突检测
- [ ] 自定义快捷键序列

### 3.3 通知系统
```
Notifications
├── Toast Notifications
├── Action Buttons
├── Sound Support
└── Notification Queue
```

**实现目标**:
- [ ] Windows: `winrt` Toast 通知
- [ ] macOS: `NSUserNotification`
- [ ] 通知点击回调

### 3.4 窗口管理
- 窗口枚举和列表
- 窗口激活/最小化/最大化
- 窗口位置/大小调整
- 前台窗口监控

---

## 四、创新功能

### 4.1 Agentic UI Builder
让 Agent 能够动态构建和修改 UI:
- 根据任务动态生成操作界面
- 运行时组件注入
- 可访问性支持

### 4.2 多模态输入融合
```
Input Fusion
├── 语音输入 → 文字 → 命令
├── 手势识别 → 操作指令
├── 截图标注 → 意图理解
└── 拖拽文件 → 自动处理
```

### 4.3 自适应自动化
- 根据页面结构自动选择最佳操作策略
- 学习用户操作模式
- 异常自动恢复

---

## 五、实施优先级

### Phase 1 (2周)
1. Canvas 模块基础架构
2. Computer Use 增强 - DOM 定位 + 表单填充
3. GUI 系统托盘基础实现

### Phase 2 (2周)
1. Canvas 交互完善 (拖拽/选择/缩放)
2. Computer Use 视觉状态检测
3. GUI 全局快捷键

### Phase 3 (1周)
1. Canvas 动画和导出
2. Computer Use 错误恢复机制
3. GUI 通知系统完善