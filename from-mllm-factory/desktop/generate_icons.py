#!/usr/bin/env python3
"""Generate PNG/ICO/ICNS icons from SVG source."""

import subprocess
import sys
from pathlib import Path

ICON_DIR = Path(__file__).parent / "icons"
SVG_PATH = ICON_DIR / "icon.svg"

# PNG sizes needed for Tauri
PNG_SIZES = [32, 128, 256, 512]

def generate_pngs():
    """Generate PNG files from SVG using rsvg-convert or cairosvg."""
    for size in PNG_SIZES:
        out = ICON_DIR / f"icon_{size}x{size}.png"
        try:
            subprocess.run(
                ["rsvg-convert", "-w", str(size), "-h", str(size), "-o", str(out), str(SVG_PATH)],
                check=True,
            )
            print(f"  ✓ {out}")
        except FileNotFoundError:
            # Fallback to cairosvg
            try:
                import cairosvg
                cairosvg.svg2png(
                    url=str(SVG_PATH),
                    output_width=size,
                    output_height=size,
                    write_to=str(out),
                )
                print(f"  ✓ {out} (cairosvg)")
            except ImportError:
                print(f"  ✗ Neither rsvg-convert nor cairosvg available for {out}")
                return False
    return True

def generate_ico():
    """Generate ICO from 256x256 PNG."""
    try:
        from PIL import Image
        img = Image.open(ICON_DIR / "icon_256x256.png")
        ico_path = ICON_DIR / "icon.ico"
        img.save(ico_path, format="ICO", sizes=[(16,16), (32,32), (48,48), (64,64), (128,128), (256,256)])
        print(f"  ✓ {ico_path}")
        return True
    except ImportError:
        print("  ✗ Pillow not available for ICO generation")
        return False

def generate_icns():
    """Generate ICNS from 512x512 PNG."""
    try:
        from PIL import Image
        img = Image.open(ICON_DIR / "icon_512x512.png")
        icns_path = ICON_DIR / "icon.icns"
        # macOS iconutil requires an iconset folder
        iconset_dir = ICON_DIR / "icon.iconset"
        iconset_dir.mkdir(exist_ok=True)
        sizes = {
            "icon_16x16.png": 16,
            "icon_16x16@2x.png": 32,
            "icon_32x32.png": 32,
            "icon_32x32@2x.png": 64,
            "icon_128x128.png": 128,
            "icon_128x128@2x.png": 256,
            "icon_256x256.png": 256,
            "icon_256x256@2x.png": 512,
            "icon_512x512.png": 512,
            "icon_512x512@2x.png": 1024,
        }
        for name, size in sizes.items():
            resized = img.resize((size, size), Image.LANCZOS)
            resized.save(iconset_dir / name)

        subprocess.run(
            ["iconutil", "-c", "icns", "-o", str(icns_path), str(iconset_dir)],
            check=True,
        )
        print(f"  ✓ {icns_path}")
        return True
    except (ImportError, FileNotFoundError) as e:
        print(f"  ✗ ICNS generation failed: {e}")
        return False

def main():
    print("Generating icons from SVG...")
    print("\n1. PNG files:")
    png_ok = generate_pngs()

    # Copy the 512x512 as the main icon.png
    if png_ok:
        import shutil
        shutil.copy(ICON_DIR / "icon_512x512.png", ICON_DIR / "icon.png")
        print(f"  ✓ {ICON_DIR / 'icon.png'}")

    print("\n2. ICO file:")
    generate_ico()

    print("\n3. ICNS file:")
    generate_icns()

    print("\nDone!")

if __name__ == "__main__":
    main()
