# Crablet 分批提交计划

## 批次1：基础设施 + 浏览器层
- CI/CD：build-desktop, ci, container, coverage, security workflows
- Dockerfile：优化构建阶段和层缓存
- Justfile：添加 dev/build/release 目标，支持跨平台
- tauri.conf.json：持久会话和窗口配置

## 批次2：感知层 + ReAct 引擎
- PagePerception：延迟加载 CLIP/OCR，添加真实图片下载和 CLIP 评分
- ReActLoop：添加观察-动作循环，自动刷新页面状态
- CLIP 集成：compute_clip_scores 真正下载图片并计算相似度

## 批次3：Agent + 工具函数
- BrowserUseAgent：13 个工具函数
- from_spider：工厂方法桥接 SmartSpider

## 批次4：包导出
- lib.rs 和 mod.rs 的重新导出
- 遗漏模块的补充

## 批次5：子模块
- smart-spider 更新
- spider_tools 桥接集成

## 批次6：杂项
- 文档和示例

## 批次7：推送
- 推送所有提交到远程
