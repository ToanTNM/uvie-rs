#!/usr/bin/env python3
"""
UVieKey icon generator - flat wordmark style with soft shadow.

Design:
  - Flat rounded square background in Rust brand color #CE422B
  - Bold "uvie" wordmark in white, centered, with a soft drop shadow
  - macOS icon shape (rounded corners)

Output: AppIcon.iconset/ with all required macOS sizes + AppIcon.icns
"""

import os
import subprocess
from PIL import Image, ImageDraw, ImageFont, ImageFilter

# ── Colors ────────────────────────────────────────────────────────────────────
RUST_BRAND = (206, 66, 43, 255)   # #CE422B
WHITE      = (255, 255, 255, 255)
SHADOW     = (0, 0, 0, 90)        # soft black shadow


def rounded_rect_mask(size: int, radius_frac: float = 0.22) -> Image.Image:
    """macOS-style rounded square mask."""
    r = int(size * radius_frac)
    mask = Image.new("L", (size, size), 0)
    d = ImageDraw.Draw(mask)
    d.rounded_rectangle([0, 0, size - 1, size - 1], radius=r, fill=255)
    return mask


def find_wordmark_font(size: int) -> ImageFont.FreeTypeFont:
    """Find a clean rounded/sans bold font for the 'uvie' wordmark."""
    candidates = [
        "/System/Library/Fonts/Supplemental/Arial Rounded Bold.ttf",
        "/Library/Fonts/Roboto-Bold.ttf",
        "/System/Library/Fonts/Supplemental/Verdana Bold.ttf",
        "/System/Library/Fonts/Supplemental/Tahoma Bold.ttf",
        "/System/Library/Fonts/SFNSRounded.ttf",
        "/System/Library/Fonts/HelveticaNeue.ttc",
        "/System/Library/Fonts/Avenir Next.ttc",
        "/Library/Fonts/Arial Unicode.ttf",
    ]
    text = "uvie"
    for path in candidates:
        try:
            font = ImageFont.truetype(path, size)
            if font.getlength(text) > 0:
                return font
        except Exception:
            continue
    raise RuntimeError("No usable font found for 'uvie' wordmark")


def fit_font_size(draw: ImageDraw.ImageDraw, text: str, target_w: float) -> ImageFont.FreeTypeFont:
    """Binary search font size so the text width matches target_w."""
    lo, hi = 1, int(target_w * 2)
    best_font = None
    for _ in range(20):
        mid = (lo + hi) // 2
        font = find_wordmark_font(mid)
        bbox = draw.textbbox((0, 0), text, font=font)
        tw = bbox[2] - bbox[0]
        if tw <= target_w:
            lo = mid
            best_font = font
        else:
            hi = mid
    if best_font is None:
        best_font = find_wordmark_font(lo)
    return best_font


def draw_soft_shadow(img: Image.Image, text: str, font: ImageFont.FreeTypeFont,
                     x: float, y: float, blur: float, opacity: int):
    """Draw a blurred drop shadow behind the text."""
    w, h = img.size
    # Create a grayscale mask of the text
    mask = Image.new("L", (w, h), 0)
    draw_mask = ImageDraw.Draw(mask)
    draw_mask.text((x, y), text, font=font, fill=255)
    # Blur the mask
    blurred = mask.filter(ImageFilter.GaussianBlur(radius=blur))
    # Convert shadow to RGBA
    shadow = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    for dx, dy, alpha in [(0, 0, opacity)]:
        pass
    # Create colored shadow layer
    shadow_color = Image.new("RGBA", (w, h), SHADOW)
    # Apply blurred mask as alpha
    shadow_rgba = Image.composite(shadow_color, Image.new("RGBA", (w, h), (0, 0, 0, 0)), blurred)
    # Offset shadow slightly
    offset_x = int(0.012 * w)
    offset_y = int(0.018 * w)
    img.alpha_composite(shadow_rgba, dest=(offset_x, offset_y))


def draw_bold_text(draw: ImageDraw.ImageDraw, text: str, font: ImageFont.FreeTypeFont,
                   x: float, y: float, fill):
    """Draw text with a thicker outline for a heavier/bolder look."""
    # Thicker dark rust outline
    outline_color = (140, 45, 25, 255)
    offsets = [
        (-2, -2), (-2, -1), (-2, 0), (-2, 1), (-2, 2),
        (-1, -2), (-1, -1), (-1, 0), (-1, 1), (-1, 2),
        ( 0, -2), ( 0, -1),          ( 0, 1), ( 0, 2),
        ( 1, -2), ( 1, -1), ( 1, 0), ( 1, 1), ( 1, 2),
        ( 2, -2), ( 2, -1), ( 2, 0), ( 2, 1), ( 2, 2),
    ]
    for dx, dy in offsets:
        draw.text((x + dx, y + dy), text, font=font, fill=outline_color)
    # Main text layered twice for extra weight
    draw.text((x, y), text, font=font, fill=fill)
    draw.text((x, y), text, font=font, fill=fill)


def make_icon(size: int) -> Image.Image:
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    mask = rounded_rect_mask(size, radius_frac=0.22)

    # Flat rust background
    bg = Image.new("RGBA", (size, size), RUST_BRAND)
    img.paste(bg, mask=mask)

    draw = ImageDraw.Draw(img)

    # "uvie" wordmark - larger and bolder
    text = "uvie"
    target_w_ratio = 0.8
    target_w = size * target_w_ratio

    font = fit_font_size(draw, text, target_w)
    bbox = draw.textbbox((0, 0), text, font=font)
    tw = bbox[2] - bbox[0]
    th = bbox[3] - bbox[1]

    # Center
    x = (size - tw) / 2 - bbox[0]
    y = (size - th) / 2 - bbox[1]

    # Soft shadow
    blur_radius = max(2, size * 0.02)
    draw_soft_shadow(img, text, font, x, y, blur=blur_radius, opacity=90)

    # Main text with slight outline for boldness
    draw_bold_text(draw, text, font, x, y, fill=WHITE)

    # Apply mask
    out = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    out.paste(img, mask=mask)
    return out


# ── macOS required sizes ──────────────────────────────────────────────────────
ICONSET_SIZES = [
    (16,  1), (16,  2),
    (32,  1), (32,  2),
    (128, 1), (128, 2),
    (256, 1), (256, 2),
    (512, 1), (512, 2),
]


def main():
    out_dir = os.path.join(os.path.dirname(__file__), "..", "UVieKey", "AppIcon.iconset")
    os.makedirs(out_dir, exist_ok=True)

    print("Generating icons...")
    for base, scale in ICONSET_SIZES:
        px = base * scale
        img = make_icon(px)
        suffix = f"@{scale}x" if scale > 1 else ""
        fname = f"icon_{base}x{base}{suffix}.png"
        path = os.path.join(out_dir, fname)
        img.save(path, "PNG")
        print(f"  ✓ {fname} ({px}x{px})")

    # Generate .icns using iconutil
    icns_path = os.path.join(os.path.dirname(__file__), "..", "UVieKey", "AppIcon.icns")
    print(f"\nConverting to .icns → {icns_path}")
    result = subprocess.run(
        ["iconutil", "-c", "icns", out_dir, "-o", icns_path],
        capture_output=True, text=True
    )
    if result.returncode == 0:
        print(f"  ✓ AppIcon.icns generated!")
    else:
        print(f"  ✗ iconutil error: {result.stderr}")
        print("  (PNG files still generated in AppIcon.iconset/)")


if __name__ == "__main__":
    main()
