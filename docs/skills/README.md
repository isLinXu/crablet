# Crablet Skill 开发指南

Crablet 支持多种类型的 Skill，可以扩展 Agent 的能力。

## Skill 类型

### 1. Local Skill (本地技能)

使用 YAML/JSON 定义，支持任何可执行脚本。

**文件结构:**
```
my-skill/
├── skill.yaml      # Skill 定义
├── main.py         # 入口脚本
└── README.md       # 文档
```

**skill.yaml 示例:**
```yaml
name: my-skill
description: Description of what this skill does
version: "1.0.0"
parameters:
  type: object
  properties:
    arg1:
      type: string
      description: First argument
    arg2:
      type: number
      description: Second argument
  required: [arg1]
entrypoint: "python main.py"
runtime: python3
requires: [python3]
triggers:
  - type: keyword
    keywords: ["keyword1", "keyword2"]
    case_sensitive: false
  - type: command
    prefix: "/mycommand"
    args_schema:
      type: object
      properties:
        arg1: { type: string }
```

### 2. OpenClaw Skill (提示词技能)

使用 Markdown + YAML frontmatter 定义，适合 LLM-based skills。

**SKILL.md 示例:**
```markdown
---
name: my-skill
description: Description of what this skill does
triggers:
  - type: keyword
    keywords: ["keyword1", "keyword2"]
  - type: command
    prefix: "/mycommand"
---

# Instructions for LLM

You are a helpful assistant specialized in...

## Current Request

{{arg1}}
```

### 3. MCP Skill

通过 MCP (Model Context Protocol) 协议集成外部工具。

MCP Skills 自动从 MCP 服务器发现，无需手动创建。

### 4. Plugin Skill (原生插件)

使用 Rust 编写的原生插件，最高性能。

## 触发器类型

Skills 可以通过以下方式触发:

| 触发器类型 | 描述 | 示例 |
|-----------|------|------|
| `keyword` | 关键词匹配 | 用户输入包含特定关键词 |
| `regex` | 正则表达式匹配 | 输入匹配正则模式 |
| `intent` | 意图分类匹配 | 分类器识别特定意图 |
| `semantic` | 语义相似度匹配 | 基于向量相似度 |
| `command` | 命令前缀匹配 | `/command args` |

## 安装 Skill

```bash
# 从 Git 仓库安装
crablet skill install https://github.com/user/skill-repo.git

# 从注册表搜索并安装
crablet skill search weather
crablet skill install weather

# 本地测试
crablet skill test weather '{"location": "Beijing"}'
```

## 开发 Skill

### 初始化项目

```bash
# 创建 Local Skill
crablet skill dev init my-skill --skill-type local

# 创建 OpenClaw Skill
crablet skill dev init my-skill --skill-type openclaw
```

### 验证

```bash
cd my-skill
crablet skill dev validate
```

### 测试

```bash
crablet skill dev test --args '{"arg1": "value1"}'
```

### 构建

```bash
crablet skill dev build
```

### 发布

```bash
crablet skill dev publish --registry https://my-registry.com
```

## Skill 清单字段

| 字段 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `name` | string | ✓ | Skill 名称 |
| `description` | string | ✓ | Skill 描述 |
| `version` | string | ✓ | 版本号 |
| `parameters` | object | ✓ | JSON Schema 参数定义 |
| `entrypoint` | string | ✓ | 入口命令 |
| `runtime` | string | | 运行时 (python3, node, etc.) |
| `requires` | array | | 系统依赖 |
| `triggers` | array | | 触发器配置 |
| `permissions` | array | | 权限列表 |
| `resources` | object | | 资源限制 |
| `author` | string | | 作者 |

## 安全等级

Skills 根据来源和配置有不同的安全等级:

| 等级 | 描述 | 执行环境 |
|------|------|----------|
| `Trust` | 系统内置、MCP、签名验证的 Skills | 原生执行 |
| `Isolated` | 用户自定义 Skills | Docker 隔离 |
| `StronglyIsolated` | 第三方不可信 Skills | WASM 沙箱 |

## 示例 Skills

### Weather Skill (Local)

```yaml
name: weather
description: Get weather information
triggers:
  - type: keyword
    keywords: ["天气", "weather"]
  - type: command
    prefix: "/weather"
```

### Calculator Skill (OpenClaw)

```markdown
---
name: calculator
description: Perform calculations
triggers:
  - type: keyword
    keywords: ["calculate", "math"]
---

Calculate: {{expression}}
```

## 与 OpenClaw 兼容

Crablet 完全兼容 OpenClaw 的 SKILL.md 格式。你可以直接导入 OpenClaw 的 Skills:

```bash
crablet skill import https://clawhub.dev/skills/example
```

## 更多信息

- [Skill Registry API](./registry-api.md)
- [Security Best Practices](./security.md)
- [Advanced Triggers](./advanced-triggers.md)
