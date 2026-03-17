# PDF OCR 改进总结

## 问题分析

用户反馈PDF文件无法提取文本内容，显示"极可能是扫描版或加密PDF"。这是因为：

1. **扫描版PDF**：内容是图片而非文本，普通PDF解析器无法提取
2. **加密PDF**：有密码保护或权限限制
3. **图片PDF**：每页都是图片格式

## 解决方案

### 1. 智能PDF解析策略

实现三级解析策略：

```
Level 1: pdf-parse (文本PDF)
    ↓ 失败或文本无效
Level 2: PDF.js + Tesseract OCR (扫描版PDF)
    ↓ 失败
Level 3: 返回友好错误提示
```

### 2. 文本有效性验证

添加 `validateExtractedText` 函数检查提取的文本：
- 非空检查
- 乱码检测（非打印字符比例）
- 有效字符比例检查

### 3. OCR识别实现

使用技术栈：
- **PDF.js**: 渲染PDF页面为图片
- **Tesseract.js**: 前端OCR识别
- **语言包**: 英文(eng) + 简体中文(chi_sim)

### 4. 用户体验优化

#### 进度反馈
- OCR开始通知 (`onOcrStart`)
- 每页处理进度 (`onProgress`)
- 状态显示："OCR 50%"

#### 视觉标识
- OCR标记：文件列表显示"OCR"标签
- 处理状态：显示"处理中"
- 结果标注：内容中标注"(OCR)"

#### 错误提示
当OCR也失败时，提供友好的错误信息：
```
[PDF 文件: xxx.pdf]
注意：无法提取 PDF 文本内容。该文件可能是扫描版 PDF、加密 PDF 或图片格式。
建议：
1. 使用支持OCR的工具转换后再上传
2. 或者手动复制文本内容粘贴到聊天中
```

## 文件修改

### 新增依赖
```bash
npm install tesseract.js pdfjs-dist
npm install --save-dev @types/tesseract.js
```

### 修改文件

1. **frontend/src/utils/fileContentExtractor.ts**
   - 添加OCR支持
   - 实现三级解析策略
   - 添加进度回调

2. **frontend/src/components/chat/ChatWindow.tsx**
   - 添加OCR状态管理
   - 显示OCR进度
   - 显示OCR标签

## 使用说明

### 对于用户
1. 上传PDF文件后，系统会自动检测类型
2. 如果是扫描版PDF，会自动启动OCR识别
3. 可以在附件列表看到"OCR"标签和处理进度
4. 如果OCR失败，会显示建议信息

### 注意事项
- OCR处理需要时间（每页约1-3秒）
- 最多处理前10页（避免耗时过长）
- 建议上传文本PDF以获得最佳体验

## 性能考虑

1. **限制页数**: 最多处理10页
2. **分辨率**: 使用2倍缩放平衡质量和速度
3. **异步处理**: 不阻塞UI
4. **缓存**: 避免重复OCR

## 后续优化建议

1. **后端OCR**: 对于大文件，考虑使用后端OCR服务
2. **批量处理**: 支持多文件队列处理
3. **预览功能**: 显示OCR前的PDF预览
4. **编辑功能**: 允许用户校正OCR结果
