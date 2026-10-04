#!/usr/bin/env python3
"""Record the terminal session the Developers hero replays: real `kshana` commands and their real output.

Runs the engine binary of the site's one engine checkout (src/data/sources.json: repo; target/release/kshana there) from a
fresh scratch directory, on byte copies of bundled scenario files (the engine writes result.json,
chart.svg and report.html next to the TOML it reads, so the worktree is never written). For every
command it keeps the exact command line, stdout and stderr line by line, the exit code and the wall
time, and for every scenario its file name, SHA-256 and seed (read from the TOML; "none" when the
kind is deterministic). Nothing is retyped: the page prints these lines as captured.

  src/data/cli-session.json   engine version and commit, capture date, and the steps

build.py never runs the engine: it passes this file to the Developers page as page data and refuses
to build when its engine version is not the site's version.
Re-run after an engine change:  /opt/homebrew/bin/python3 src/tools/record_cli_session.py
"""
import datetime, hashlib, json, os, shutil, subprocess, sys, tempfile, time, tomllib

HERE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from _sources import engine_tree  # noqa: E402  (src/data/sources.json: the one engine checkout)
WT = engine_tree()
ENGINE = os.path.join(WT, "target", "release", "kshana")
OUT = os.path.join(SRC, "data", "cli-session.json")
SCENARIOS = ["jamming-demo", "spoof-detect", "clock-holdover"]
STEPS = [["--version"], ["--validate", "jamming-demo.toml"]] + [[f + ".toml"] for f in SCENARIOS]


def sh(*a, **k):
    return subprocess.run(a, capture_output=True, text=True, check=True, **k).stdout.strip()


def main():
    version = sh(ENGINE, "--version").split()[-1]
    commit = sh("git", "-C", WT, "rev-parse", "HEAD")[:7]
    branch = sh("git", "-C", WT, "rev-parse", "--abbrev-ref", "HEAD")
    tmp = tempfile.mkdtemp(prefix="kshana-cli-")
    scen = {}
    try:
        for f in SCENARIOS:
            raw = open(os.path.join(WT, "scenarios", f + ".toml"), "rb").read()
            open(os.path.join(tmp, f + ".toml"), "wb").write(raw)
            seed = tomllib.loads(raw.decode("utf-8")).get("seed")
            scen[f + ".toml"] = {"sha256": hashlib.sha256(raw).hexdigest(), "seed": seed,
                                 "kind": tomllib.loads(raw.decode("utf-8")).get("kind", "clock")}  # a file without kind runs as clock
        steps = []
        for args in STEPS:
            t0 = time.perf_counter()
            p = subprocess.run([ENGINE] + args, capture_output=True, text=True, cwd=tmp)
            ms = (time.perf_counter() - t0) * 1000
            lines = [ln for ln in (p.stdout + p.stderr).splitlines() if ln.strip()]
            steps.append({"cmd": "kshana " + " ".join(args), "out": lines, "rc": p.returncode, "ms": round(ms, 1),
                          "scenario": next((a for a in args if a.endswith(".toml")), None)})
            if p.returncode:
                sys.exit(f"kshana {' '.join(args)} exited {p.returncode}: {p.stderr}")
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    doc = {"recorded": datetime.date.today().isoformat(), "engine": {"version": version, "commit": commit, "tree": branch,
           "binary": "target/release/kshana"}, "scenarios": scen, "steps": steps}
    with open(OUT, "w", encoding="utf-8") as fh:
        json.dump(doc, fh, ensure_ascii=False, indent=1)
        fh.write("\n")
    print(f"wrote {os.path.relpath(OUT, SRC)}: engine {version} ({commit}), {len(steps)} commands")


if __name__ == "__main__":
    main()
