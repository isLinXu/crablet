# ModelScope Skills 集成文档

## 概述

Crablet 现已完整集成 ModelScope (魔搭社区) Skills 平台，支持从 ModelScope 官方 MS-Agent 仓库和社区维护的 skills 仓库搜索、安装、测试和验证技能。

## ModelScope 平台介绍

### 官方资源
- **官网**: https://www.modelscope.cn
- **Skills Central**: https://www.modelscope.cn/skills
- **MCP Marketplace**: https://www.modelscope.cn/mcp
- **MS-Agent GitHub**: https://github.com/modelscope/ms-agent
- **Agent Skills 协议**: https://docs.claude.com/en/docs/agents-and-tools/agent-skills

### Agent Skills 协议特点

ModelScope Skills 基于 Anthropic 提出的 Agent Skills 技术协议，具有以下特点：
- **结构化技能定义**: SKILL.md + scripts/ + resources/
- **多层渐进式加载**: Level 1-4 逐步加载上下文
- **智能检索**: FAISS 密集检索 + BM25 稀疏检索 + LLM 过滤
- **DAG 执行**: 有向无环图管理技能依赖
- **沙箱执行**: Docker 沙箱安全隔离

## 新增功能

### 1. ModelScope Client (`src/skills/model_scope.rs`)

#### 配置选项
```rust
pub struct ModelScopeConfig {
    pub api_base: Option<String>,              // API 基础 URL
    pub github_repo: Option<String>,            // 官方 MS-Agent 仓库
    pub community_repo: Option<String>,         // 社区维护的 Skills 仓库
    pub timeout_secs: Option<u64>,            // 超时时间
    pub api_token: Option<String>,             // API Token (可选)
    pub default_install_dir: Option<PathBuf>,   // 默认安装目录
    pub max_retries: Option<u32>,           // 最大重试次数
}
```

#### 核心功能

1. **搜索技能**
   - `list_skills()`: 获取官方技能列表
   - `list_community_skills()`: 获取社区技能列表
   - `search_skills(query)`: 同时搜索官方和社区技能
   - `get_recommended_skills()`: 获取推荐技能
   - `get_skills_by_category(category)`: 按分类获取

2. **安装技能**
   - `install_skill(name, target_dir)`: 安装单个技能
   - `install_batch(skill_names, target_dir)`: 批量安装
   - `install_from_repo(name, target_dir, repo)`: 从指定仓库安装
   - `install_via_git(name, target_dir, repo)`: 通过 git clone 安装
   - `download_directory()`: 递归下载技能目录

3. **测试和验证**
   - `test_skill(skill_path)`: 运行技能测试 (py/sh/ts/js)
   - `validate_skill(skill_path)`: 验证技能结构
   - `get_local_skill_info(skill_path)`: 获取本地技能信息

4. **管理功能**
   - `list_installed(install_dir)`: 列出已安装的技能
   - `uninstall_skill(skill_name, install_dir)`: 卸载技能
   - `get_skill(name)`: 获取单个技能详情

### 2. REST API Handlers (`src/gateway/skill_handlers.rs`)

#### ModelScope API 端点

| 方法 | 端点 | 描述 |
|------|--------|------|
| GET | `/api/v1/modelscope/search?q={query}` | 搜索技能 |
| GET | `/api/v1/modelscope/list` | 获取所有技能列表 |
| GET | `/api/v1/modelscope/featured` | 获取推荐/精选技能 |
| GET | `/api/v1/modelscope/skills/:name` | 获取技能详情 |
| GET | `/api/v1/modelscope/categories` | 获取技能分类 |
| GET | `/api/v1/modelscope/installed` | 列出已安装的技能 |
| POST | `/api/v1/modelscope/install` | 安装技能 |
| POST | `/api/v1/modelscope/install/batch` | 批量安装技能 |
| POST | `/api/v1/modelscope/test` | 测试技能 |
| POST | `/api/v1/modelscope/validate` | 验证技能 |
| DELETE | `/api/v1/modelscope/skills/:name` | 卸载技能 |

#### API 请求示例

