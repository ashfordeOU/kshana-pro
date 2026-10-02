#!/usr/bin/env python3
"""Build the body-imagery atlas behind the Capabilities solar-system viewer.

Every body the engine's `solar-system` kind returns (the Sun, the eight planets, Pluto, the Moon,
Phobos, Deimos, Io, Europa, Ganymede, Callisto and Titan) is drawn with a real global map: NASA,
USGS and JPL mosaics (public domain) from Wikimedia Commons, the Solar System Scope textures
(CC BY 4.0, built from NASA mission data) for the Sun and the planets without a public-domain
global colour mosaic, and the site's own NASA maps for Earth, the Moon and Mars.

Writes (one published file, to stay inside the host's file-count limit):
  assets/capabilities/bodies.jpg        the atlas: one equirectangular tile per body
  src/tools/bodies-atlas.json           tile index, source, licence and credit per body, and the
                                        Saturn ring profile (colour and opacity by radius); merged
                                        into assets/capabilities/capabilities.json by
                                        gen_capabilities.py (not published on its own)

Maps narrower than 2:1 (a mosaic that stops short of the poles) are padded at the top and bottom
with the mean colour of their polar-most rows; the atlas records which maps were padded. Downloads are cached in
.build/imagery/. Standard library plus Pillow.
Usage: /opt/homebrew/bin/python3 src/tools/fetch_body_imagery.py
"""
import json, os, sys, time, urllib.parse, urllib.request
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
SITE = os.path.dirname(os.path.dirname(HERE))
CACHE = os.path.join(SITE, ".build", "imagery")
OUT_IMG = os.path.join(SITE, "assets", "capabilities", "bodies.jpg")
OUT_JSON = os.path.join(HERE, "bodies-atlas.json")
UA = "kshana-site-build/1.0 (https://kshana.dev)"
TW, TH, COLS = 768, 384, 4

SSS = ("CC BY 4.0", "Solar System Scope (solarsystemscope.com), from NASA mission data")
# body -> (source, licence, credit). "local:" is a map already published with the site.
SOURCES = {
    "Sun": ("File:Solarsystemscope texture 2k sun.jpg",) + SSS,
    "Mercury": ("File:Solarsystemscope texture 2k mercury.jpg",) + SSS,
    "Venus": ("File:Solarsystemscope texture 2k venus atmosphere.jpg",) + SSS,
    "Earth": ("local:assets/planets/earth-day-2048.jpg", "Public domain", "NASA Visible Earth, Blue Marble"),
    "Moon": ("local:assets/planets/moon-2048.jpg", "Public domain", "NASA Scientific Visualization Studio, CGI Moon Kit"),
    "Mars": ("local:assets/planets/mars-1440.jpg", "Public domain", "NASA/JPL"),
    "Phobos": ("File:Phobos Viking Mosaic DLRcontrol 7200.jpg", "Public domain", "USGS Astrogeology / Planetary Data System, Viking mosaic (P. Stooke)"),
    "Deimos": ("File:Deimos color map.jpg", "CC BY-SA 3.0", "Oleg-Pluton, from NASA Viking imagery"),
    "Jupiter": ("File:Solarsystemscope texture 2k jupiter.jpg",) + SSS,
    "Io": ("File:Io from Galileo and Voyager missions.jpg", "Public domain", "NASA/JPL/USGS, Galileo and Voyager mosaic"),
    "Europa": ("File:Europa Voyager GalileoSSI global mosaic.jpg", "Public domain", "USGS / Planetary Data System, Voyager and Galileo SSI mosaic"),
    "Ganymede": ("File:Ganymede map NASA JPL Voyager.jpg", "Public domain", "Caltech/JPL/USGS, Voyager mosaic"),
    "Callisto": ("File:Callisto USGS global small.jpg", "Public domain", "USGS Astrogeology Science Center"),
    "Saturn": ("File:Solarsystemscope texture 2k saturn.jpg",) + SSS,
    "Titan": ("File:PIA22770-SaturnMoon-Titan-Surface-20181206.jpg", "Public domain", "NASA/JPL-Caltech/University of Arizona, Cassini mosaic PIA22770"),
    "Uranus": ("File:Solarsystemscope texture 2k uranus.jpg",) + SSS,
    "Neptune": ("File:Solarsystemscope texture 2k neptune.jpg",) + SSS,
    "Pluto": ("File:PIA20658-Pluto-Global-released20160502.jpg", "Public domain", "NASA/Johns Hopkins APL/Southwest Research Institute, New Horizons PIA20658"),
}
RING = "File:Solarsystemscope texture 2k saturn ring alpha.png"


