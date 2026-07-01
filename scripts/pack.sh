#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
# pack.sh — Crablet 统一打包脚本 (v3)
#
# 一键完成：前端构建 → sidecar 编译 → UI 同步 → Tauri 打包 → 签名 → DMG/EXE
#
# 用法:
#   ./scripts/pack.sh                  # 完整打包（当前平台）
#   ./scripts/pack.sh --quick          # 跳过前端构建（使用已有 dist）
#   ./scripts/pack.sh --app-only       # 只出 .app，不创 DMG
#   ./scripts/pack.sh --sign-only      # 只签名已有 .app
#   ./scripts/pack.sh --dmg-only       # 只创 DMG（需先有 .app）
#   ./scripts/pack.sh --ci             # CI 模式（无交互、Ad-hoc 签名）
#
# 环境变量:
#   CODE_SIGN_IDENTITY  — Apple 证书名（留空则 Ad-hoc 签名）
#   CRABLET_VERSION     — 版本覆盖（留空则从 Cargo.toml 读取）
#   SKIP_FRONTEND       — "true" 跳过前端构建
# ═══════════════════════════════════════════════════════════════════════════

set -euo pipefail

# ─── 颜色输出 ──────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

info()  { echo -e "${BLUE}ℹ${NC}  $*"; }
ok()    { echo -e "${GREEN}✅${NC} $*"; }
warn()  { echo -e "${YELLOW}⚠️${NC} $*"; }
err()   { echo -e "${RED}❌${NC} $*" >&2; }
step()  { echo -e "\n${BOLD}${BLUE}━━━ $* ━━━${NC}"; }

# ─── 路径常量 ──────────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DESKTOP_DIR="$PROJECT_ROOT/desktop"
FRONTEND_DIR="$PROJECT_ROOT/frontend"
CARGO_TOML="$PROJECT_ROOT/crablet/Cargo.toml"
TARGET_DIR="$PROJECT_ROOT/target/release"
BUNDLE_DIR="$TARGET_DIR/bundle"
BINARIES_DIR="$DESKTOP_DIR/binaries"
UI_DIR="$DESKTOP_DIR/ui"

# ─── 版本号（单一真相源：crablet/Cargo.toml）────────────────────────────────
VERSION="${CRABLET_VERSION:-$(grep -m1 '^version' "$CARGO_TOML" | sed 's/.*= "\([^"]*\)".*/\1/')}"
APP_NAME="Crablet"
APP_BUNDLE="${APP_NAME}.app"
APP_PATH="$BUNDLE_DIR/macos/${APP_BUNDLE}"

# ─── 平台检测 ──────────────────────────────────────────────────────────────
OS="$(uname -s)"
ARCH="$(uname -m)"
PLATFORM="unknown"
SIDECAR_TARGET=""
DMG_ARCH=""

case "$OS" in
    Darwin)
        PLATFORM="macos"
        if [[ "$ARCH" == "arm64" ]]; then
            SIDECAR_TARGET="crablet-aarch64-apple-darwin"
            DMG_ARCH="aarch64"
        elif [[ "$ARCH" == "x86_64" ]]; then
            SIDECAR_TARGET="crablet-x86_64-apple-darwin"
            DMG_ARCH="x64"
        else
            err "未知 macOS 架构: $ARCH"; exit 1
        fi
        ;;
    Linux)
        PLATFORM="linux"
        if [[ "$ARCH" == "x86_64" ]]; then
            SIDECAR_TARGET="crablet-x86_64-unknown-linux-gnu"
        elif [[ "$ARCH" == "aarch64" ]]; then
            SIDECAR_TARGET="crablet-aarch64-unknown-linux-gnu"
        fi
        ;;
    MINGW*|MSYS*|CYGWIN*)
        PLATFORM="windows"
        SIDECAR_TARGET="crablet-x86_64-pc-windows-msvc.exe"
        ;;
    *)
        err "不支持的操作系统: $OS"; exit 1
        ;;
esac

# ─── 模式解析 ──────────────────────────────────────────────────────────────
MODE="${1:---all}"
QUICK=false
CI_MODE=false

case "$MODE" in
    --quick)        QUICK=true; MODE="--all" ;;
    --app-only)     ;;
    --dmg-only)     ;;
    --sign-only)    ;;
    --ci)           CI_MODE=true; MODE="--all" ;;
    --all)          ;;
    *)
        err "未知模式: $MODE"
        echo "用法: $0 [--quick|--app-only|--dmg-only|--sign-only|--ci]"
        exit 1
        ;;
