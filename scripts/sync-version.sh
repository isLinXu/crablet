#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
# sync-version.sh — Crablet 版本号同步工具
#
# 从 crablet/Cargo.toml（单一真相源）读取版本号，
# 同步到所有需要版本号的配置文件。
#
# 用法:
#   ./scripts/sync-version.sh              # 同步到所有目标文件
#   ./scripts/sync-version.sh --check      # 只检查，不修改（CI 用）
# ═══════════════════════════════════════════════════════════════════════════

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# 从 crablet/Cargo.toml 读取版本号
SOURCE="$PROJECT_ROOT/crablet/Cargo.toml"
VERSION=$(grep -m1 '^version' "$SOURCE" | sed 's/.*= "\([^"]*\)".*/\1/')

if [[ -z "$VERSION" ]]; then
    echo "❌ 无法从 $SOURCE 读取版本号"
    exit 1
fi

CHECK_ONLY=false
[[ "${1:-}" == "--check" ]] && CHECK_ONLY=true

echo "📌 版本号: $VERSION (来源: crablet/Cargo.toml)"
echo ""

# 需要同步的目标文件
declare -a TARGETS=(
    "desktop/Cargo.toml|version = \"${VERSION}\""
    "desktop/tauri.conf.json|\"version\": \"${VERSION}\""
    "desktop/package.json|\"version\": \"${VERSION}\""
)

CHANGES=0

for entry in "${TARGETS[@]}"; do
    file="${entry%%|*}"
    pattern="${entry##*|}"
    filepath="$PROJECT_ROOT/$file"

    if [[ ! -f "$filepath" ]]; then
        echo "  ⚠️  跳过（文件不存在）: $file"
        continue
    fi

    # 提取当前版本
    current=$(grep -oE '"?version"?\s*[=:]\s*"([^"]+)"' "$filepath" | head -1 | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' || echo "")

    if [[ "$current" == "$VERSION" ]]; then
        echo "  ✅ $file ($VERSION)"
        continue
    fi

    if [[ "$CHECK_ONLY" == "true" ]]; then
        echo "  ❌ $file: $current → $VERSION (需要更新)"
        CHANGES=$((CHANGES + 1))
        continue
    fi

    # 执行替换
    if [[ "$file" == *"Cargo.toml" ]]; then
        # Cargo.toml: version = "x.y.z"
        sed -i.bak "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" "$filepath"
    elif [[ "$file" == *"tauri.conf.json" ]]; then
        # JSON: "version": "x.y.z"
        sed -i.bak 's/"version": "[^"]*"/"version": "'"$VERSION"'"/' "$filepath"
    elif [[ "$file" == *"package.json" ]]; then
        # JSON: "version": "x.y.z"
        sed -i.bak 's/"version": "[^"]*"/"version": "'"$VERSION"'"/' "$filepath"
    fi
    rm -f "$filepath.bak"
    echo "  🔧 $file: $current → $VERSION"
done

echo ""
if [[ "$CHECK_ONLY" == "true" ]]; then
    if [[ "$CHANGES" -gt 0 ]]; then
        echo "❌ $CHANGES 个文件版本不一致，运行 ./scripts/sync-version.sh 修复"
        exit 1
    else
        echo "✅ 所有文件版本一致 ($VERSION)"
    fi
else
    echo "✅ 版本同步完成 ($VERSION)"
fi
