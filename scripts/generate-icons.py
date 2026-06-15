#!/usr/bin/env python3
# ============================================================
# generate-icons.py — 从源 PNG 生成 Tauri 桌面端全套图标
# 纯 Python 实现，依赖 Pillow（pip install pillow）
# 跨平台: macOS / Linux / Windows
# ============================================================
import sys
import os
import io
import struct
from pathlib import Path
from PIL import Image

# --------------------------------------------------
# 配置
# --------------------------------------------------
SOURCE_PNG = Path("/Users/gatilin/Downloads/微信图片_20260601105735_9720_209.png")
OUT_DIR = Path(__file__).parent.parent / "desktop" / "icons"

if len(sys.argv) >= 2:
    SOURCE_PNG = Path(sys.argv[1])
if len(sys.argv) >= 3:
    OUT_DIR = Path(sys.argv[2])

if not SOURCE_PNG.exists():
    print(f"❌ 源 PNG 不存在: {SOURCE_PNG}")
    print(f"用法: python3 {sys.argv[0]} [源 PNG 路径] [输出目录]")
    sys.exit(1)

OUT_DIR.mkdir(parents=True, exist_ok=True)
print(f"🎨 图标源: {SOURCE_PNG}")
print(f"📁 输出目录: {OUT_DIR}")


def resize_png(src: Path, dst: Path, width: int, height: int):
    """将源 PNG 缩放到指定尺寸，高质量重采样。"""
    with Image.open(src) as img:
        img = img.convert("RGBA")
        resized = img.resize((width, height), Image.Resampling.LANCZOS)
        resized.save(dst, "PNG")
    print(f"  ✅ {dst.name} ({width}x{height})")


def make_ico(png_paths: list[Path], ico_path: Path):
    """将多个 PNG 合并为多尺寸 Windows ICO 文件。"""
    images = []
    for p in png_paths:
        with Image.open(p) as img:
            img = img.convert("RGBA")
            images.append(img)

    # Pillow 的 Image.save 支持多帧 ICO
    images[0].save(
        ico_path,
        format="ICO",
        sizes=[(img.width, img.height) for img in images],
        append_images=images[1:],
    )
    sizes = [f"{img.width}x{img.height}" for img in images]
    print(f"  ✅ {ico_path.name} (全尺寸: {', '.join(sizes)})")


# 1) macOS iconset (16~1024)
iconset_dir = OUT_DIR / "icon.iconset"
iconset_dir.mkdir(exist_ok=True)
mac_sizes = [16, 32, 64, 128, 256, 512, 1024]
for size in mac_sizes:
    resize_png(SOURCE_PNG, iconset_dir / f"icon_{size}x{size}.png", size, size)

# 2) Windows ICO (16,32,48,64,128,256)
ico_sizes = [16, 32, 48, 64, 128, 256]
ico_tmp = OUT_DIR / ".ico_tmp"
ico_tmp.mkdir(exist_ok=True)
ico_pngs = []
for size in ico_sizes:
    p = ico_tmp / f"icon_{size}.png"
    resize_png(SOURCE_PNG, p, size, size)
    ico_pngs.append(p)
make_ico(ico_pngs, OUT_DIR / "icon.ico")
import shutil
shutil.rmtree(ico_tmp)

# 3) Tauri bundle PNGs
tauri_sizes = [32, 64, 128]
for size in tauri_sizes:
    resize_png(SOURCE_PNG, OUT_DIR / f"{size}x{size}.png", size, size)
resize_png(SOURCE_PNG, OUT_DIR / "128x128@2x.png", 256, 256)
resize_png(SOURCE_PNG, OUT_DIR / "icon.png", 1024, 1024)

# 4) Windows Store PNGs
store_sizes = [30, 44, 71, 89, 107, 142, 150, 284, 310]
for size in store_sizes:
    resize_png(SOURCE_PNG, OUT_DIR / f"Square{size}x{size}Logo.png", size, size)
resize_png(SOURCE_PNG, OUT_DIR / "StoreLogo.png", 50, 50)

# 5) Android mipmap
android_dirs = [
    ("mipmap-mdpi", 48),
    ("mipmap-hdpi", 72),
    ("mipmap-xhdpi", 96),
    ("mipmap-xxhdpi", 144),
    ("mipmap-xxxhdpi", 192),
]
for dname, size in android_dirs:
    d = OUT_DIR / "android" / dname
    d.mkdir(parents=True, exist_ok=True)
    resize_png(SOURCE_PNG, d / "ic_launcher.png", size, size)

# 6) iOS AppIcon (可选，存到 ios/AppIcon.appiconset)
ios_sizes = [
    (20, 1), (20, 2), (20, 3),
    (29, 1), (29, 2), (29, 3),
    (40, 1), (40, 2), (40, 3),
    (60, 2), (60, 3),
    (76, 1), (76, 2),
    (83.5, 2),
    (1024, 1),
]
ios_dir = OUT_DIR / "ios" / "AppIcon.appiconset"
ios_dir.mkdir(parents=True, exist_ok=True)
for base, scale in ios_sizes:
    size = int(base * scale)
    resize_png(SOURCE_PNG, ios_dir / f"AppIcon-{base}x{base}@{scale}x.png", size, size)

# 7) 生成 Contents.json (iOS iconset 元数据)
contents_json = {
    "images": [
        {"idiom": "iphone", "size": "20x20", "scale": "2x"},
        {"idiom": "iphone", "size": "20x20", "scale": "3x"},
        {"idiom": "iphone", "size": "29x29", "scale": "1x"},
        {"idiom": "iphone", "size": "29x29", "scale": "2x"},
        {"idiom": "iphone", "size": "29x29", "scale": "3x"},
        {"idiom": "iphone", "size": "40x40", "scale": "2x"},
        {"idiom": "iphone", "size": "40x40", "scale": "3x"},
        {"idiom": "iphone", "size": "60x60", "scale": "2x"},
        {"idiom": "iphone", "size": "60x60", "scale": "3x"},
        {"idiom": "ipad", "size": "20x20", "scale": "1x"},
        {"idiom": "ipad", "size": "20x20", "scale": "2x"},
        {"idiom": "ipad", "size": "29x29", "scale": "1x"},
        {"idiom": "ipad", "size": "29x29", "scale": "2x"},
        {"idiom": "ipad", "size": "40x40", "scale": "1x"},
        {"idiom": "ipad", "size": "40x40", "scale": "2x"},
        {"idiom": "ipad", "size": "76x76", "scale": "1x"},
        {"idiom": "ipad", "size": "76x76", "scale": "2x"},
        {"idiom": "ipad", "size": "83.5x83.5", "scale": "2x"},
        {"idiom": "ios-marketing", "size": "1024x1024", "scale": "1x"},
    ],
    "info": {"author": "crablet", "version": 1}
}
import json
with open(ios_dir / "Contents.json", "w", encoding="utf-8") as f:
    json.dump(contents_json, f, indent=2)
print(f"  ✅ iOS Contents.json")

print("")
print(f"🎉 全部图标生成完成！输出: {OUT_DIR}")
