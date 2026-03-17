# Crablet TOOLS - 动态工具系统

> **动态工具** | **Dynamic Tools**  
> **存储位置**: agent-workspace/skills/  
> **最后更新**: 2026-03-15

---

## 工具层概述

TOOLS 层是四层记忆架构中的第三层，负责管理当前可用的工具和技能。与 SOUL 层的不可变性不同，TOOLS 层是**动态加载**的，可以根据需要随时添加、移除或更新。

```
┌─────────────────────────────────────────────────────────────┐
│  L3: TOOLS (动态工具)                                         │
│  ├── 可用技能列表                                            │
│  ├── 扩展插件                                                │
│  └── API 集成                                                │
└─────────────────────────────────────────────────────────────┘
```

---

## 技能目录结构

```
agent-workspace/skills/
├── README.md                    # 技能索引和说明
├── weather/                     # 天气查询技能
│   ├── skill.yaml              # 技能配置
│   ├── main.wasm               # 编译后的 WebAssembly
│   └── icon.png                # 技能图标
├── calculator/                  # 计算器技能
│   ├── skill.yaml
│   ├── main.wasm
│   └── icon.png
├── web_search/                  # 网页搜索技能
│   ├── skill.yaml
│   ├── main.wasm
│   └── icon.png
└── ...
```

---

## 技能配置格式 (skill.yaml)

```yaml
# 技能元数据
metadata:
  name: "weather"
  version: "1.0.0"
  description: "查询全球天气信息"
  author: "Crablet Team"
  license: "MIT"
  icon: "icon.png"

# 技能能力
capabilities:
  - name: "get_current_weather"
    description: "获取指定城市的当前天气"
    parameters:
      - name: "city"
        type: "string"
        required: true
        description: "城市名称，如：北京、上海"
      - name: "unit"
        type: "string"
        required: false
        default: "celsius"
        enum: ["celsius", "fahrenheit"]
    returns:
      type: "object"
      properties:
        temperature:
          type: "number"
        condition:
          type: "string"
        humidity:
          type: "number"

# 运行时配置
runtime:
  type: "wasm"
  entry: "main.wasm"
  permissions:
    - "network:http://api.weather.com"
    - "filesystem:read"
  resources:
    memory_limit: "64MB"
    timeout: "30s"

# 触发条件
triggers:
  - type: "intent"
    patterns:
      - "查询{city}天气"
      - "{city}今天天气怎么样"
      - "weather in {city}"
```

---

## 内置工具列表

### 1. 文件处理工具

| 工具名 | 功能 | 状态 |
|--------|------|------|
| `file.read` | 读取文件内容 | ✅ 已内置 |
| `file.write` | 写入文件内容 | ✅ 已内置 |
| `file.ocr` | OCR 文字识别 | ✅ 已内置 |
| `file.parse_pdf` | PDF 解析 | ✅ 已内置 |

### 2. 知识检索工具

| 工具名 | 功能 | 状态 |
|--------|------|------|
| `knowledge.search` | 向量语义搜索 | ✅ 已内置 |
| `knowledge.add` | 添加知识文档 | ✅ 已内置 |
| `knowledge.delete` | 删除知识文档 | ✅ 已内置 |

### 3. 外部 API 工具

| 工具名 | 功能 | 状态 |
|--------|------|------|
| `http.get` | HTTP GET 请求 | 🚧 开发中 |
| `http.post` | HTTP POST 请求 | 🚧 开发中 |
| `web.search` | 网页搜索 | 🚧 开发中 |

### 4. 系统工具

| 工具名 | 功能 | 状态 |
|--------|------|------|
| `system.time` | 获取当前时间 | ✅ 已内置 |
| `system.execute` | 执行系统命令（受限） | 🚧 开发中 |

---

## 工具调用示例

### 示例 1: 调用天气查询

```json
{
  "tool": "weather.get_current_weather",
  "parameters": {
    "city": "北京",
    "unit": "celsius"
  }
}
```

**返回结果**:
```json
{
  "success": true,
  "data": {
    "temperature": 22,
    "condition": "晴",
    "humidity": 45
  }
}
```

### 示例 2: 调用文件 OCR

```json
{
  "tool": "file.ocr",
  "parameters": {
    "file_path": "/uploads/document.pdf",
    "language": "chi_sim+eng"
  }
}
```

---

## 工具发现机制

### 1. 启动时扫描

```rust
// 启动时扫描 skills/ 目录
pub fn scan_skills() -> Vec<Skill> {
    let skills_dir = Path::new("agent-workspace/skills");
    let mut skills = Vec::new();
    
    for entry in fs::read_dir(skills_dir).unwrap() {
        let entry = entry.unwrap();
        let skill_yaml = entry.path().join("skill.yaml");
        
        if skill_yaml.exists() {
            let skill = Skill::load(&skill_yaml);
            skills.push(skill);
        }
    }
    
    skills
}
```

