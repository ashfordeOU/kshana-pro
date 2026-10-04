#!/usr/bin/env python3
"""Link-check the site tree pages.yml staged (the run artefact `pages-site` that the local
pages-build job keeps), then serve it and fetch every HTML page over HTTP.

Checks: every local href/src/srcset/url() in staged .html/.css resolves to a staged file
(directory -> index.html); no staged file is a test, smoke or build script; every .html
answers 200 from a real HTTP server; 404.html exists. Exit 1 on any finding.
usage: check_site.py <artifacts-dir>
"""
import sys, re, zipfile, pathlib, shutil, subprocess, time, urllib.request, urllib.parse, socket

art = pathlib.Path(sys.argv[1])
cands = sorted(art.rglob("pages-site*"), key=lambda p: p.stat().st_mtime)
if not cands:
    sys.exit("no pages-site artefact under " + str(art))
src = cands[-1]
site = art.parent / "site-check"
shutil.rmtree(site, ignore_errors=True)
site.mkdir(parents=True)
if src.is_dir():
    files = [p for p in src.rglob("*") if p.is_file()]
    if len(files) == 1 and files[0].suffix == ".zip":
        zipfile.ZipFile(files[0]).extractall(site)
    else:
        for p in files:
            d = site / p.relative_to(src)
            d.parent.mkdir(parents=True, exist_ok=True)
            # act's v4 artifact store may keep each file gzip-wrapped under its own name
            data = p.read_bytes()
            if data[:2] == b"\x1f\x8b" and p.suffix not in (".gz", ".tgz"):
                import gzip
                try:
                    data = gzip.decompress(data)
                except OSError:
                    pass
            d.write_bytes(data)
elif src.suffix == ".zip":
    zipfile.ZipFile(src).extractall(site)

problems = []
allf = [p for p in site.rglob("*") if p.is_file()]
print(f"staged files: {len(allf)}")
for p in allf:
    n = p.name
    if n.endswith(".test.mjs") or n in ("smoke.mjs", "build.sh"):
        problems.append(f"non-site file staged: {p.relative_to(site)}")
if not (site / "404.html").is_file():
    problems.append("no 404.html")

ref = re.compile(r'''(?:href|src)\s*=\s*["']([^"'#?]+)|url\(\s*["']?([^"')#?]+)''', re.I)
checked = 0
for p in allf:
    if p.suffix not in (".html", ".css"):
        continue
    import html as _html
    text = _html.unescape(p.read_text(errors="replace"))
    # A browser resolves relative references against <base href> when the page sets one
    # (the 0.29.3 Studio Advanced view sets <base href="../">).
    bm = re.search(r'<base\s[^>]*href\s*=\s*["\']([^"\']+)', text, re.I) if p.suffix == ".html" else None
    if bm and bm.group(0) not in p.read_text(errors="replace"):
        bm = None  # only a real tag counts, not an escaped &lt;base&gt; in prose
    base_dir = (p.parent / bm.group(1)) if bm and not re.match(r"^(https?:)?//", bm.group(1)) else p.parent
    for m in ref.finditer(text):
        u = (m.group(1) or m.group(2) or "").strip()
        if not u or re.match(r"^(https?:|mailto:|data:|javascript:|tel:|//|\$\{|\{)", u):
            continue
        u = urllib.parse.unquote(u).split("#")[0]
        if not u:
            continue
        target = (site / u.lstrip("/")) if u.startswith("/") else (base_dir / u)
        target = target.resolve()
        if target.is_dir():
            target = target / "index.html"
        elif not target.exists() and not target.suffix and target.with_name(target.name + ".html").is_file():
            # Clean addresses (0.29.1): GitHub Pages serves /evidence from evidence.html.
            target = target.with_name(target.name + ".html")
        checked += 1
        if not target.exists():
            problems.append(f"{p.relative_to(site)} -> {u} (missing)")
print(f"local references checked: {checked}")

s = socket.socket(); s.bind(("127.0.0.1", 0)); port = s.getsockname()[1]; s.close()
srv = subprocess.Popen([sys.executable, "-m", "http.server", str(port), "-d", str(site), "--bind", "127.0.0.1"],
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
try:
    time.sleep(1.0)
    pages = [p for p in allf if p.suffix == ".html"]
    ok = 0
    for p in pages:
        url = f"http://127.0.0.1:{port}/{p.relative_to(site).as_posix()}"
        try:
            with urllib.request.urlopen(url, timeout=10) as r:
                ok += r.status == 200
        except Exception as e:
            problems.append(f"GET {url}: {e}")
    print(f"HTML pages served 200: {ok} of {len(pages)}")
finally:
    srv.terminate()

for x in problems[:60]:
    print("PROBLEM:", x)
print(f"problems: {len(problems)}")
sys.exit(1 if problems else 0)
