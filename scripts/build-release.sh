#!/usr/bin/env bash
# ============================================================
# build-release.sh — Crablet 桌面端统一分发打包脚本
# 编译多平台 sidecar → 复制到 binaries/ → 构建 Tauri
# 支持 macOS (Intel + Apple Silicon) / Linux / Windows
# ============================================================

set -euo pipefail

# 颜色
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# 项目目录（脚本在 scripts/ 下，项目根目录是上一级）
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DESKTOP_DIR="$PROJECT_ROOT/desktop"
BINARIES_DIR="$DESKTOP_DIR/binaries"
CARGO_TOML="$PROJECT_ROOT/crablet/Cargo.toml"

# 当前版本（从 Cargo.toml 读取，作为 sidecar 版本标签）
VERSION="$(grep -m1 '^version' "$CARGO_TOML" | sed 's/.*= "\([^"]*\)".*/\1/')"
SIDEcar_NAME="crablet"
TARGET_DIR="$PROJECT_ROOT/target/release"
BUNDLE_DIR="$TARGET_DIR/bundle"

echo -e "${GREEN}🦀 Crablet 桌面端打包脚本 v${VERSION}${NC}"
echo "项目根目录: $PROJECT_ROOT"
echo ""

# 功能开关（通过环境变量控制）
BUILD_SIDEcar="${BUILD_SIDEcar:-true}"   # 是否编译 sidecar
BUILD_TAURI="${BUILD_TAURI:-true}"     # 是否构建 Tauri
SKIP_SIDEcar="${SKIP_SIDEcar:-}"       # 跳过哪些平台 sidecar

# 平台检测
OS="$(uname -s)"
ARCH="$(uname -m)"
HOST_TARGET=""

if [[ "$OS" == "Darwin" ]]; then
    if [[ "$ARCH" == "arm64" ]]; then
        HOST_TARGET="aarch64-apple-darwin"
    else
        HOST_TARGET="x86_64-apple-darwin"
    fi
elif [[ "$OS" == "Linux" ]]; then
    HOST_TARGET="x86_64-unknown-linux-gnu"
else
    echo -e "${YELLOW}⚠️ 当前在 Windows 上运行，建议用 PowerShell 脚本 (build-release.ps1)${NC}"
    exit 1
fi

# 支持的 sidecar 目标平台
SIDEcar_TARGETS=(
    "aarch64-apple-darwin"
    "x86_64-apple-darwin"
    "x86_64-unknown-linux-gnu"
    "aarch64-unknown-linux-gnu"
    "x86_64-pc-windows-msvc"
    "aarch64-pc-windows-msvc"
)

# --------------------------------------------------
# 0) 构建前端 SPA
# --------------------------------------------------
echo -e "${GREEN}📦 Step 0: 构建前端 SPA${NC}"
FRONTEND_DIR="$PROJECT_ROOT/frontend"
FRONTEND_DIST="$PROJECT_ROOT/frontend/dist"
if [ -d "${FRONTEND_DIST}" ] && [ -f "${FRONTEND_DIST}/index.html" ]; then
    echo -e "  ${GREEN}✅ 前端产物已就绪${NC}"
elif [ -f "${FRONTEND_DIR}/package.json" ] && command -v npm &>/dev/null; then
    echo "  🔨 正在构建前端..."
    cd "${FRONTEND_DIR}"
    npm install
    npm run build
    cd "${PROJECT_ROOT}"
    echo -e "  ${GREEN}✅ 前端构建完成${NC}"
else
    echo -e "  ${YELLOW}⚠️ 警告: 前端产物不存在且无法自动构建${NC}"
    echo -e "  ${YELLOW}   桌面端启动后可能无法显示前端界面。请手动构建：cd frontend && npm install && npm run build${NC}"
fi
echo ""

# --------------------------------------------------
# 1) 编译 sidecar（各平台）
# --------------------------------------------------
if [[ "$BUILD_SIDEcar" == "true" ]]; then
    echo -e "${GREEN}📦 Step 1: 编译 sidecar${NC}"
    mkdir -p "$BINARIES_DIR"

    for target in "${SIDEcar_TARGETS[@]}"; do
        # 跳过标记
        if [[ "$SKIP_SIDEcar" == *"$target"* ]]; then
            echo -e "  ${YELLOW}⏭️  跳过 $target${NC}"
            continue
        fi

        # 检查是否已安装目标工具链
        if ! rustup target list --installed | grep -q "$target"; then
            echo -e "  ${YELLOW}⏭️  工具链未安装，跳过 $target (运行: rustup target add $target)${NC}"
            continue
        fi

        echo -e "  ${GREEN}🔨 编译 $target ...${NC}"
        cd "$PROJECT_ROOT/crablet"

        if [[ "$target" == *"windows"* ]]; then
            # Windows 需要 cross 编译或本地构建；这里用 cargo build 并指定 target
            cargo build --release --target "$target" 2>/dev/null || {
                echo -e "  ${RED}❌ $target 编译失败 (可能需要交叉编译工具链)${NC}"
                continue
            }
            cp "target/$target/release/${SIDEcar_NAME}.exe" "$BINARIES_DIR/${SIDEcar_NAME}-${target}.exe" 2>/dev/null || true
        else
            cargo build --release --target "$target" 2>/dev/null || {
                echo -e "  ${RED}❌ $target 编译失败 (可能需要交叉编译工具链)${NC}"
                continue
            }
            cp "target/$target/release/${SIDEcar_NAME}" "$BINARIES_DIR/${SIDEcar_NAME}-${target}" 2>/dev/null || true
        fi

        if [ -f "$BINARIES_DIR/${SIDEcar_NAME}-${target}" ] || [ -f "$BINARIES_DIR/${SIDEcar_NAME}-${target}.exe" ]; then
            echo -e "  ${GREEN}✅ $target 完成${NC}"
        else
            echo -e "  ${YELLOW}⚠️ $target 未输出二进制（可能已是最新）${NC}"
        fi
    done

    # 如果是本地构建，同时创建无后缀的通用名（Tauri 在当前平台会查找无后缀名）
    if [[ "$HOST_TARGET" == "aarch64-apple-darwin" ]] && [ -f "$BINARIES_DIR/${SIDEcar_NAME}-aarch64-apple-darwin" ]; then
        cp "$BINARIES_DIR/${SIDEcar_NAME}-aarch64-apple-darwin" "$BINARIES_DIR/${SIDEcar_NAME}" 2>/dev/null || true
        echo -e "  ${GREEN}✅ 创建通用 sidecar 副本 (无后缀)${NC}"
    fi
    if [[ "$HOST_TARGET" == "x86_64-apple-darwin" ]] && [ -f "$BINARIES_DIR/${SIDEcar_NAME}-x86_64-apple-darwin" ]; then
        cp "$BINARIES_DIR/${SIDEcar_NAME}-x86_64-apple-darwin" "$BINARIES_DIR/${SIDEcar_NAME}" 2>/dev/null || true
        echo -e "  ${GREEN}✅ 创建通用 sidecar 副本 (无后缀)${NC}"
    fi

    echo ""
fi

# --------------------------------------------------
# 2) 构建 Tauri 桌面端
# --------------------------------------------------
if [[ "$BUILD_TAURI" == "true" ]]; then
    echo -e "${GREEN}🚀 Step 2: 构建 Tauri 桌面端${NC}"
    cd "$DESKTOP_DIR"

    # 安装前端依赖（如果有）
    if [ -f "package.json" ]; then
        echo -e "  ${GREEN}📦 安装前端依赖 ...${NC}"
        npm install --silent 2>/dev/null || true
    fi

    # 构建 Tauri
    echo -e "  ${GREEN}🔨 执行 tauri build ...${NC}"
    npm run tauri build 2>/dev/null || cargo tauri build 2>/dev/null || {
        echo -e "  ${RED}❌ Tauri 构建失败，请检查环境：${NC}"
        echo -e "     npm install -g @tauri-apps/cli"
        echo -e "     cargo install tauri-cli"
        exit 1
    }

    echo -e "  ${GREEN}✅ Tauri 构建完成${NC}"
    echo ""
fi

# --------------------------------------------------
# 3) 输出分发物清单
# --------------------------------------------------
echo -e "${GREEN}📂 分发物清单${NC}"
find "$BUNDLE_DIR" -type f 2>/dev/null | while read -r f; do
    size=$(du -h "$f" | cut -f1)
    echo -e "  ${GREEN}• $(basename "$f")${NC} ($size)"
done

echo ""
echo -e "${GREEN}🎉 全部完成！${NC}"
echo "  macOS .app: $BUNDLE_DIR/macos/Crablet.app"
echo "  macOS DMG:  $BUNDLE_DIR/dmg/Crablet_${VERSION}_*.dmg"
echo "  Windows:    $BUNDLE_DIR/nsis/Crablet_${VERSION}_setup.exe"
echo "  Linux:      $BUNDLE_DIR/appimage/Crablet_*.AppImage"
echo "  Linux:      $BUNDLE_DIR/deb/Crablet_*.deb"
