# Crablet 深度分析、优化与打包计划

## 项目概览

- **项目名称**: Crablet 🦀 — AI 智能体操作系统
- **路径**: `/Users/gatilin/PycharmProjects/crablet-git`
- **后端**: Rust 65.2% (Tokio + Axum + SQLite + Qdrant + Neo4j + Redis)
- **前端**: TypeScript 28.8% (React 19 + Vite + Tailwind + Zustand + React-Flow + D3)
- **桌面**: Tauri 2 (dmg + nsis 目标已配置)
- **智能体**: 四层认知架构 (System1/2/3/4) + 元认知 + Swarm + MCP + ReAct + 记忆系统 + RAG
- **工作部分**: 自动化工作流、智能体系统、爬虫、工具调用、画布
- **Logo**: `/Users/gatilin/Downloads/微信图片_20260601105735_9720_209.png`

## 执行阶段

### Stage 1 — 深度分析（并行）

| 子代理 | 分析范围 | 交付物 |
|--------|----------|--------|
| 分析员_后端 | crablet/src/ 核心 Rust 后端架构、依赖、代码质量、性能瓶颈 | 后端分析.md |
| 分析员_前端 | frontend/src/ React 前端、组件结构、构建配置、性能 | 前端分析.md |
| 分析员_桌面 | desktop/ Tauri 配置、构建流程、资源引用、打包问题 | 桌面分析.md |
| 分析员_智能体 | 认知层/智能体/记忆/Swarm/元认知/技能系统 | 智能体分析.md |
| 分析员_构建 | 构建脚本、CI/CD、Docker、Justfile、构建流程 | 构建分析.md |

### Stage 2 — 优化实施（基于 Stage 1 结果）

- 后端优化：编译警告、依赖清理、性能热点、代码结构
- 前端优化：构建体积、代码分割、状态管理、组件重构
- 桌面优化：Tauri 配置修正、Logo 替换、资源引用修正
- 智能体优化：认知循环优化、Swarm 协调、记忆系统、工具调用效率

### Stage 3 — 打包构建

- macOS DMG: `cargo tauri build --target aarch64-apple-darwin` / `x86_64-apple-darwin`
- Windows EXE/NSIS: `cargo tauri build --target x86_64-pc-windows-msvc`
- Logo 替换: 将用户提供的 PNG 转换为各尺寸图标 + icns + ico
- 安装包验证

### Stage 4 — 整合报告

- 合并所有分析结果为综合报告
- 输出优化建议清单
- 交付打包产物