def commons(title, width=1536):
    q = urllib.parse.urlencode(dict(action="query", format="json", titles=title, prop="imageinfo", iiprop="url|size", iiurlwidth=width))
    for i in range(5):
        try:
            d = json.load(urllib.request.urlopen(urllib.request.Request("https://commons.wikimedia.org/w/api.php?" + q, headers={"User-Agent": UA}), timeout=40))
            p = next(iter(d["query"]["pages"].values()))
            ii = p["imageinfo"][0]
            return ii.get("thumburl") or ii["url"], "https://commons.wikimedia.org/wiki/" + urllib.parse.quote(title.replace(" ", "_"))
        except Exception as e:  # rate limits: back off and retry
            print("  retry", title, e, file=sys.stderr)
            time.sleep(4 * (i + 1))
    sys.exit(f"could not resolve {title}")


def fetch(title):
    os.makedirs(CACHE, exist_ok=True)
    fn = os.path.join(CACHE, title[5:].replace(" ", "_").replace("/", "_"))
    url, page = commons(title)
    if not os.path.exists(fn):
        for i in range(5):
            try:
                open(fn, "wb").write(urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": UA}), timeout=120).read())
                break
            except Exception as e:
                print("  retry download", title, e, file=sys.stderr)
                time.sleep(4 * (i + 1))
        time.sleep(1.5)
    return fn, page


def to_tile(img):
    """Equirectangular tile TW x TH; a map short of 2:1 is padded top and bottom with its edge rows."""
    img = img.convert("RGB")
    w, h = img.size
    padded = False
    if w / h > 2.02:
        full_h = round(w / 2)
        canvas = Image.new("RGB", (w, full_h))
        top = (full_h - h) // 2
        canvas.paste(img, (0, top))
        # the polar gaps take the mean colour of the map's first and last 2 % of rows
        band = max(1, h // 50)
        mt = img.crop((0, 0, w, band)).resize((1, 1), Image.BOX).getpixel((0, 0))
        mb = img.crop((0, h - band, w, h)).resize((1, 1), Image.BOX).getpixel((0, 0))
        canvas.paste(mt, (0, 0, w, top))
        canvas.paste(mb, (0, top + h, w, full_h))
        img, padded = canvas, True
    return img.resize((TW, TH), Image.LANCZOS), padded


def main():
    names = list(SOURCES)
    rows = (len(names) + COLS - 1) // COLS
    atlas = Image.new("RGB", (TW * COLS, TH * rows), (0, 0, 0))
    meta = {"tile": [TW, TH], "cols": COLS, "rows": rows, "bodies": {}}
    for i, name in enumerate(names):
        src, lic, credit = SOURCES[name]
        if src.startswith("local:"):
            path, page = os.path.join(SITE, src[6:]), None
        else:
            path, page = fetch(src)
        tile, padded = to_tile(Image.open(path))
        atlas.paste(tile, ((i % COLS) * TW, (i // COLS) * TH))
        meta["bodies"][name] = {"i": i, "licence": lic, "credit": credit, "source": page or src[6:], "padded": padded}
        print(f"  {name:9s} {lic:14s} {'padded ' if padded else ''}{os.path.basename(path)}")
    # Saturn's rings: colour and opacity along the ring radius (the texture runs inner to outer edge).
    rp, page = fetch(RING)
    ring = Image.open(rp).convert("RGBA")
    strip = ring.resize((128, 1), Image.BOX)
    meta["ring"] = {"source": page, "licence": SSS[0], "credit": SSS[1],
                    "rgba": [list(strip.getpixel((x, 0))) for x in range(128)]}
    os.makedirs(os.path.dirname(OUT_IMG), exist_ok=True)
    atlas.save(OUT_IMG, quality=80, optimize=True, progressive=True)
    json.dump(meta, open(OUT_JSON, "w"), indent=1)
    print(f"wrote {os.path.relpath(OUT_IMG, SITE)} ({os.path.getsize(OUT_IMG) / 1e6:.2f} MB, {atlas.size[0]}x{atlas.size[1]}) and {os.path.relpath(OUT_JSON, SITE)}")


if __name__ == "__main__":
    main()
