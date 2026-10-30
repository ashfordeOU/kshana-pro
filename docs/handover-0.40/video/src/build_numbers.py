#!/usr/bin/env python3
"""Turn data/raw/raw.json (collect.py) into data/numbers.json (what the scenes draw) and NUMBERS.md (each source file, the
commit it was read at and the values taken from it). Nothing is typed here that a committed engine output or results file
at the tag does not contain."""
import json, re
from pathlib import Path
HERE = Path(__file__).resolve().parent
D = json.loads((HERE / "data" / "raw" / "raw.json").read_text())
ver, commit = D["engine_version"], D["engine_commit"]
th, c1, a1, a2, em, lg = D["thread"], D["c1"], D["a1"], D["a2"], D["emitter"], D["ledger"]
ses = em["session"]
feats = {}
for f in em["features"]:
    p = f["properties"]
    key = p["kind"] + (f"_{int(round(p['confidence'] * 100))}" if "confidence" in p else "")
    feats[key] = f["geometry"]
N = {
    "engine": {"version": ver, "commit": commit, "tag": "v" + ver},
    "thread": {"title": th["title"], "tier": th["tier"], "phases": th["phases"], "series": th["series"], "weakest": th["weakest"], "recorded_commit": th["recorded_engine_commit"],
               "evidence_tier": th["evidence"]["tier"], "evidence_ids": [r["id"] for r in th["evidence"]["rows"]], "kinds": D["kinds"], "kinds_from": D["growth"]["kinds_from"]},
    "c1": {"max_pos_km": round(c1["max_pos_m"] / 1000, 1), "bar_km": c1["bars"]["position_m"] / 1000, "vel_mps": round(c1["vel_end_mps"], 2), "bar_mps": c1["bars"]["velocity_mps"],
           "status": c1["status"], "per_epoch": [[e["hours"], round(e["position_difference_m"] / 1000, 2)] for e in c1["per_epoch"]], "diag_median_m": c1["diag_median_m"]},
    "a1": {"max_m": round(a1["max_3d_difference_m"]), "bar_m": a1["bars"]["max_3d_m"], "rms_m": round(a1["rms_radial_difference_m"], 1), "rms_bar_m": a1["bars"]["rms_radial_m"], "status": a1["status"]},
    "a2": {"end_km": round(a2["end_position_difference_m"] / 1000, 1), "bar_km": a2["bars"]["end_position_m"] / 1000, "peri_km": round(a2["periapsis_radius_difference_m"] / 1000, 1), "peri_bar_km": a2["bars"]["periapsis_radius_m"] / 1000, "status": a2["status"]},
    "emitter": {"geom": feats, "regions": ses["regions"], "ellipse": ses["uncertainty_ellipse_95"], "centroid": ses["centroid"], "duration_min": ses["duration_min"], "affected_distinct": ses["affected_distinct"],
                "observed_distinct": ses["observed_distinct"], "reports": em["input"]["reports_used"], "jt2025": em["jt2025"], "evidence_tier": em["evidence"]["tier"], "evidence_ids": [r["id"] for r in em["evidence"]["rows"]]},
    "clock": D["clock"],
    "bench40": [{"id": re.match(r"([A-Z]\d)", b["bar"]).group(1), "exploratory": "EXPLORATORY" in b["bar"], "measured": b["measured"], "verdict": b["verdict"]} for b in D["bench40"]],
    "bench_missed": {"A2": D["bench_missed"]["A2"][0], "D1": D["bench_missed"]["D1"][0], "R1": D["bench_missed"]["R1"]},
    "langs": D["langs"],
    "security": D["security"],
    "growth": D["growth"],
    "ledger": lg,
}
(HERE / "data").mkdir(exist_ok=True)
(HERE / "data" / "numbers.json").write_text(json.dumps(N, indent=1, ensure_ascii=False))

L = ["# Numbers in the 0.40 release video\n",
     f"Kshana {ver}, tag `v{ver}`, commit `{commit}` (public repository ashfordeOU/kshana). **Not re-run for the video**: building and running the engine was not "
     "permitted in the environment the video was made in, so every number is read from engine output and results files that the repository commits at that tag, "
     "by `src/collect.py`. Each file is listed below with its SHA-256 at the tag. The Studio's recorded mission-thread result records its own engine version "
     f"({th['recorded_engine_version']}) and the commit it was recorded at (`{th['recorded_engine_commit']}`). The commands that regenerate each output from a build of the "
     "tag are under \"Live commands\". Inputs marked synthetic are synthetic and say so on screen.\n",
     "## Files read at the tag\n", "| file | SHA-256 | taken from it |", "|---|---|---|"]
L += [f"| `{s['path']}` | `{s['sha256'][:16]}…` | {s['what']} |" for s in D["sources"] if not s["path"].startswith("i18n/")]
L.append(f"| `i18n/<code>/engine.ftl` ×{len(D['langs']['codes'])} | (per file in `src/data/raw/raw.json`) | the advisory line `engine-receiver-trust-chart-advisory` |")
L.append("\n## Numbers used on screen, and where each comes from\n")
ph = th["phases"]
L.append(f"- Mission thread (`{th['file']}`, recorded run): \"{th['title']}\", {len(ph)} phases: " + ", ".join(f"{p['id']} ({p['kind']})" for p in ph) +
         f". Peak 1-sigma position per phase (m): " + ", ".join(f"{p['id']} {p['peak_sigma_pos_m']:.1f}" for p in ph) +
         f". Weakest link: phase `{th['weakest']['phase_id']}`, margin {th['weakest']['margin']:.2f} against its stated limit, set by \"{th['weakest']['limiting_source']}\". Tier {th['tier']}; "
         f"evidence rows {', '.join(N['thread']['evidence_ids'])} (tier {N['thread']['evidence_tier']}). Every number MODELLED; the example's inputs are ILLUSTRATIVE.")
