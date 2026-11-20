#!/usr/bin/env python3
"""Generate KillEverything app icons and tray icons using only Python stdlib + iconutil."""

import math
import os
import shutil
import struct
import subprocess
import tempfile
import zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ICONS_DIR = os.path.join(ROOT, "src-tauri", "icons")


def encode_png(width: int, height: int, rgba: bytearray) -> bytes:
    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    raw = bytearray()
    stride = width * 4
    for y in range(height):
        raw.append(0)
        raw.extend(rgba[y * stride : (y + 1) * stride])

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    idat = zlib.compress(bytes(raw), 9)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", idat) + chunk(b"IEND", b"")


def point_in_poly(x: float, y: float, poly: list[tuple[float, float]]) -> bool:
    inside = False
    n = len(poly)
    j = n - 1
    for i in range(n):
        xi, yi = poly[i]
        xj, yj = poly[j]
        if ((yi > y) != (yj > y)) and (x < (xj - xi) * (y - yi) / ((yj - yi) or 1e-9) + xi):
            inside = not inside
        j = i
    return inside


def dist_point_to_segment(px: float, py: float, ax: float, ay: float, bx: float, by: float) -> float:
    dx, dy = bx - ax, by - ay
    if dx == 0 and dy == 0:
        return math.hypot(px - ax, py - ay)
    t = max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy)))
    return math.hypot(px - (ax + t * dx), py - (ay + t * dy))


def sdf_rounded_rect(x: float, y: float, half: float, radius: float) -> float:
    qx = abs(x) - (half - radius)
    qy = abs(y) - (half - radius)
    return math.hypot(max(qx, 0.0), max(qy, 0.0)) + min(max(qx, qy), 0.0) - radius


def blend(dst: tuple[int, int, int, int], src: tuple[int, int, int, int]) -> tuple[int, int, int, int]:
    sa = src[3] / 255.0
    da = dst[3] / 255.0
    oa = sa + da * (1.0 - sa)
    if oa <= 1e-6:
        return (0, 0, 0, 0)
    r = int((src[0] * sa + dst[0] * da * (1.0 - sa)) / oa)
    g = int((src[1] * sa + dst[1] * da * (1.0 - sa)) / oa)
    b = int((src[2] * sa + dst[2] * da * (1.0 - sa)) / oa)
    return (max(0, min(255, r)), max(0, min(255, g)), max(0, min(255, b)), int(oa * 255))


# Stylized lightning bolt normalized in [-1, 1]
BOLT_POLY = [
    (0.08, -0.52),
    (-0.34, 0.06),
    (-0.04, 0.06),
    (-0.14, 0.52),
    (0.36, -0.06),
    (0.05, -0.06),
]


def render_app_icon(size: int) -> bytes:
    buf = bytearray(size * size * 4)
    aa = 1.6 / size
    ring_r = 0.66
    ring_w = 0.065

    for py in range(size):
        ny = (py + 0.5) / size * 2.0 - 1.0
        for px in range(size):
            nx = (px + 0.5) / size * 2.0 - 1.0

            # Squircle background
            d_box = sdf_rounded_rect(nx, ny, 0.90, 0.36)
            alpha_box = max(0.0, min(1.0, (0.0 - d_box) / (aa * 1.4) + 0.5))
            if alpha_box <= 0.0:
                continue

            # Dark slate gradient (#1e2433 -> #0f121b)
            t_grad = (ny + 1.0) * 0.5
            bg_r = int(32 * (1 - t_grad) + 14 * t_grad)
            bg_g = int(38 * (1 - t_grad) + 17 * t_grad)
            bg_b = int(54 * (1 - t_grad) + 27 * t_grad)

            # Subtle border highlight
            border_alpha = max(0.0, min(1.0, 1.0 - abs(d_box + 0.02) / 0.03)) * 0.25
            color = (
                int(bg_r * (1 - border_alpha) + 148 * border_alpha),
                int(bg_g * (1 - border_alpha) + 163 * border_alpha),
                int(bg_b * (1 - border_alpha) + 184 * border_alpha),
                int(alpha_box * 255),
            )

            # Crosshair ring + 4 ticks (coral red #ef4444)
            r_dist = math.hypot(nx, ny)
            d_ring = abs(r_dist - ring_r) - ring_w * 0.5
            tick_h = max(abs(ny) - 0.04, abs(abs(nx) - 0.72) - 0.14)
            tick_v = max(abs(nx) - 0.04, abs(abs(ny) - 0.72) - 0.14)
            d_cross = min(d_ring, tick_h, tick_v)
            cross_cov = max(0.0, min(1.0, (0.0 - d_cross) / (aa * 1.3) + 0.5))
            if cross_cov > 0.0:
                color = blend(color, (239, 68, 68, int(cross_cov * 235)))

            # Lightning bolt (amber-to-coral gradient)
            # Supersample 3x3 inside potential bounding box
            if -0.45 <= nx <= 0.45 and -0.60 <= ny <= 0.60:
                hits = 0
                for sy in (-0.33, 0.0, 0.33):
                    for sx in (-0.33, 0.0, 0.33):
                        ux = nx + sx * (2.0 / size)
                        uy = ny + sy * (2.0 / size)
                        if point_in_poly(ux, uy, BOLT_POLY):
                            hits += 1
                if hits > 0:
                    bt = max(0.0, min(1.0, (ny + 0.52) / 1.04))
                    br = int(251 * (1 - bt) + 244 * bt)
                    bg = int(191 * (1 - bt) + 63 * bt)
                    bb = int(36 * (1 - bt) + 94 * bt)
                    color = blend(color, (br, bg, bb, int((hits / 9.0) * 255)))

            idx = (py * size + px) * 4
            buf[idx : idx + 4] = bytes(color)

    return encode_png(size, size, buf)


