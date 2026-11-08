#!/usr/bin/env python3
"""Generate the Kshana feature register (features.json + FEATURES.md) FROM THE REPOSITORY.

Every row is enumerated from a file or a command in the engine repository; nothing here is a
list typed from memory. The only hand-kept inputs are:

  * placements.toml  where each feature is shown on the site (page, section, Studio control,
                     docs anchor). A feature with no entry is reported UNPLACED, never defaulted.
  * PUBLIC_EDITION_SOURCES below  the Pro and Custom-study rows. Each carries an exact quote that
                     must still be present in a PUBLIC file of the engine repository (README,
                     docs/, web/, LICENSING.md, GOVERNANCE.md). A quote that no longer matches
                     makes the row UNSOURCED, which the checker fails.
  * descriptions for CLI options (CLI_DESC) because main.rs carries no help text per flag; a
                     flag that exists in main.rs but not in CLI_DESC is still enumerated and gets
                     a "no description" marker the checker reports.

Standard library only (python3 >= 3.11 for tomllib). The engine repository is only READ:
the release binary is run for `kinds --json`, `example` and `--validate`, and git is asked for
HEAD and the status of scenarios/ (read-only commands).

Usage:  python3 gen_features.py [--repo <engine checkout>]   (default: the site's src/data/sources.json: repo)
"""
from __future__ import annotations

import argparse
import datetime as _dt
import glob
import html
import json
import os
import re
import subprocess
import sys
import tomllib

HERE = os.path.dirname(os.path.abspath(__file__))
SITE_SOURCES = os.path.join(HERE, "..", "explorations", "2026-09-27-combo", "src", "data", "sources.json")


def _default_repo() -> str:
    """The engine checkout the site is built from (the site's src/data/sources.json: repo), so the
    register and the site always describe the same engine. KSHANA_REPO overrides it."""
    d = os.environ.get("KSHANA_REPO")
    if not d and os.path.exists(SITE_SOURCES):
        d = json.load(open(SITE_SOURCES, encoding="utf-8")).get("repo")
    return os.path.expanduser(d or "~/Code/kshana")


DEFAULT_REPO = _default_repo()

CATEGORIES = ["kind", "scenario", "cli", "mcp-tool", "binding", "export", "clock", "band",
              "body", "validation", "pro", "study"]
PAGES = ["Home", "Missions", "Capabilities", "Evidence", "Developers", "Editions", "Docs", "Studio"]
EDITIONS = ["Free core", "Pro", "Custom study"]

# ---------------------------------------------------------------------------------------------
# Public edition sources. Each quote must be found (whitespace-, tag- and markdown-insensitive)
# in the named PUBLIC file. Never add a source outside the engine repository.
# A quote the documentation branch carries before main does is found in that checkout of the same
# repository (docs_checkout below) and the row's source names the branch.
# ---------------------------------------------------------------------------------------------
FREE_CORE_SOURCE = {
    # The README's Editions table (web/index.html became the multi-page site's home in 0.29.0).
    "file": "README.md",
    "quote": "scenario kinds, no feature gate",
}
FREE_CORE_SOURCE_2 = {"file": "README.md", "quote": "The open engine is complete and usable on its own."}

PUBLIC_EDITION_SOURCES = [
    # ---- Pro ----
    dict(id="pro:calibrated-clock-twin", category="pro", edition="Pro",
         name="Calibrated clock digital twin",
         description="A digital twin of a clock calibrated to the device's published Allan-deviation budget (the standard measure of clock stability).",
         file="web/index.html",
         quote="clock digital twins calibrated to a device's published Allan budget",
         probes=[("phrase", "clock digital twin"), ("phrase", "calibrated clock twin"), ("phrase", "clock twin")],
         claim=r"clock (digital )?twin"),
    dict(id="pro:ranked-trade-study", category="pro", edition="Pro",
         name="Architecture trade study ranked on a figure of merit",
         description="Candidate positioning, navigation and timing (PNT) architectures compared and ranked on a single figure of merit.",
         file="web/index.html",
         quote="architecture trade studies ranked on a figure of merit",
         probes=[("phrase", "trade studies ranked"), ("phrase", "ranked trade study"), ("phrase", "ranks architecture options"), ("phrase", "ranks the options on a figure of merit")],
         claim=r"trade stud(y|ies)|ranks? architecture|ranked answers"),
    dict(id="pro:evidence-pack", category="pro", edition="Pro",
         name="Audit-grade, reproducible evidence pack",
         description="The evidence a review or procurement board asks for, packaged so every figure can be reproduced.",
         file="web/index.html",
         quote="audit-grade, reproducible evidence packs",
         probes=[("phrase", "evidence pack"), ("phrase", "evidence packs")],
         claim=r"evidence pack|evidence you can hand"),
    dict(id="pro:mbse-programme-tooling", category="pro", edition="Pro",
         name="Model-based systems-engineering and programme tooling",
         description="Model-based systems-engineering (MBSE) and programme tooling that checks a programme's requirements against runs of the open engine and produces a verification cross-reference matrix (VCRM), a Systems Modeling Language version 2 (SysML v2) model, a change-impact report and an offline evidence pack. It reads only what the open engine publishes: each run's result.json and report.json, the scenario file, the field-units schema and the verification matrix's labels.",
         file="README.md",
         quote="Kshana Pro's model-based systems-engineering (MBSE) and programme tooling checks a programme's requirements against runs of the open engine: it imports requirements as comma-separated values (CSV), or as a documented subset of the Requirements Interchange Format (ReqIF) or of Systems Modeling Language version 2 (SysML v2) text, evaluates each acceptance criterion, with its unit, against the values a run wrote, and produces a verification cross-reference matrix (VCRM), a SysML v2 model, a change-impact report and an offline evidence pack. It re-runs and recomputes nothing, and reads only what the open engine publishes: each run's result.json (the fields a criterion names, their units, the reproducibility stamp of scenario hash, seed, engine version and schema version, and the run's label and figure tiers), its report.json (kind, reproducibility record and capability labels), the scenario file (its hash, kind, seed and parameters), the field-units schema (each field's unit, provenance and evidence tier) and the verification matrix (each capability's VALIDATED, MODELLED or PARTNER label).",
         probes=[("phrase", "systems-engineering"), ("phrase", "systems engineering"), ("phrase", "programme tooling")],
         claim=r"systems[- ]engineering|programme tooling"),
    # ---- Pro: the same engine, amplified (public text: docs/PRO.md, found in the public-text
    #      checkout named by the site's src/data/pro-power.json until it is merged; see pro_text_checkout)
    dict(id="pro:strict-superset", category="pro", edition="Pro",
         name="Kshana Pro is a strict superset of open Kshana",
         description="Pro depends on the open engine as a library and never forks it; every open scenario kind runs in Pro unchanged, adds no physical model, and each Pro figure keeps the tier the open engine gave it.",
         file="docs/PRO.md",
         quote="Pro depends on the open engine as a library and never forks it. Every open scenario kind runs in Pro unchanged, and an open scenario runs in Pro with no licence at all.",
         probes=[("phrase", "strict superset"), ("phrase", "same engine, amplified")],
         claim=r"strict superset|same engine, amplified"),
    dict(id="pro:design-optimiser", category="pro", edition="Pro",
         name="Design optimiser: the Pareto front of a design space",
         description="The Pareto front of a stated design space over any open scenario kind, its knee, and each front design written out as a plain scenario file whose result the free engine reproduces.",
         file="docs/PRO.md",
         quote="A Pro user states the design space (each variable a scenario setting with its allowed values), the constraints and two or three objectives, and gets the whole Pareto front: the designs that no other design beats on every objective at once.",
         probes=[("phrase", "design optimiser"), ("phrase", "Pareto front")],
         claim=r"design optimi[sz]er|pareto front"),
    dict(id="pro:uncertainty-sensitivity", category="pro", edition="Pro",
         name="Uncertainty and sensitivity over thousands of runs",
         description="Stated input uncertainties give confidence bands, the probability of meeting a limit and a ranking of the inputs that drive the result (Sobol' indices or standardised regression coefficients); a long study can be stopped and resumed.",
         file="docs/PRO.md",
         quote="A Pro user states how uncertain each input is and gets confidence bands, the probability of meeting a limit, and a ranking of the inputs that drive the result.",
         probes=[("phrase", "uncertainty and sensitivity"), ("phrase", "confidence band"), ("phrase", "confidence bands")],
         claim=r"uncertainty and sensitivity|confidence bands?|probability of meeting a limit"),
    dict(id="pro:mission-dossier", category="pro", edition="Pro",
         name="One-command mission dossier",
         description="Requirements checked against real runs, a verification matrix with its open items, and a PDF (Portable Document Format), HTML (HyperText Markup Language) and JSON (JavaScript Object Notation) dossier in which every number can be re-derived with the free engine.",
         file="docs/PRO.md",
         quote="A Pro user gets one mission dossier in one command: requirements checked against real runs, a verification matrix with its open items, and a PDF in which every number can be re-derived with the free engine.",
         probes=[("phrase", "mission dossier")],
         claim=r"mission dossier"),
    dict(id="pro:scenario-regression-check", category="pro", edition="Pro",
         name="Scenario regression check",
         description="Runs a manifest of scenarios, compares every figure of merit with the previous baseline within a per-figure tolerance, writes an HTML (HyperText Markup Language) report and exits non-zero on a regression.",
         file="docs/PRO.md",
         quote="It writes an HTML report and exits with a non-zero code when a figure regresses, so a continuous-integration pipeline can stop on it.",
         probes=[("phrase", "regression check")],
         claim=r"regression check"),
    dict(id="pro:study-dossier", category="pro", edition="Pro",
         name="Study dossier",
         description="One scenario written up as a reproducible technical note in HTML (HyperText Markup Language) and JSON (JavaScript Object Notation), with a tier on every figure.",
         file="docs/PRO.md",
         quote="One scenario becomes a reproducible technical note in HTML and JSON: scope, method, results with a tier on every figure, limitations, and the exact scenario text run.",
         probes=[("phrase", "study dossier")],
         claim=r"study dossier"),
    dict(id="pro:on-premises-service", category="pro", edition="Pro",
         name="On-premises service",
         description="A small Hypertext Transfer Protocol (HTTP) service on the customer's own network that reports health, version and kinds and runs a posted scenario; without a licence it serves the open engine only.",
         file="docs/PRO.md",
         quote="Pro can run as a small HTTP service on your own network: it reports its health, its version and the kinds it can run, and runs a scenario posted to it.",
         probes=[("phrase", "on-premises service")],
         claim=r"on-premises service"),
    dict(id="pro:campaign-watch", category="pro", edition="Pro",
         name="Campaign watch",
         description="Named figures in a set of scenarios, each with what worse means, gated in a continuous-integration pipeline: Monte Carlo figures judged by statistical tests, every run appended to a hash-chained history, a trend page and a figure-by-figure diff.",
         file="docs/PRO.md",
         quote="A Pro user names the figures that matter in a set of scenarios, says what worse means for each, and gets a pipeline that fails when a change to a scenario, a model or the engine makes one of them worse, or stops meeting its requirement.",
         probes=[("phrase", "campaign watch")],
         claim=r"campaign watch"),
    dict(id="pro:spectrum-coexistence", category="pro", edition="Pro",
         name="Spectrum coexistence",
         description="Candidate signal plans tested against every open-service global navigation satellite system (GNSS) signal the engine models, in both directions, over a worldwide grid, with a verdict per plan and signal against a stated limit (a modelled upper bound, not a regulatory finding).",
         file="docs/PRO.md",
         quote="A Pro user tests one or more candidate signal plans, each a signal design and a constellation, against every open-service GNSS signal the engine models, with every satellite of the plan in view at once, and gets a verdict per plan and signal against a limit the study states.",
         probes=[("phrase", "spectrum coexistence")],
         claim=r"spectrum coexistence"),
    dict(id="pro:on-premises-job-service", category="pro", edition="Pro",
         name="On-premises job service",
         description="A job queue on the team's own network that survives a restart, a stored result per job, a job identifier that is the hash of the request, and a hash-chained delivery ledger with a check.",
         file="docs/PRO.md",
         quote="A Pro team queues studies on its own network, lets them run while no one watches, and can show afterwards exactly which result was delivered for which request.",
         probes=[("phrase", "job service")],
         claim=r"job service"),
    # ---- Custom study (bespoke work) ----
    dict(id="study:modelled-resilience-study", category="study", edition="Custom study",
         name="Modelled PNT-resilience study (for example an optimism-gap audit)",
         description="A bespoke, modelled positioning, navigation and timing (PNT) resilience study, reproducible from scenario, seed and engine version, every figure labelled; not a certification.",
         file="web/index.html",
         quote="PNT-resilience study — e.g. an optimism-gap audit — reproducible from scenario + seed + engine version",
         probes=[("phrase", "optimism-gap audit"), ("phrase", "Request a study")],
         claim=r"optimism-gap audit|request a study|resilience study"),
    dict(id="study:custom-sensor-models", category="study", edition="Custom study",
         name="Custom sensor and resilience models calibrated to your hardware",
         description="Sensor and resilience models calibrated to the customer's own hardware, maintained in a private overlay.",
         file="README.md",
         quote="Custom sensor models — calibrated to your hardware, including export-sensitive resilience models maintained in a private overlay.",
         probes=[("phrase", "calibrated to your hardware")],
         claim=r"calibrated to your hardware|sensor and resilience models"),
    dict(id="study:export-controlled-work", category="study", edition="Custom study",
         name="Export-controlled resilience work under clearance and NDA",
         description="Export-controlled resilience work delivered under the appropriate clearance and non-disclosure agreement (NDA).",
         file="web/index.html",
         quote="export-controlled resilience work delivered under the appropriate clearance and NDA",
         probes=[("phrase", "export-controlled resilience work"), ("phrase", "export-sensitive")],
         claim=r"export[- ](controlled|sensitive)"),
    dict(id="study:support-integration", category="study", edition="Custom study",
         name="Commercial support and integration",
         description="Embedding Kshana in the customer's toolchain, custom scenarios and priority fixes.",
         file="README.md",
         quote="Commercial support & integration — embedding Kshana in your toolchain, custom scenarios, and priority fixes.",
         probes=[("phrase", "integration, training"), ("phrase", "support and integration"), ("phrase", "priority fixes"), ("phrase", "embedding Kshana")],
         claim=r"support (and|&) integration|priority fixes|integration, training"),
    dict(id="study:training-consulting", category="study", edition="Custom study",
         name="Training and consulting",
         description="Training and consulting on quantum and classical positioning, navigation and timing (PNT) performance analysis.",
         file="README.md",
         quote="Training & consulting on quantum/classical PNT performance analysis.",
         probes=[("phrase", "training and consulting"), ("phrase", "consulting on quantum"), ("phrase", "training & consulting")],
         claim=r"training|consulting"),
    # ---- The commercial licence is a licence of the OPEN CORE, not a Pro feature ----
    dict(id="pro:commercial-licence", category="pro", edition="Free core",
         name="Commercial licence of the open core (dual licence, not Pro)",
         description="The same open-core code under a commercial licence for closed or proprietary integration without Affero General Public License (AGPL) obligations. Listed beside Pro because buyers ask for it there, but the public source ties it to the open core.",
         file="web/index.html",
         quote="free under the AGPL-3.0 (or a commercial licence for closed/proprietary use)",
         probes=[("phrase", "commercial licence")],
         claim=r"commercial licen[cs]e|closed integration|proprietary integration"),
]

