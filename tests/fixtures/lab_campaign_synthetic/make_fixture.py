"""Writes the synthetic lab-campaign fixture `basic/` (stdlib only, deterministic).

Every value here is HAND-MADE for schema, loader and rendering tests. Nothing is the output of
any run and nothing is a measurement of any receiver. The shape follows the public engine's
docs/design/LAB-CAMPAIGN.md section 3 (proposal @ f69efe9). campaign.json's shape is
provisional (see src/lab_campaign/model.rs).

    python3 make_fixture.py        # rewrites basic/
"""
import hashlib, json, os, shutil

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "basic")
ENGINE = "0.34.0"


def canon(v):
    # serde_json compact, sorted keys. The hashed values here are strings only, so this
    # matches serde_json byte for byte.
    return json.dumps(v, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def h(s):
    return hashlib.sha256(s.encode()).hexdigest()


def pretty(v):
    return json.dumps(v, indent=2, sort_keys=True) + "\n"


RECORDINGS = [
    {
        "id": "synthetic-rec-a",
        "conditions": {
            "schema": "kshana.test-conditions/1",
            "recording": {"id": "synthetic-rec-a", "path": "scene-a.sigmf-meta", "settle_s": 5.0},
            "receiver": {"dut": "synthetic scene (no receiver)", "front_end": "ideal", "notes": ""},
            "expected": [{"signal": "gps-l1ca", "ids": [3, 11], "nominal_cn0_dbhz": 45.0}],
            "event": [{
                "id": "ramp-1", "kind": "interference", "type": "broadband",
                "onset_s": 60.0, "offset_s": 180.0, "affects": "all",
                "power": {"quantity": "js_db", "interpolation": "linear",
                          "points": [[60.0, 20.0], [180.0, 50.0]]},
            }],
        },
    },
    {
        "id": "synthetic-rec-b",
        "conditions": {
            "schema": "kshana.test-conditions/1",
            "recording": {"id": "synthetic-rec-b", "path": "scene-b.sigmf-meta", "settle_s": 5.0},
            "receiver": {"dut": "synthetic scene (no receiver)", "front_end": "ideal", "notes": ""},
            "expected": [{"signal": "gps-l1ca", "ids": [6, 22]}],
            "event": [{"id": "gap-1", "kind": "outage", "type": "unknown",
                       "onset_s": 30.0, "offset_s": 40.0, "affects": [6]}],
        },
    },
]
for r in RECORDINGS:
    r["sha256"] = h("recording:" + r["id"])
    r["conditions_hash"] = h(canon(r["conditions"]))

FRONTENDS = [
    {"name": "raw", "params": {}},
    {"name": "notch+2bit", "params": {"notch": True, "bits": 2}},
]
DESIGNS = [
    {"name": "baseline", "params": {"pll_order": 2, "pll_bw_hz": 15.0, "fll_bw_hz": 10.0,
                                     "dll_bw_hz": 2.0, "coherent_ms": 1, "spacing_chips": 0.5}},
    {"name": "narrow-pll", "params": {"pll_order": 3, "pll_bw_hz": 8.0, "fll_bw_hz": 5.0,
                                       "dll_bw_hz": 1.0, "coherent_ms": 4, "spacing_chips": 0.5}},
    {"name": "wide-pll", "params": {"pll_order": 2, "pll_bw_hz": 25.0, "fll_bw_hz": 10.0,
                                     "dll_bw_hz": 2.0, "coherent_ms": 1, "spacing_chips": 0.5}},
]
for x in FRONTENDS + DESIGNS:
    x["hash"] = h(canon({"name": x["name"], "params": x["params"]}))

SCORING = {"baseline_window_s": 10.0, "js_bin_db": 5.0, "false_lock_doppler_hz": "auto",
           "reacq_grace_s": 30.0, "reference_curve": True, "bars": {"max_reacq_s": 5.0}}
RUN_HASH = h("run:synthetic-basic")
SCORING_HASH = h(canon({"scoring": "synthetic-basic"}))

# Hand-made per-design behaviour: (time to loss at the ramp, J/S at loss, re-acq time).
BEHAVIOUR = {
    "baseline": (21.0, 25.25, 3.5),
    "narrow-pll": (27.0, 26.75, 6.0),
    "wide-pll": (18.0, 24.5, 2.5),
}
FE_SHIFT = {"raw": 0.0, "notch+2bit": -1.5}


def ramp_event(design, fe, sat_id):
    ttl, js, reacq = BEHAVIOUR[design]
    ttl += FE_SHIFT[fe] + 0.25 * (sat_id % 2)
    curve = []
    for k, js_db in enumerate([20.0, 25.0, 30.0]):
        meas = round(45.0 - [0.5, 3.0, None][k], 2) if k < 2 else None
        curve.append({
            "js_db": js_db, "n": [5000, 4000, 0][k],
            "measured_cn0_dbhz": meas,
            "measured_degradation_db": None if meas is None else round(45.0 - meas, 2),
            "modelled_cn0_dbhz": [44.6, 41.9, 37.4][k],
            "modelled_degradation_db": [0.4, 3.1, 7.6][k],
        })
    return {
        "event_id": "ramp-1", "type": "broadband", "lost": True,
        "time_to_loss_s": ttl, "js_at_loss_db": js, "reacquired": True, "reacq_time_s": reacq,
        "availability": round(ttl / 150.0, 4), "pll_jitter_deg": 9.5, "dll_jitter_chips": 0.03,
        "false_lock_episodes": 0, "cn0_curve": curve,
        "modelled": {"label": "MODELLED", "q": 2.0, "chip_rate_hz": 1.023e6,
                     "formula": "(C/N0)eff = [1/(C/N0) + (J/S)/(Q*Rc)]^-1"},
        "pass": {"max_reacq_s": reacq <= 5.0},
    }


def gap_event(design):
    reacq = {"baseline": 1.5, "narrow-pll": 2.25, "wide-pll": 1.0}[design]
    return {
        "event_id": "gap-1", "type": "unknown", "lost": True,
        "time_to_loss_s": 0.5, "js_at_loss_db": None, "reacquired": True, "reacq_time_s": reacq,
        "availability": 0.0, "pll_jitter_deg": None, "dll_jitter_chips": None,
        "false_lock_episodes": 0, "cn0_curve": [], "modelled": None, "pass": {},
    }


def sat(rec, design, fe, sid):
    events = []
    if rec["id"] == "synthetic-rec-a":
        events.append(ramp_event(design, fe, sid))
    elif sid == 6:
        events.append(gap_event(design))
    lost = len(events)
    return {
        "signal": "gps-l1ca", "id": sid,
        "whole_run": {
            "availability": round(1.0 - 0.1 * lost - (0.02 if design == "narrow-pll" else 0.0), 4),
            "locked_s": 270.0 - 30.0 * lost, "phase_lock_frac": 0.97 - 0.1 * lost,
            "code_lock_frac": 0.99 - 0.08 * lost, "baseline_cn0_dbhz": 45.0,
            "pll_jitter_deg": {"baseline": 4.0, "narrow-pll": 3.0, "wide-pll": 5.5}[design],
            "dll_jitter_chips": 0.012, "false_lock_episodes": 0, "false_lock_per_hour": 0.0,
            "loss_count": lost, "reacq_count": lost,
        },
        "events": events,
    }


def main():
    if os.path.isdir(OUT):
        shutil.rmtree(OUT)
    os.makedirs(os.path.join(OUT, "cells"))
    keys, pairs = [], []
    for rec in RECORDINGS:
        ids = rec["conditions"]["expected"][0]["ids"]
        for fe in FRONTENDS:
            for de in DESIGNS:
                key = h(canon({
                    "recording_sha256": rec["sha256"], "conditions_hash": rec["conditions_hash"],
                    "frontend_hash": fe["hash"], "design_hash": de["hash"],
                    "run_hash": RUN_HASH, "scoring_hash": SCORING_HASH, "engine_version": ENGINE,
                }))
                cell = {
                    "schema": "kshana.campaign-cell/1", "key": key, "engine_version": ENGINE,
                    "recording": {"id": rec["id"], "sha256": rec["sha256"],
                                  "conditions_hash": rec["conditions_hash"]},
                    "frontend": {"name": fe["name"], "hash": fe["hash"]},
                    "design": {"name": de["name"], "hash": de["hash"]},
                    "run_hash": RUN_HASH, "scoring_hash": SCORING_HASH,
                    "samples_processed": 300 * 4_092_000,
                    "satellites": [sat(rec, de["name"], fe["name"], i) for i in ids],
                }
                text = pretty(cell)
                with open(os.path.join(OUT, "cells", key + ".json"), "w") as f:
                    f.write(text)
                keys.append(key)
                pairs.append([key, hashlib.sha256(text.encode()).hexdigest()])
    manifest = {
        "schema": "kshana.campaign/1", "name": "synthetic-basic", "engine_version": ENGINE,
        "data_class": "synthetic",
        "recordings": [{k: r[k] for k in ("id", "sha256", "conditions_hash", "conditions")}
                       for r in RECORDINGS],
        "frontends": FRONTENDS, "designs": DESIGNS,
        "run_hash": RUN_HASH, "scoring_hash": SCORING_HASH, "scoring": SCORING, "cells": keys,
    }
    with open(os.path.join(OUT, "campaign.json"), "w") as f:
        f.write(pretty(manifest))
    pairs.sort()
    with open(os.path.join(OUT, "DIGEST"), "w") as f:
        f.write(h(canon(pairs)) + "\n")


if __name__ == "__main__":
    main()