```bash
# 搜索技能
curl "http://localhost:8080/api/v1/modelscope/search?q=cli"

# 获取推荐技能
curl "http://localhost:8080/api/v1/modelscope/featured"

# 获取技能详情
curl "http://localhost:8080/api/v1/modelscope/skills/modelscope-cli"

# 安装技能
curl -X POST "http://localhost:8080/api/v1/modelscope/install" \
  -H "Content-Type: application/json" \
  -d '{"name": "modelscope-cli", "target_dir": "./skills"}'

# 批量安装
curl -X POST "http://localhost:8080/api/v1/modelscope/install/batch" \
  -H "Content-Type: application/json" \
  -d '{"skills": ["modelscope-cli", "modelscope-image"], "target_dir": "./skills"}'

# 测试技能
curl -X POST "http://localhost:8080/api/v1/modelscope/test" \
  -H "Content-Type: application/json" \
  -d '{"skill_name": "modelscope-cli"}'

# 卸载技能
curl -X DELETE "http://localhost:8080/api/v1/modelscope/skills/modelscope-cli"
```

### 3. 统一平台管理 (`src/skills/china_platforms.rs`)

#### ChinaPlatformManager

支持同时管理多个国内技能平台：

```rust
let manager = ChinaPlatformManager::new();

// 从所有平台搜索
let result = manager.search_all("cli").await?;

// 从指定平台搜索
let result = manager.search("cli", Some(SkillPlatform::ModelScope)).await?;

// 安装技能
manager.install("modelscope-cli", SkillPlatform::ModelScope, PathBuf::from("./skills")).await?;

// 获取精选榜单
let featured = manager.get_featured(SkillPlatform::ModelScope).await?;
```

## Skill 格式

### 标准 Agent Skills 目录结构

```
skill-name/
├── SKILL.md              # 主技能定义 (必填)
├── reference.md          # 详细参考资料 (可选)
├── LICENSE.txt           # 许可证信息 (可选)
├── skill.yaml           # 配置文件 (可选)
├── resources/            # 附加资源 (可选)
│   ├── template.xlsx     # 示例文件
│   └── data.json         # 数据文件
├── scripts/             # 可执行脚本 (可选)
│   ├── main.py          # 主实现
│   └── helper.py        # 辅助函数
└── tests/               # 测试文件 (可选)
    ├── test_basic.py
    └── test_integration.sh
```

### SKILL.md 格式

```markdown
---
name: modelscope-cli
description: Execute ModelScope Hub commands via natural language
version: 1.0.0
author: ModelScope Team
tags: [cli, model hub, automation]
category: development
---

# ModelScope CLI Skill

## Description
Execute ModelScope Hub commands via natural language, including downloading models, datasets, and managing repositories.

## Examples

### Download a model
"Download the Qwen2.5-7B model"

### Search for a dataset
"Find image classification datasets"

### Get model details
"Show me details of the Qwen2.5-7B model"

## Usage
This skill provides natural language interface to ModelScope Hub operations.

## Requirements
- ModelScope SDK
- Python 3.8+
```

## 测试框架

### 支持的测试类型

1. **Python 测试** (`tests/*.py`)
2. **Shell 测试** (`tests/*.sh`)
3. **TypeScript/JavaScript 测试** (`tests/*.ts`, `tests/*.js`)

### 测试执行

测试会自动根据文件扩展名选择运行器：
- `*.py`: `python3 test.py`
- `*.sh`: `bash test.sh`
- `*.ts`, `*.js`: `deno run test.ts` 或 `npx ts-node test.ts`

### 测试结果

```json
{
  "passed": true,
  "test_name": "test_basic",
  "output": "All tests passed",
  "error": null,
  "duration_ms": 125
}
```

## 与 SkillHub 的对比

| 特性 | SkillHub | ModelScope |
|------|-----------|------------|
| 平台 | 腾讯 | 阿里云 |
| 仓库 | skillhub.tencent.com | github.com/modelscope/ms-agent |
| CLI 工具 | skillhub CLI | modelscope CLI (可选) |
| 技能协议 | ClawHub 格式 | Agent Skills 协议 |
| 国内加速 | ✅ COS 镜像 | ✅ GitHub (部分地区需代理) |
| 分类 | 分类标签 | 分类 + 标签 |
| 测试支持 | ✅ py/sh/ts/js | ✅ py/sh/ts/js |
| 批量安装 | ✅ | ✅ |

## 使用示例

### Rust 代码示例