# Plain-word descriptions of the command-line options (main.rs has no per-flag help text).
CLI_DESC = {
    "scenario version": "Print the scenario schema version this engine reads.",
    "scenario": "State a scenario file's schema version and upgrade older files to it (kshana scenario migrate, kshana scenario version).",
    "obs": "Write a receiver log as a kshana.obs/1 observation file, from a file or a stream as it arrives, and check such a file.",
    "ntrip": "Fetch a correction stream from a caster of the Networked Transport of correction messages over Internet Protocol, unchanged, and count the frames that arrive intact.",
    "device": "Run a device file's test vectors, and decode a binary sensor described by a kshana.device/1 file into JSON Lines (JavaScript Object Notation, one record a line).",
    "planner": "Monitoring-network planner: per site cell, the detection probabilities and the time-difference-of-arrival location bound; choose stations from candidates. Modelled.",
    "station": "Run a Kshana Station: every configured receiver stream recorded as kshana.obs/1 files, rotated, with a status page; check its file; write a service definition for the system's service manager.",
    "connect": "Map any comma- or tab-separated, JavaScript Object Notation or JSON Lines export to a kshana.obs/1 file through a mapping file, with no code; also from a watched folder or a message stream.",
    "obs convert": "Write a receiver log as a kshana.obs/1 observation file.",
    "obs check": "Check a kshana.obs/1 observation file.",
    "connect load-test": "Load-test the mapping path: many simulated stations at a set rate, in memory or through a message broker, with the throughput and memory it reaches.",
    "connect check": "Run Connect conformance vectors (input, mapping or reader, expected kshana.obs/1 output) and fail on any byte difference.",
    "connect check-live": "Replay the Connect conformance vectors over a Message Queuing Telemetry Transport broker (with a forced reconnection), a WebSocket and the folder watch, and check order, holding, reconnection and latency.",
    "connect watch": "Follow a folder of growing or rotating logs and print new kshana.obs/1 records.",
    "connect adapters": "List the readers a mapping file can use.",
    "connect nats": "Subscribe to a subject on a NATS message server and print kshana.obs/1 records.",
    "connect ws": "Read rows streamed over a WebSocket and print kshana.obs/1 records.",
    "connect mqtt": "Subscribe to a topic on a Message Queuing Telemetry Transport broker and print kshana.obs/1 records.",
    "connect map": "Map an export to a kshana.obs/1 file through a mapping file.",
    "scenario migrate": "Upgrade an older scenario file to the current schema version.",
    "jammertest": "Read the published Jammertest test plan as scheduled truth and score a receiver log against it, test by test.",
    "bench": "Score a kshana-bench submission against its truth file and render result files as the static leaderboard.",
    "spec": "Check the LunaNet augmented forward signal implementation against the specification's printed tables and worked examples.",
    "symbols": "Read a stream of hard-decision symbols and check its synchronisation pattern, header and frame spacing.",
    "generate": "Write a test stream of placeholder frames for checking a decoder: symbols only, never a waveform.",
    "--format": "Name the layout of an input file, for example csv or euroc for an IMU log.",
    "--axis": "Choose the accelerometer axis (x, y or z) that an IMU fit reads.",
    "--id": "Name the sensor table that an IMU fit writes.",
    "--expected-mean": "State the specific force the fitted axis reads at rest, in metres per second squared.",
    "run": "Run one scenario file and write its result, chart and report next to it.",
    "--study": "Run a named suite of scenarios together into one stamped, comparable study (study.json and study.html).",
    "--study-name": "Title a single run: stamp study metadata into the result and name the output files after the title.",
    "--validate": "Check a scenario file before running it: the kind resolves and its required fields are present.",
    "--export-sp3": "Write the constellation of an orbit scenario as an SP3-c precise-ephemeris file.",
    "--export-omm": "Write the mean elements of an orbit scenario as a CCSDS OMM (Orbit Mean-elements Message) catalogue.",
    "--export-oem": "Write the propagated states of an orbit scenario as a CCSDS OEM 2.0 (Orbit Ephemeris Message) file.",
    "--eop": "Inline a real IERS finals2000A Earth-orientation file so the ground track uses real UT1 and polar motion.",
    "--animate": "Also write the run's time series as something that moves: an animated SVG (Scalable Vector Graphics) file, a single self-contained HTML player, or numbered SVG frames for a video encoder.",
    "--animate-fps": "Set the playback rate of an --animate export, in frames per second.",
    "--animate-duration": "Set the playback length of an --animate export, in seconds.",
    "--export": "Also write the scenario's geometry, or its synthesised signal, in the formats other tools read: CZML (Cesium Language), KML (Keyhole Markup Language), GeoJSON, the STK (Systems Tool Kit) ephemeris file and SigMF (Signal Metadata Format).",
    "--import-route": "Read a route drawn as a GeoJSON line and use it as the scenario's trajectory.",
    "--help": "Print the command-line usage.",
    "--version": "Print the engine version, to tie a result file to the build that wrote it.",
    "evidence": "Make, check and timestamp a signed evidence pack for a window of a vessel's receiver log, and verify one offline against the signer's public key.",
    "live": "Score a vessel's receiver trust epoch by epoch from a live NMEA stream (standard input, a file, or a network port), optionally gating a downstream consumer on the score.",
    "--out": "Name the output files of a command that writes several: the test-bench export (motion, events, NMEA) or the compliance report.",
    "--epoch": "Give the UTC start time of a test-bench export (YYYY-MM-DDTHH:MM:SS), so its time columns and NMEA sentences carry a real date.",
    "--mapping": "Print the mapping tables from Kshana outputs to the clauses of public frameworks, with the wording rule, without reading any run.",
    "--sources": "Print the public documents each compliance mapping cites, by clause number only.",
    "kinds": "List every built-in scenario kind.",
    "kinds --json": "List every built-in scenario kind with its description and required and optional fields, as JSON.",
    "example": "List the bundled reference scenarios, or print one to a file so a registry install has something to run.",
}

