#!/usr/bin/env python3
"""从高分辨率 PNG 生成真正的多尺寸 ICO 文件。

ICO 格式规范：
- 每个 ICO 条目必须是独立尺寸的 PNG
- 支持 16×16, 32×32, 48×48, 256×256
"""

import subprocess
import sys
from pathlib import Path

ICON_SRC = Path("/Users/gatilin/Downloads/微信图片_20260601105735_9720_209.png")
OUT_DIR = Path("/Users/gatilin/PycharmProjects/crablet-git/desktop")

# ICO 所需的标准尺寸
sizes = [(16, 16), (32, 32), (48, 48), (256, 256)]

# 先用 magick 将原图缩放到各尺寸并保存为独立 PNG
for w, h in sizes:
    tmp = OUT_DIR / f"icon_{w}x{h}.png"
    subprocess.run([
        "magick", str(ICON_SRC),
        "-resize", f"{w}x{h}",
        str(tmp)
    ], check=True)

# 再用 PNG 拼接成 ICO
# ICO 格式要求：每个条目是一个完整帧
pngs = [str(OUT_DIR / f"icon_{w}x{h}.png") for w, h in sizes]

# 构建 ICO
cmd = ["magick"] + pngs + [str(OUT_DIR / "icon.ico")]
subprocess.run(cmd, check=True)

print("✅ ICO 生成完成:", OUT_DIR / "icon.ico")
