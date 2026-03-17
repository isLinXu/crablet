# 文件上传和知识库功能修复报告

## 问题描述

用户反馈了两个问题：
1. **上传文件添加到知识库失败** - 显示 "Server Error (501)" 和 "归档失败"
2. **文件上下文丢失** - 上传文件后询问文件内容，助手无法获取文件信息

## 根本原因分析

### 问题1：知识库功能 501 错误

**原因**：后端编译时未启用 `knowledge` feature

在 `crablet/Cargo.toml` 中：
```toml
default = ["qdrant-support", "web", "auto-working"]
knowledge = ["dep:fastembed", "dep:pdf-extract", "dep:neo4rs", "qdrant-support"]
```

默认 features 中不包含 `knowledge`，而 `install.sh` 中使用的是 `cargo build --release`，没有启用 knowledge feature。

当 knowledge feature 未启用时，`upload_knowledge` 处理函数返回 `StatusCode::NOT_IMPLEMENTED` (501)。

### 问题2：文件上下文丢失

**原因**：前端代码只将文件名添加到 prompt，没有读取文件内容

在 `ChatWindow.tsx` 的 `handleSend` 函数中：
```typescript
const attachmentSummary = attachments
  .filter((a) => a.status === 'uploaded')
  .map((a) => `[文件] ${a.file.name}`)  // 只添加了文件名
  .join('\n');
```

这导致模型只能看到文件名，无法获取文件内容进行分析和回答。

## 修复方案

### 修复1：启用 knowledge feature

**修改文件**：`install.sh`

```bash
# 修改前
cargo build --release

# 修改后
cargo build --release --features knowledge
```

### 修复2：添加 EmbeddingService 的 knowledge 实现

**修改文件**：`crablet/src/skills/semantic_search.rs`

添加 knowledge feature 下的 `EmbeddingService` 实现：

```rust
impl EmbeddingService {
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        #[cfg(feature = "knowledge")]
        {
            use fastembed::{TextEmbedding, EmbeddingModel, InitOptions};
            
            let options = InitOptions::new(EmbeddingModel::BGESmallENV15)
                .with_show_download_progress(false);
            
            let mut model = TextEmbedding::try_new(options)?;
            
            let embeddings = model.embed(vec![text], None)?;
            if let Some(embedding) = embeddings.first() {
                Ok(embedding.clone())
            } else {
                Ok(vec![])
            }
        }
        
        #[cfg(not(feature = "knowledge"))]
        {
            let _ = text;
            Ok(vec![])
        }
    }
}
```

### 修复3：读取文件内容并添加到 prompt

**修改文件**：`frontend/src/components/chat/ChatWindow.tsx`

修改 `handleSend` 函数，读取已上传文件的内容：

```typescript
const handleSend = async () => {
  // ...
  
  // 读取已上传文件的内容
  const uploadedAttachments = attachments.filter((a) => a.status === 'uploaded');
  let fileContents = '';
  
  for (const attachment of uploadedAttachments) {
    try {
      const text = await attachment.file.text();
      const truncatedText = text.length > 10000 ? text.slice(0, 10000) + '\n... (内容已截断)' : text;
      fileContents += `\n\n[文件内容: ${attachment.file.name}]\n${truncatedText}`;
    } catch (e) {
      fileContents += `\n\n[文件: ${attachment.file.name}] (无法读取内容)`;
    }
  }
  
  // 构建最终 prompt，包含文件内容
  let finalPrompt = input;
  if (attachmentSummary) {
    finalPrompt += `\n\n[附件列表]\n${attachmentSummary}`;
  }
  if (fileContents) {
    finalPrompt += `\n\n[文件内容]${fileContents}`;
  }
  // ...
};
```

## 修复效果

### 知识库功能
- 文件可以成功上传到知识库
- 支持 PDF、DOC、TXT、MD 等多种格式
- 文件会被自动索引，支持语义搜索

### 文件问答功能
- 上传文件后，文件内容会被读取并添加到 prompt
- 支持最大 10000 字符的内容（超出部分会被截断）
- 可以直接询问文件内容，模型能够基于文件内容回答

## 测试验证

1. 重新运行 `./install.sh` 编译后端（启用 knowledge feature）
2. 启动服务 `./start.sh`
3. 上传文本文件
4. 点击"添加到知识库"按钮，应该显示成功
5. 询问文件内容，模型应该能够正确回答

## 注意事项

1. 首次启用 knowledge feature 编译时，需要下载 fastembed 模型，可能需要较长时间
2. 大文件（>10MB）可能需要更长的处理时间
3. 二进制文件（如图片、视频）无法直接读取文本内容
