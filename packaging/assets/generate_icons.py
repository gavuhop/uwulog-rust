#!/usr/bin/env python3
"""
generate_icons.py - Generate cross-platform icons for Windows, macOS, and Linux
from avatar.png (1024x1024).
"""
import os
import sys
from PIL import Image

def main():
    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.abspath(os.path.join(script_dir, "..", ".."))
    src_avatar = os.path.join(project_root, "avatar.png")

    if not os.path.exists(src_avatar):
        print(f"Error: Source image not found at {src_avatar}")
        sys.exit(1)

    print(f"Loading {src_avatar}...")
    img = Image.open(src_avatar).convert("RGBA")

    # 1. Generate icon.ico for Windows (multi-layer)
    ico_path = os.path.join(script_dir, "icon.ico")
    ico_sizes = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    img.save(ico_path, format="ICO", sizes=ico_sizes)
    print(f"Generated Windows icon: {ico_path}")

    # 2. Generate icon.icns for macOS
    icns_path = os.path.join(script_dir, "icon.icns")
    try:
        img.save(icns_path, format="ICNS")
        print(f"Generated macOS icon: {icns_path}")
    except Exception as e:
        print(f"Warning: Could not save ICNS directly via Pillow: {e}")

    # 3. Generate icon_512.png and icon_256.png for Linux
    png_512_path = os.path.join(script_dir, "icon_512.png")
    img_512 = img.resize((512, 512), Image.Resampling.LANCZOS)
    img_512.save(png_512_path, format="PNG")
    print(f"Generated Linux 512x512 icon: {png_512_path}")

    png_256_path = os.path.join(script_dir, "icon_256.png")
    img_256 = img.resize((256, 256), Image.Resampling.LANCZOS)
    img_256.save(png_256_path, format="PNG")
    print(f"Generated 256x256 icon: {png_256_path}")

    print("All icons successfully generated!")

if __name__ == "__main__":
    main()