def render_tray_icon(size: int) -> bytes:
    """Monochrome black+alpha template icon (macOS menu bar / Linux & Windows tray)."""
    buf = bytearray(size * size * 4)
    aa = 1.6 / size
    ring_r = 0.70
    ring_w = 0.11

    for py in range(size):
        ny = (py + 0.5) / size * 2.0 - 1.0
        for px in range(size):
            nx = (px + 0.5) / size * 2.0 - 1.0
            r_dist = math.hypot(nx, ny)
            d_ring = abs(r_dist - ring_r) - ring_w * 0.5

            # Gap near top-right/bottom-left or clean crosshair ticks
            tick_h = max(abs(ny) - 0.06, abs(abs(nx) - 0.74) - 0.16)
            tick_v = max(abs(nx) - 0.06, abs(abs(ny) - 0.74) - 0.16)
            d_ring_all = min(d_ring, tick_h, tick_v)
            ring_cov = max(0.0, min(1.0, (0.0 - d_ring_all) / (aa * 1.4) + 0.5))

            bolt_hits = 0
            if -0.45 <= nx <= 0.45 and -0.60 <= ny <= 0.60:
                for sy in (-0.33, 0.0, 0.33):
                    for sx in (-0.33, 0.0, 0.33):
                        ux = nx + sx * (2.0 / size)
                        uy = ny + sy * (2.0 / size)
                        if point_in_poly(ux, uy, BOLT_POLY):
                            bolt_hits += 1
            bolt_cov = bolt_hits / 9.0

            cov = max(ring_cov, bolt_cov)
            if cov > 0.0:
                idx = (py * size + px) * 4
                buf[idx : idx + 4] = bytes((0, 0, 0, int(cov * 255)))

    return encode_png(size, size, buf)


def write_ico(frames: list[tuple[int, bytes]], out_path: str) -> None:
    count = len(frames)
    header = struct.pack("<HHH", 0, 1, count)
    offset = 6 + count * 16
    entries = bytearray()
    payload = bytearray()
    for dim, png in frames:
        w_byte = 0 if dim >= 256 else dim
        h_byte = 0 if dim >= 256 else dim
        entries.extend(struct.pack("<BBBBHHII", w_byte, h_byte, 0, 0, 1, 32, len(png), offset))
        payload.extend(png)
        offset += len(png)
    with open(out_path, "wb") as f:
        f.write(header + entries + payload)


def main() -> None:
    os.makedirs(ICONS_DIR, exist_ok=True)
    sizes = {
        "32x32.png": 32,
        "128x128.png": 128,
        "128x128@2x.png": 256,
        "icon.png": 512,
    }
    rendered: dict[int, bytes] = {}
    for name, sz in sizes.items():
        png = render_app_icon(sz)
        rendered[sz] = png
        with open(os.path.join(ICONS_DIR, name), "wb") as f:
            f.write(png)

    # Monochrome tray template icons
    for name, sz in [("trayTemplate.png", 22), ("trayTemplate@2x.png", 44)]:
        with open(os.path.join(ICONS_DIR, name), "wb") as f:
            f.write(render_tray_icon(sz))

    # Multi-resolution Windows .ico
    write_ico([(32, rendered[32]), (256, rendered[256])], os.path.join(ICONS_DIR, "icon.ico"))

    # macOS .icns via iconutil
    if shutil.which("iconutil"):
        tmp_dir = tempfile.mkdtemp(prefix="ke_iconset_")
        try:
            iconset = os.path.join(tmp_dir, "KillEverything.iconset")
            os.makedirs(iconset, exist_ok=True)
            specs = [
                ("icon_16x16.png", 16),
                ("icon_16x16@2x.png", 32),
                ("icon_32x32.png", 32),
                ("icon_32x32@2x.png", 64),
                ("icon_128x128.png", 128),
                ("icon_128x128@2x.png", 256),
                ("icon_256x256.png", 256),
                ("icon_256x256@2x.png", 512),
                ("icon_512x512.png", 512),
                ("icon_512x512@2x.png", 1024),
            ]
            for fname, sz in specs:
                data = rendered.get(sz) or render_app_icon(sz)
                rendered[sz] = data
                with open(os.path.join(iconset, fname), "wb") as f:
                    f.write(data)
            subprocess.run(
                ["iconutil", "-c", "icns", iconset, "-o", os.path.join(ICONS_DIR, "icon.icns")],
                check=True,
            )
        finally:
            shutil.rmtree(tmp_dir, ignore_errors=True)

    print("Generated icons in", ICONS_DIR)


if __name__ == "__main__":
    main()