EXPORT_DESC = {
    "result.json": "The complete result document of a run: figures of merit, curves, units and provenance (scenario hash, seed, engine version).",
    "chart.svg": "The chart of a run as a scalable vector graphics (SVG) file.",
    "report.html": "A self-contained HTML report of a run: summary, figures and the exact scenario text.",
    "report.json": "The run report as a machine-readable JSON document, with the same content as report.html: inputs, results, verification labels, what was left out and the reproducibility record.",
    "animation.svg": "The run as one animated SVG (Scalable Vector Graphics) file, with no script: traces draw in behind a moving time cursor. Written by --animate svg.",
    "animation.html": "The run as one self-contained HTML player with play, pause, a scrubber and speed control. Written by --animate html.",
    "frames": "The run as a folder of numbered SVG frames with a manifest, for a video encoder. Written by --animate frames.",
    "table.csv": "The byte-stable reproducibility table (comma-separated values) that the papers cite; only some kinds emit one.",
    "sp3": "SP3-c precise-ephemeris text of an orbit scenario's constellation, the standard GNSS post-processing format.",
    "omm": "CCSDS OMM (Orbit Mean-elements Message) catalogue of an orbit scenario, one message per satellite.",
    "oem": "CCSDS OEM 2.0 (Orbit Ephemeris Message) with position and velocity, read by GMAT, Orekit and STK.",
    "study.json": "The aggregated, comparable result of a study suite run with --study.",
    "study.html": "The HTML report of a study suite run with --study.",
}

# Bands: each row is anchored to a regex that must match in the named engine source file.
BAND_PROBES = [
    ("band:l1-e1", "GPS L1 / Galileo E1 (1575.42 MHz)", "src/jamming.rs", r"pub const L1_HZ: f64 = 1_575_420_000\.0",
     "The L1 carrier used for jamming, tracking loops, measurement simulation, Doppler and positioning; Galileo E1 shares it.",
     [("ident", "L1"), ("phrase", "1575.42")]),
    ("band:l2", "GPS L2 (1227.60 MHz)", "src/ionex.rs", r"pub const GPS_L2_HZ",
     "The L2 carrier, used for the dual-frequency ionosphere delay and positioning.",
     [("ident", "L2"), ("phrase", "1227.6")]),
    ("band:l5-e5a", "GPS L5 / Galileo E5a (1176.45 MHz)", "src/timetransfer_adv.rs", r"F_L5",
     "The wideband L5 carrier for the dual-frequency ionosphere-free satellite-based augmentation system (SBAS) combination and layered-PNT studies.",
     [("ident", "L5"), ("ident", "E5a"), ("phrase", "1176.45")]),
    ("band:lunar-s-afs", "Lunar S-band augmented forward signal (2492.028 MHz)", "src/lunar_service.rs", r"pub const LSIS_AFS_CARRIER_HZ",
     "The S-band carrier of the LunaNet augmented forward signal (AFS), 2492.028 MHz from LSIS-020 (the rounded 2.4 GHz default until 0.30), used for lunar jamming, attack-surface and service-volume analysis.",
     [("phrase", "augmented forward signal"), ("ident", "AFS")]),
    ("band:deep-space-s", "Deep-space radiometric S-band", "src/radiometric.rs", r"Band::S => 2\.115e9",
     "S-band deep-space tracking link for radiometric (range and Doppler) navigation.",
     [("phrase", "S-band"), ("phrase", "S · X · Ka")]),
    ("band:deep-space-x", "Deep-space radiometric X-band", "src/radiometric.rs", r"Band::X => 7\.179e9",
     "X-band deep-space tracking link for radiometric (range and Doppler) navigation.",
     [("phrase", "X-band"), ("phrase", "S · X · Ka")]),
    ("band:deep-space-ka", "Deep-space radiometric Ka-band", "src/radiometric.rs", r"Band::Ka => 34\.4e9",
     "Ka-band deep-space tracking link for radiometric (range and Doppler) navigation.",
     [("phrase", "Ka-band"), ("phrase", "S · X · Ka")]),
    ("band:optical-1550", "Optical 1550 nm link", "src/optical_linkbudget.rs", r"pub const WAVELENGTH_1550_NM_M",
     "The 1550 nanometre optical link for optical positioning and inter-satellite time transfer (optical telecom C-band, not RF C-band).",
     [("phrase", "1550 nm")]),
]

BODY_DESC = {
    "earth": ("Earth", "Earth gravity (J2 to J6 zonal harmonics), rotation and pole; the default central body."),
    "mars": ("Mars", "Mars gravity, rotation and IAU (International Astronomical Union) pole for Mars positioning and deep-space tracking."),
    "moon": ("Moon", "Moon gravity, rotation and pole for lunar orbits, lunar positioning and lunar time."),
    "sun": ("Sun", "Sun gravitational parameter and pole for heliocentric transfers and third-body forces."),
}
# The rest of the solar-system catalogue (src/body.rs): one sentence each, built from the body's
# class and parent, no figures. A body that src/body.rs adds and this table lacks is still
# enumerated and gets the "add a description" marker.
_PLANET = "{0} gravity, radius, rotation and IAU (International Astronomical Union) pole, for the solar-system kind and for positioning around it with the body-pnt kind."
_MOON = "{0}, a moon of {1}: gravity, radius, rotation and IAU (International Astronomical Union) pole, for the solar-system kind and for positioning around it with the body-pnt kind."
for _b in ("mercury", "venus", "jupiter", "saturn", "uranus", "neptune"):
    BODY_DESC[_b] = (_b.capitalize(), _PLANET.format(_b.capitalize()))
BODY_DESC["pluto"] = ("Pluto", "Pluto, a dwarf planet: gravity, radius, rotation and IAU (International Astronomical Union) pole, for the solar-system kind and for positioning around it with the body-pnt kind.")
for _b, _parent in (("phobos", "Mars"), ("deimos", "Mars"), ("io", "Jupiter"), ("europa", "Jupiter"), ("ganymede", "Jupiter"), ("callisto", "Jupiter"), ("titan", "Saturn")):
    BODY_DESC[_b] = (_b.capitalize(), _MOON.format(_b.capitalize(), _parent))

# ---------------------------------------------------------------------------------------------
# helpers
# ---------------------------------------------------------------------------------------------

def rel(repo: str, path: str) -> str:
    return os.path.relpath(path, repo)


def read(path: str) -> str:
    with open(path, encoding="utf-8") as f:
        return f.read()


def find_line(path: str, pattern: str, flags: int = 0) -> int | None:
    rx = re.compile(pattern, flags)
    try:
        with open(path, encoding="utf-8") as f:
            for i, line in enumerate(f, 1):
                if rx.search(line):
                    return i
    except FileNotFoundError:
        return None
    return None


def run(cmd: list[str], cwd: str) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=120)


def normalise_public(text: str, is_html: bool) -> tuple[str, list[int]]:
    """Strip tags / markdown emphasis, unescape entities, collapse whitespace.
    Returns the normalised text plus, per character, the 1-based source line it came from."""
    out_chars: list[str] = []
    out_lines: list[int] = []
    line = 1
    i = 0
    n = len(text)
    in_tag = False
    last_space = True
    while i < n:
        ch = text[i]
        if ch == "\n":
            line += 1
        if is_html:
            if ch == "<":
                in_tag = True
                i += 1
                continue
            if in_tag:
                if ch == ">":
                    in_tag = False
                i += 1
                continue
            if ch == "&":
                m = re.match(r"&(#\d+|#x[0-9a-fA-F]+|[a-zA-Z]+);", text[i:])
                if m:
                    ch = html.unescape(m.group(0))
                    i += len(m.group(0)) - 1
        else:
            if ch in "*`":
                i += 1
                continue
        if ch.isspace() or ch == "\u00a0":
            if not last_space:
                out_chars.append(" ")
                out_lines.append(line)
            last_space = True
        else:
            out_chars.append(ch)
            out_lines.append(line)
            last_space = False
        i += 1
    return "".join(out_chars), out_lines


def locate_quote(repo: str, file: str, quote: str, commit: str = "") -> int | None:
    """The 1-based line of `quote` in `file` of the checkout `repo`: its working tree, or, with
    `commit`, the committed text at that commit (git show), whatever the checkout's HEAD is now."""
    if commit:
        out = run(["git", "show", f"{commit}:{file}"], repo)
        if out.returncode != 0:
            return None
        text = out.stdout
    else:
        path = os.path.join(repo, file)
        if not os.path.exists(path):
            return None
        text = read(path)
    norm, lines = normalise_public(text, file.endswith(".html"))
    q, _ = normalise_public(quote, False)
    idx = norm.find(q)
    if idx < 0:
        # tolerate straight vs typographic quotes
        idx = norm.replace("’", "'").find(q.replace("’", "'"))
    return lines[idx] if idx >= 0 else None


# ---- abbreviation handling -----------------------------------------------------------------

IGNORE_ABBR: set[str] = set()


def load_abbreviations(repo: str, placements: dict) -> dict[str, str]:
    IGNORE_ABBR.update((placements.get("abbreviations_ignore") or {}).get("tokens", []))
    abbr: dict[str, str] = {}
    gpath = os.path.join(repo, "docs/GLOSSARY.md")
    if os.path.exists(gpath):
        for m in re.finditer(r"^\*\*([A-Z][A-Za-z0-9/\-]{1,12}) — ([^*]+?)\.\*\*", read(gpath), re.M):
            abbr[m.group(1)] = m.group(2).strip()
    # every public doc: "Words Spelled Out (WSO)" and "(WSO: words spelled out)" forms
    for k, v in docs_abbreviations(repo).items():
        abbr.setdefault(k, v)
    for k, v in (placements.get("abbreviations") or {}).items():
        abbr[k] = v
    return abbr


