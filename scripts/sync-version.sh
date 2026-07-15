#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SOURCE="$PROJECT_ROOT/crablet/Cargo.toml"
CHECK_ONLY=false
OVERRIDE_VERSION=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --check) CHECK_ONLY=true; shift ;;
    --version) [[ $# -ge 2 ]] || { echo "--version 缺少值" >&2; exit 2; }; OVERRIDE_VERSION="${2#v}"; shift 2 ;;
    -h|--help) echo "用法: $0 [--check] [--version <semver>]"; exit 0 ;;
    *) echo "未知参数: $1" >&2; exit 2 ;;
  esac
done
SOURCE_VERSION=$(grep -m1 '^version[[:space:]]*=' "$SOURCE" | sed 's/.*= "\([^"]*\)".*/\1/')
VERSION="${OVERRIDE_VERSION:-$SOURCE_VERSION}"
SEMVER_RE='^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$'
[[ "$VERSION" =~ $SEMVER_RE ]] || { echo "非法 SemVer: ${VERSION:-<empty>}" >&2; exit 1; }
TARGETS=("desktop/Cargo.toml" "desktop/tauri.conf.json" "desktop/package.json")
CHANGES=0
read_version() { if [[ "$1" == *.toml ]]; then grep -m1 '^version[[:space:]]*=' "$1" | sed 's/.*= "\([^"]*\)".*/\1/'; else grep -m1 -E '^[[:space:]]*"version"[[:space:]]*:' "$1" | sed 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/'; fi; }
write_version() { if [[ "$1" == *.toml ]]; then sed -i.bak "0,/^version[[:space:]]*=/{s/^version[[:space:]]*=.*/version = \"$VERSION\"/;}" "$1"; else sed -i.bak "0,/^[[:space:]]*\"version\"[[:space:]]*:/{s/\(\"version\"[[:space:]]*:[[:space:]]*\)\"[^\"]*\"/\1\"$VERSION\"/;}" "$1"; fi; rm -f "$1.bak"; }
for file in "${TARGETS[@]}"; do
  filepath="$PROJECT_ROOT/$file"; [[ -f "$filepath" ]] || { echo "目标文件不存在: $file" >&2; exit 1; }
  current="$(read_version "$filepath")"
  if [[ "$current" == "$VERSION" ]]; then echo "✅ $file ($VERSION)"; elif [[ "$CHECK_ONLY" == true ]]; then echo "❌ $file: $current → $VERSION"; CHANGES=$((CHANGES+1)); else write_version "$filepath"; echo "🔧 $file: $current → $VERSION"; fi
done
if [[ "$CHECK_ONLY" == true && "$CHANGES" -gt 0 ]]; then exit 1; fi
echo "✅ 版本${CHECK_ONLY:+校验}完成 ($VERSION)"
