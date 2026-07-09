#!/usr/bin/env bash
# ============================================================
# build-release.sh — [已弃用] 请使用 scripts/pack.sh
#
# 本脚本已被 scripts/pack.sh（v3 统一打包脚本）取代。
# 所有参数会被转发到 scripts/pack.sh，行为完全一致。
#
# 迁移指南：
#   ./scripts/build-release.sh           →  ./scripts/pack.sh
#   ./scripts/build-release.sh --quick   →  ./scripts/pack.sh --quick
#   ./scripts/build-release.sh --app     →  ./scripts/pack.sh --app-only
#   ./scripts/build-release.sh --dmg     →  ./scripts/pack.sh --dmg-only
#   ./scripts/build-release.sh --sign    →  ./scripts/pack.sh --sign-only
#
# 多平台交叉编译 sidecar（旧脚本的核心功能）可通过环境变量实现：
#   rustup target add <target>
#   cargo build --release -p crablet --target <target>
#   cp target/<target>/release/crablet desktop/binaries/crablet-<target>
# ============================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "⚠️  scripts/build-release.sh 已弃用，转发到 scripts/pack.sh ..." >&2
echo "   建议今后直接使用: ./scripts/pack.sh [选项]" >&2
echo "" >&2

ARG="${1:---all}"
case "$ARG" in
    --all)  exec bash "${PROJECT_ROOT}/scripts/pack.sh" ;;
    --quick) exec bash "${PROJECT_ROOT}/scripts/pack.sh" --quick ;;
    --app)  exec bash "${PROJECT_ROOT}/scripts/pack.sh" --app-only ;;
    --dmg)  exec bash "${PROJECT_ROOT}/scripts/pack.sh" --dmg-only ;;
    --sign) exec bash "${PROJECT_ROOT}/scripts/pack.sh" --sign-only ;;
    *)      exec bash "${PROJECT_ROOT}/scripts/pack.sh" "$@" ;;
esac