def docs_abbreviations(repo: str) -> dict[str, str]:
    """Expansions the public docs spell out, found with `git grep` across docs/ (the whole tree,
    not just a row's own source). A "Words (ABBR)" hit counts only when the initials of the
    words just before the parenthesis spell the abbreviation, so an unrelated parenthesis is
    never read as an expansion."""
    found: dict[str, str] = {}
    p = run(["git", "grep", "-h", "-E", r"\(([A-Z][A-Za-z0-9]{1,7})(\)|: )", "--", "docs"], repo)
    for line in p.stdout.splitlines():
        for m in re.finditer(r"\(([A-Z][A-Z0-9]{1,7}): ([^;)]{3,80})[;)]", line):
            found.setdefault(m.group(1), m.group(2).strip())
        for m in re.finditer(r"((?:[A-Za-z][\w'’/\-]*\s+){1,10})\(([A-Z][A-Z0-9]{1,7})\)", line):
            ab, words = m.group(2), m.group(1).split()
            letters = [ch for ch in ab if ch.isalpha()]
            for n in range(1, len(words) + 1):
                cand = words[-n:]
                parts = [x for w in cand for x in re.split(r"[-/]", w) if x]
                initials = [x[0].upper() for x in parts if x[0].isalpha() and x.lower() not in ("of", "and", "the", "for", "a", "an", "to", "in", "on")]
                if initials == letters:
                    found.setdefault(ab, " ".join(cand))
                    break
    return found


STATUS_WORDS = {"MODELLED", "VALIDATED", "PARTNER", "PASS", "FAIL", "NOT", "AND", "OR"}
EMPHASIS = {"REAL", "RATE", "OWN", "ONLY", "BOTH", "EACH", "FROM", "WITH", "ALSO", "NEVER", "MUST", "THIS", "THAT", "THE", "ONE", "ANY", "KNOWN", "AT"}


def plain(text: str, abbr: dict[str, str], limit: int = 240) -> str:
    t = re.sub(r"[`*_]{1,2}(?=\S)|(?<=\S)[`*_]{1,2}", "", text)
    t = t.replace("→", " to ").replace("↔", " and ").replace("&", "and").replace("≈", "about ")
    t = re.sub(r"\bvs\.?(?=\s)", "versus", t)
    t = re.sub(r"\be\.g\.", "for example", t)
    t = re.sub(r"\bi\.e\.", "that is", t)
    t = re.sub(r"\s+", " ", t).strip()
    for w in STATUS_WORDS:
        t = re.sub(rf"\b{w}\b(?=\s+[a-z])", w.capitalize(), t)
    # all-caps emphasis words (SIMULATED, CAMPAIGN, ...) are not abbreviations: lower-case them
    t = re.sub(r"\b(OWN|THE|ONE|ANY|AT)\b(?=\s+[a-z])", lambda m: m.group(1).lower(), t)
    t = re.sub(r"\b([A-Z]{4,})\b", lambda m: m.group(1) if m.group(1) in abbr else (m.group(1).lower() if m.group(1) in EMPHASIS or len(m.group(1)) >= 6 else m.group(1)), t)
    # first sentence
    m = re.search(r"(?<=[a-z0-9\)\]])\. (?=[A-Z(])", t)
    if m and m.start() > 40:
        t = t[: m.start() + 1]
    if len(t) > limit:
        cut = t.rfind(" ", 0, limit)
        t = t[: cut if cut > 60 else limit].rstrip(",;:—- ") + "…"
    # expand the first use of each known abbreviation, longest first
    slots: list[str] = []
    for a in sorted(abbr, key=len, reverse=True):
        exp = abbr[a]
        rx = re.compile(rf"(?<![A-Za-z0-9\-(]){re.escape(a)}(?![A-Za-z0-9\-])")
        m = rx.search(t)
        if not m:
            continue
        after = t[m.end(): m.end() + 2]
        if after.startswith(" (") or exp.lower() in t.lower():
            continue
        # insert behind a placeholder so a later, shorter abbreviation (PNT) is never expanded
        # inside this expansion ("Resilient PNT Conformance Framework")
        slots.append(f"{exp} ({a})")
        t = t[: m.start()] + f"\x00{len(slots) - 1}\x00" + t[m.end():]
    return re.sub(r"\x00(\d+)\x00", lambda m: slots[int(m.group(1))], t)


def unexpanded(text: str, abbr: dict[str, str]) -> list[str]:
    found = []
    for m in re.finditer(r"(?<![A-Za-z0-9\-])([A-Z][A-Z0-9]{1,}s?)(?![A-Za-z0-9\-])", text):
        tok = m.group(1)
        if tok in STATUS_WORDS or tok in EMPHASIS or tok in abbr or tok.rstrip("s") in abbr or tok in IGNORE_ABBR or re.fullmatch(r"[A-Z]\d{1,2}", tok):
            continue
        # already expanded in the "Expansion (ABBR)" form
        if text[m.end(): m.end() + 1] == ")" and text[max(0, m.start() - 1)] == "(":
            continue
        found.append(tok)
    return sorted(set(found))


# ---------------------------------------------------------------------------------------------
# enumerators
# ---------------------------------------------------------------------------------------------

def feature(**kw) -> dict:
    base = dict(id="", category="", subtype="", name="", description="", source="", edition="Free core",
                edition_source="", identifiers=[], probes=[], claim="", facts={})
    base.update(kw)
    return base


def enum_kinds(repo: str, binary: str, abbr) -> tuple[list[dict], list[str]]:
    warnings: list[str] = []
    p = run([binary, "kinds", "--json"], repo)
    if p.returncode != 0:
        sys.exit(f"error: `kshana kinds --json` failed: {p.stderr.strip()}")
    kinds = json.loads(p.stdout)
    api = os.path.join(repo, "src/api.rs")
    api_names = set(re.findall(r'ScenarioMeta \{ name: "([a-z0-9-]+)"', read(api)))
    bin_names = {k["name"] for k in kinds}
    if api_names != bin_names:
        warnings.append(f"release binary and src/api.rs disagree on kinds: only-binary={sorted(bin_names - api_names)} only-source={sorted(api_names - bin_names)} (binary may be stale)")
    rows = []
    for k in kinds:
        name = k["name"]
        ln = find_line(api, rf'ScenarioMeta \{{ name: "{re.escape(name)}"')
        rows.append(feature(
            id=f"kind:{name}", category="kind", subtype="scenario kind", name=name,
            description=plain(k["description"], abbr),
            source=f"src/api.rs:{ln} · kshana kinds --json" if ln else "kshana kinds --json",
            identifiers=[name], probes=[["ident", name]],
            facts={"required_fields": k.get("required_fields", []), "optional_fields": k.get("optional_fields", [])},
        ))
    return rows, warnings


def scenario_header(text: str) -> str:
    lines = []
    for line in text.splitlines():
        if not line.startswith("#"):
            break
        s = line.lstrip("#").strip()
        if not s:
            if lines:
                break
            continue
        lines.append(s)
    h = " ".join(lines)
    h = re.sub(r"^Kshana reference scenario:\s*", "", h)
    return h


def enum_scenarios(repo: str, binary: str, abbr) -> tuple[list[dict], list[str]]:
    warnings = []
    p = run([binary, "example"], repo)
    bundled = set()
    for line in p.stdout.splitlines():
        s = line.strip()
        if re.fullmatch(r"[a-z0-9][a-z0-9.\-]*", s):
            bundled.add(s)
    st = run(["git", "status", "--porcelain", "--", "scenarios"], repo).stdout
    untracked = {os.path.basename(l[3:]).removesuffix(".toml") for l in st.splitlines() if l.startswith("??")}
    modified = {os.path.basename(l[3:]).removesuffix(".toml") for l in st.splitlines() if l[:2].strip() == "M"}
    rows = []
    for path in sorted(glob.glob(os.path.join(repo, "scenarios", "*.toml"))):
        sid = os.path.basename(path)[:-5]
        text = read(path)
        is_suite = bool(re.search(r"^scenarios\s*=\s*\[", text, re.M))
        if is_suite:
            kind = "study-suite"
        else:
            v = run([binary, "--validate", path], repo)
            m = re.match(r"ok: ([a-z0-9-]+)", v.stdout) or re.match(r"([a-z0-9-]+):", v.stderr)
            kind = m.group(1) if m else "unknown"
            if v.returncode != 0:
                warnings.append(f"scenario {sid}: --validate failed: {v.stderr.strip().splitlines()[0] if v.stderr.strip() else '?'}")
        header = scenario_header(text) or f"{kind} scenario"
        status = "uncommitted (new)" if sid in untracked else ("modified" if sid in modified else "committed")
        rows.append(feature(
            id=f"scenario:{sid}", category="scenario", subtype="study suite" if is_suite else "bundled scenario",
            name=sid, description=plain(header, abbr),
            source=f"scenarios/{sid}.toml:1",
            identifiers=[sid, f"{sid}.toml"], probes=[["ident", sid]],
            facts={"kind": kind, "bundled": sid in bundled, "git": status},
        ))
    return rows, warnings


