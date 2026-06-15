#!/usr/bin/env bash
# build-desktop.sh — Crablet 桌面应用一键打包脚本（优化版 v2）
#
# 用法：
#   ./build-desktop.sh           # 构建 .app + 签名 + DMG
#   ./build-desktop.sh --app     # 只构建 .app（含签名）
#   ./build-desktop.sh --dmg     # 只创建 DMG（需先有 .app）
#   ./build-desktop.sh --sign    # 只签名已有的 .app
#
# 前置条件：
#   - Rust 工具链 (rustc + cargo)
#   - cargo-tauri (cargo install tauri-cli --version "^2")
#   - crablet 主项目已编译 (target/release/crablet)
#
# 代码签名：
#   - 无 Apple Developer 证书时：自动 Ad-hoc 签名（macOS 会弹出安全警告，但可运行）
#   - 有证书时：设置 CODE_SIGN_IDENTITY 环境变量，如：
#     CODE_SIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" ./build-desktop.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DESKTOP_DIR="${PROJECT_ROOT}/desktop"
TARGET_DIR="${PROJECT_ROOT}/target/release"
BUNDLE_DIR="${TARGET_DIR}/bundle"
APP_NAME="Crablet"
APP_VERSION="0.1.0"
ARCH=$(uname -m)
SIDECAR_TARGET="crablet-aarch64-apple-darwin"

# macOS 架构检测
if [ "${ARCH}" = "arm64" ]; then
    SIDECAR_TARGET="crablet-aarch64-apple-darwin"
elif [ "${ARCH}" = "x86_64" ]; then
    SIDECAR_TARGET="crablet-x86_64-apple-darwin"
fi

echo "🦀 Crablet 桌面打包 v2"
echo "   架构: ${ARCH}"
echo "   Sidecar target: ${SIDECAR_TARGET}"
echo ""

# ─── Step 0: 构建前端 SPA ───
echo "📦 Step 0: 构建前端 SPA..."
FRONTEND_DIR="${PROJECT_ROOT}/frontend"
FRONTEND_DIST="${PROJECT_ROOT}/frontend/dist"
if [ ! -d "${FRONTEND_DIST}" ] || [ ! -f "${FRONTEND_DIST}/index.html" ]; then
    if [ -f "${FRONTEND_DIR}/package.json" ] && command -v npm &>/dev/null; then
        echo "   🔨 前端产物缺失，正在构建..."
        cd "${FRONTEND_DIR}"
        npm install
        npm run build
        cd "${PROJECT_ROOT}"
    else
        echo "   ⚠️ 警告: 前端产物 ${FRONTEND_DIST} 不存在，且无法自动构建（缺少 npm 或 package.json）"
        echo "   ⚠️ 桌面端启动后可能无法显示前端界面。请手动构建：cd frontend && npm install && npm run build"
    fi
else
    echo "   ✅ 前端产物已就绪 (${FRONTEND_DIST})"
fi
echo ""

# ─── Step 1: 检查 crablet 主二进制 ───
if [ ! -f "${TARGET_DIR}/crablet" ]; then
    echo "📦 编译 crablet 主二进制 (release)..."
    cd "${PROJECT_ROOT}" && cargo build --release -p crablet
else
    echo "✅ crablet 主二进制已就绪 ($(du -sh "${TARGET_DIR}/crablet" | cut -f1))"
fi

# ─── Step 2: 复制 sidecar 二进制 ───
# Tauri v2 要求 sidecar 二进制文件名包含 target triple 后缀。
echo "📋 复制 sidecar 二进制..."
mkdir -p "${DESKTOP_DIR}/binaries"
cp "${TARGET_DIR}/crablet" "${DESKTOP_DIR}/binaries/${SIDECAR_TARGET}"
chmod +x "${DESKTOP_DIR}/binaries/${SIDECAR_TARGET}"
echo "   ✅ 已复制到: ${DESKTOP_DIR}/binaries/${SIDECAR_TARGET}"

# ─── Step 3: 构建 Tauri 应用 ───
BUILD_MODE="${1:---all}"

if [ "${BUILD_MODE}" = "--dmg" ]; then
    echo "📦 跳过编译，直接创建 DMG..."
elif [ "${BUILD_MODE}" = "--sign" ]; then
    echo "🔏 跳过编译，直接签名..."
elif [ "${BUILD_MODE}" = "--app" ] || [ "${BUILD_MODE}" = "--all" ]; then
    echo "🔨 构建 Tauri 应用..."
    cd "${DESKTOP_DIR}" && cargo tauri build --bundles app
    echo "✅ .app 构建完成: ${BUNDLE_DIR}/macos/${APP_NAME}.app"
fi

# ─── Step 4: 修复 sidecar 路径 ───
# Tauri 构建后，sidecar 二进制可能被放在不同位置。
# 我们需要确保所有可能的位置都有带 target triple 的二进制。
APP_PATH="${BUNDLE_DIR}/macos/${APP_NAME}.app"

