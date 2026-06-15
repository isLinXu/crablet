#!/usr/bin/env bash
# ============================================================
# generate-icons.sh — 从源 PNG 生成 Tauri 桌面端全套图标
# 使用 macOS sips + ImageMagick (可选)，支持 Linux/macOS
# ============================================================

set -euo pipefail

SOURCE_PNG="${1:-/Users/gatilin/Downloads/微信图片_20260601105735_9720_209.png}"
OUT_DIR="${2:-$(pwd)/icons}"

if [ ! -f "$SOURCE_PNG" ]; then
    echo "❌ 源 PNG 不存在: $SOURCE_PNG"
    echo "用法: $0 <源 PNG 路径> [输出目录]"
    exit 1
fi

mkdir -p "$OUT_DIR"
echo "🎨 图标源: $SOURCE_PNG"
echo "📁 输出目录: $OUT_DIR"

# --------------------------------------------------
# 1) 生成 macOS iconset 所需尺寸（16~1024）
# --------------------------------------------------
MAC_SIZES=(16 32 64 128 256 512 1024)
ICONSET_DIR="$OUT_DIR/icon.iconset"
mkdir -p "$ICONSET_DIR"

for size in "${MAC_SIZES[@]}"; do
    if command -v sips &>/dev/null; then
        sips -z "$size" "$size" "$SOURCE_PNG" --out "$ICONSET_DIR/icon_${size}x${size}.png" >/dev/null 2>&1
        echo "  ✅ macOS ${size}x${size}"
    elif command -v magick &>/dev/null; then
        magick "$SOURCE_PNG" -resize "${size}x${size}!" "$ICONSET_DIR/icon_${size}x${size}.png"
        echo "  ✅ macOS ${size}x${size} (ImageMagick)"
    else
        echo "  ⚠️ 跳过 macOS ${size}x${size} — 缺少 sips/ImageMagick"
    fi
done

# 生成 iconset 中 @2x 变体（如果 sips 可用）
if command -v iconutil &>/dev/null && [ -d "$ICONSET_DIR" ]; then
    for size in 16 32 128 256 512; do
        double=$((size * 2))
        cp "$ICONSET_DIR/icon_${double}x${double}.png" "$ICONSET_DIR/icon_${size}x${size}@2x.png" 2>/dev/null || true
    done
    iconutil -c icns "$ICONSET_DIR" -o "$OUT_DIR/icon.icns" 2>/dev/null || echo "  ⚠️ iconutil 失败，保留 iconset 目录"
    echo "  ✅ icon.icns"
fi

# --------------------------------------------------
# 2) 生成 Windows .ico（16,32,48,64,128,256 全尺寸）
# --------------------------------------------------
WIN_SIZES=(16 32 48 64 128 256)
TMP_ICO_DIR="$OUT_DIR/.ico_tmp"
mkdir -p "$TMP_ICO_DIR"

for size in "${WIN_SIZES[@]}"; do
    if command -v sips &>/dev/null; then
        sips -z "$size" "$size" "$SOURCE_PNG" --out "$TMP_ICO_DIR/icon_${size}.png" >/dev/null 2>&1
    elif command -v magick &>/dev/null; then
        magick "$SOURCE_PNG" -resize "${size}x${size}!" "$TMP_ICO_DIR/icon_${size}.png"
    fi
done