esac

# ═══════════════════════════════════════════════════════════════════════════
# 主流程
# ═══════════════════════════════════════════════════════════════════════════

echo -e "${BOLD}${GREEN}"
echo "  ╔═══════════════════════════════════════════════╗"
echo "  ║          🦀 Crablet 打包脚本 v3              ║"
echo "  ╠═══════════════════════════════════════════════╣"
echo -e "  ║  版本: ${VERSION}                                ║"
echo -e "  ║  平台: ${PLATFORM} (${ARCH})                       ║"
echo -e "  ║  Sidecar: ${SIDECAR_TARGET}    ║"
echo "  ╚═══════════════════════════════════════════════╝"
echo -e "${NC}"

# ─── Step 0: 前端构建 ──────────────────────────────────────────────────────
build_frontend() {
    step "Step 0: 前端 SPA 构建"

    if [[ "$QUICK" == "true" ]] || [[ "${SKIP_FRONTEND:-}" == "true" ]]; then
        warn "跳过前端构建 (--quick 或 SKIP_FRONTEND=true)"
        return
    fi

    local dist_dir="$FRONTEND_DIR/dist"
    if [[ -f "$dist_dir/index.html" ]]; then
        ok "前端产物已存在 ($dist_dir)"
    else
        info "构建前端..."
        if ! command -v npm &>/dev/null; then
            err "npm 未安装，无法构建前端"
            [[ "$CI_MODE" == "true" ]] && exit 1 || return 1
        fi
        cd "$FRONTEND_DIR"
        npm install
        npm run build
        cd "$PROJECT_ROOT"
        ok "前端构建完成"
    fi

    # 同步 frontend/dist → desktop/ui（Tauri frontendDist 指向 ./ui）
    info "同步前端产物到 desktop/ui/"
    rm -rf "$UI_DIR"
    cp -R "$FRONTEND_DIR/dist" "$UI_DIR"
    # 清理无用文件
    find "$UI_DIR" -name '.DS_Store' -delete 2>/dev/null || true
    ok "UI 同步完成 ($(du -sh "$UI_DIR" | cut -f1))"
}

# ─── Step 1: Sidecar 编译 ──────────────────────────────────────────────────
build_sidecar() {
    step "Step 1: Sidecar 二进制编译"

    local binary="$TARGET_DIR/crablet"
    if [[ "$PLATFORM" == "windows" ]]; then
        binary="$TARGET_DIR/crablet.exe"
    fi

    if [[ -f "$binary" ]]; then
        ok "Sidecar 已存在 ($(du -sh "$binary" | cut -f1))"
    else
        info "编译 release 二进制 (cargo build --release -p crablet)..."
        cd "$PROJECT_ROOT"
        cargo build --release -p crablet --no-default-features --features web
        ok "Sidecar 编译完成 ($(du -sh "$binary" | cut -f1))"
    fi

    # 复制到 binaries/ 并按 target triple 命名
    info "复制 sidecar 到 desktop/binaries/"
    mkdir -p "$BINARIES_DIR"
    local dest="$BINARIES_DIR/$SIDECAR_TARGET"
    cp -f "$binary" "$dest"
    chmod +x "$dest"
    ok "Sidecar: $dest"

    # 同时创建无后缀通用副本（Tauri runtime 回退用）
    local plain_dest="$BINARIES_DIR/crablet"
    if [[ "$PLATFORM" != "windows" ]]; then
        cp -f "$binary" "$plain_dest"
        chmod +x "$plain_dest"
    fi
}

# ─── Step 2: Tauri 构建 ────────────────────────────────────────────────────
build_tauri() {
    step "Step 2: Tauri 桌面端构建"

    cd "$DESKTOP_DIR"

    # 确保 node 依赖就绪
    if [[ -f "package.json" ]]; then
        info "检查桌面端 node 依赖..."
        npm install --silent 2>/dev/null || true
    fi

    local bundle_flag=""
    case "$PLATFORM" in
        macos)  bundle_flag="app" ;;
        windows) bundle_flag="nsis" ;;
        linux)  bundle_flag="deb,appimage" ;;
    esac

    info "执行 cargo tauri build --bundles $bundle_flag"
    if command -v cargo-tauri &>/dev/null; then
        cargo tauri build --bundles "$bundle_flag"
    elif [[ -f "node_modules/.bin/tauri" ]]; then
        npx tauri build --bundles "$bundle_flag"
    else
        err "tauri-cli 未安装。运行: cargo install tauri-cli --version '^2'"
        exit 1
    fi
    ok "Tauri 构建完成"
}

