#!/usr/bin/env python3
"""Read the engine output committed at kshana tag v0.40.0 behind every number in the 0.40 release video.

    python3 collect.py --engine <kshana checkout at v0.40.0> --out data/raw

Nothing is run here. The video was made where building and running the engine was not permitted, so every number is
read from engine output that the kshana repository itself commits at the tag: the Studio's recorded results
(web/studio/recorded/*.json, each recording its engine version and commit), the committed example outputs, the
pre-registered comparison results under xval/ and tests/fixtures/, the verification matrix, and the i18n catalogues.
Each file read is listed with its path, its SHA-256 and the commit it was read at; build_numbers.py turns raw.json into
data/numbers.json (what the scenes draw) and NUMBERS.md (each source and its value). LIVE lists the engine commands that
regenerate the same outputs from a build of the tag, for anyone who can run them.
"""
import argparse, hashlib, json, re, subprocess
from collections import Counter
from pathlib import Path

ap = argparse.ArgumentParser()
ap.add_argument("--engine", required=True)
ap.add_argument("--out", required=True)
a = ap.parse_args()
ENG, OUT = Path(a.engine).resolve(), Path(a.out).resolve()
OUT.mkdir(parents=True, exist_ok=True)
commit = subprocess.run(["git", "-C", str(ENG), "rev-parse", "HEAD"], capture_output=True, text=True, check=True).stdout.strip()
version = [l for l in (ENG / "Cargo.toml").read_text().splitlines() if l.startswith("version")][0].split('"')[1]
SOURCES = []  # every file read: path, sha256, what was taken from it


def rd(rel, what):
    b = (ENG / rel).read_bytes()
    SOURCES.append({"path": rel, "sha256": hashlib.sha256(b).hexdigest(), "what": what})
    return b.decode("utf-8")


def js(rel, what):
    return json.loads(rd(rel, what))


out = {"engine_commit": commit, "engine_version": version}