if command -v magick &>/dev/null; then
    magick "$TMP_ICO_DIR"/*.png "$OUT_DIR/icon.ico"
    echo "  ✅ icon.ico (全尺寸: ${WIN_SIZES[*]})"
else
    echo "  ⚠️ 缺少 ImageMagick，无法生成 icon.ico — 请手动使用 Python 脚本生成"
fi
rm -rf "$TMP_ICO_DIR"

# --------------------------------------------------
# 3) 生成通用 PNG 图标（Tauri bundle 用）
# --------------------------------------------------
TAURI_SIZES=(32 64 128)
for size in "${TAURI_SIZES[@]}"; do
    if command -v sips &>/dev/null; then
        sips -z "$size" "$size" "$SOURCE_PNG" --out "$OUT_DIR/${size}x${size}.png" >/dev/null 2>&1
    elif command -v magick &>/dev/null; then
        magick "$SOURCE_PNG" -resize "${size}x${size}!" "$OUT_DIR/${size}x${size}.png"
    fi
    echo "  ✅ ${size}x${size}.png"
done

# 128x128@2x
if command -v sips &>/dev/null; then
    sips -z 256 256 "$SOURCE_PNG" --out "$OUT_DIR/128x128@2x.png" >/dev/null 2>&1
elif command -v magick &>/dev/null; then
    magick "$SOURCE_PNG" -resize 256x256! "$OUT_DIR/128x128@2x.png"
fi
echo "  ✅ 128x128@2x.png"

# 主 icon.png (1024×1024)
if command -v sips &>/dev/null; then
    sips -z 1024 1024 "$SOURCE_PNG" --out "$OUT_DIR/icon.png" >/dev/null 2>&1
elif command -v magick &>/dev/null; then
    magick "$SOURCE_PNG" -resize 1024x1024! "$OUT_DIR/icon.png"
fi
echo "  ✅ icon.png (1024×1024)"

# --------------------------------------------------
# 4) Windows Store 全套 PNGs
# --------------------------------------------------
STORE_SIZES=(30 44 71 89 107 142 150 284 310)
for size in "${STORE_SIZES[@]}"; do
    if command -v sips &>/dev/null; then
        sips -z "$size" "$size" "$SOURCE_PNG" --out "$OUT_DIR/Square${size}x${size}Logo.png" >/dev/null 2>&1
    elif command -v magick &>/dev/null; then
        magick "$SOURCE_PNG" -resize "${size}x${size}!" "$OUT_DIR/Square${size}x${size}Logo.png"
    fi
    echo "  ✅ Square${size}x${size}Logo.png"
done

# StoreLogo (50x50)
if command -v sips &>/dev/null; then
    sips -z 50 50 "$SOURCE_PNG" --out "$OUT_DIR/StoreLogo.png" >/dev/null 2>&1
elif command -v magick &>/dev/null; then
    magick "$SOURCE_PNG" -resize 50x50! "$OUT_DIR/StoreLogo.png"
fi
echo "  ✅ StoreLogo.png"

# --------------------------------------------------
# 5) Android / iOS mipmap（可选）
# --------------------------------------------------
ANDROID_DIR="$OUT_DIR/android"
mkdir -p "$ANDROID_DIR/mipmap-mdpi" "$ANDROID_DIR/mipmap-hdpi" \
         "$ANDROID_DIR/mipmap-xhdpi" "$ANDROID_DIR/mipmap-xxhdpi" \
         "$ANDROID_DIR/mipmap-xxxhdpi"

ANDROID_SIZES=(48 72 96 144 192)
ANDROID_DIRS=(mipmap-mdpi mipmap-hdpi mipmap-xhdpi mipmap-xxhdpi mipmap-xxxhdpi)

for i in "${!ANDROID_SIZES[@]}"; do
    size="${ANDROID_SIZES[$i]}"
    dir="$ANDROID_DIR/${ANDROID_DIRS[$i]}"
    if command -v sips &>/dev/null; then
        sips -z "$size" "$size" "$SOURCE_PNG" --out "$dir/ic_launcher.png" >/dev/null 2>&1
    elif command -v magick &>/dev/null; then
        magick "$SOURCE_PNG" -resize "${size}x${size}!" "$dir/ic_launcher.png"
    fi
    echo "  ✅ Android ${ANDROID_DIRS[$i]} (${size}x${size})"
done

echo ""
echo "🎉 全部图标生成完成！输出: $OUT_DIR"