def enum_cli(repo: str, abbr) -> tuple[list[dict], list[str]]:
    warnings = []
    main = os.path.join(repo, "src/main.rs")
    src = read(main)
    rows = []
    usage_ln = find_line(main, r'usage: kshana <scenario\.toml>')
    rows.append(feature(id="cli:run", category="cli", subtype="command", name="kshana <scenario.toml>",
                        description=CLI_DESC["run"], source=f"src/main.rs:{usage_ln}",
                        identifiers=["kshana <scenario.toml>"],
                        probes=[["phrase", "kshana <scenario>.toml"], ["phrase", "kshana <scenario.toml>"], ["regex", r"kshana\s+(scenarios/)?[a-z0-9-]+\.toml"]]))
    seen_sub = set()
    # A word matched inside run_<parent>_cli (or run_<parent>_load_test) is a subcommand of the
    # top-level command <parent> (0.40+: `kshana obs convert`, `kshana connect watch`, `kshana
    # scenario migrate`), so it is registered as `kshana <parent> <word>`, not as a command of its
    # own. The words with a hand-made probe below (live, spec, symbols, generate) keep their rows.
    fns = [(f.start(), f.group(1)) for f in re.finditer(r"^(?:pub\s+)?fn\s+([a-z_0-9]+)", src, re.M)]
    top = {w.group(1) for w in re.finditer(r'Some\("([a-z]+)"\)', src)
           if [n for p, n in fns if p < w.start()][-1:] == ["main_in_language"]}
    def enclosing(pos):
        enc = [n for p, n in fns if p < pos][-1:]
        return enc[0] if enc else ""
    def parent_of(pos):
        pm = re.match(r"^run_([a-z_]+?)_cli$", enclosing(pos))
        par = pm.group(1).replace("_", "-") if pm else None
        return par if par in top else None
    for m in re.finditer(r'Some\("([a-z][a-z-]*)"\)', src):
        sub = m.group(1)
        fn_name = enclosing(m.start())
        par = None if sub in ("live", "spec", "symbols", "generate") else parent_of(m.start())
        # A word in a helper of a subcommand (run_connect_load_test's `--via memory`) is an option
        # value, not a command; a hyphenated word is a command only inside a subcommand parser.
        if not par and (("-" in sub) or (fn_name.startswith("run_") and not fn_name.endswith("_cli") and fn_name != "run_lunanet_afs_cli")):
            pm = re.match(r"^run_([a-z_]+?)_", fn_name)
            if fn_name.startswith("run_") and pm and pm.group(1).replace("_", "-") in top:
                continue
            if "-" in sub:
                continue
        if par:
            key = f"{par} {sub}"
            if key in seen_sub:
                continue
            seen_sub.add(key)
            ln = src[: m.start()].count("\n") + 1
            rows.append(feature(id=f"cli:{par}-{sub}", category="cli", subtype="subcommand", name=f"kshana {key}",
                                description=CLI_DESC.get(key, "NO DESCRIPTION: add one to CLI_DESC"),
                                source=f"src/main.rs:{ln}", identifiers=[f"kshana {key}"],
                                probes=[["regex", rf"kshana\s+{par}\s+{sub}\b"], ["regex", rf"(?<![\w-]){par}\s+{sub}\b"]]))
            continue
        if sub in seen_sub:  # a word matched in two places (0.35: `evidence` is also a `receiver-trust` action)
            continue
        seen_sub.add(sub)
        ln = src[: m.start()].count("\n") + 1
        rows.append(feature(id=f"cli:{sub}", category="cli", subtype="subcommand", name=f"kshana {sub}",
                            description=CLI_DESC.get(sub, "NO DESCRIPTION: add one to CLI_DESC"),
                            source=f"src/main.rs:{ln}", identifiers=[f"kshana {sub}"],
                            probes=[["regex", rf"(?:kshana\s+(?:receiver-trust\s+)?|receiver-trust\s+){sub}\b" if sub == "live" else (rf"kshana\s+(?:lunanet-afs-check\s+)?{sub}\b" if sub in ("spec", "symbols", "generate") else rf"kshana\s+{sub}\b")]]))
        if sub == "kinds" and re.search(r'flag == "--json"', src):
            ln2 = find_line(main, r'flag == "--json"')
            rows.append(feature(id="cli:kinds--json", category="cli", subtype="subcommand", name="kshana kinds --json",
                                description=CLI_DESC["kinds --json"], source=f"src/main.rs:{ln2}",
                                identifiers=["kshana kinds --json"], probes=[["regex", r"kshana\s+kinds\s+--json"]]))
    seen = set()
    for m in re.finditer(r'^\s*"(--[a-z0-9-]+)"(?:\s*\|\s*"[^"]+")*\s*=>', src, re.M):
        flag = m.group(1)
        if flag in seen:
            continue
        seen.add(flag)
        ln = src[: m.start()].count("\n") + 1
        desc = CLI_DESC.get(flag)
        if desc is None:
            warnings.append(f"cli flag {flag} (src/main.rs:{ln}) has no description in CLI_DESC")
            desc = "NO DESCRIPTION: add one to CLI_DESC"
        rows.append(feature(id=f"cli:{flag}", category="cli", subtype="option", name=flag, description=desc,
                            source=f"src/main.rs:{ln}", identifiers=[flag], probes=[["ident", flag]]))
    for k in CLI_DESC:
        if k.startswith("--") and k not in seen:
            warnings.append(f"CLI_DESC names {k} but src/main.rs has no such option (removed?)")
    return rows, warnings


def _mcp_repo(repo: str) -> str:
    """The checkout the MCP server's tool list is read from: the site's src/data/sources.json
    mcp_repo while the branch that adds tools is not merged into `repo` (the same rule build.py
    uses), else `repo`. KSHANA_MCP_REPO overrides it."""
    d = os.environ.get("KSHANA_MCP_REPO")
    if not d and os.path.exists(SITE_SOURCES):
        d = json.load(open(SITE_SOURCES, encoding="utf-8")).get("mcp_repo")
    d = os.path.expanduser(d) if d else repo
    return d if os.path.exists(os.path.join(d, "mcp/kshana-mcp/src/server.rs")) else repo


def enum_mcp(repo: str, abbr) -> list[dict]:
    # The tool functions live in server.rs, in iq.rs (since 0.33.0) and in marine.rs (since 0.35.0);
    # server.rs first, then any other source file of the crate, so a new module is not missed.
    rows = []
    srcdir = os.path.join(_mcp_repo(repo), "mcp/kshana-mcp/src")
    others = sorted(f for f in os.listdir(srcdir) if f.endswith(".rs") and f != "server.rs") if os.path.isdir(srcdir) else []
    for fname in ["server.rs"] + others:
        path = os.path.join(_mcp_repo(repo), "mcp/kshana-mcp/src", fname)
        if not os.path.exists(path):
            continue
        src = read(path)
        for m in re.finditer(r'#\[tool\(\s*description\s*=\s*"((?:[^"\\]|\\.)*)"\s*\)\]\s*fn\s+([a-z_0-9]+)', src, re.S):
            desc, name = m.group(1), m.group(2)
            ln = src[: m.end()].count("\n") + 1
            rows.append(feature(id=f"mcp-tool:{name}", category="mcp-tool", subtype="MCP tool", name=name,
                                description=plain(desc.replace('\\"', '"'), abbr),
                                source=f"mcp/kshana-mcp/src/{fname}:{ln}", identifiers=[name],
                                probes=[["ident", name]]))
    return rows


def enum_bindings(repo: str, abbr) -> list[dict]:
    rows = []
    cargo = os.path.join(repo, "Cargo.toml")
    ver = tomllib.load(open(cargo, "rb"))["package"]["version"]
    rows.append(feature(id="binding:crate-kshana", category="binding", subtype="channel", name="Rust crate and command line (crates.io: kshana)",
                        description="The engine as a Rust library and the kshana command, installed with cargo.",
                        source=f"Cargo.toml:{find_line(cargo, r'^name = \"kshana\"')}", identifiers=["cargo install kshana", "cargo add kshana"],
                        probes=[["phrase", "cargo install kshana"], ["phrase", "cargo add kshana"]], facts={"version": ver}))
    mcargo = os.path.join(repo, "mcp/kshana-mcp/Cargo.toml")
    if os.path.exists(mcargo):
        rows.append(feature(id="binding:crate-kshana-mcp", category="binding", subtype="channel", name="MCP server crate (crates.io: kshana-mcp)",
                            description="The Model Context Protocol (MCP) server that lets an AI assistant run Kshana, installed with cargo.",
                            source=f"mcp/kshana-mcp/Cargo.toml:{find_line(mcargo, r'^name')}", identifiers=["kshana-mcp", "cargo install kshana-mcp"],
                            probes=[["ident", "kshana-mcp"]]))
    sj = os.path.join(repo, "mcp/kshana-mcp/server.json")
    if os.path.exists(sj):
        d = json.load(open(sj))
        rows.append(feature(id="binding:mcp-registry", category="binding", subtype="channel", name=f"Official MCP registry entry ({d['name']})",
                            description="The server listed in the official Model Context Protocol (MCP) registry so assistants can discover it.",
                            source=f"mcp/kshana-mcp/server.json:{find_line(sj, r'\"name\"')}", identifiers=[d["name"]],
                            probes=[["phrase", d["name"]]]))
        for pkg in d.get("packages", []):
            kind = pkg.get("registryType")
            ident = pkg["identifier"].split(":")[0]
            ln = find_line(sj, r'"identifier": "' + re.escape(pkg["identifier"].split(":")[0]))
            if kind == "oci":
                rows.append(feature(id="binding:ghcr-image", category="binding", subtype="channel", name=f"Container image ({ident})",
                                    description="The MCP server as a multi-architecture container image, no Rust toolchain needed.",
                                    source=f"mcp/kshana-mcp/server.json:{ln}", identifiers=[ident], probes=[["phrase", ident]]))
            elif kind == "npm":  # 0.35.0: the launcher packages the registry lists beside the image
                rows.append(feature(id="binding:mcp-npx", category="binding", subtype="channel", name=f"npx launcher (npm: {ident})",
                                    description="The MCP server started with npx, which fetches the verified prebuilt binary for the system; no Rust toolchain needed.",
                                    source=f"mcp/kshana-mcp/server.json:{ln}", identifiers=[f"npx -y {ident}"], probes=[["phrase", f"npx -y {ident}"]]))
            elif kind == "pypi":
                rows.append(feature(id="binding:mcp-uvx", category="binding", subtype="channel", name=f"uvx launcher (PyPI: {ident})",
                                    description="The MCP server started with uvx or pipx, which fetches the verified prebuilt binary for the system; no Rust toolchain needed.",
                                    source=f"mcp/kshana-mcp/server.json:{ln}", identifiers=[f"uvx {ident}"], probes=[["phrase", f"uvx {ident}"]]))
    py = os.path.join(repo, "pyproject.toml")
    rows.append(feature(id="binding:pypi", category="binding", subtype="channel", name="Python package (PyPI: kshana)",
                        description="The engine as a Python module, installed with pip.",
                        source=f"pyproject.toml:{find_line(py, r'^name = ')}", identifiers=["pip install kshana"],
                        probes=[["phrase", "pip install kshana"]]))
    npmj = os.path.join(repo, "web/pkg/package.json")
    rows.append(feature(id="binding:npm", category="binding", subtype="channel", name="JavaScript and WebAssembly package (npm: kshana)",
                        description="The engine compiled to WebAssembly for browsers and Node.js, installed with npm.",
                        source=f"src/wasm.rs:1 · web/pkg/package.json:{find_line(npmj, r'\"name\"') or 1}", identifiers=["npm install kshana"],
                        probes=[["phrase", "npm install kshana"]]))
    rows.append(feature(id="binding:browser-studio", category="binding", subtype="channel", name="In-browser engine (Kshana Studio)",
                        description="The WebAssembly engine running locally in the browser; nothing is installed or uploaded.",
                        source="web/engine.mjs:1 · src/wasm.rs:1", identifiers=[],
                        probes=[["phrase", "WebAssembly"], ["phrase", "in the browser"]]))
    px = glob.glob(os.path.join(repo, "ide/jetbrains/src/**/plugin.xml"), recursive=True)
    if px:
        pxr = rel(repo, px[0])
        rows.append(feature(id="binding:jetbrains", category="binding", subtype="channel", name="JetBrains IDE plugin (Kshana - PNT simulator)",
                            description="Right-click a scenario file in any JetBrains IDE, choose Run Kshana Scenario, read the figures in a tool window.",
                            source=f"{pxr}:{find_line(px[0], r'<name>')}", identifiers=["Run Kshana Scenario"],
                            probes=[["phrase", "Run Kshana Scenario"], ["phrase", "JetBrains"]]))
    # API surfaces, function by function
    pyi = os.path.join(repo, "kshana.pyi")
    for m in re.finditer(r"^(def|class) ([A-Za-z_][A-Za-z0-9_]*)", read(pyi), re.M):
        name = m.group(2)
        ln = find_line(pyi, rf"^{m.group(1)} {name}\b")
        rows.append(feature(id=f"binding:python.{name}", category="binding", subtype="Python function" if m.group(1) == "def" else "Python class",
                            name=f"kshana.{name}", description=_api_desc(name, "Python"), source=f"kshana.pyi:{ln}",
                            identifiers=[name, f"kshana.{name}"], probes=[["ident", f"kshana.{name}"], ["ident", name]]))
    wasm = os.path.join(repo, "src/wasm.rs")
    wsrc = read(wasm)
    for m in re.finditer(r"^pub fn ([a-z_0-9]+)\(", wsrc, re.M):
        name = m.group(1)
        ln = wsrc[: m.start()].count("\n") + 1
        rows.append(feature(id=f"binding:wasm.{name}", category="binding", subtype="JavaScript/WebAssembly function", name=name,
                            description=_api_desc(name, "JavaScript"), source=f"src/wasm.rs:{ln}",
                            identifiers=[name], probes=[["ident", name]]))
    api = os.path.join(repo, "src/api.rs")
    asrc = read(api)
    for m in re.finditer(r"^pub fn ([a-z_0-9]+)\(", asrc, re.M):
        name = m.group(1)
        ln = asrc[: m.start()].count("\n") + 1
        rows.append(feature(id=f"binding:rust.{name}", category="binding", subtype="Rust API function", name=f"kshana::api::{name}",
                            description=_api_desc(name, "Rust"), source=f"src/api.rs:{ln}",
                            identifiers=[name, f"kshana::api::{name}", f"api::{name}"],
                            probes=[["ident", f"kshana::api::{name}"], ["ident", f"api::{name}"], ["ident", name]]))
    return rows


