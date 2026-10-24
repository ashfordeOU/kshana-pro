#!/usr/bin/env python3
"""Run the real engine commands behind every number in the 0.38 + 0.39 release video.

    python3 collect.py --engine <kshana checkout> --bin <kshana binary> --plan <dir> --out data/raw

* --engine is the kshana checkout the binary was built from (its HEAD commit is recorded with every command);
* --bin is that build (`kshana --version` must equal the checkout's Cargo.toml version);
* --plan is a directory fetched with <engine>/xval/jammertest-plan/fetch.sh (the published Jammertest catalogue and
  plan, SHA-256 checked there; nothing from it is committed here).

Needs `fonttools` (pip install fonttools): the engine's figure generator imports it, and this script reuses that
generator's loaders and its synthetic-log writers, so the video's inputs are the README figures' inputs.
Every command's stdout, the files it writes and its exit status are saved under --out; build_numbers.py turns them
into data/numbers.json (what the scenes draw) and NUMBERS.md (each command, its commit and its output).
"""
import argparse, json, os, shutil, subprocess, sys, tempfile
from pathlib import Path

ap = argparse.ArgumentParser()
ap.add_argument("--engine", required=True)
ap.add_argument("--bin", required=True)
ap.add_argument("--plan", required=True)
ap.add_argument("--out", required=True)
a = ap.parse_args()
ENG, BIN, PLAN, OUT = Path(a.engine).resolve(), str(Path(a.bin).resolve()), Path(a.plan).resolve(), Path(a.out).resolve()
OUT.mkdir(parents=True, exist_ok=True)
sys.path.insert(0, str(ENG / "tools"))
import gen_figures_038 as f  # noqa: E402  (the engine's own loaders and synthetic-log writers)

commit = subprocess.run(["git", "-C", str(ENG), "rev-parse", "HEAD"], capture_output=True, text=True, check=True).stdout.strip()
version = subprocess.run([BIN, "--version"], capture_output=True, text=True, check=True).stdout.split()[-1]
cargo = [l for l in (ENG / "Cargo.toml").read_text().splitlines() if l.startswith("version")][0].split('"')[1]
assert version == cargo, f"binary {version} is not the checkout's {cargo}"
LOG = []  # one entry per command: what was run, in which directory, what it printed


def sh(work, *args, ok=(0,), note=""):
    p = subprocess.run([BIN, *args], cwd=work, capture_output=True, text=True)
    LOG.append({"cmd": "kshana " + " ".join(args), "cwd": Path(work).name, "rc": p.returncode, "stdout": p.stdout, "stderr": p.stderr[-400:], "note": note})
    if p.returncode not in ok:
        sys.exit(f"`kshana {' '.join(args)}` exited {p.returncode}: {p.stderr[:300]}")
    return p.stdout


tmp = Path(tempfile.mkdtemp(prefix="kshana-video-"))
run = f.Run(BIN, tmp)
out = {"engine_commit": commit, "engine_version": version}

# 1. Jammertest scoring: the published plan (Monday 2025-09-15, Test Area 2, its first four test slots, local time
#    14:16 to 15:16) against a SYNTHETIC receiver log written here. t = 0 is 12:00:00Z = 14:00 CEST, so every slot
#    has a clean lead-in. The jammer is on from the start of slots 1 and 4, only from 150 s into slot 2 (late), never in
#    slot 3 (missed), plus two 60 s bursts outside every slot (false alarms).
jt = tmp / "jt"; jt.mkdir()
plan = json.loads((PLAN / "plan-monday-2025-09-15.json").read_text())
loc = [l for l in plan["locations"] if l["location_id"] == 2][0]
loc["tests"] = loc["tests"][3:7]
(jt / "plan4.json").write_text(json.dumps({"locations": [loc]}))
shutil.copyfile(PLAN / "testcatalog.json", jt / "testcatalog.json")
JAM = [(400, 460), (990, 1660), (1740, 1800), (2070, 2640), (3850, 4550)]
(jt / "log.nmea").write_text(f.nmea_log(4800, JAM, 20.0))
(jt / "session.toml").write_text('[log]\nformat = "nmea"\npath = "log.nmea"\n')
args = ["jammertest", "score", "session.toml", "--catalog", "testcatalog.json", "--plan", "plan4.json", "--location", "2", "--local-offset-h", "2"]
out["jammertest_text"] = sh(jt, *args)
out["jammertest"] = json.loads(sh(jt, *args, "--json"))
sh(jt, "evidence", "keygen", "--out", "signer.key")
sh(jt, *args, "--key", "signer.key", "--out", "pack", "--created-utc", "2025-09-15T15:00:00Z", note="signed evidence pack")
out["pack_files"] = sorted(str(p.relative_to(jt / "pack")) for p in (jt / "pack").rglob("*") if p.is_file())
pub = json.loads(Path(jt / "pack" / "manifest.json").read_text()) if (jt / "pack" / "manifest.json").exists() else {}
vr = subprocess.run([BIN, "evidence", "verify", "pack", "--pubkey", "signer.key.pub", "--json"], cwd=jt, capture_output=True, text=True)
LOG.append({"cmd": "kshana evidence verify pack --pubkey signer.key.pub --json", "cwd": "jt", "rc": vr.returncode, "stdout": vr.stdout, "stderr": vr.stderr[-300:], "note": "verifies the signed pack"})
out["pack_verify_rc"] = vr.returncode
out["jt_synthetic"] = {"secs": 4800, "jam_s": JAM, "plan": "NPRA/jammertest-plan @ 75c0b2c, monday 2025-09-15, Test Area 2, tests 1.1.4, 1.1.8, 1.1.12, 1.1.13"}

