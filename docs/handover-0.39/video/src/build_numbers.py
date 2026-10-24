#!/usr/bin/env python3
"""Turn data/raw/raw.json (collect.py) into data/numbers.json (what the scenes draw) and NUMBERS.md (each command, its
commit and its output). Nothing is typed here that the engine did not print; the recorded verdicts (bench bars, booklet
comparison) are read from the committed results files and are labelled as such."""
import json, re, sys
from pathlib import Path
HERE = Path(__file__).resolve().parent
RAW = json.loads((HERE / "data" / "raw" / "raw.json").read_text())
D = RAW
ver, commit = D["engine_version"], D["engine_commit"]

jt = D["jammertest"]
tests = [{"id": t["test_id"], "name": t["test_name"], "onset": t["onset_s"], "end": t["end_s"], "outcome": t["outcome"],
          "latency": t.get("latency_s"), "recover": t.get("time_to_recover_s")} for t in jt["tests"]]
verify = json.loads([l for l in D["log"] if l["cmd"].startswith("kshana evidence verify")][0]["stdout"])
fl = D["flags"]["result"]["receiver_self_report"]
tm = D["timing"]
std = [{"pack": p["pack"], "title": p["title"], **{k: p["summary"][k] for k in ("met", "not_met", "needs_input")}} for p in D["standards"]]
bench = []
for k, v in sorted(D["bench"].items()):
    m = v["metrics"]
    bench.append({"name": v["system"]["name"].replace("impairment-eval baseline: ", ""), "auc_in": m["auc_in"], "auc_out": m["auc_out"], "gap": m["auc_in"] - m["auc_out"]})
res2 = D["bench_results_v2"]
def row(label):
    m = re.search(r"^\|\s*" + re.escape(label) + r"[^|]*\|([^|]*)\|([^|]*)\|", res2, re.M)
    return [x.strip().strip("*").strip() for x in m.groups()] if m else None
a2, d1 = row("A2"), row("D1")
heo = D["ssv_heo"]["result"]
comb = None
for b in heo["bands"]:
    for g in b["groups"]:
        if g["name"] == "combined":
            comb = comb or {}
            comb[b["band"]] = {"at_least_1": g["availability"]["at_least_1_pct"], "at_least_4": g["availability"]["at_least_4_pct"], "max_outage_4_min": g["availability"]["max_outage_4_min"]}
od, ode = D["lunar_od"]["consistency"], D["lunar_od_error"]["consistency"]
cf = D["lunar_carrier_full"]["arcs"]
mx = D["matrix"]["rows"]
from collections import Counter
cnt = Counter(r["status"] for r in mx)
N = {
    "engine": {"version": ver, "commit": commit},
    "jt": {"tests": tests, "summary": jt["summary"], "options": jt["options"], "jam_s": D["jt_synthetic"]["jam_s"], "secs": D["jt_synthetic"]["secs"], "plan": D["jt_synthetic"]["plan"],
           "pack_files": D["pack_files"], "verify": [{"name": c["name"], "status": c["status"]} for c in verify["checks"]], "verify_ok": verify["ok"], "tamper_rc": D["pack_tampered_rc"], "tamper_failures": D["pack_tampered_failures"], "label": jt["label"]},
    "flags": fl,
    "timing": {"verdicts": tm["verdicts"], "steps": [{"name": s["name"], "max_abs_te_ns": s["max_abs_te_ns"], "start": s["start_s"], "end": s["end_s"]} for s in tm["steps"]], "masks": [m.get("name") or m.get("id") for m in tm["masks"]], "series_ns": tm["record"]["series"]["time_error_ns"], "interval_s": tm["record"]["sample_interval_s"], "checks": {m["id"]: [{k: c.get(k) for k in ("metric", "limit_ns", "value_ns", "verdict", "source")} for c in m["checks"]] for m in tm["masks"]}},
    "standards": std,
    "bench": {"baselines": bench, "A2": a2, "D1": d1, "spec": D["bench"][sorted(D["bench"])[0]]["spec_version"]},
    "ssv": {"heo_orbit": D["ssv_heo"]["orbit"], "perigee_km": heo.get("perigee_alt_km"), "apogee_km": heo["apogee_alt_km"], "cn0_min_dbhz": D["ssv_heo"]["cn0_min_dbhz"], "combined": comb, "nav_final_err_m": D["ssv_nav"]["result"]["final_pos_err_m"], "formation_final_err_m": D["ssv_formation"]["result"]["final_pos_err_m"]},
    "lunar": {"od_mean_q": od["mean_q"], "od_expected": od["expected_mean_q"], "od_trials": od["trials"], "od_err_mean_q": ode["mean_q"], "od_err_accel": ode["model_error_accel_m_s2"],
              "od_frac_exceed_err": ode["fraction_exceeding_95"], "carrier_arcs": [{"days": a["arc_days"], "success": a["monte_carlo_success_rate"], "ambiguities": a["double_difference_ambiguities"]} for a in cf]},
    "ledger": {"rows": len(mx), "VALIDATED": cnt["VALIDATED"], "MODELLED": cnt["MODELLED"], "PARTNER": cnt["PARTNER"]},
}
(HERE / "data").mkdir(exist_ok=True)
(HERE / "data" / "numbers.json").write_text(json.dumps(N, indent=1))
# NUMBERS.md
L = [f"# Numbers in the 0.38 + 0.39 release video\n", f"Engine: kshana {ver}, commit `{commit}` (integ/0.39.0), built from that checkout. Every command below was run by `src/collect.py`; the output is what the engine printed. "
     "Logs written by the script are synthetic and say so on screen. The Jammertest catalogue and plan are the published NPRA/jammertest-plan files at commit 75c0b2ceea10e470c3799d14a8ef18842d8375a7 (MIT; fetched and SHA-256 checked, not vendored).\n"]
