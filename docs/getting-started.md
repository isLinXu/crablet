# 快速开始

## 前置要求

- **Rust** 1.80+ （推荐通过 [rustup](https://rustup.rs) 安装）
- **Docker**（可选，用于沙箱和 Neo4j）
- **Git**

## 安装方式

### 方式一：从源码构建

```bash
# 1. 克隆项目
git clone https://github.com/yourusername/crablet.git
cd crablet

# 2. 最小构建（仅 CLI + Web，约 5 分钟）
cargo build --release --no-default-features --features web

# 3. 完整构建（包含所有功能，约 15-20 分钟）
cargo build --release

# 4. 初始化
./target/release/crablet init
```

> **构建优化**：安装 [sccache](https://github.com/mozilla/sccache) 加速编译：
> ```bash
> cargo install sccache
> export RUSTC_WRAPPER=sccache
> ```

### 方式二：Docker 一键部署

```bash
# 1. 设置环境变量
export OPENAI_API_KEY=sk-xxx

# 2. 启动所有服务
docker-compose up -d

# 3. 访问 Web UI
open http://localhost:3000
```

## 基本使用

```bash
# 1. 交互式聊天
crablet chat

# 2. 单次执行
crablet run "查询北京今天天气"

# 3. 启动 Web 服务
crablet serve-web --port 3000

# 4. 启动网关（WebSocket + JSON-RPC）
crablet gateway --port 18789

# 5. 技能管理
crablet skill list
crablet skill install https://github.com/user/my-skill.git
crablet skill create weather

# 6. 知识管理
crablet knowledge extract --file document.pdf
crablet knowledge query "Rust 所有权模型"

# 7. 系统状态
crablet status

# 8. 运行 Lua 脚本
crablet script run examples/scripts/summarize_paper.lua
```
