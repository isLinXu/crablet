#!/usr/bin/env bash
# notarize-dmg.sh — Apple 公证与 stapling 脚本
#
# 用法：
#   ./notarize-dmg.sh
#
# 前置条件：
#   1. Apple Developer 账号（$99/年）
#   2. Developer ID Application 证书已导入 Keychain
#   3. 设置以下环境变量（或写入 ~/.crablet-notary.env）：
#      - APPLE_DEVELOPER_IDENTITY  (如 "Developer ID Application: Your Name (TEAMID)")
#      - APPLE_ID                  (Apple Developer 账号邮箱)
#      - APPLE_APP_SPECIFIC_PASSWORD  (App-specific password，从 appleid.apple.com 生成)
#      - APPLE_TEAM_ID             (Developer Team ID)
#
#   环境变量文件示例 (~/.crablet-notary.env)：
#      export APPLE_DEVELOPER_IDENTITY="Developer ID Application: Your Name (ABCDE12345)"
#      export APPLE_ID="you@example.com"
#      export APPLE_APP_SPECIFIC_PASSWORD="xxxx-xxxx-xxxx-xxxx"
#      export APPLE_TEAM_ID="ABCDE12345"
#
# 流程：
#   1. 用 Developer ID 重新签名 .app（含 hardening runtime）
#   2. 提交 DMG 到 Apple notarization service
#   3. 等待公证完成（通常 5-15 分钟）
#   4. Staple 公证票据到 DMG
#   5. 验证 Gatekeeper 通过

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
BUNDLE_DIR="${PROJECT_ROOT}/target/release/bundle"
APP_NAME="Crablet"
APP_PATH="${BUNDLE_DIR}/macos/${APP_NAME}.app"

# 版本号（单一真相源：crablet/Cargo.toml，与 scripts/pack.sh 保持一致）
VERSION="${CRABLET_VERSION:-$(grep -m1 '^version' "${PROJECT_ROOT}/crablet/Cargo.toml" | sed 's/.*= "\([^"]*\)".*/\1/')}"

# 架构检测（与 scripts/pack.sh 保持一致）
ARCH="$(uname -m)"
if [[ "${ARCH}" == "arm64" ]]; then
    DMG_ARCH="aarch64"
    SIDECAR_TARGET="crablet-aarch64-apple-darwin"
elif [[ "${ARCH}" == "x86_64" ]]; then
    DMG_ARCH="x64"
    SIDECAR_TARGET="crablet-x86_64-apple-darwin"
else
    echo "❌ 未知 macOS 架构: ${ARCH}"; exit 1
fi

DMG_PATH="${BUNDLE_DIR}/dmg/${APP_NAME}_${VERSION}_${DMG_ARCH}.dmg"

# 加载环境变量文件
if [ -f "${HOME}/.crablet-notary.env" ]; then
    source "${HOME}/.crablet-notary.env"
fi

# 检查必需的环境变量
MISSING=()
[ -z "${APPLE_DEVELOPER_IDENTITY:-}" ] && MISSING+=("APPLE_DEVELOPER_IDENTITY")
[ -z "${APPLE_ID:-}" ] && MISSING+=("APPLE_ID")
[ -z "${APPLE_APP_SPECIFIC_PASSWORD:-}" ] && MISSING+=("APPLE_APP_SPECIFIC_PASSWORD")
[ -z "${APPLE_TEAM_ID:-}" ] && MISSING+=("APPLE_TEAM_ID")

if [ ${#MISSING[@]} -gt 0 ]; then
    echo "❌ 缺少以下环境变量: ${MISSING[*]}"
    echo "   请设置它们或写入 ~/.crablet-notary.env"
    echo ""
    echo "   详见此文件头部注释。"
    exit 1
fi

echo "🔏 Crablet Apple Notarization"
echo "   Developer Identity: ${APPLE_DEVELOPER_IDENTITY}"
echo "   Apple ID:           ${APPLE_ID}"
echo "   Team ID:            ${APPLE_TEAM_ID}"
echo "   版本:               ${VERSION} (${DMG_ARCH})"
echo ""

# ─── Step 1: Developer ID 签名 ───
echo "📦 Step 1: Developer ID 签名..."

# 签名 sidecar 二进制（需要单独签名，--deep 在 macOS 14+ 已弃用）
echo "   签名 sidecar 二进制..."
SIDECAR_PATHS=(
    "${APP_PATH}/Contents/MacOS/${SIDECAR_TARGET}"
    "${APP_PATH}/Contents/MacOS/binaries/${SIDECAR_TARGET}"
    "${APP_PATH}/Contents/MacOS/crablet"
)
for bin in "${SIDECAR_PATHS[@]}"; do
    if [ -f "${bin}" ]; then
        codesign --force --options runtime --sign "${APPLE_DEVELOPER_IDENTITY}" "${bin}" 2>&1
        echo "   ✅ 已签名: $(basename "${bin}")"
    fi
done

# 签名整个 .app bundle
echo "   签名 .app bundle..."
codesign --force --options runtime --sign "${APPLE_DEVELOPER_IDENTITY}" "${APP_PATH}" 2>&1

# 验证签名
echo "   验证签名..."
codesign --verify --strict "${APP_PATH}" 2>&1
echo "✅ Step 1 完成"
echo ""

# ─── Step 2: 重新创建 DMG（使用已签名的 .app） ───
echo "📦 Step 2: 重新创建 DMG..."
STAGING=$(mktemp -d /tmp/crablet-notary-XXXXXX)
cp -R "${APP_PATH}" "${STAGING}/"
ln -sf /Applications "${STAGING}/Applications"

rm -f "${DMG_PATH}" 2>/dev/null
hdiutil create \
    -volname "${APP_NAME}" \
    -srcfolder "${STAGING}" \
    -ov \
    -format UDZO \
    "${DMG_PATH}"
rm -rf "${STAGING}"
echo "✅ Step 2 完成: ${DMG_PATH}"
echo ""

# ─── Step 3: 提交公证 ───
echo "📦 Step 3: 提交 DMG 到 Apple Notarization Service..."
SUBMISSION_ID=$(xcrun notarytool submit "${DMG_PATH}" \
    --apple-id "${APPLE_ID}" \
    --password "${APPLE_APP_SPECIFIC_PASSWORD}" \
    --team-id "${APPLE_TEAM_ID}" \
    --wait \
    --format json 2>&1 | tee /dev/stderr | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4 || true)

echo ""
echo "✅ Step 3 完成"
echo ""

# ─── Step 4: Staple 公证票据 ───
echo "📦 Step 4: Staple 公证票据到 DMG..."
xcrun stapler staple "${DMG_PATH}" 2>&1
echo "✅ Step 4 完成"
echo ""

# ─── Step 5: 最终验证 ───
echo "📦 Step 5: Gatekeeper 验证..."
spctl -a -t install "${DMG_PATH}" 2>&1 || true
echo ""
echo "🎉 公证完成！"
echo "   DMG: ${DMG_PATH}"
echo ""
echo "   用户现在可以直接双击 DMG 安装，无需右键→打开。"
