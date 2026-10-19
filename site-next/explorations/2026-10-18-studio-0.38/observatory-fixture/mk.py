# Writes three INVENTED observatory days (GeoJSON, incidents, manifest, PNG each) in the shape of docs/observatory/METHOD.md, to review the viewer page.
# Every cell, count and hash is made up (a pseudo-random pattern on a Baltic-shaped box); none of it is observatory output or real data, and the dataset is
# labelled "synthetic sample". Day 2 is a day-wide-effect day; day 3 has no degraded cell. Run: python3 mk.py OUTDIR
import hashlib, json, os, random, struct, sys, zlib
out = sys.argv[1]; os.makedirs(out, exist_ok=True)
S, W, N, E, D = 54.0, 12.0, 60.0, 24.0, 0.5
NOTICE = "A degraded cell means a high share of aircraft reported low navigation accuracy there that day. No cause is attributed. A cell that is not observed, or absent, was not assessed, which is not the same as clear. Advisory only: not an approval, certification or statement of conformance."
DATA = {"dataset": "custom", "name": "Synthetic sample", "licence": "ODbL-1.0", "licence_url": "https://opendatacommons.org/licenses/odbl/1-0/", "attribution": "synthetic sample (invented for a page review)", "coverage_notes": "invented"}
def png(w, h, px):
    raw = b"".join(b"\0" + bytes(sum((px(x, y) for x in range(w)), ())) for y in range(h))
    def ch(t, d): c = struct.pack(">I", len(d)) + t + d; return c + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    return b"\x89PNG\r\n\x1a\n" + ch(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + ch(b"IDAT", zlib.compress(raw)) + ch(b"IEND", b"")
def day(date, seed, wide=False, none=False):
    rnd = random.Random(seed); cells = []; ni = int((N - S) / D); nj = int((E - W) / D)
    for i in range(ni):
        for j in range(nj):
            if rnd.random() < 0.45: continue
            r = rnd.random(); sampled = rnd.choice([None, 6, 9, 14, 30, 55])
            if sampled is None: st, reason = "not_observed", "insufficient_sample"
            elif wide: st, reason = "not_observed", "day_wide_effect"
            elif not none and 8 <= i <= 10 and 20 <= j <= 23 and r < .8: st, reason = "degraded", None
            elif r > .93: st, reason = "not_observed", "no_day_background"
            else: st, reason = "not_degraded", None
            aff = (int(sampled * .7) if st == "degraded" else None) if sampled else None
            cells.append((i, j, st, reason, sampled, aff))
    feats = []
    for i, j, st, reason, sampled, aff in cells:
        s, w = S + i * D, W + j * D
        p = {"cell_i": i, "cell_j": j, "state": st, "aircraft_observed": sampled and sampled + 2, "aircraft_sampled": sampled, "aircraft_affected": aff, "affected_share": (round(aff / sampled, 4) if aff and sampled and aff >= 5 else None)}
        if reason and st == "not_observed": p["reason"] = reason
        feats.append({"type": "Feature", "properties": p, "geometry": {"type": "Polygon", "coordinates": [[[w, s], [w + D, s], [w + D, s + D], [w, s + D], [w, s]]]}})
    cnt = lambda k: sum(1 for c in cells if c[2] == k)
    summary = {"cells_published": len(cells), "degraded": cnt("degraded"), "not_degraded": cnt("not_degraded"), "not_observed_published": cnt("not_observed"), "day_wide_effect": wide}
    nxt = {"2026-03-01": "2026-03-02", "2026-03-02": "2026-03-03", "2026-03-03": "2026-03-04"}[date]
    k = {"schema": "kshana-observatory-day/1", "product": "kshana-observatory/adsb/v1", "source_kind": "adsb", "date": date, "states": ["degraded", "not_degraded", "not_observed"], "grid": {"type": "fixed_lat_lon", "cell_deg": D}, "region": {"south": S, "west": W, "north": N, "east": E}, "release": {"min_delay_hours": 24, "not_before_utc": nxt + "T00:00:00Z"}, "summary": summary, "method": {"scope": "region: this box's reports only"}, "day": {"day_confounded": wide}, "data": DATA, "kshana_version": "0.38.0", "notice": NOTICE}
    gj = json.dumps({"type": "FeatureCollection", "kshana_observatory": k, "features": feats}, indent=1).encode()
    deg = [c for c in cells if c[2] == "degraded"]
    incs = []
    if deg:
        incs.append({"id": f"KOBS-{date}-001", "state": "degraded", "cell_count": len(deg), "bbox": {"south": S + min(c[0] for c in deg) * D, "west": W + min(c[1] for c in deg) * D, "north": S + (max(c[0] for c in deg) + 1) * D, "east": W + (max(c[1] for c in deg) + 1) * D}, "cells": [{"cell_i": c[0], "cell_j": c[1], "south": S + c[0] * D, "west": W + c[1] * D, "north": S + (c[0] + 1) * D, "east": W + (c[1] + 1) * D, "aircraft_sampled": c[4], "aircraft_affected": c[5], "affected_share": round(c[5] / c[4], 4) if c[5] and c[5] >= 5 else None} for c in deg]})
    inc = json.dumps({"schema": "kshana-incident-report/1", "product": "kshana-observatory/adsb/v1", "date": date, "source_kind": "adsb", "region": k["region"], "grid_cell_deg": D, "release": k["release"], "summary": summary, "incidents": incs, "cause": "not attributed", "day_wide_effect": wide, "reporting_cycle": "This file is an input to a report, confirm and notify cycle. It is a report of what published position data showed on one day, not a confirmation and not a NOTAM.", "method_id": "kshana-observatory/adsb/v1", "data": DATA, "kshana_version": "0.38.0", "notice": NOTICE}, indent=1).encode()
    col = {"degraded": (229, 56, 59), "not_degraded": (31, 85, 240)}
    cm = {(c[0], c[1]): c[2] for c in cells}; sc = 8
    def px(x, y):
        i, j = (N - S) / D - 1 - (y // sc), x // sc
        st = cm.get((int(i), int(j)))
        if st in col: return col[st]
        if st == "not_observed": return (150, 150, 150) if (x + y) % 6 < 2 else (225, 228, 235)
        return (245, 247, 251)
    pngb = png(int((E - W) / D) * sc, int((N - S) / D) * sc, px)
    base = f"observatory-adsb-synthetic-{date}"
    files = {f"{base}.geojson": gj, f"{base}.png": pngb, f"{base}.incidents.json": inc}
    man = json.dumps({"schema": "kshana-observatory-manifest/1", "product": "kshana-observatory/adsb/v1", "date": date, "kshana_version": "0.38.0", "method_id": "kshana-observatory/adsb/v1", "dataset": DATA, "input_stats": {"rows": 1000000 + seed, "used": 20000 + seed, "excluded_outside_region": 900000, "trace_files": 500}, "files": [{"name": n, "bytes": len(b), "sha256": hashlib.sha256(b).hexdigest()} for n, b in files.items()]}, indent=1).encode()
    files[f"{base}.manifest.json"] = man
    for n, b in files.items(): open(os.path.join(out, n), "wb").write(b)
day("2026-03-01", 1); day("2026-03-02", 2, wide=True); day("2026-03-03", 3, none=True)
