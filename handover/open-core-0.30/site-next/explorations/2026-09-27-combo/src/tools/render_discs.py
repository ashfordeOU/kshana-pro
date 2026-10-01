#!/usr/bin/env python3
"""Orthographic discs of the NASA maps under assets/planets/, for places where a WebGL globe is
too heavy (a bento tile, the hero's first paint): the same imagery, reprojected, never drawn.

  earth-disc-640.webp  Blue Marble, centred on 30° N, 10° E (the campaign receiver's side)
  moon-disc-320.webp   CGI Moon Kit, the near side (0°, 0°)

Usage: /opt/homebrew/bin/python3 src/tools/render_discs.py   (needs Pillow and NumPy; run once)
"""
import os
import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
PLANETS = os.path.normpath(os.path.join(HERE, "..", "..", "assets", "planets"))


def disc(src, out, size, lat0, lon0, shade=0.35):
    img = np.asarray(Image.open(os.path.join(PLANETS, src)).convert("RGB")).astype(np.float32)
    H, W, _ = img.shape
    n = size
    y, x = np.mgrid[0:n, 0:n].astype(np.float32)
    u = (x + 0.5) / n * 2 - 1
    v = 1 - (y + 0.5) / n * 2
    r2 = u * u + v * v
    inside = r2 <= 1
    z = np.sqrt(np.clip(1 - r2, 0, 1))
    la0, lo0 = np.radians(lat0), np.radians(lon0)
    # inverse orthographic projection
    lat = np.arcsin(np.clip(z * np.sin(la0) + v * np.cos(la0), -1, 1))
    lon = lo0 + np.arctan2(u, z * np.cos(la0) - v * np.sin(la0))
    px = ((np.degrees(lon) + 180) % 360) / 360 * W
    py = (90 - np.degrees(lat)) / 180 * H
    col = img[np.clip(py.astype(int), 0, H - 1), np.clip(px.astype(int), 0, W - 1)]
    # soft limb darkening so the disc reads as a sphere (lighting, not data)
    k = (1 - shade) + shade * z
    col = col * k[..., None]
    a = np.clip((1 - np.sqrt(r2)) * n / 1.5, 0, 1) * 255
    rgba = np.dstack([np.clip(col, 0, 255), np.where(inside, a, 0)]).astype(np.uint8)
    Image.fromarray(rgba, "RGBA").save(os.path.join(PLANETS, out), quality=82, method=6)
    print(out, os.path.getsize(os.path.join(PLANETS, out)), "bytes")


if __name__ == "__main__":
    disc("earth-day-2048.jpg", "earth-disc-640.webp", 640, 30, 10)
    disc("moon-2048.jpg", "moon-disc-320.webp", 320, 0, 0)