# ─── Step 3: Sidecar 路径修复 + 前端资源注入 ─────────────────────────────
fix_sidecar_paths() {
    step "Step 3: Sidecar 路径修复 + 前端资源注入"

    if [[ "$PLATFORM" != "macos" ]]; then
        info "非 macOS 平台，跳过路径修复"
        return
    fi

    if [[ ! -d "$APP_PATH" ]]; then
        err ".app 不存在: $APP_PATH"
        return 1
    fi

    # ── 3a: 复制前端资源到 Resources/（sidecar 通过 CRABLET_RESOURCE_DIR 定位）──
    local resources_dir="$APP_PATH/Contents/Resources"
    if [[ -f "$UI_DIR/index.html" ]]; then
        info "注入前端资源到 Resources/"
        # 复制 index.html + assets/ + splash.html + vite.svg
        cp -f "$UI_DIR/index.html" "$resources_dir/"
        cp -f "$UI_DIR/splash.html" "$resources_dir/" 2>/dev/null || true
        cp -f "$UI_DIR/vite.svg" "$resources_dir/" 2>/dev/null || true
        if [[ -d "$UI_DIR/assets" ]]; then
            rm -rf "$resources_dir/assets"
            cp -R "$UI_DIR/assets" "$resources_dir/assets"
        fi
        ok "前端资源已注入 Resources/ ($(du -sh "$resources_dir/assets" 2>/dev/null | cut -f1))"
    else
        warn "desktop/ui/index.html 不存在，跳过前端资源注入"
        warn "sidecar 将无法在非开发环境 serve 前端！"
    fi

    # 需要检查的目录（Tauri 可能放置 sidecar 的所有位置）
    local check_dirs=(
        "$APP_PATH/Contents/MacOS/binaries"
        "$APP_PATH/Contents/Resources/binaries"
        "$APP_PATH/Contents/MacOS"
        "$APP_PATH/Contents/Resources"
    )

    for dir in "${check_dirs[@]}"; do
        [[ ! -d "$dir" ]] && continue

        local target_file="$dir/$SIDECAR_TARGET"

        if [[ -f "$target_file" ]]; then
            ok "$target_file (已存在)"
            continue
        fi

        # 查找无 triple 后缀的 crablet 二进制
        local plain="$dir/crablet"
        if [[ -f "$plain" ]]; then
            # 验证确实是 sidecar 而非 Tauri 主程序：
            # sidecar 支持 serve-web 子命令
            if "$plain" --help 2>/dev/null | grep -q "serve-web"; then
                info "创建硬链接: $SIDECAR_TARGET → crablet"
                ln -f "$plain" "$target_file" 2>/dev/null || {
                    warn "硬链接失败，回退到复制"
                    cp -f "$plain" "$target_file"
                }
                ok "$target_file (硬链接)"
            else
                warn "$plain 不是 sidecar（无 serve-web 命令），跳过"
            fi
        fi
    done

    # 兜底：从 binaries/ 直接复制（确保万无一失）
    local mos_binaries="$APP_PATH/Contents/MacOS/binaries"
    mkdir -p "$mos_binaries"
    if [[ ! -f "$mos_binaries/$SIDECAR_TARGET" ]] && [[ -f "$BINARIES_DIR/$SIDECAR_TARGET" ]]; then
        cp -f "$BINARIES_DIR/$SIDECAR_TARGET" "$mos_binaries/$SIDECAR_TARGET"
        chmod +x "$mos_binaries/$SIDECAR_TARGET"
        ok "$mos_binaries/$SIDECAR_TARGET (从 binaries/ 复制)"
    fi
}

# ─── Step 4: 代码签名 ──────────────────────────────────────────────────────
code_sign() {
    step "Step 4: 代码签名"

    if [[ "$PLATFORM" != "macos" ]]; then
        info "非 macOS 平台，跳过签名"
        return
    fi

    if [[ ! -d "$APP_PATH" ]]; then
        err ".app 不存在: $APP_PATH"
        return 1
    fi

    local identity="${CODE_SIGN_IDENTITY:--}"

    if [[ "$identity" == "-" ]]; then
        warn "未设置 CODE_SIGN_IDENTITY，使用 Ad-hoc 签名"
        warn "首次运行需：右键 → 打开 → 确认"
    else
        info "使用证书: $identity"
    fi

    codesign --force --deep --sign "$identity" "$APP_PATH" 2>&1
    codesign --verify --deep --strict "$APP_PATH" 2>&1
    ok "签名完成 ($identity)"
}