API_WORDS = {
    "env_baseline": "Compute the environmental baseline of a list of site-hours, the same bytes as the command line.",
    "planner": "Run the monitoring-network planner (map or place) on a kshana.planner/1 document, the same bytes as the command line.",
    "area_trust_timeline": "One trust state for an area from several reference stations: the timeline of a kshana.area-stations/1 document, the same bytes as the command line.",
    "referee_location_score": "Score location reports against the referee's location truth file, the same bytes as the command line.",
    "connect_map": "Apply a kshana.connect/1 mapping to a log's text for a station and return the kshana.obs/1 records, in memory.",
    "run": "Run a scenario and return the result JSON.",
    "run_full": "Run a scenario and return the result JSON, the summary and the chart together.",
    "run_typed": "Run a scenario and return a typed result object.",
    "run_all": "Run a scenario and return result, chart, summary and reproducibility table in one call.",
    "run_toml": "Run a scenario from its TOML text and return result, chart, summary and table.",
    "run_scenario": "Run a scenario from its TOML text, returning a typed error on failure.",
    "RunOutput": "The typed result of a run: result JSON, summary, chart and table.",
    "scenario_kinds": "List every scenario kind with its fields, as Python objects.",
    "list_kinds": "List every scenario kind with its fields, as JSON text.",
    "list_scenario_kinds": "List every scenario kind with its fields.",
    "list_scenario_kinds_json": "List every scenario kind with its fields, as JSON text.",
    "validate_toml": "Check a scenario before running it and return the problems found.",
    "validate_scenario": "Check a scenario before running it and return the problems found.",
    "error_kind": "Classify why a scenario failed, so a caller can react to the kind of error.",
    "version": "Return the engine version.",
    "chart_svg": "Return the chart of a run as scalable vector graphics (SVG).",
    "summary": "Return the plain-text summary of a run.",
    "table_csv": "Return the reproducibility table of a run as comma-separated values (CSV), when the kind has one.",
    "encode_permalink": "Pack a scenario into a compact share-link token.",
    "decode_permalink": "Unpack a share-link token back into the scenario text.",
    "export_sp3": "Export an orbit scenario as SP3-c precise-ephemeris text.",
    "export_omm": "Export an orbit scenario as a CCSDS OMM (Orbit Mean-elements Message) catalogue.",
    "export_oem": "Export an orbit scenario as CCSDS OEM 2.0 (Orbit Ephemeris Message) text.",
    "auto_export_sp3": "Export SP3 automatically when the scenario asks for it.",
    "auto_export_omm": "Export OMM automatically when the scenario asks for it.",
    "auto_export_oem": "Export OEM automatically when the scenario asks for it.",
    "inject_eop": "Inline a real IERS finals2000A Earth-orientation file into a scenario.",
    "with_study_meta": "Stamp study metadata (title and generation time) into a result.",
    "scenarios_reference_md": "Generate the scenario-kind reference document as Markdown.",
    "animate_toml": "Render a scenario's run as an animation (animated SVG, HTML player or frames) and return it.",
    "without_figure_tiers": "Return a result with the figure-tier labels removed, for byte comparisons.",
}


def _api_desc(name: str, lang: str) -> str:
    return API_WORDS.get(name, f"NO DESCRIPTION: {lang} API `{name}` (add it to API_WORDS)")


def enum_exports(repo: str) -> list[dict]:
    main = os.path.join(repo, "src/main.rs")
    src = read(main)
    rows = []
    for m in re.finditer(r'with_extension\("([a-z0-9.]+)"\)', src):
        ext = m.group(1)
        if not ext or f"export:{ext}" in {r["id"] for r in rows}:
            continue
        ln = src[: m.start()].count("\n") + 1
        idents = [ext, f".{ext}"]
        probes = [["ident", ext]]
        claim = ""
        if ext == "result.json":
            idents += ["scenario_hash", "engine_version", "schema_version"]
            claim = r"reproducibility bundle|every figure traces to scenario, seed and engine version"
        rows.append(feature(id=f"export:{ext}", category="export", subtype="file written by the CLI", name=ext if "." in ext else f".{ext}",
                            description=EXPORT_DESC.get(ext, "NO DESCRIPTION: add it to EXPORT_DESC"), source=f"src/main.rs:{ln}",
                            identifiers=idents, claim=claim,
                            probes=probes if "." in ext else [["regex", rf"(?<![A-Za-z0-9_])\.?(?:{ext}|{ext.upper()})(?![A-Za-z0-9_])"]]))
    kif = os.path.join(repo, "src/interchange.rs")
    rows.append(feature(id="export:kif", category="export", subtype="interchange envelope", name="Kshana Interchange Format (KIF) envelope",
                        description="A neutral, versioned, self-describing envelope that wraps any artifact (scenario, result, trade study) for exchange.",
                        source=f"src/interchange.rs:{find_line(kif, r'Kshana Interchange Format')}", identifiers=["KIF"],
                        probes=[["ident", "KIF"], ["phrase", "Kshana Interchange Format"]]))
    tdm = os.path.join(repo, "src/ccsds_tdm.rs")
    rows.append(feature(id="export:ccsds-tdm", category="export", subtype="library format (no CLI flag)", name="CCSDS Tracking Data Message (TDM)",
                        description="Parser and writer for the CCSDS Tracking Data Message (TDM), the standard format for raw radiometric tracking measurements; library use only.",
                        source=f"src/ccsds_tdm.rs:{find_line(tdm, r'Tracking Data Message')}", identifiers=["TDM"],
                        probes=[["ident", "TDM"], ["phrase", "Tracking Data Message"]]))
    wasm = os.path.join(repo, "src/wasm.rs")
    rows.append(feature(id="export:permalink", category="export", subtype="share token", name="Scenario share link (permalink token)",
                        description="The whole scenario packed into a compact token, so a link reproduces the run.",
                        source=f"src/wasm.rs:{find_line(wasm, r'pub fn encode_permalink')}", identifiers=["permalink"],
                        probes=[["phrase", "share link"], ["phrase", "permalink"]]))
    return rows