### 2. 运行时热加载

```rust
// 支持运行时安装新技能
pub async fn install_skill(&mut self, skill_package: &[u8]) -> Result<(), Error> {
    // 1. 验证技能包
    // 2. 解压到 skills/ 目录
    // 3. 加载 skill.yaml
    // 4. 注册到工具表
    // 5. 无需重启即可使用
}
```

### 3. 工具列表 API

```rust
// 获取当前可用工具列表
pub fn list_available_tools(&self) -> Vec<ToolInfo> {
    self.tools.values()
        .map(|t| t.to_info())
        .collect()
}
```

---

## 工具权限管理

### 权限级别

```rust
pub enum Permission {
    // 网络访问
    Network { host: String, port: Option<u16> },
    // 文件系统
    FileSystem { path: String, access: FileAccess },
    // 系统资源
    System { resource: SystemResource },
    // 敏感操作
    Sensitive { operation: String },
}

pub enum FileAccess {
    Read,
    Write,
    ReadWrite,
}
```

### 权限申请流程

```
技能请求权限 → 系统检查策略 → 用户确认 → 授予/拒绝权限
     ↑                                              ↓
     └────────────── 记录权限使用日志 ←───────────────┘
```

---

## 工具链编排

### 顺序执行

```yaml
workflow:
  name: "research_topic"
  steps:
    - tool: "web.search"
      input: "{{topic}}"
      output: "search_results"
      
    - tool: "text.summarize"
      input: "{{search_results}}"
      output: "summary"
      
    - tool: "knowledge.add"
      input: "{{summary}}"
```

### 并行执行

```yaml
workflow:
  name: "analyze_document"
  parallel:
    - tool: "file.ocr"
      input: "{{document}}"
      output: "text_content"
      
    - tool: "image.analyze"
      input: "{{document}}"
      output: "visual_description"
      
  merge:
    tool: "text.combine"
    inputs: ["{{text_content}}", "{{visual_description}}"]
```

---

## 技能开发指南

### 1. 创建技能模板

```bash
# 使用 CLI 创建新技能
crablet skill create my-skill

# 生成目录结构
my-skill/
├── skill.yaml
├── src/
│   └── main.rs
├── Cargo.toml
└── tests/
```

### 2. 技能代码示例

```rust
// src/main.rs
use crablet_sdk::prelude::*;

#[skill]
async fn get_current_weather(params: WeatherParams) -> Result<WeatherData> {
    let client = HttpClient::new();
    let response = client
        .get("https://api.weather.com/v1/current")
        .query("city", &params.city)
        .send()
        .await?;
    
    let data: WeatherData = response.json().await?;
    Ok(data)
}
```

### 3. 编译和打包

```bash
# 编译为 WebAssembly
cargo build --target wasm32-wasi --release

# 打包技能
crablet skill pack ./my-skill -o my-skill.cskill
```

### 4. 发布技能

```bash
# 发布到 Skill Store
crablet skill publish my-skill.cskill

# 或本地安装
crablet skill install my-skill.cskill
```

---

## 工具使用统计

### 统计维度

| 维度 | 说明 |
|------|------|
| 调用次数 | 每个工具的总调用次数 |
| 成功率 | 工具调用成功的比例 |
| 平均耗时 | 工具执行的平均时间 |
| Token 消耗 | 工具调用消耗的 Token 数 |
| 用户评分 | 用户对工具结果的满意度 |

### 示例统计报告

```yaml
period: "2026-03-01 to 2026-03-15"
tools:
  - name: "file.ocr"
    invocations: 1523
    success_rate: 98.5%
    avg_duration: "2.3s"
    
  - name: "weather.get_current_weather"
    invocations: 89
    success_rate: 95.5%
    avg_duration: "0.8s"
```

---

## 版本管理

### 技能版本策略

```yaml
# 语义化版本控制
version: "1.2.3"
# 主版本.次版本.修订号
# 1 - 重大变更，不兼容
# 2 - 新功能，向后兼容
# 3 - Bug 修复，向后兼容
```

### 自动更新

```yaml
update_policy:
  # 自动更新策略
  mode: "auto"  # auto / manual / notify
  
  # 允许自动更新的版本范围
  semver_range: "^1.0.0"
  
  # 更新时间窗口
  schedule: "03:00"
```

---

*本文件定义了 Crablet 的动态工具系统，支持技能的动态加载、权限管理和工作流编排。*
