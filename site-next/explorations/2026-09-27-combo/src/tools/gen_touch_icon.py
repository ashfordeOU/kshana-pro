"""Home-screen icon (apple-touch-icon) from the founder's exact mark.

The mark is never redrawn: this paints assets/brand/kshana-mark-mask.png the way .brand-mark
does in src/site.css (--ink, with the --cyan hub and star), on the light ground --bg, at
180 x 180 px with the mark at 70 % so the platform's rounded-corner crop never cuts it.
Colours are read from src/tokens.css and src/theme.css, so a token change flows through.

Run: /opt/homebrew/bin/python3 src/tools/gen_touch_icon.py   (writes assets/brand/apple-touch-icon.png)
"""
import re
from pathlib import Path

from PIL import Image, ImageDraw

HERE = Path(__file__).resolve().parents[2]
SIZE, SCALE = 180, 0.70


def token(css: str, name: str) -> tuple[int, int, int]:
    hexv = re.search(rf"--{name}:\s*#([0-9A-Fa-f]{{6}})", css).group(1)
    return tuple(int(hexv[i:i + 2], 16) for i in (0, 2, 4))


def main() -> None:
    tokens = (HERE / "src/tokens.css").read_text()
    theme = (HERE / "src/theme.css").read_text()
    bg, ink, cyan = token(tokens, "bg"), token(tokens, "ink"), token(theme, "cyan")

    side = round(SIZE * SCALE)
    alpha = Image.open(HERE / "assets/brand/kshana-mark-mask.png").convert("RGBA").split()[3]
    alpha = alpha.resize((side, side), Image.LANCZOS)

    # The paint under the mask: ink, with the two cyan spots .brand-mark draws as radial
    # gradients (hub at 46.5 % / 49.5 %, radius 5 %; star at 87.5 % / 12 %, radius 4 %).
    paint = Image.new("RGB", (side, side), ink)
    draw = ImageDraw.Draw(paint)
    for cx, cy, r in ((0.465, 0.495, 0.05), (0.875, 0.12, 0.04)):
        x, y, rr = cx * side, cy * side, r * side * 1.6
        draw.ellipse((x - rr, y - rr, x + rr, y + rr), fill=cyan)

    out = Image.new("RGB", (SIZE, SIZE), bg)
    off = (SIZE - side) // 2
    out.paste(paint, (off, off), alpha)
    dest = HERE / "assets/brand/apple-touch-icon.png"
    out.save(dest, optimize=True)
    print(f"wrote {dest.relative_to(HERE)} ({dest.stat().st_size:,} bytes)")


if __name__ == "__main__":
    main()
