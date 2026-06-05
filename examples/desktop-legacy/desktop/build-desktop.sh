#!/usr/bin/env bash
# ──────────────────────────────────────────────────────────────
# Crablet Desktop Build Script
#
# Builds the Tauri desktop app with sidecar, creates DMG/NSIS,
# and handles code signing (Ad-hoc by default, Developer ID via
# CODE_SIGN_IDENTITY env var).
#
# Usage:
#   ./build-desktop.sh                    # Ad-hoc signing
#   CODE_SIGN_IDENTITY="Developer ID Application: Your Name (TEAM_ID)" ./build-desktop.sh
# ──────────────────────────────────────────────────────────────
set -euo pipefail

# ─── Configuration ────────────────────────────────────────────
APP_NAME="Crablet"
APP_VERSION="0.1.0"
BUNDLE_ID="com.crablet.app"

# Sidecar binary
SIDECAR_TARGET_TRIPLE="${SIDECAR_TARGET_TRIPLE:-aarch64-apple-darwin}"
SIDECAR_BIN="desktop/binaries/crablet-${SIDECAR_TARGET_TRIPLE}"

# Output paths
DMG_DIR="target/release/bundle/dmg"
DMG_NAME="Crablet_${APP_VERSION}_aarch64.dmg"

# ─── Colors ────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}╔══════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  Crablet Desktop Build v${APP_VERSION}            ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════╝${NC}"

# ─── Step 1: Verify sidecar binary ───────────────────────────
echo -e "\n${BLUE}[1/5]${NC} Checking sidecar binary..."
if [ ! -f "${SIDECAR_BIN}" ]; then
    echo -e "${YELLOW}Sidecar binary not found at ${SIDECAR_BIN}${NC}"
    echo -e "${YELLOW}Building sidecar from source...${NC}"
    cargo build --release --target "${SIDECAR_TARGET_TRIPLE}"
    mkdir -p desktop/binaries
    cp "target/${SIDECAR_TARGET_TRIPLE}/release/crablet" "${SIDECAR_BIN}"
fi
echo -e "${GREEN}✓${NC} Sidecar binary ready: ${SIDECAR_BIN}"

# ─── Step 2: Code signing configuration ──────────────────────
echo -e "\n${BLUE}[2/5]${NC} Configuring code signing..."
CODE_SIGN_IDENTITY="${CODE_SIGN_IDENTITY:-}"

if [ -z "${CODE_SIGN_IDENTITY}" ]; then
    echo -e "${YELLOW}No CODE_SIGN_IDENTITY set → Ad-hoc signing${NC}"
    SIGN_CMD="codesign --sign -"
else
    echo -e "${GREEN}Using Developer ID: ${CODE_SIGN_IDENTITY}${NC}"
    SIGN_CMD="codesign --sign '${CODE_SIGN_IDENTITY}' --force --options runtime"
fi

# ─── Step 3: Build Tauri app ────────────────────────────────
echo -e "\n${BLUE}[3/5]${NC} Building Tauri app..."
export CRABLET_RESOURCE_DIR="${CRABLET_RESOURCE_DIR:-$(pwd)/desktop/binaries}"
cargo tauri build --target "${SIDECAR_TARGET_TRIPLE}"
echo -e "${GREEN}✓${NC} Tauri build complete"

# ─── Step 4: Code sign the .app bundle ──────────────────────
echo -e "\n${BLUE}[4/5]${NC} Code signing..."
APP_BUNDLE=$(find "${DMG_DIR}" -name "*.app" -maxdepth 1 | head -1)

if [ -n "${APP_BUNDLE}" ] && [ -d "${APP_BUNDLE}" ]; then
    echo "Signing: ${APP_BUNDLE}"
    ${SIGN_CMD} --deep "${APP_BUNDLE}"
    echo -e "${GREEN}✓${NC} Code signing complete"
else
    echo -e "${YELLOW}⚠ No .app bundle found in ${DMG_DIR}${NC}"
fi

# ─── Step 5: Create DMG ─────────────────────────────────────
echo -e "\n${BLUE}[5/5]${NC} Creating DMG..."
if command -v create-dmg &>/dev/null; then
    create-dmg \
        --volname "${APP_NAME}" \
        --app-drop-link /Applications \
        "${DMG_DIR}/${DMG_NAME}" \
        "${DMG_DIR}/"
elif command -v hdiutil &>/dev/null; then
    hdiutil create -format UDZO \
        -volname "${APP_NAME}" \
        -srcfolder "${DMG_DIR}" \
        -o "${DMG_DIR}/${DMG_NAME}"
else
    echo -e "${YELLOW}⚠ Neither create-dmg nor hdiutil found, skipping DMG creation${NC}"
fi

# Sign the DMG itself
if [ -f "${DMG_DIR}/${DMG_NAME}" ]; then
    ${SIGN_CMD} "${DMG_DIR}/${DMG_NAME}"
    echo -e "${GREEN}✓${NC} DMG signed: ${DMG_DIR}/${DMG_NAME}"
fi

# ─── Summary ──────────────────────────────────────────────────
echo -e "\n${GREEN}╔══════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║  Build Complete!                         ║${NC}"
echo -e "${GREEN}╚══════════════════════════════════════════╝${NC}"
echo ""
echo "  App:     ${APP_BUNDLE:-N/A}"
echo "  DMG:     ${DMG_DIR}/${DMG_NAME}"
echo "  Sidecar: ${SIDECAR_BIN}"
echo "  Signing: ${CODE_SIGN_IDENTITY:-(Ad-hoc)}"
echo ""
echo -e "${YELLOW}Note: DMG must be dragged to /Applications before running${NC}"
echo -e "${YELLOW}      (read-only volume blocks sidecar launch)${NC}"
