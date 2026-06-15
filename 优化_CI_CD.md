# CI/CD Action 版本修复总结

> 修复时间：2026-06-13  
> 修复范围：`.github/workflows/` 下全部 6 个 workflow 文件

---

## 问题概述

部分 GitHub Actions 引用了 Marketplace 中不存在的版本，会导致所有 CI 工作流直接失败。主要问题如下：

| 错误版本 | 实际可用版本 | 影响文件 |
|---------|-------------|---------|
| `actions/checkout@v6` | `v4` | 全部 6 个 workflow |
| `actions/upload-artifact@v7` | `v4` | `coverage.yml`, `build-desktop.yml` |
| `docker/build-push-action@v7` | `v6` | `container.yml` |

---

## 逐文件修改详情

### 1. `ci.yml` — 后端质量检查、测试
- **修改**：`actions/checkout@v6` → `actions/checkout@v4`
- **处数**：5 处
  - `backend-quality` job (line 45)
  - `backend-tests` job (line 89)
  - `backend-msrv` job (line 113)
  - `backend-build` job (line 141)
  - `frontend-checks` job (line 165)
- **其他说明**：`actions/setup-node@v6` 经验证在 Marketplace 中可用，保留未改；`codecov/codecov-action@v6` 同样可用，保留未改。

### 2. `build-desktop.yml` — 桌面打包（macOS/Windows）
- **修改 1**：`actions/checkout@v6` → `actions/checkout@v4`
- **处数**：2 处
  - `build-macos` job (line 30)
  - `build-windows` job (line 126)
- **修改 2**：`actions/upload-artifact@v7` → `actions/upload-artifact@v4`
- **处数**：2 处
  - macOS DMG 上传 (line 116)
  - Windows installer 上传 (line 183)

### 3. `coverage.yml` — 覆盖率报告
- **修改 1**：`actions/checkout@v6` → `actions/checkout@v4`
- **处数**：2 处
  - `backend-coverage` job (line 50)
  - `frontend-coverage` job (line 108)
- **修改 2**：`actions/upload-artifact@v7` → `actions/upload-artifact@v4`
- **处数**：3 处
  - backend LCOV 上传 (line 74)
  - frontend LCOV 上传 (line 125)
  - frontend HTML 覆盖率上传 (line 132)

### 4. `security.yml` — 安全审计
- **修改**：`actions/checkout@v6` → `actions/checkout@v4`
- **处数**：5 处
  - `dependency-review` job (line 45)
  - `rust-audit` job (line 61)
  - `cargo-deny` job (line 93)
  - `frontend-audit` job (line 121)
  - `trivy-fs` job (line 141)

### 5. `container.yml` — 容器构建
- **修改 1**：`actions/checkout@v6` → `actions/checkout@v4`
- **处数**：3 处
  - `compose-validate` job (line 40)
  - `image-build` job (line 58)
  - `image-publish` job (line 84)
- **修改 2**：`docker/build-push-action@v7` → `docker/build-push-action@v6`
- **处数**：2 处
  - `image-build` job (line 64)
  - `image-publish` job (line 110)
- **其他说明**：`docker/metadata-action@v6` 经验证在 Marketplace 中可用，保留未改。

### 6. `docs.yml` — 文档部署
- **修改**：`actions/checkout@v6` → `actions/checkout@v4`
- **处数**：1 处
  - `build` job (line 25)
- **保留**：`actions/upload-pages-artifact@v3` 经验证在 Marketplace 中可用，保留未改。
- **保留**：`actions/deploy-pages@v4` 经验证可用，保留未改。

---

## 其他检查结果

在修复过程中，对以下 Action 进行了可用性验证，确认当前版本可用，未做修改：

| Action | 当前版本 | 验证结果 |
|--------|---------|---------|
| `actions/setup-node` | `v6` | ✅ 可用，保留 |
| `codecov/codecov-action` | `v6` | ✅ 可用，保留 |
| `docker/metadata-action` | `v6` | ✅ 可用，保留 |
| `docker/setup-buildx-action` | `v4` | ✅ 可用，保留 |
| `docker/setup-qemu-action` | `v4` | ✅ 可用，保留 |
| `docker/login-action` | `v4` | ✅ 可用，保留 |
| `actions/upload-pages-artifact` | `v3` | ✅ 可用，保留 |
| `actions/deploy-pages` | `v4` | ✅ 可用，保留 |

---

## 修改统计

| 文件 | 修改 Action 数 | 修改处数 |
|------|--------------|---------|
| `ci.yml` | 1 | 5 |
| `build-desktop.yml` | 2 | 4 |
| `coverage.yml` | 2 | 5 |
| `security.yml` | 1 | 5 |
| `container.yml` | 2 | 5 |
| `docs.yml` | 1 | 1 |
| **合计** | **9** | **25** |

---

> 修复后所有 workflow 文件中的 Action 版本均可在 GitHub Marketplace 中正常解析，CI 工作流应能正常触发。