if [ -d "${APP_PATH}" ]; then
    echo "🔧 修复 sidecar 路径..."

    # 定义所有需要检查的目录
    SIDEAR_DIRS=(
        "${APP_PATH}/Contents/MacOS/binaries"
        "${APP_PATH}/Contents/Resources/binaries"
        "${APP_PATH}/Contents/MacOS"
    )

    for dir in "${SIDEAR_DIRS[@]}"; do
        if [ -d "${dir}" ]; then
            # 检查是否已有带 triple 的二进制
            if [ -f "${dir}/${SIDECAR_TARGET}" ]; then
                echo "   ✅ ${dir}/${SIDECAR_TARGET} 已存在"
            # 检查是否有不带 triple 的 crablet 二进制（需要排除 crablet-desktop）
            elif [ -f "${dir}/crablet" ]; then
                # 确认这是 sidecar 而非 Tauri 主二进制
                FILE_SIZE=$(stat -f%z "${dir}/crablet" 2>/dev/null || stat -c%s "${dir}/crablet" 2>/dev/null || echo 0)
                if [ "${FILE_SIZE}" -gt 1000000 ]; then
                    # 大于 1MB，很可能是 sidecar 二进制
                    echo "   创建硬链接: ${dir}/${SIDECAR_TARGET} -> crablet (${FILE_SIZE} bytes)"
                    ln -f "${dir}/crablet" "${dir}/${SIDECAR_TARGET}"
                fi
            fi
        fi
    done

    echo "   ✅ sidecar 路径修复完成"
fi

# ─── Step 5: 代码签名 ───
if [ "${BUILD_MODE}" != "--dmg" ] && [ -d "${APP_PATH}" ]; then
    echo "🔏 代码签名..."

    SIGN_IDENTITY="${CODE_SIGN_IDENTITY:--}"

    if [ "${SIGN_IDENTITY}" = "-" ]; then
        echo "   ⚠️  未设置 CODE_SIGN_IDENTITY，使用 Ad-hoc 签名"
        echo "   （macOS 首次运行会弹出安全警告，右键→打开即可）"
    else
        echo "   🔑 使用证书: ${SIGN_IDENTITY}"
    fi

    codesign --force --deep --sign "${SIGN_IDENTITY}" "${APP_PATH}" 2>&1
    codesign --verify --deep --strict "${APP_PATH}" 2>&1
    echo "✅ 签名完成"
fi

# ─── Step 6: 创建 DMG ───
if [ "${BUILD_MODE}" = "--dmg" ] || [ "${BUILD_MODE}" = "--all" ]; then
    DMG_DIR="${BUNDLE_DIR}/dmg"
    DMG_PATH="${DMG_DIR}/${APP_NAME}_${APP_VERSION}_aarch64.dmg"

    if [ ! -d "${APP_PATH}" ]; then
        echo "❌ .app 不存在，请先运行 --app 或 --all"
        exit 1
    fi

    echo "📦 创建 DMG 安装包..."

    # 清理旧文件
    rm -rf "${DMG_PATH}" 2>/dev/null
    mkdir -p "${DMG_DIR}"

    # 创建临时 staging 目录
    STAGING="${PROJECT_ROOT}/.session_tmps/dmg-staging"
    rm -rf "${STAGING}" 2>/dev/null
    mkdir -p "${STAGING}"

    # 复制 .app 和 Applications 快捷方式
    cp -R "${APP_PATH}" "${STAGING}/"
    ln -sf /Applications "${STAGING}/Applications"

    # 使用 hdiutil 创建 DMG（一步法，直接创建压缩 DMG）
    hdiutil create \
        -volname "${APP_NAME}" \
        -srcfolder "${STAGING}" \
        -ov \
        -format UDZO \
        "${DMG_PATH}"

    # 清理 staging
    rm -rf "${STAGING}"

    DMG_SIZE=$(du -sh "${DMG_PATH}" | cut -f1)
    echo "✅ DMG 创建完成: ${DMG_PATH} (${DMG_SIZE})"
fi

echo ""
echo "🎉 打包完成！"
echo "   .app: ${BUNDLE_DIR}/macos/${APP_NAME}.app"
if [ -f "${BUNDLE_DIR}/dmg/${APP_NAME}_${APP_VERSION}_aarch64.dmg" ]; then
    echo "   DMG:  ${BUNDLE_DIR}/dmg/${APP_NAME}_${APP_VERSION}_aarch64.dmg"
fi
echo ""
echo "💡 提示："
echo "   - Ad-hoc 签名的应用首次打开需右键→打开"
echo "   - 如需正式签名，设置 CODE_SIGN_IDENTITY 环境变量后重新运行"
echo "   - Windows 打包需在 Windows 环境下运行（见 .github/workflows/build-desktop.yml）"
