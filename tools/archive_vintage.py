#!/usr/bin/env python3
"""Capture today's as-issued vintages into vintages/<UTC date>/ with a manifest and SHA256SUMS.

A failed source is recorded in the manifest and never fatal. Nothing that needs a login is fetched.
Run from the root of the data/vintage-archive branch: python3 tools/archive_vintage.py
"""
import datetime as dt
import hashlib
import pathlib
import re
import subprocess

MAX_FILE = 100 * 1024 * 1024
IGS = "https://igs.bkg.bund.de/root_ftp/IGS/products"
EDC = "https://edc.dgfi.tum.de/pub/slr/data/npt_crd"
LUNAR = ["apollo11", "apollo14", "apollo15", "luna17", "luna21"]

now = dt.datetime.now(dt.timezone.utc)
out = pathlib.Path("vintages") / now.strftime("%Y-%m-%d")
out.mkdir(parents=True, exist_ok=True)
rows = []


def curl(url, timeout=120):
    r = subprocess.run(["curl", "-sS", "-L", "-m", str(timeout), "--retry", "3", "-w", "\n%{http_code}", url],
                       capture_output=True)
    body, _, code = r.stdout.rpartition(b"\n")
    return (int(code) if code.strip().isdigit() else 0), body


def get(name, url):
    code, body = curl(url)
    note = ""
    if code == 200 and len(body) <= MAX_FILE:
        (out / name).write_bytes(body)
    elif code == 200:
        note = "skipped: over 100 MB"
    rows.append((name, url, str(code), dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"), note))


def listing(url):
    code, body = curl(url, 60)
    return body.decode("utf-8", "replace") if code == 200 else ""


get("bulletinA.txt", "https://datacenter.iers.org/data/latestVersion/bulletinA.txt")
get("finals.all.iau2000.txt", "https://datacenter.iers.org/data/latestVersion/finals.all.iau2000.txt")

week = (now.replace(tzinfo=None) - dt.datetime(1980, 1, 6)).days // 7
names = sorted(set(re.findall(r"IGS0OPSULT_\d+_02D_15M_ORB\.SP3\.gz", listing(f"{IGS}/{week}/"))))
if names:
    orb = names[-1]
    get(orb, f"{IGS}/{week}/{orb}")
    erp = orb.replace("_02D_15M_ORB.SP3.gz", "_02D_01D_ERP.ERP.gz")
    get(erp, f"{IGS}/{week}/{erp}")
else:
    rows.append(("IGS ultra-rapid", f"{IGS}/{week}/", "none-listed", now.strftime("%Y-%m-%dT%H:%M:%SZ"), ""))

# Lunar laser ranging normal points: files modified in the last two days, per lunar target.
for tgt in LUNAR:
    base = f"{EDC}/{tgt}/{now.year}/"
    for f in sorted(set(re.findall(r'href="([^"/?]+\.np2[^"]*)"', listing(base)))):
        if (now - dt.timedelta(days=2)).strftime("%Y%m%d") <= (re.findall(r"(20\d{6})", f) or ["0"])[-1]:
            get(f"llr_{tgt}_{f}", base + f)

lines = ["file\turl\thttp\tretrieved_utc\tnote"] + ["\t".join(r) for r in rows]
(out / "manifest.tsv").write_text("\n".join(lines) + "\n")
sums = [f"{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}" for p in sorted(out.iterdir())
        if p.name not in ("manifest.tsv", "SHA256SUMS")]
(out / "SHA256SUMS").write_text("\n".join(sums) + "\n")
print("\n".join(lines))
