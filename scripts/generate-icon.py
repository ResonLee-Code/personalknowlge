#!/usr/bin/env python3
"""生成应用图标源图（1024x1024 PNG）。

只依赖 Python 标准库：用 zlib 手写 PNG，避免引入额外依赖。
输出：src-tauri/icons/source.png，随后由 `pnpm tauri icon` 生成各平台图标。
"""

import math
import pathlib
import struct
import zlib

SIZE = 1024
OUTPUT = pathlib.Path(__file__).resolve().parent.parent / "src-tauri" / "icons" / "source.png"


def clamp(value: float, low: float = 0.0, high: float = 1.0) -> float:
    return max(low, min(high, value))


def smoothstep(distance: float) -> float:
    """基于距离的 1px 抗锯齿覆盖率。"""
    return clamp(0.5 - distance)


def rounded_rect_distance(x: float, y: float, cx: float, cy: float, hw: float, hh: float, r: float) -> float:
    dx = abs(x - cx) - (hw - r)
    dy = abs(y - cy) - (hh - r)
    outside = math.hypot(max(dx, 0.0), max(dy, 0.0))
    return outside + min(max(dx, dy), 0.0) - r


def circle_distance(x: float, y: float, cx: float, cy: float, radius: float) -> float:
    return math.hypot(x - cx, y - cy) - radius


def blend(base, top, alpha):
    return tuple(int(round(base[i] * (1 - alpha) + top[i] * alpha)) for i in range(4))


def build_pixels():
    background_a = (99, 102, 241)   # indigo-500
    background_b = (67, 56, 202)    # indigo-700
    foreground = (248, 250, 252, 255)
    accent = (129, 140, 248, 255)   # indigo-400

    bars = [
        (0.34, 0.30, 0.30, 0.055),
        (0.34, 0.44, 0.24, 0.055),
        (0.34, 0.58, 0.30, 0.055),
        (0.34, 0.72, 0.16, 0.055),
    ]

    pixels = bytearray()
    for y in range(SIZE):
        row = bytearray()
        for x in range(SIZE):
            px, py = x / SIZE, y / SIZE

            colour = (0, 0, 0, 0)
            distance = rounded_rect_distance(px, py, 0.5, 0.5, 0.46, 0.46, 0.22)
            alpha = smoothstep(distance * SIZE)
            if alpha > 0:
                ratio = clamp((px + py) / 2)
                shade = tuple(
                    int(round(background_a[i] * (1 - ratio) + background_b[i] * ratio)) for i in range(3)
                ) + (255,)
                colour = blend(colour, shade, alpha)

            for cx, cy, half_width, half_height in bars:
                distance = rounded_rect_distance(
                    px, py, cx, cy, half_width, half_height, half_height
                )
                alpha = smoothstep(distance * SIZE)
                if alpha > 0:
                    colour = blend(colour, foreground, alpha)

            distance = circle_distance(px, py, 0.74, 0.32, 0.075)
            alpha = smoothstep(distance * SIZE)
            if alpha > 0:
                colour = blend(colour, accent, alpha)

            row += bytes(colour)
        pixels += b"\x00" + row
    return bytes(pixels)


def chunk(tag: bytes, data: bytes) -> bytes:
    return (
        struct.pack(">I", len(data))
        + tag
        + data
        + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
    )


def main() -> None:
    raw = build_pixels()
    header = struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0)
    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", header)
    png += chunk(b"IDAT", zlib.compress(raw, 9))
    png += chunk(b"IEND", b"")
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_bytes(png)
    print(f"已生成 {OUTPUT} ({len(png)} bytes)")


if __name__ == "__main__":
    main()