for i, l in enumerate(D["log"], 1):
    L.append(f"## {i}. `{l['cmd']}`\n- directory: `{l['cwd']}`; exit status {l['rc']}" + (f"; {l['note']}" if l["note"] else ""))
    so = l["stdout"].strip()
    if so and not so.startswith("{") and len(so) < 3200:
        L.append("```\n" + so + "\n```")
    L.append("")
L.append("## Numbers used on screen, and where each comes from\n")
L.append(f"- Jammertest: {N['jt']['summary']['tests']} tests (published plan, Test Area 2, {', '.join(t['id'] for t in tests)}); outcomes {', '.join(t['id']+' '+t['outcome'] for t in tests)}; latencies {', '.join('-' if t['latency'] is None else str(t['latency']) for t in tests)} s; median latency {N['jt']['summary']['median_latency_s']} s; false-alarm runs {N['jt']['summary']['false_alarm_runs']}, {N['jt']['summary']['false_alarm_epochs']} of {N['jt']['summary']['clean_epochs']} clean epochs. Source: commands 1 and 2 above (`--json`). The log is synthetic.")
L.append(f"- Evidence pack: files {', '.join(N['jt']['pack_files'])}; `kshana evidence verify` ok={N['jt']['verify_ok']}, checks {', '.join(c['name']+' '+c['status'] for c in N['jt']['verify'])}.")
L.append(f"- Tampered copy (one byte appended to summary.html): `kshana evidence verify` exit status {D['pack_tampered_rc']}; failures {D['pack_tampered_failures']}.")
L.append(f"- Receiver self-report (synthetic UBX log, {fl['epochs_reported']} epochs): receiver flag epochs {fl['receiver_flag_epochs']}, both {fl['both']}, receiver only {fl['receiver_only']}, Kshana only {fl['kshana_only']}; first Kshana alarm {fl['first_kshana_alarm_s']} s, first receiver flag {fl['first_receiver_flag_s']} s.")
L.append(f"- Holdover test (`holdover-test-example.toml`, a synthetic time-error record): verdicts {tm['verdicts']}; masks {N['timing']['masks']}; max |TE| per step {[(s['name'], s['max_abs_te_ns']) for s in N['timing']['steps']]} ns.")
L.append("- Standards packs: " + "; ".join(f"{s['pack']} {s['met']} met, {s['not_met']} not met, {s['needs_input']} needs input" for s in std) + ".")
L.append("- kshana-bench task D baselines on the test split (live run at the version above): " + "; ".join(f"{b['name']} AUC {b['auc_in']:.3f} in-distribution, {b['auc_out']:.3f} shifted" for b in bench) + f". Recorded verdicts (committed `xval/bench/RESULTS-v2.md`, not re-run for the video): A2 AUC {a2[0]}, {a2[1]}; D1 {d1[0]}, {d1[1].split('**')[0].strip()} ({d1[1].split('**')[-1].strip(' ()')}); task (b) R1 NOT MET (carried, not re-run), per `docs/BENCH.md`.")
L.append(f"- Space-user GNSS, HEO run (`gnss-ssv-heo.toml`): combined-constellation availability {json.dumps(comb)}; nav run final position error {N['ssv']['nav_final_err_m']:.0f} m; formation {N['ssv']['formation_final_err_m']:.0f} m. All MODELLED.")
L.append(f"- Lunar infrastructure: OD consistency mean q {od['mean_q']:.2f} against {od['expected_mean_q']:g} expected ({od['trials']} trials); with a model error {ode['model_error_accel_m_s2']:g} m/s2, mean q {ode['mean_q']:.2f}, fraction above the 95 % point {ode['fraction_exceeding_95']:.3f}; carrier-phase fixing success per arc {[(a['arc_days'], a['monte_carlo_success_rate']) for a in cf]} (committed full run `tests/fixtures/lunar_infra/phase_full_run.json`). All MODELLED.")
L.append(f"- Ledger: {len(mx)} rows: {cnt['VALIDATED']} VALIDATED, {cnt['MODELLED']} MODELLED, {cnt['PARTNER']} PARTNER (`web/data/verification-matrix.json` at the commit above).")
(HERE.parent / "NUMBERS.md").write_text("\n".join(L) + "\n")
print("wrote numbers.json and NUMBERS.md;", "A2", a2, "D1", d1, "comb", comb)
