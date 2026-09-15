#!/usr/bin/env python3
# pyright: reportUnknownMemberType=false
"""
generate_icons.py - Cross-platform icon generator for Uwu Log.

Generates standard icon formats for:
  - Windows: packaging/assets/icon.ico (Multi-res: 16, 24, 32, 48, 64, 128, 256 px)
  - macOS:   packaging/assets/icon.icns (Apple Retina multi-size)
  - Linux:   packaging/assets/icon_<size>.png (512, 256, 128, 64, 48, 32, 16 px)

Usage:
  python generate_icons.py [optional_path_to_image.png]
"""

import os
import sys
from PIL import Image


def clean_and_center_image(img: Image.Image, target_size: int = 1024, margin: int = 48) -> Image.Image:
    """Filter alpha noise, crop to non-transparent content, and center in a square canvas."""
    img = img.convert("RGBA")
    # Extract alpha channel directly
    alpha = img.getchannel("A")

    # Use a 256-value lookup table to filter alpha < 10 (fast in C and strict type-safe)
    lut = [float(i) if i >= 10 else 0.0 for i in range(256)]
    clean_a = alpha.point(lut)
    img.putalpha(clean_a)

    bbox = clean_a.getbbox()
    if bbox is None:
        return img.resize((target_size, target_size), Image.Resampling.LANCZOS)

    cropped = img.crop(bbox)
    cw, ch = cropped.size

    avail_size = target_size - 2 * margin
    scale = min(avail_size / cw, avail_size / ch)
    nw = int(cw * scale)
    nh = int(ch * scale)
    resized = cropped.resize((nw, nh), Image.Resampling.LANCZOS)

    centered = Image.new("RGBA", (target_size, target_size), (0, 0, 0, 0))
    ox = (target_size - nw) // 2
    oy = (target_size - nh) // 2
    centered.paste(resized, (ox, oy), resized)
    return centered


def find_source_image(script_dir: str, project_root: str) -> str:
    """Find source image from CLI argument or default candidate paths."""
    if len(sys.argv) > 1 and os.path.exists(sys.argv[1]):
        return os.path.abspath(sys.argv[1])

    candidates = [
        os.path.join(script_dir, "app-icon.png"),
        os.path.join(script_dir, "icon_512.png"),
        os.path.join(project_root, "avatar.png"),
    ]

    for candidate in candidates:
        if os.path.exists(candidate):
            return candidate

    raise SystemExit(
        "Error: No source image found!\n"
        + "Please provide a path to an image: python generate_icons.py <path_to_image>"
    )


def main():
    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.abspath(os.path.join(script_dir, "..", ".."))

    src_path = find_source_image(script_dir, project_root)
    print(f"==> Source image: {src_path}")

    raw_img = Image.open(src_path)
    centered = clean_and_center_image(raw_img, target_size=1024, margin=48)

    # Save master app-icon.png in packaging/assets/
    master_path = os.path.join(script_dir, "app-icon.png")
    centered.save(master_path, format="PNG")
    print(f"  -> Master asset: {master_path} (1024x1024)")

    # 1. Windows: icon.ico (multi-resolution: 16, 24, 32, 48, 64, 128, 256)
    ico_path = os.path.join(script_dir, "icon.ico")
    ico_sizes = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    centered.save(ico_path, format="ICO", sizes=ico_sizes)
    print(f"  -> Windows icon: {ico_path} (7 sizes: 16-256px)")

    # 2. macOS: icon.icns (Apple Retina multi-size)
    icns_path = os.path.join(script_dir, "icon.icns")
    try:
        centered.save(icns_path, format="ICNS")
        print(f"  -> macOS Apple icon: {icns_path}")
    except (OSError, ValueError) as err:
        print(f"  -> Warning: Could not save ICNS ({err})")

    # 3. Linux & egui: hicolor PNG sizes (512, 256, 128, 64, 48, 32, 16)
    png_sizes = [512, 256, 128, 64, 48, 32, 16]
    for s in png_sizes:
        png_path = os.path.join(script_dir, f"icon_{s}.png")
        resized = centered.resize((s, s), Image.Resampling.LANCZOS)
        resized.save(png_path, format="PNG")
        print(f"  -> Linux PNG ({s}x{s}): {png_path}")

    print("\n[OK] All icon assets successfully generated and centered!")


if __name__ == "__main__":
    main()