L.append(f"- Scenario kinds: {D['kinds']} (README \"What's new in 0.40\"; CHANGELOG [0.40.0]: from {D['growth']['kinds_from']} to {D['growth']['kinds_to']}).")
L.append(f"- C1, cislunar propagation against the public Artemis I trajectory, 72 h (`xval/mission-thread/results_oracle.json`): largest position difference {c1['max_pos_m']:.1f} m = {N['c1']['max_pos_km']} km against {N['c1']['bar_km']:g} km; "
         f"velocity at 72 h {c1['vel_end_mps']:.4f} m/s (shown as {N['c1']['vel_mps']} m/s) against {N['c1']['bar_mps']:g} m/s; per epoch (h, km) {N['c1']['per_epoch']}: **{c1['status']}**. Diagnostic (after the verdict, not part of it): median hourly residual {c1['diag_median_m']} m between the events.")
L.append(f"- A1, lunar propagator against the public LRO reconstructed trajectory, 24 h (`xval/mission-thread-lunar/results_oracles.json`): max 3-D {a1['max_3d_difference_m']:.2f} m against {a1['bars']['max_3d_m']:g} m, RMS radial {a1['rms_radial_difference_m']:.2f} m against {a1['bars']['rms_radial_m']:g} m: **{a1['status']}**.")
L.append(f"- A2, the same propagator against the public CAPSTONE orbit, one revolution: end position {a2['end_position_difference_m']:.1f} m = {N['a2']['end_km']} km against {N['a2']['bar_km']:g} km; periapsis radius {a2['periapsis_radius_difference_m']:.1f} m = {N['a2']['peri_km']} km against {N['a2']['peri_bar_km']:g} km: **{a2['status']}**.")
L.append(f"- Emitter region (`examples/emitter-region/output/`, the committed synthetic example, engine {em['version']}): one session {ses['start_utc']} to {ses['end_utc']} ({ses['duration_min']:g} min), {ses['affected_distinct']} affected of {ses['observed_distinct']} distinct aircraft, {em['input']['reports_used']} reports; "
         f"regions {', '.join(f'{int(r['confidence']*100)} % {r['area_km2']:g} km2' for r in ses['regions'])}; 95 % ellipse {ses['uncertainty_ellipse_95']['semi_major_km']} by {ses['uncertainty_ellipse_95']['semi_minor_km']} km, azimuth {ses['uncertainty_ellipse_95']['azimuth_deg']:g} deg; "
         f"evidence {', '.join(N['emitter']['evidence_ids'])} ({N['emitter']['evidence_tier']}). Input: \"{em['attribution']}\". Jammertest 2025 comparison (`docs/emitter-region/RESULTS.md`): " + "; ".join(f"{j['source']} {j['scheduled']} scheduled sessions, {j['outcome']}" for j in em["jt2025"]) + ".")
L.append(f"- Measured clocks: eight formats (CHANGELOG [0.40.0]); the fit misses F1(a) on {D['clock']['f1a']} records and F2 on {D['clock']['f2']} (README); both MODELLED.")
L.append("- Benchmark re-campaign (`xval/bench/RESULTS-0.40.md`): " + "; ".join(f"bar \"{b['bar']}\": {b['measured']}: {b['verdict']}" for b in D["bench40"]) +
         f". Unchanged and still NOT MET: A2 AUC {N['bench_missed']['A2']} against 0.80 (`xval/bench/RESULTS-v2.md`), D1 {N['bench_missed']['D1']}, R1 ({N['bench_missed']['R1']}, `docs/BENCH.md`).")
L.append(f"- Languages: {len(D['langs']['codes'])} catalogues under `i18n/` ({', '.join(D['langs']['codes'])}), English and {len(D['langs']['codes']) - 1} machine translations; the advisory line shown in each is the catalogue's `engine-receiver-trust-chart-advisory`.")
L.append(f"- Ledger (`web/data/verification-matrix.json`): {lg['rows']} rows, {lg['VALIDATED']} VALIDATED, {lg['MODELLED']} MODELLED, {lg['PARTNER']} PARTNER; CHANGELOG [0.40.0]: from {D['growth']['rows_from']} to {D['growth']['rows_to']} rows, no row promoted.")
L.append(f"- Security: {D['security']['advisory']} is fixed in the CHANGELOG section {D['security']['changelog_section']}, not [0.40.0]; minimum Rust {D['security']['msrv']} (`Cargo.toml` rust-version at the tag). Shown on screen only as \"since 0.39\".")
L.append("\n## Live commands\n\nFrom a build of the tag (`cargo build --release --bin kshana`), these regenerate the outputs above:\n")
L += [f"- `{c}`" for c in D["live"]]
(HERE.parent / "NUMBERS.md").write_text("\n".join(L) + "\n")
print("wrote numbers.json and NUMBERS.md")