def enum_clocks(repo: str) -> list[dict]:
    rows = []
    cs = os.path.join(repo, "src/clock_state.rs")
    src = read(cs)
    m = re.search(r"pub fn id\(self\) -> &'static str \{\s*match self \{(.*?)\}", src, re.S)
    for mm in re.finditer(r'ClockClass::(\w+) => "([a-z]+)"', m.group(1) if m else ""):
        var, cid = mm.group(1), mm.group(2)
        doc = re.search(rf"/// \*\*([A-Z]+)\*\* — ([^:\n]+?)[:,]", src[: src.find(f"    {var},")][-900:])
        abbrv = doc.group(1) if doc else cid.upper()
        long = doc.group(2).strip() if doc else ""
        long = re.sub(r"^an? ", "", long)
        rows.append(feature(id=f"clock:class:{cid}", category="clock", subtype="clock class", name=f"{abbrv} ({long})" if long else abbrv,
                            description=f"Clock class {abbrv}: {long}; a default stability for holdover, deep-space and slot-timing budgets.",
                            source=f"src/clock_state.rs:{find_line(cs, rf'ClockClass::{var} => \"{cid}\"')}",
                            identifiers=[cid, abbrv], probes=[["ident_cs", abbrv], ["ident", cid]]))
    tt = os.path.join(repo, "src/telecom_timing.rs")
    tsrc = read(tt)
    pre = tsrc[tsrc.find("pub const PRESETS"):]
    pre = pre[: pre.find("\n];")]
    for mm in re.finditer(r'id: "([a-z]+)",\s*class: "([^"]+)",\s*model: "([^"]+)"', pre):
        pid, cls, model = mm.groups()
        part = re.search(r"(OX-208|8040C|5071A|SA\.45s)", model)
        rows.append(feature(id=f"clock:preset:{pid}", category="clock", subtype="oscillator preset", name=f"{pid} preset: {model}",
                            description=f"Holdover oscillator preset, {cls}, carrying the stability, aging and temperature figures of the {model} datasheet.",
                            source=f"src/telecom_timing.rs:{find_line(tt, rf'^        id: \"{pid}\",')}",
                            identifiers=[pid], probes=[["phrase", part.group(1)] if part else ["ident", pid], ["ident", pid]]))
    masks = tsrc[tsrc.find("pub const MASKS"):]
    masks = masks[: masks.find("\n];")]
    for mm in re.finditer(r'id: "([a-z0-9-]+)",\s*title: "([^"]+)"', masks):
        mid, title = mm.groups()
        ab = re.search(r"\(([^)]+)\)", title)
        probes = [["ident", mid], ["phrase", title]]
        if ab:
            probes.append(["phrase", ab.group(1)])
        rows.append(feature(id=f"clock:mask:{mid}", category="clock", subtype="ITU-T timing mask", name=title,
                            description=f"International Telecommunication Union (ITU-T) timing mask: {title}; checked with a pass or fail and a margin.",
                            source=f"src/telecom_timing.rs:{find_line(tt, rf'^        id: \"{mid}\",')}",
                            identifiers=[mid], probes=probes))
    sp = os.path.join(repo, "src/clock_specs.rs")
    ssrc = read(sp)
    for mm in re.finditer(r'LunarClock::(\w+) => "([a-z-]+)"', ssrc):
        var, lid = mm.groups()
        rows.append(feature(id=f"clock:lunar:{lid}", category="clock", subtype="lunar clock class", name=lid,
                            description=f"On-board lunar timing clock class `{lid}` used in the lunar time budget.",
                            source=f"src/clock_specs.rs:{find_line(sp, rf'LunarClock::{var} => \"{lid}\"')}",
                            identifiers=[lid], probes=[["ident", lid]]))
    # clock models defined by bundled scenarios ([clock], [clock_quantum], [clock_classical])
    models: dict[str, list[str]] = {}
    for path in sorted(glob.glob(os.path.join(repo, "scenarios", "*.toml"))):
        try:
            d = tomllib.load(open(path, "rb"))
        except Exception:
            continue
        sid = os.path.basename(path)[:-5]

        def walk(x, key):
            if isinstance(x, dict):
                if key.split(".")[-1] in ("clock", "clock_quantum", "clock_classical") and isinstance(x.get("id"), str):
                    models.setdefault(x["id"], []).append(sid)
                for k, v in x.items():
                    walk(v, f"{key}.{k}" if key else k)
            elif isinstance(x, list):
                for v in x:
                    walk(v, key)
        walk(d, "")
    for mid, sids in sorted(models.items()):
        first = sorted(sids)[0]
        rows.append(feature(id=f"clock:model:{mid}", category="clock", subtype="scenario clock model", name=mid,
                            description=f"Clock model `{mid}` defined in {len(sids)} bundled scenario(s), for example {first}.",
                            source=f"scenarios/{first}.toml:{find_line(os.path.join(repo, 'scenarios', first + '.toml'), rf'^id = \"{re.escape(mid)}\"')}",
                            identifiers=[mid], probes=[["ident", mid]], facts={"scenarios": sorted(set(sids))}))
    return rows


def enum_bands(repo: str) -> tuple[list[dict], list[str]]:
    rows, warnings = [], []
    for bid, name, file, rx, desc, probes in BAND_PROBES:
        ln = find_line(os.path.join(repo, file), rx)
        if ln is None:
            warnings.append(f"band {bid}: anchor /{rx}/ not found in {file}; the band may no longer be modelled")
            continue
        rows.append(feature(id=bid, category="band", subtype="signal band", name=name, description=desc,
                            source=f"{file}:{ln}", identifiers=[], probes=[list(p) for p in probes]))
    return rows, warnings


def enum_bodies(repo: str) -> list[dict]:
    path = os.path.join(repo, "src/body.rs")
    rows = []
    for m in re.finditer(r"^    pub fn ([a-z]+)\(\) -> Self", read(path), re.M):
        b = m.group(1)
        nm, desc = BODY_DESC.get(b, (b.capitalize(), f"Central body `{b}` (add a description to BODY_DESC)."))
        rows.append(feature(id=f"body:{b}", category="body", subtype="central body", name=nm, description=desc,
                            source=f"src/body.rs:{find_line(path, rf'pub fn {b}\(\) -> Self')}", identifiers=[],
                            probes=[["ident_cs", nm]]))
    return rows


def enum_validation(repo: str, abbr) -> list[dict]:
    vm = os.path.join(repo, "web/data/verification-matrix.json")
    d = json.load(open(vm))
    ver = os.path.join(repo, "src/verification.rs")
    vsrc = read(ver).splitlines()
    rows = []
    used = {}
    for i, r in enumerate(d["rows"], 1):
        req = r["requirement"]
        slug = re.sub(r"[^a-z0-9]+", "-", req.lower()).strip("-")[:60]
        n = used.get(slug, 0) + 1
        used[slug] = n
        vid = f"validation:{slug}" + (f"-{n}" if n > 1 else "")
        ln = None
        needle = f'requirement: "{req[:40]}'
        for j, line in enumerate(vsrc, 1):
            if needle in line:
                ln = j
                break
        rows.append(feature(id=vid, category="validation", subtype=f"verification-matrix row ({r['status']})", name=req,
                            description=plain(r["capability"], abbr), source=f"src/verification.rs:{ln}" if ln else f"web/data/verification-matrix.json#rows[{i-1}]",
                            identifiers=[], probes=[["exact", req.split("`")[0].strip() if "`" in req else req]], facts={"status": r["status"], "module": r.get("module"), "oracle": r.get("oracle")}))
    return rows


def docs_checkout(repo: str) -> str | None:
    """The engine checkout whose committed documentation the site renders (the site's
    src/data/sources.json: docs_repo), when it is not `repo` itself. A public quote that the
    documentation branch already carries, and `repo` does not yet, is looked up there."""
    cfg = os.path.join(HERE, "..", "explorations", "2026-09-27-combo", "src", "data", "sources.json")
    d = os.environ.get("KSHANA_DOCS_REPO")
    if not d and os.path.exists(cfg):
        d = json.load(open(cfg, encoding="utf-8")).get("docs_repo")
    if not d:
        return None
    d = os.path.abspath(os.path.expanduser(d))
    return d if os.path.isdir(d) and os.path.realpath(d) != os.path.realpath(repo) else None


def pro_text_checkout():
    """The open-repository checkout that carries the public Kshana Pro text (docs/PRO.md) before the
    site's engine checkout does: the site's src/data/pro-power.json names it (public_text), written
    by src/tools/gen_pro_power.py. The quote is read from the committed text at the recorded
    commit (git show), the same text build.py's pq() checks, so the checkout may move on or carry
    uncommitted edits without breaking the check. KSHANA_PRO_TEXT_REPO overrides the checkout; its
    committed HEAD is then used."""
    cfg = os.path.join(HERE, "..", "explorations", "2026-09-27-combo", "src", "data", "pro-power.json")
    d = os.environ.get("KSHANA_PRO_TEXT_REPO")
    commit = ""
    if not d and os.path.exists(cfg):
        pt = json.load(open(cfg, encoding="utf-8")).get("public_text") or {}
        d, commit = pt.get("checkout"), pt.get("commit", "")
    if not d:
        return None
    d = os.path.abspath(os.path.expanduser(d))
    if not os.path.isdir(d):
        return None
    commit = commit or run(["git", "rev-parse", "HEAD"], d).stdout.strip()
    if not commit or run(["git", "cat-file", "-e", f"{commit}^{{commit}}"], d).returncode != 0:
        return None
    return d, run(["git", "rev-parse", "--abbrev-ref", "HEAD"], d).stdout.strip(), commit


def enum_editions(repo: str) -> tuple[list[dict], list[str]]:
    rows, warnings = [], []
    docs = docs_checkout(repo)
    for e in PUBLIC_EDITION_SOURCES:
        ln = locate_quote(repo, e["file"], e["quote"])
        src = f"{e['file']}:{ln}" if ln else ""
        if not ln and docs:
            ln = locate_quote(docs, e["file"], e["quote"])
            if ln:
                branch = run(["git", "rev-parse", "--abbrev-ref", "HEAD"], docs).stdout.strip() or "documentation checkout"
                src = f"{e['file']}:{ln} (branch {branch})"
                warnings.append(f"{e['id']}: public quote found on branch {branch} only, not yet in {os.path.basename(repo)}/{e['file']}")
        pt = pro_text_checkout()
        if not ln and pt:
            ln = locate_quote(pt[0], e["file"], e["quote"], pt[2])
            if ln:
                src = f"{e['file']}:{ln} (branch {pt[1]} {pt[2][:8]})"
                warnings.append(f"{e['id']}: public quote found on branch {pt[1]} only, not yet in {os.path.basename(repo)}/{e['file']}")
        if not ln:
            warnings.append(f"{e['id']}: public quote not found in {e['file']}: {e['quote']!r}")
        rows.append(feature(id=e["id"], category=e["category"], subtype="edition offer", name=e["name"],
                            description=e["description"], source=src or f"{e['file']} (QUOTE NOT FOUND)",
                            edition=e["edition"], edition_source=f"{src} “{e['quote']}”" if src else "UNSOURCED",
                            identifiers=[], probes=[list(p) for p in e["probes"]], claim=e["claim"],
                            facts={"quote": e["quote"], "quote_found": bool(ln)}))
    return rows, warnings


