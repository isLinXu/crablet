# Crablet 客户端安装与生产启动清单

## 本机快速打包

```bash
./scripts/pack.sh
```

脚本会依次执行：

1. 同步版本号
2. 使用 `npm ci` 安装前端依赖
3. 构建前端 SPA
4. 编译 `crablet` sidecar
5. 构建 Tauri 桌面应用
6. 校验并注入 sidecar 与前端资源
7. 生成当前平台安装包

## 常用模式

```bash
./scripts/pack.sh --quick     # 复用已有前端 dist
./scripts/pack.sh --app-only  # 只生成 .app
./scripts/pack.sh --ci        # CI 无交互模式，强制重新编译 sidecar
```

## macOS 安装

生成物位于 `desktop/target/<target>/release/bundle/`：

- `.app`：可直接运行或复制到 Applications
- `.dmg`：双击后将 Crablet 拖入 Applications

未配置 Apple Developer ID 时，产物为 Ad-hoc 签名。首次启动可通过右键选择“打开”；正式分发前应配置 Developer ID 签名和 notarization。

## 运行时行为

桌面端启动后会自动启动内置 `crablet` sidecar，并通过 loopback 健康检查等待 API 就绪。API Key 通过系统 Keychain 保存，不应写入仓库或 `.env` 文件。

## 生产配置建议

- 使用正式模型供应商和最小权限 API Key
- 开启并验证 `knowledge` feature 后再使用知识图谱能力
- 生产环境保留 `enable_auto_optimization=false`，仅在注入可审计的配置管理器和策略执行器后启用优化
- 为长任务配置 `Swarm::with_message_timeout(...)`
- 首次发布前执行前端 `type-check`、`lint:ci`、`test:ci` 和 Rust `cargo check --all-targets`

## 当前平台限制

- 当前打包脚本拒绝未经验证的 macOS universal2 产物
- Windows/Linux 安装包需在对应平台或具备对应交叉编译工具链的 CI runner 上构建
- 未配置代码签名时，操作系统可能显示开发者未验证提示
