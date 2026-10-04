#!/usr/bin/env python3
"""Record a real kshana-mcp session for the Home page (src/data/mcp-session.json).

Drives the kshana-mcp binary over standard input and output, message by message:
initialize, tools/list, validate_scenario and run_scenario on a bundled scenario. It keeps the
server identity, every tool name with the first sentence of its description, the validation reply,
the run summary, the result's identity fields, each clock's figures of merit, every 4th sample of
each returned series (plus the last) and the outage window. Nothing is typed by hand.

Usage:
  python3 tools/record_mcp_session.py <path/to/kshana-mcp> [scenario.toml] [out.json]

Build the binary from the engine repository first (cargo build --release -p kshana-mcp).
Defaults: scenario = ~/Code/kshana/scenarios/clock-holdover.toml,
out = explorations/2026-09-27-combo/src/data/mcp-session.json. Then run build.py, which passes it to
Home as page data (D.mcp).
"""
import datetime
import json
import os
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
BIN = sys.argv[1]
SCN = sys.argv[2] if len(sys.argv) > 2 else os.path.expanduser("~/Code/kshana/scenarios/clock-holdover.toml")
OUT = sys.argv[3] if len(sys.argv) > 3 else os.path.join(HERE, "..", "explorations", "2026-09-27-combo", "src", "data", "mcp-session.json")

toml_text = open(SCN).read()
p = subprocess.Popen([BIN], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
log = []


def send(msg):
    log.append({"dir": "client->server", "t": time.time(), "msg": msg})
    p.stdin.write(json.dumps(msg) + "\n")
    p.stdin.flush()


def recv():
    m = json.loads(p.stdout.readline())
    log.append({"dir": "server->client", "t": time.time(), "msg": m})
    return m


send({"jsonrpc": "2.0", "id": 1, "method": "initialize",
      "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "kshana-site-recorder", "version": "1"}}})
init = recv()["result"]
send({"jsonrpc": "2.0", "method": "notifications/initialized"})
send({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})
tools = recv()["result"]["tools"]
send({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "validate_scenario", "arguments": {"toml": toml_text}}})
val = recv()["result"]["content"][0]["text"]
send({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "run_scenario", "arguments": {"toml": toml_text}}})
run_res = recv()["result"]["content"]
p.stdin.close()
p.wait(timeout=30)

summary = run_res[0]["text"]
j = json.loads(run_res[1]["text"])
lines = toml_text.splitlines()


def first(s):
    return s.split(". ")[0].rstrip(".") + "."


out = {
    "recorded": datetime.date.fromtimestamp(log[0]["t"]).isoformat(),
    "protocol": init["protocolVersion"],
    "server": {k: init["serverInfo"][k] for k in ("name", "version")},
    "tools": [{"name": t["name"], "does": first(t["description"])} for t in sorted(tools, key=lambda t: t["name"])],
    "file": os.path.basename(SCN),
    "tomlHead": lines[:6], "tomlLines": len(lines),
    "validate": val,
    "summary": summary,
    "result": {k: j[k] for k in ("schema_version", "engine_version", "scenario_hash", "seed", "threshold_ns")},
    "clocks": [{"role": k, "id": j[k]["spec"]["id"],
                "fom": {f: j[k]["fom"][f] for f in ("holdover_s", "timing_p95_ns", "timing_rms_ns", "availability")},
                "series": [[q["t"], round(q["error_ns"], 6)] for i, q in enumerate(j[k]["series"]) if i % 4 == 0 or i == len(j[k]["series"]) - 1],
                "outage": [min(q["t"] for q in j[k]["series"] if q["gnss"] != "nominal"), max(q["t"] for q in j[k]["series"])]}
               for k in ("classical", "quantum")],
    "tiers": sorted({f["tier"] for f in j["figure_tiers"]["figures"]}),
}
with open(OUT, "w") as fh:
    json.dump(out, fh, ensure_ascii=False, separators=(",", ":"))
print(f"recorded {len(log)} messages from {out['server']['name']} {out['server']['version']} -> {OUT}")
