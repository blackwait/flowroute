#!/usr/bin/env python3
"""Generate FlowRoute macOS icon set."""

from __future__ import annotations

import math
import struct
import subprocess
import zlib
from pathlib import Path

try:
    from PIL import Image, ImageDraw, ImageFilter
except ImportError:
    subprocess.check_call(["pip3", "install", "pillow", "-q"])
    from PIL import Image, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parents[1]
ICON_DIR = ROOT / "src-tauri" / "icons"
ICONSET = ICON_DIR / "icon.iconset"


def lerp(a: float, b: float, t: float) -> float:
    return a + (b - a) * t


def render_icon(size: int) -> Image.Image:
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    s = size

    # Soft squircle glow backdrop
    for i in range(18):
        t = i / 17
        pad = int(s * (0.04 + t * 0.02))
        alpha = int(34 * (1 - t))
        color = (
            int(lerp(8, 14, t)),
            int(lerp(24, 42, t)),
            int(lerp(58, 92, t)),
            alpha,
        )
        draw.rounded_rectangle(
            (pad, pad, s - pad, s - pad),
            radius=int(s * 0.22),
            fill=color,
        )

    # Main squircle body
    body_pad = int(s * 0.08)
    body = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    body_draw = ImageDraw.Draw(body)
    body_draw.rounded_rectangle(
        (body_pad, body_pad, s - body_pad, s - body_pad),
        radius=int(s * 0.22),
        fill=(10, 22, 48, 255),
    )

    # Inner gradient wash
    grad = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    gdraw = ImageDraw.Draw(grad)
    for y in range(body_pad, s - body_pad):
        t = (y - body_pad) / max(1, s - 2 * body_pad)
        color = (
            int(lerp(14, 8, t)),
            int(lerp(40, 24, t)),
            int(lerp(92, 58, t)),
            int(lerp(120, 40, t)),
        )
        gdraw.line([(body_pad, y), (s - body_pad, y)], fill=color)
    body = Image.alpha_composite(body, grad)

    mask = Image.new("L", (size, size), 0)
    mask_draw = ImageDraw.Draw(mask)
    mask_draw.rounded_rectangle(
        (body_pad, body_pad, s - body_pad, s - body_pad),
        radius=int(s * 0.22),
        fill=255,
    )
    img.paste(body, (0, 0), mask)

    draw = ImageDraw.Draw(img)
    cx, cy = s * 0.5, s * 0.56
    node_r = max(3, int(s * 0.055))

    def draw_curve(points, color, width):
        if len(points) < 2:
            return
        for i in range(len(points) - 1):
            draw.line([points[i], points[i + 1]], fill=color, width=width)

    # Source node
    draw.ellipse(
        (cx - node_r, cy - node_r, cx + node_r, cy + node_r),
        fill=(56, 189, 248, 255),
    )
    draw.ellipse(
        (cx - node_r * 0.45, cy - node_r * 0.45, cx + node_r * 0.45, cy + node_r * 0.45),
        fill=(224, 247, 255, 255),
    )

    # Direct path (left, green)
    left_points = []
    for t in range(0, 41):
        tt = t / 40
        x = cx - tt * s * 0.28
        y = cy - tt * s * 0.30
        left_points.append((x, y))
    draw_curve(left_points, (34, 197, 94, 255), max(2, int(s * 0.042)))
    lx, ly = left_points[-1]
    draw.ellipse(
        (lx - node_r * 0.75, ly - node_r * 0.75, lx + node_r * 0.75, ly + node_r * 0.75),
        fill=(74, 222, 128, 255),
    )

    # Proxy path (right, cyan arc)
    right_points = []
    for t in range(0, 41):
        tt = t / 40
        x = cx + tt * s * 0.30
        y = cy - tt * s * 0.18 + math.sin(tt * math.pi) * s * 0.08
        right_points.append((x, y))
    draw_curve(right_points, (14, 165, 233, 255), max(2, int(s * 0.042)))
    rx, ry = right_points[-1]
    draw.ellipse(
        (rx - node_r * 0.75, ry - node_r * 0.75, rx + node_r * 0.75, ry + node_r * 0.75),
        fill=(125, 211, 252, 255),
    )

    # Small orbit ring for "routing" feel
    ring_r = s * 0.30
    bbox = (cx - ring_r, cy - ring_r - s * 0.05, cx + ring_r, cy + ring_r - s * 0.05)
    draw.arc(bbox, start=200, end=340, fill=(56, 189, 248, 90), width=max(1, int(s * 0.018)))

    # Highlight gloss
    gloss = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    g = ImageDraw.Draw(gloss)
    g.ellipse(
        (s * 0.18, s * 0.10, s * 0.62, s * 0.42),
        fill=(255, 255, 255, 28),
    )
    gloss = gloss.filter(ImageFilter.GaussianBlur(radius=max(1, int(s * 0.03))))
    img = Image.alpha_composite(img, gloss)

    return img


def save_png(path: Path, size: int) -> None:
    render_icon(size).save(path, format="PNG")


def build_iconset() -> None:
    ICON_DIR.mkdir(parents=True, exist_ok=True)
    if ICONSET.exists():
        for item in ICONSET.iterdir():
            item.unlink()
    else:
        ICONSET.mkdir(parents=True)

    mapping = {
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
    for name, size in mapping.items():
        save_png(ICONSET / name, size)

    save_png(ICON_DIR / "icon.png", 1024)
    save_png(ICON_DIR / "32x32.png", 32)
    save_png(ICON_DIR / "128x128.png", 128)
    save_png(ICON_DIR / "128x128@2x.png", 256)

    subprocess.run(
        ["iconutil", "-c", "icns", str(ICONSET), "-o", str(ICON_DIR / "icon.icns")],
        check=True,
    )
    print(f"Generated icons in {ICON_DIR}")


if __name__ == "__main__":
    build_iconset()