```rust
use crablet::skills::ModelScopeClient;

#[tokio::main]
async fn main() -> Result<()> {
    // 创建客户端
    let client = ModelScopeClient::default_config();
    
    // 搜索技能
    let skills = client.search_skills("cli").await?;
    println!("Found {} skills", skills.len());
    
    // 安装技能
    let result = client.install_skill("modelscope-cli", PathBuf::from("./skills")).await?;
    println!("Install result: {}", result.success);
    
    // 测试技能
    let test_results = client.test_skill(&result.install_path).await?;
    for test in test_results {
        println!("Test {}: {}", test.test_name, if test.passed { "PASSED" } else { "FAILED" });
    }
    
    Ok(())
}
```

### CLI 示例 (如果 modelscope CLI 已安装)

```bash
# 搜索技能
modelscope search cli

# 安装技能
modelscope install modelscope-cli

# 列出已安装的技能
modelscope list
```

### HTTP API 示例

```bash
# 搜索技能
curl -s "http://localhost:8080/api/v1/modelscope/search?q=cli" | jq '.items[].name'

# 安装技能
curl -X POST http://localhost:8080/api/v1/modelscope/install \
  -H "Content-Type: application/json" \
  -d '{"name": "modelscope-cli", "target_dir": "./skills"}' | jq

# 获取已安装技能列表
curl -s http://localhost:8080/api/v1/modelscope/installed | jq '.skills[]'
```

## 配置说明

### 环境变量

```bash
# API Token (可选)
export MODELSCOPE_API_TOKEN="your-token"

# 默认安装目录
export SKILL_INSTALL_DIR="~/.workbuddy/skills"
```

### 配置文件

可以在代码中自定义配置：

```rust
let config = ModelScopeConfig {
    api_base: Some("https://custom-api.example.com".to_string()),
    github_repo: Some("custom/repo".to_string()),
    community_repo: Some("custom/community-repo".to_string()),
    timeout_secs: Some(60),
    max_retries: Some(5),
    ..Default::default()
};

let client = ModelScopeClient::new(config);
```

## 故障排查

### 问题: GitHub API 限制
**现象**: 下载技能时遇到 403 错误
**解决**: 
- 使用 CLI 方式（如果 modelscope CLI 已安装）
- 配置 GitHub Token
- 使用社区仓库备用源

### 问题: 递归下载失败
**现象**: 下载多层级目录时报错
**解决**: 已通过 Box::pin 修复递归调用问题

### 问题: 测试执行失败
**现象**: 测试脚本执行错误
**解决**:
- 检查脚本权限 `chmod +x tests/*.sh`
- 确保运行环境已安装 (python3, deno 等)
- 查看测试输出的错误信息

## 编译状态

✅ **编译通过**
- 0 errors
- ~50 warnings (都是无害的 warning)

### 已测试的模块
- `src/skills/model_scope.rs`: ✅
- `src/skills/china_platforms.rs`: ✅
- `src/gateway/skill_handlers.rs`: ✅
- `src/gateway/server.rs`: ✅

## 后续优化建议

1. **缓存机制**: 实现技能列表的本地缓存，减少网络请求
2. **依赖管理**: 自动检测和安装技能依赖的 Python 包/NPM 包
3. **版本冲突**: 实现技能版本冲突检测和解决
4. **沙箱执行**: 集成 ModelScope 的沙箱执行环境 (ms-enclave)
5. **技能签名**: 实现技能签名验证，确保来源可信

## 参考

- [ModelScope 官方文档](https://www.modelscope.cn/docs)
- [MS-Agent GitHub](https://github.com/modelscope/ms-agent)
- [Anthropic Agent Skills 协议](https://docs.claude.com/en/docs/agents-and-tools/agent-skills)
- [SkillHub 集成文档](./skillhub_integration.md)

## 总结

ModelScope Skills 集成为 Crablet 提供了：
- ✅ 完整的 ModelScope 官方技能支持
- ✅ 社区技能仓库作为备用源
- ✅ 统一的 REST API 接口
- ✅ 技能测试和验证框架
- ✅ 批量安装和管理
- ✅ 与 SkillHub 的统一管理接口

现在用户可以通过 Crablet 轻松访问和管理来自 SkillHub 和 ModelScope 两个国内主流技能平台的技能资源。
