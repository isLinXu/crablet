# Crablet 分步分批提交计划

## 批次概览

| 批次 | 主题 | 文件范围 | 预估文件数 |
|------|------|---------|-----------|
| Batch 1 | 图标资源替换与生成工具 | 所有图标文件 + 生成脚本 | 40+ |
| Batch 2 | 前端依赖修复与构建优化 | package.json/vite.config.ts + 新组件 | 7 |
| Batch 3 | 桌面端打包配置与启动流程 | tauri.conf.json/build-desktop.sh/main.rs + 分发脚本 | 5 |
| Batch 4 | 后端 Gateway 静态资源解析 | server.rs resolve_static_dir 修复 | 1 |
| Batch 5 | 构建脚本统一 | build.sh FEATURES 统一 | 1 |
| Batch 6 | 分析文档与优化报告 | plan.md + 所有分析/优化报告 | 11 |

---

## Batch 1: 图标资源替换与生成工具

**文件**:
- 修改: desktop/icons/ 下所有 *.png, *.icns, *.ico 文件
- 新增: desktop/icons/icon.icns.dir/, desktop/icons/icon.iconset/icon_1024x1024.png, icon_64x64.png
- 新增: desktop/icons/ios/AppIcon.appiconset/ (全套 iOS 图标 + Contents.json)
- 新增: scripts/generate-icons.sh, scripts/generate-icons.py, scripts/generate-icons.ps1

**Commit Message**:
```
feat(icons): regenerate all platform icons from new logo PNG

- Replace all icon assets with new branding from 微信图片_20260601105735_9720_209.png
- Generate macOS iconset (16~1024), icns, Windows ico (16~256)
- Generate Android mipmap (48~192), iOS AppIcon.appiconset (20~1024)
- Generate Tauri bundle PNGs (32, 64, 128, 256@2x) and Windows Store logos
- Add cross-platform icon generation scripts (Python/Pillow, Bash/sips, PowerShell/.NET)
```

---

## Batch 2: 前端依赖修复与构建优化

**文件**:
- 修改: frontend/package.json (TypeScript ~5.7.3, @types/node ^22.15.3, @vitejs/plugin-react ^4.4.1, 移除弃用类型包)
- 修改: frontend/vite.config.ts (新增 d3-vendor, ocr-vendor, pdf-vendor chunk 拆分)
- 修改: frontend/tsconfig.app.json (确认无需修改)
- 修改: frontend/package-lock.json (依赖锁定更新)
- 新增: frontend/src/components/activity/activityTypes.ts
- 新增: frontend/src/components/activity/useActivityState.ts
- 新增: frontend/src/components/settings/settingsHelpers.ts

**Commit Message**:
```
fix(frontend): resolve dependency version conflicts and optimize build chunks

- Fix TypeScript version: ~6.0.2 (non-existent) → ~5.7.3
- Fix @types/node: ^25.5.0 (non-existent) → ^22.15.3
- Fix @vitejs/plugin-react: ^6.0.1 → ^4.4.1 (peer dep compatibility with Vite 7)
- Remove deprecated stub packages: @types/dompurify, @types/tesseract.js
- Add manualChunks for heavy deps: d3-vendor, ocr-vendor, pdf-vendor
- Add ActivityCenter types and Settings vendor helpers
```

---

## Batch 3: 桌面端打包配置与启动流程

**文件**:
- 修改: desktop/tauri.conf.json (resources: ["../frontend/dist"], CSP 策略)
- 修改: desktop/build-desktop.sh (新增 Step 0 前端构建检测)
- 修改: desktop/src/main.rs (注入 CRABLET_ALLOW_ANY_ORIGIN=true)
- 新增: scripts/build-release.sh (macOS/Linux 统一分发打包)
- 新增: scripts/build-release.ps1 (Windows 统一分发打包)

**Commit Message**:
```
feat(desktop): embed frontend SPA and auto-launch backend on DMG install

- tauri.conf.json: bundle resources to include frontend/dist for self-contained app
- build-desktop.sh: add Step 0 frontend build detection before packaging
- main.rs: inject CRABLET_ALLOW_ANY_ORIGIN=true for local desktop CORS
- Add build-release.sh/ps1: unified multi-platform sidecar + Tauri packaging
- Desktop app now opens splash → auto-starts backend → serves frontend SPA
```

---

## Batch 4: 后端 Gateway 静态资源解析

**文件**:
- 修改: crablet/src/gateway/server.rs (resolve_static_dir 增加 CRABLET_RESOURCE_DIR/frontend/dist 检查)

**Commit Message**:
```
fix(gateway): resolve static dir from Tauri bundle Resources/frontend/dist

- resolve_static_dir(): check CRABLET_RESOURCE_DIR/frontend/dist for desktop bundle
- Fixes: backend could not find frontend SPA when launched from .app/Contents/MacOS/
- Enables: desktop app serves embedded frontend without manual path configuration
```

---

## Batch 5: 构建脚本统一

**文件**:
- 修改: scripts/build.sh (FEATURES "knowledge,web" → "knowledge,auto-working,web")

**Commit Message**:
```
fix(scripts): align build.sh FEATURES with install.sh

- build.sh default FEATURES: "knowledge,web" → "knowledge,auto-working,web"
- Consistent with install.sh feature set for full desktop functionality
```

---

## Batch 6: 分析文档与优化报告

**文件**:
- 新增: plan.md
- 新增: 分析_后端.md, 分析_前端.md, 分析_桌面.md, 分析_智能体.md, 分析_构建.md
- 新增: 优化_前端.md, 优化_CI_CD.md, 优化_构建脚本.md, 优化_桌面打包.md, 优化_智能体方案.md
- 新增: 综合报告.md

**Commit Message**:
```
docs: add comprehensive analysis and optimization reports

- Deep analysis: backend (137K LoC), frontend (React 19 + Vite), desktop (Tauri 2),
  agent system (System 1/2/3/4 + Meta-Cognitive), build system (CI/CD + Docker)
- Optimization reports: frontend deps, CI/CD action versions, build scripts,
  desktop packaging, agent cognitive improvements
- Final integration report with prioritized fix checklist and packaging guide
```