# ─── Step 5: DMG 创建 ──────────────────────────────────────────────────────
create_dmg() {
    step "Step 5: DMG 安装包创建"

    if [[ "$PLATFORM" != "macos" ]]; then
        info "非 macOS 平台，跳过 DMG"
        return
    fi

    if [[ ! -d "$APP_PATH" ]]; then
        err ".app 不存在，请先运行 --app-only 或默认模式"
        return 1
    fi

    local dmg_dir="$BUNDLE_DIR/dmg"
    local dmg_name="${APP_NAME}_${VERSION}_${DMG_ARCH}.dmg"
    local dmg_path="$dmg_dir/$dmg_name"

    # 使用系统临时目录，不用项目内 .session_tmps
    local staging
    staging=$(mktemp -d /tmp/crablet-dmg-XXXXXX)

    info "DMG 文件: $dmg_path"

    # 清理旧 DMG
    rm -f "$dmg_path"
    mkdir -p "$dmg_dir"

    # 准备 staging 目录
    cp -R "$APP_PATH" "$staging/"
    ln -sf /Applications "$staging/Applications"

    # 创建 DMG（UDZO = 压缩只读）
    hdiutil create \
        -volname "$APP_NAME" \
        -srcfolder "$staging" \
        -ov \
        -format UDZO \
        "$dmg_path"

    # 清理
    rm -rf "$staging"

    local dmg_size
    dmg_size=$(du -sh "$dmg_path" | cut -f1)
    ok "DMG 创建完成: $dmg_path ($dmg_size)"
}

# ─── Step 6: 产物清单 ──────────────────────────────────────────────────────
print_manifest() {
    step "产物清单"

    info "平台: $PLATFORM ($ARCH)"
    info "版本: $VERSION"
    echo ""

    # 遍历 bundle 目录
    if [[ -d "$BUNDLE_DIR" ]]; then
        find "$BUNDLE_DIR" -type f \( -name '*.dmg' -o -name '*.app' -o -name '*.exe' -o -name '*.deb' -o -name '*.AppImage' -o -name '*.msi' \) | while read -r f; do
            local size
            size=$(du -sh "$f" | cut -f1)
            local rel_path="${f#$BUNDLE_DIR/}"
            echo -e "  ${GREEN}•${NC} $rel_path ${BLUE}($size)${NC}"
        done
    fi

    echo ""
    echo -e "${BOLD}${GREEN}🎉 打包完成！${NC}"
    echo ""

    if [[ "$PLATFORM" == "macos" ]]; then
        echo -e "  ${BOLD}安装方式:${NC}"
        echo -e "  1. 双击 .dmg 文件"
        echo -e "  2. 将 Crablet 拖入 Applications"
        echo -e "  3. 首次打开：右键 → 打开 → 确认"
        echo ""
        if [[ -f "$BUNDLE_DIR/dmg/${APP_NAME}_${VERSION}_${DMG_ARCH}.dmg" ]]; then
            echo -e "  ${BOLD}DMG:${NC} $BUNDLE_DIR/dmg/${APP_NAME}_${VERSION}_${DMG_ARCH}.dmg"
        fi
        echo -e "  ${BOLD}APP:${NC} $APP_PATH"
    elif [[ "$PLATFORM" == "windows" ]]; then
        echo -e "  ${BOLD}NSIS:${NC} $BUNDLE_DIR/nsis/"
    elif [[ "$PLATFORM" == "linux" ]]; then
        echo -e "  ${BOLD}DEB:${NC}  $BUNDLE_DIR/deb/"
        echo -e "  ${BOLD}AppImage:${NC} $BUNDLE_DIR/appimage/"
    fi
}

# ─── 执行 ──────────────────────────────────────────────────────────────────

# 签名模式：只签名
if [[ "$MODE" == "--sign-only" ]]; then
    code_sign
    exit 0
fi

# DMG 模式：只创 DMG
if [[ "$MODE" == "--dmg-only" ]]; then
    create_dmg
    print_manifest
    exit 0
fi

# 完整 / app-only 模式
build_frontend
build_sidecar
build_tauri
fix_sidecar_paths
code_sign

# --app-only 跳过 DMG
if [[ "$MODE" != "--app-only" ]]; then
    create_dmg
fi

print_manifest
