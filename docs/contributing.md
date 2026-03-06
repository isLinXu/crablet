# 贡献指南

感谢你有兴趣为 Crablet 做出贡献！这份指南将帮助你快速上手。

## 开发环境

**前置要求：**

- Rust 1.80+（通过 [rustup](https://rustup.rs) 安装）
- Docker（可选，用于沙箱和 Neo4j）
- sccache（推荐，加速编译）

```bash
# 克隆并构建
git clone https://github.com/YOUR_USERNAME/crablet.git
cd crablet

# 最小化构建（推荐用于日常开发）
cargo build --no-default-features --features web

# 完整构建
cargo build --release

# 运行测试
cargo test --release

# 代码检查
cargo clippy -- -D warnings

# 格式化
cargo fmt --check
```

## 贡献方式

### 报告 Bug

通过 [GitHub Issues](https://github.com/YOUR_USERNAME/crablet/issues) 提交，请使用 Bug Report 模板并尽量包含：

- Crablet 版本（`crablet --version`）
- 操作系统和 Rust 版本
- 最小可复现步骤
- 预期行为 vs 实际行为

### 功能建议

同样通过 Issues 提交，请使用 Feature Request 模板，描述清楚：

- 你想解决的问题
- 你设想的解决方案
- 为什么这个功能对 Crablet 有价值

### 提交 Pull Request

1. Fork 并创建你的分支：`git checkout -b feature/my-feature`
2. 编写代码和测试
3. 确保通过所有检查：`cargo test && cargo clippy && cargo fmt --check`
4. 提交 PR 并描述你的修改

## 开发方向

以下是目前最欢迎的贡献方向：

### 新增工具插件

在 `src/tools/` 目录下实现 `Plugin` trait：

```rust
use crate::plugins::Plugin;

pub struct MyTool;

#[async_trait]
impl Plugin for MyTool {
    fn name(&self) -> &str { "my_tool" }
    fn description(&self) -> &str { "Description of my tool" }
    async fn execute(&self, input: &str) -> Result<String> { ... }
}
```

### 新增接入通道

在 `src/channels/` 中添加新的平台接入，参考现有的 `telegram` 或 `discord` 实现。

### 中间件开发

在 `src/cognitive/middleware.rs` 中实现 `CognitiveMiddleware` trait，为 ReAct 引擎增加预处理/后处理能力。

### 技能包开发

创建 `SKILL.md`（指令型技能）或 `skill.yaml`（可执行技能），参考 `skills/` 目录下的示例。

### LLM 适配器

在 `src/cognitive/llm/` 中实现新的 `LlmClient` trait，接入更多 LLM 提供商。

## 代码规范

- 遵循 Rust 标准风格（`cargo fmt`）
- 所有 public API 添加文档注释
- 新功能须附带单元测试或集成测试
- 尽量通过 `cargo clippy` 无警告

## 提交信息

采用约定式提交格式：

```
feat: add weather tool with OpenMeteo API
fix: resolve memory leak in working memory consolidation
docs: update README with Docker deployment guide
refactor: extract middleware pipeline into separate module
test: add integration tests for ReAct engine
```

## 行为准则

我们致力于为所有人提供一个友好、安全和包容的参与环境。在参与此项目时，请保持尊重和专业。

---

再次感谢你的贡献！如果有任何疑问，可以通过 Issues 或 Discussions 联系我们。