# 2. the receiver's own flag beside Kshana's alarm (a synthetic UBX log)
fl = f.load_flags(run); LOG.append({"cmd": "kshana receiver-trust session.toml", "cwd": "flags", "rc": 0, "stdout": "(writes session.result.json)", "stderr": "", "note": "synthetic UBX log, 200 epochs"})
out["flags"] = fl
# 3. a holdover test scored against the ITU-T masks
out["timing"] = run.scenario("scenarios/holdover-test-example.toml"); LOG.append({"cmd": "kshana holdover-test-example.toml", "cwd": "run", "rc": 0, "stdout": "(writes holdover-test-example.result.json)", "stderr": "", "note": ""})
# 4. the standards packs
out["standards"] = f.load_standards(run)
for pid in ("dhs-rpcf", "imo-circ-1575", "icao-doc9849-appf"):
    LOG.append({"cmd": f"kshana standards-pack run {pid} --json", "cwd": "run", "rc": 0, "stdout": "", "stderr": "", "note": ""})
# 5. kshana-bench task D baselines on the test split, and the committed results file for the bars
out["bench"] = f.load_bench(run); LOG.append({"cmd": "kshana bench baseline --task d --split test --out bench", "cwd": "run", "rc": 0, "stdout": "", "stderr": "", "note": ""})
out["bench_results_v2"] = (ENG / "xval" / "bench" / "RESULTS-v2.md").read_text()
out["bench_doc"] = (ENG / "docs" / "BENCH.md").read_text()
# 6. space-user GNSS and lunar infrastructure (MODELLED)
for key, scn in [("ssv_shell", "gnss-ssv-shell"), ("ssv_heo", "gnss-ssv-heo"), ("ssv_nav", "gnss-ssv-nav-lunar"), ("ssv_formation", "gnss-ssv-formation-heo"),
                 ("lunar_od", "lunar-infrastructure-od-consistency"), ("lunar_od_error", "lunar-infrastructure-od-consistency-model-error"),
                 ("lunar_frame", "lunar-infrastructure-frame-coestimation"), ("lunar_tie", "lunar-infrastructure-space-tie"), ("lunar_geodesy", "lunar-infrastructure-geodesy-sensitivity"),
                 ("lunar_carrier", "lunar-infrastructure-carrier-phase")]:
    out[key] = run.scenario(f"scenarios/{scn}.toml"); LOG.append({"cmd": f"kshana {scn}.toml", "cwd": "run", "rc": 0, "stdout": "(writes result.json)", "stderr": "", "note": ""})
out["ssv_booklet"] = f.load_ssv_booklet(run)
out["lunar_carrier_full"] = json.loads((ENG / "tests" / "fixtures" / "lunar_infra" / "phase_full_run.json").read_text())
# 7. the ledger
out["matrix"] = json.loads((ENG / "web" / "data" / "verification-matrix.json").read_text())
out["log"] = LOG
(OUT / "raw.json").write_text(json.dumps(out, indent=1, default=str))
print("wrote", OUT / "raw.json", "engine", version, commit[:8], "commands", len(LOG))
shutil.rmtree(tmp, ignore_errors=True)