def known_tokens(repo: str, rows: list[dict]) -> list[str]:
    """Identifiers that are legitimately printed on the site but are not features themselves:
    result-document field names, scenario TOML keys, kind fields, sensor/link model ids. The
    checker uses them so a field name in <code> is not mistaken for an unregistered feature."""
    tok: set[str] = set()

    def keys(x):
        if isinstance(x, dict):
            for k, v in x.items():
                tok.add(str(k))
                if k == "id" and isinstance(v, str):
                    tok.add(v)
                keys(v)
        elif isinstance(x, list):
            for v in x:
                keys(v)
    for path in glob.glob(os.path.join(repo, "scenarios", "*.result.json")):
        try:
            keys(json.load(open(path)))
        except Exception:
            pass
    for path in glob.glob(os.path.join(repo, "scenarios", "*.toml")):
        try:
            keys(tomllib.load(open(path, "rb")))
        except Exception:
            pass
    for r in rows:
        for f in r["facts"].get("required_fields", []) + r["facts"].get("optional_fields", []):
            tok.add(f)
    # every hyphenated or snake_case identifier the engine repository itself prints (module and
    # function names, fields, workflow names, CelesTrak groups in comments): a site token found
    # NOWHERE in the repository is foreign; one found here is at least real.
    pats = ["src/**/*.rs", "mcp/kshana-mcp/src/*.rs", "tests/*.rs", "scenarios/*.toml", "docs/**/*.md", "*.md",
            ".github/workflows/*.yml", "web/*.mjs", "web/*.js", "web/*.html", "kshana.pyi", "Cargo.toml", "pyproject.toml"]
    rx = re.compile(r"(?<![A-Za-z0-9_\-])([a-z][a-z0-9]*(?:[-_][a-z0-9]+)+)(?![A-Za-z0-9_\-])")
    # The MCP server's source is read from the checkout its tool list comes from (_mcp_repo).
    scan = [(repo, pat) for pat in pats] + ([(_mcp_repo(repo), "mcp/kshana-mcp/src/*.rs")] if _mcp_repo(repo) != repo else [])
    for base, pat in scan:
        for path in glob.glob(os.path.join(base, pat), recursive=True):
            try:
                tok.update(rx.findall(read(path)))
            except (UnicodeDecodeError, OSError):
                pass
        for path in glob.glob(os.path.join(repo, pat), recursive=True):
            tok.add(os.path.splitext(os.path.basename(path))[0])
    return sorted(t for t in tok if re.fullmatch(r"[A-Za-z0-9_.\-]{2,128}", t))


# ---------------------------------------------------------------------------------------------

def load_placements(path: str) -> dict:
    if not os.path.exists(path):
        return {"placements": {}}
    with open(path, "rb") as f:
        return tomllib.load(f)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--repo", default=DEFAULT_REPO)
    ap.add_argument("--placements", default=os.path.join(HERE, "placements.toml"))
    ap.add_argument("--out", default=HERE)
    args = ap.parse_args()
    repo = os.path.abspath(os.path.expanduser(args.repo))
    binary = os.path.join(repo, "target/release/kshana")
    if not os.access(binary, os.X_OK):
        sys.exit(f"error: no release binary at {binary}; build it in the engine repo first (this script never builds)")

    pl = load_placements(args.placements)
    abbr = load_abbreviations(repo, pl)
    warnings: list[str] = []

    free_ln = locate_quote(repo, FREE_CORE_SOURCE["file"], FREE_CORE_SOURCE["quote"])
    free_ln2 = locate_quote(repo, FREE_CORE_SOURCE_2["file"], FREE_CORE_SOURCE_2["quote"])
    free_src = (f"{FREE_CORE_SOURCE['file']}:{free_ln} “…no feature gate”" if free_ln else "UNSOURCED")
    if free_ln2:
        free_src += f" · {FREE_CORE_SOURCE_2['file']}:{free_ln2} “{FREE_CORE_SOURCE_2['quote']}”"
    if not free_ln:
        warnings.append(f"the public 'no feature gate' quote was not found in {FREE_CORE_SOURCE['file']}")

    rows: list[dict] = []
    k, w = enum_kinds(repo, binary, abbr); rows += k; warnings += w
    s, w = enum_scenarios(repo, binary, abbr); rows += s; warnings += w
    c, w = enum_cli(repo, abbr); rows += c; warnings += w
    rows += enum_mcp(repo, abbr)
    rows += enum_bindings(repo, abbr)
    rows += enum_exports(repo)
    rows += enum_clocks(repo)
    b, w = enum_bands(repo); rows += b; warnings += w
    rows += enum_bodies(repo)
    rows += enum_validation(repo, abbr)
    e, w = enum_editions(repo); rows += e; warnings += w

    for r in rows:
        if r["category"] not in ("pro", "study"):
            r["edition"] = "Free core"
            r["edition_source"] = free_src

    # kinds -> scenarios that exercise them
    by_kind: dict[str, list[str]] = {}
    for r in rows:
        if r["category"] == "scenario":
            by_kind.setdefault(r["facts"]["kind"], []).append(r["name"])
    for r in rows:
        if r["category"] == "kind":
            r["facts"]["scenarios"] = sorted(by_kind.get(r["name"], []))

    ids = [r["id"] for r in rows]
    dupes = sorted({i for i in ids if ids.count(i) > 1})
    if dupes:
        sys.exit(f"error: duplicate feature ids: {dupes}")

    placements = pl.get("placements", {})
    unplaced, stale = [], sorted(set(placements) - set(ids))
    for r in rows:
        p = placements.get(r["id"])
        if not p:
            r.update(page="", section="", studio="", docs="", also=[], placed=False)
            unplaced.append(r["id"])
            continue
        r.update(page=p.get("page", ""), section=p.get("section", ""), studio=p.get("studio", ""),
                 docs=p.get("docs", ""), also=p.get("also", []), placed=True)
        if p.get("pro_link"):
            r["pro_link"] = p["pro_link"]
        if p.get("description"):
            r["description"] = p["description"]
        if p.get("note"):
            r["note"] = p["note"]
        if not r["page"]:
            r["placed"] = False
            unplaced.append(r["id"])
    for r in rows:
        r["unexpanded_abbreviations"] = unexpanded(r["description"], abbr)

    known = known_tokens(repo, rows)

    head = run(["git", "rev-parse", "--short", "HEAD"], repo).stdout.strip()
    dirty = run(["git", "status", "--porcelain"], repo).stdout.strip()
    version = run([binary, "--version"], repo).stdout.strip().split()[-1]
    counts = {c: sum(1 for r in rows if r["category"] == c) for c in CATEGORIES}
    doc = {
        "generated_by": "features/gen_features.py",
        "generated_at": _dt.datetime.now(_dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "engine": {"repo": repo, "version": version, "git_head": head, "worktree_dirty": bool(dirty)},
        "counts": counts,
        "total": len(rows),
        "unplaced": unplaced,
        "stale_placements": stale,
        "warnings": warnings,
        "features": rows,
    }
    with open(os.path.join(args.out, "known-tokens.json"), "w", encoding="utf-8") as f:
        json.dump({"note": "identifiers printed by the engine repository that are not features; used by check_features.py", "tokens": known}, f, indent=0)
        f.write("\n")
    with open(os.path.join(args.out, "features.json"), "w", encoding="utf-8") as f:
        json.dump(doc, f, indent=1, ensure_ascii=False)
        f.write("\n")
    write_md(doc, os.path.join(args.out, "FEATURES.md"))
    print(f"features: {len(rows)} rows  " + "  ".join(f"{c}={n}" for c, n in counts.items()))
    print(f"unplaced: {len(unplaced)}  stale placements: {len(stale)}  warnings: {len(warnings)}")
    for w_ in warnings:
        print(f"  warning: {w_}")
    return 0


def md_cell(s: str) -> str:
    return str(s).replace("|", "\\|").replace("\n", " ")


def write_md(doc: dict, path: str) -> None:
    rows = doc["features"]
    L = []
    L.append("# Kshana feature register")
    L.append("")
    L.append(f"<!-- Generated by features/gen_features.py from {doc['engine']['repo']} at {doc['engine']['git_head']}"
             f"{' (uncommitted changes present)' if doc['engine']['worktree_dirty'] else ''}. Do not edit by hand: "
             "edit placements.toml (placement) or the engine repository (features), then run `python3 gen_features.py`. -->")
    L.append("")
    L.append(f"Engine v{doc['engine']['version']}, {doc['total']} features. Every row is enumerated from the repository; "
             "its **source** column names the file and line (or the command) it came from. Placement (page, section, "
             "Studio control, docs anchor) comes from `placements.toml`; a feature without an entry is listed as UNPLACED.")
    L.append("")
    L.append("**Editions.** The engine has no feature gate: every engine feature is Free core "
             "(public source: `web/index.html`, the open-core card, and `README.md`, Support & professional services). "
             "Pro and Custom-study rows exist only where a public file of the engine repository says so, and each quotes that file.")
    L.append("")
    L.append("| Category | Features |")
    L.append("|---|---:|")
    for c, n in doc["counts"].items():
        L.append(f"| {c} | {n} |")
    L.append(f"| **total** | **{doc['total']}** |")
    L.append("")
    if doc["unplaced"]:
        L.append(f"## UNPLACED ({len(doc['unplaced'])})")
        L.append("")
        for i in doc["unplaced"]:
            L.append(f"- `{i}`")
        L.append("")
    if doc["stale_placements"]:
        L.append(f"## Stale placements ({len(doc['stale_placements'])}): entries in placements.toml for no feature")
        L.append("")
        for i in doc["stale_placements"]:
            L.append(f"- `{i}`")
        L.append("")
    if doc["warnings"]:
        L.append("## Generator warnings")
        L.append("")
        for w in doc["warnings"]:
            L.append(f"- {w}")
        L.append("")
    L.append("## Editions at a glance")
    L.append("")
    L.append("| Edition | What it is | Public source |")
    L.append("|---|---|---|")
    for r in rows:
        if r["category"] in ("pro", "study"):
            L.append(f"| {r['edition']} | **{md_cell(r['name'])}**. {md_cell(r['description'])} | {md_cell(r['edition_source'])} |")
    L.append("")
    for c in CATEGORIES:
        sub = [r for r in rows if r["category"] == c]
        if not sub:
            continue
        L.append(f"## {c} ({len(sub)})")
        L.append("")
        L.append("| id | name | description | edition | page § section | Studio | docs | source |")
        L.append("|---|---|---|---|---|---|---|---|")
        for r in sub:
            place = f"{r['page']} § {r['section']}" if r.get("placed") else "**UNPLACED**"
            extra = []
            if c == "scenario":
                extra.append(f"kind `{r['facts']['kind']}`")
                if r["facts"]["git"] != "committed":
                    extra.append(r["facts"]["git"])
                if not r["facts"]["bundled"]:
                    extra.append("not bundled in the binary")
            if c == "validation":
                extra.append(r["facts"]["status"])
            if c == "kind":
                extra.append(f"{len(r['facts']['scenarios'])} scenario file(s)")
            desc = md_cell(r["description"]) + (f" _({'; '.join(extra)})_" if extra else "")
            L.append(f"| `{r['id']}` | {md_cell(r['name'])} | {desc} | {r['edition']} | {md_cell(place)} | "
                     f"{md_cell(r.get('studio') or '')} | {md_cell(r.get('docs') or '')} | {md_cell(r['source'])} |")
        L.append("")
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(L))


if __name__ == "__main__":
    sys.exit(main())
