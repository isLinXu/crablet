#!/usr/bin/env bash
# build-desktop.sh — [已弃用] 请使用 scripts/pack.sh
#
# 本脚本已被 scripts/pack.sh（v3 统一打包脚本）取代，保留此文件仅为向后兼容旧文档/命令。
# 所有参数会被转发到 scripts/pack.sh，行为完全一致。
#
# 迁移指南：
#   ./desktop/build-desktop.sh --all   →  ./scripts/pack.sh
#   ./desktop/build-desktop.sh --app   →  ./scripts/pack.sh --app-only
#   ./desktop/build-desktop.sh --dmg   →  ./scripts/pack.sh --dmg-only
#   ./desktop/build-desktop.sh --sign  →  ./scripts/pack.sh --sign-only

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "⚠️  desktop/build-desktop.sh 已弃用，转发到 scripts/pack.sh ..." >&2
echo "   建议今后直接使用: ./scripts/pack.sh [选项]" >&2
echo "" >&2

ARG="${1:---all}"
case "$ARG" in
    --all)  exec bash "${PROJECT_ROOT}/scripts/pack.sh" ;;
    --app)  exec bash "${PROJECT_ROOT}/scripts/pack.sh" --app-only ;;
    --dmg)  exec bash "${PROJECT_ROOT}/scripts/pack.sh" --dmg-only ;;
    --sign) exec bash "${PROJECT_ROOT}/scripts/pack.sh" --sign-only ;;
    *)      exec bash "${PROJECT_ROOT}/scripts/pack.sh" "$@" ;;
esac