# 1. the Artemis-class mission thread: the Studio's recorded run of scenarios/mission-thread-artemis-class.toml
rec = js("web/studio/recorded/mission-thread-artemis-class.json", "recorded run of scenarios/mission-thread-artemis-class.toml (phases, peaks, margins, weakest link, evidence)")
r = rec["json"] if isinstance(rec["json"], dict) else json.loads(rec["json"])
phases = [{k: p.get(k) for k in ("index", "id", "kind", "start_s", "end_s", "peak_sigma_pos_m", "peak_sigma_vel_mps", "margin", "requirement", "limiting_source", "nav_sources")} | {"tier": p["label"]["tier"]} for p in r["phases"]]
tl = r["timeline"]
series = []  # sigma_pos per phase, at most 60 points each (the chart draws equal width per phase)
for p in phases:
    pts = [x for x in tl if x["phase_index"] == p["index"]]
    step = max(1, len(pts) // 60)
    series.append([[x["t_s"], x["sigma_pos_m"]] for x in pts[::step]] + ([[pts[-1]["t_s"], pts[-1]["sigma_pos_m"]]] if pts else []))
out["thread"] = {"file": rec["file"], "recorded_engine_version": rec["engine_version"], "recorded_engine_commit": rec["engine_commit"], "summary": rec["summary"],
                 "title": r["title"], "notice": r["notice"], "tier": r["label"]["tier"], "weakest": r["weakest_link"], "phases": phases, "series": series, "evidence": r["evidence"]}
# 2. the pre-registered comparisons
c1 = js("xval/mission-thread/results_oracle.json", "row C1: cislunar propagation against the public Artemis I trajectory, 72 h")
out["c1"] = {"bars": c1["bars"], "status": c1["kernel_moon"]["status"], "max_pos_m": c1["kernel_moon"]["max_position_difference_m"], "vel_end_mps": c1["kernel_moon"]["velocity_difference_at_end_mps"],
             "per_epoch": c1["kernel_moon"]["per_epoch"]}
diag = rd("xval/mission-thread/RESULTS.md", "diagnostic: median hourly residual between the five events")
m = re.search(r"to a median of ([0-9.]+) m", diag)
out["c1"]["diag_median_m"] = float(m.group(1))
ao = js("xval/mission-thread-lunar/results_oracles.json", "rows A1 (LRO, 24 h) and A2 (CAPSTONE, one revolution)")
out["a1"], out["a2"] = ao["a1"], ao["a2"]
# 3. the emitter region: the committed synthetic example (examples/emitter-region/regenerate.sh)
gj = js("examples/emitter-region/output/emitter-adsb-custom-2026-03-01.geojson", "regions at 50/90/95 %, the 95 % ellipse and the centroid (synthetic input)")
sm = js("examples/emitter-region/output/emitter-adsb-custom-2026-03-01-summary.json", "session, region areas, ellipse, input counts, evidence (synthetic input)")
e = sm["kshana_emitter_region"]
out["emitter"] = {"features": gj["features"], "session": e["sessions"][0], "input": e["method"]["input_stats"], "attribution": e["data"]["attribution"], "version": e["kshana_version"], "evidence": sm["evidence"]}
er = rd("docs/emitter-region/RESULTS.md", "Jammertest 2025 comparison: outcome per source")
out["emitter"]["jt2025"] = [{"source": m.group(1), "published": int(m.group(2)), "scheduled": int(m.group(3)), "outcome": m.group(4)} for m in re.finditer(r"^\| (ADS-B|AIS) \| (\d+) \| (\d+) \| \*\*([A-Z ]+)\*\*", er, re.M)]
# 4. measured clocks and the fitted holdover
rm = rd("README.md", "What's new in 0.40 (clock-fit misses, 92 kinds); README is the release text")
out["clock"] = {"f1a": re.search(r"misses\*\* F1\(a\) on ([a-z]+ of [a-z]+) records", rm).group(1), "f2": re.search(r"and F2 on ([a-z]+)", rm).group(1)}
cf = rd("docs/CLOCK-LOG-FORMATS.md", "the eight clock-log formats")
out["clock"]["formats_doc_head"] = cf[:4000]
out["kinds"] = int(re.search(r"(\d+) scenario kinds in all", rm).group(1))
# 5. data-partner check: the committed synthetic submission
out["partner"] = rd("examples/partner-submission/submission.toml", "synthetic data-partner submission")
# 6. bench re-campaign and the bars that stay missed
b4 = rd("xval/bench/RESULTS-0.40.md", "0.40 re-campaign verdicts A3, D3, R3, R4")
out["bench40"] = [{"bar": re.sub(r"\*", "", m.group(1)).strip(), "measured": m.group(2).strip(), "verdict": m.group(3).strip()} for m in re.finditer(r"^\| (\*\*[AD]3[^|]*|R[34][^|]*)\|([^|]*)\|([^|]*)\|", b4, re.M)]
b2 = rd("xval/bench/RESULTS-v2.md", "A2 and D1 recorded verdicts (unchanged in 0.40)")
row = lambda lab: [x.strip().strip("*").strip() for x in re.search(r"^\|\s*" + lab + r"[^|]*\|([^|]*)\|([^|]*)\|", b2, re.M).groups()]
out["bench_missed"] = {"A2": row("A2"), "D1": row("D1"), "R1": "NOT MET" if "task (b) R1 is NOT MET" in rd("docs/BENCH.md", "R1 verdict") else None}
# 7. languages: the advisory line from every engine catalogue
langs = sorted(p.name for p in (ENG / "i18n").iterdir() if p.is_dir())
adv = {}
for l in langs:
    t = rd(f"i18n/{l}/engine.ftl", "engine-receiver-trust-chart-advisory")
    adv[l] = re.search(r"^engine-receiver-trust-chart-advisory = (.*)$", t, re.M).group(1).strip()
out["langs"] = {"codes": langs, "advisory": adv}
# 8. security, ledger, release text
cg = rd("CHANGELOG.md", "[0.40.0]: security entry, matrix growth, kinds growth")
s040 = cg.split("## [0.40.0]")[1].split("\n## [")[0]
# the RUSTSEC-2026-0387 fix is in the [0.39.0] section, not [0.40.0]: recorded with the section it is in, shown only as "since 0.39"
sec = [h for h in re.split(r"\n## ", cg) if "RUSTSEC-2026-0387" in h][0]
out["security"] = {"advisory": "RUSTSEC-2026-0387", "changelog_section": sec.split("\n")[0].strip(), "msrv": re.search(r'^rust-version = "([0-9.]+)"', rd("Cargo.toml", "rust-version"), re.M).group(1)}
g = re.search(r"grows from (\d+) to (\d+) rows", s040); k = re.search(r"kinds grow from (\d+) to (\d+)", s040)
out["growth"] = {"rows_from": int(g.group(1)), "rows_to": int(g.group(2)), "kinds_from": int(k.group(1)), "kinds_to": int(k.group(2))}
mx = js("web/data/verification-matrix.json", "the ledger: rows and status counts")["rows"]
out["ledger"] = {"rows": len(mx), **Counter(r["status"] for r in mx)}
out["sources"] = SOURCES
out["live"] = [
    "kshana scenarios/mission-thread-artemis-class.toml",
    "cargo run --release --example mission_thread_oracle -- <dir>   # xval/mission-thread/fetch_data.sh first",
    "kshana interference-map locate examples/emitter-region/input/adsb-synthetic.csv --source adsb --dataset custom --licence CC0-1.0 ... --out out",
    "kshana partner check examples/partner-submission/submission.toml",
    "kshana --lang <code> scenarios/<file>.toml",
]
(OUT / "raw.json").write_text(json.dumps(out, indent=1, ensure_ascii=False))
print("wrote", OUT / "raw.json", "tag", version, commit[:8], "files read", len(SOURCES))
