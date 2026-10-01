#!/usr/bin/env python3
"""Build the multi-page kshana.dev redesign from src/ and the Kshana repository.

Reads (never writes) the engine checkout named in src/data/sources.json. Every count, install command, scenario name, ledger row and
chart on the site is derived here, at build time, from the repository or from a recorded run of
the engine; nothing is retyped by hand:

  version, package names   Cargo.toml, pyproject.toml, mcp/kshana-mcp/Cargo.toml + server.json
  install channels         the manifests above, cross-checked against README*.md (channels.json)
  scenario kinds / files   src/api.rs list_scenario_kinds(), scenarios/*.toml (git-tracked only)
  ledger + matrix          web/data/verification-matrix.json
  capability cards         web/capabilities.json
  MCP tools                mcp/kshana-mcp/src/server.rs (#[tool] functions)
  charts                   the Studio's recorded runs (the folder src/data/sources.json names as
                           studio), through the Studio's own view code: ../../tools/build_viz.mjs
  Studio links             the Studio's own link list (its tools_links.mjs): .build/studio-links.json,
                           js/studio-links.mjs
  docs                     git-tracked Markdown of the docs checkout (src/data/sources.json:
                           docs_repo), rendered by ../../tools/build_docs.mjs

Outputs (next to this file): index.html, missions.html, capabilities.html, evidence.html,
developers.html, editions.html, docs/, site.css, site.js, tokens.css, js/, data/, assets/,
channels.json, studio/ (a copy of the Studio without its dev files, its recordings packed
into a few files by the Studio's tools_bundle.mjs, its data files taken from the engine
checkout) and PUBLISH.txt (the exact file list to publish).

Usage: python3 build.py          After a Kshana release: re-run it; nothing else changes by hand.
"""
import posixpath
import glob, html, json, math, os, re, shutil, subprocess, sys
from html.parser import HTMLParser

HERE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(HERE, "src")
SOURCES = json.load(open(os.path.join(SRC, "data", "sources.json"), encoding="utf-8"))
REPO = os.path.expanduser(os.environ.get("KSHANA_REPO") or SOURCES["repo"])
# The docs section renders the committed Markdown of this checkout (src/data/sources.json).
DOCS_REPO = os.path.expanduser(os.environ.get("KSHANA_DOCS_REPO") or SOURCES.get("docs_repo") or SOURCES["repo"])
# The Studio this site is built with (src/data/sources.json: studio, relative to this folder).
# ../2026-09-27-playground is the previous Studio, kept untouched as the fallback.
PG_SRC = os.path.normpath(os.path.join(HERE, os.path.expanduser(os.environ.get("KSHANA_STUDIO") or SOURCES.get("studio") or "../2026-09-30-studio-next")))
# Optional Studio scenario groups to leave out of this build (their folder names, comma-separated,
# as listed in the Studio's groups.json). The Studio itself is withheld-safe: deleting a group's
# folder and its groups.json entry has the same effect.
PG_WITHHOLD = {g.strip().rstrip("/") + "/" for g in (os.environ.get("KSHANA_STUDIO_WITHHOLD") or "").split(",") if g.strip()}
TOOLS = os.path.normpath(os.path.join(HERE, "..", "..", "tools"))
SCRATCH = os.path.join(HERE, ".build")
os.makedirs(SCRATCH, exist_ok=True)


def read(p):
    with open(os.path.join(REPO, p), encoding="utf-8") as f:
        return f.read()


def src(p):
    with open(os.path.join(SRC, p), encoding="utf-8") as f:
        return f.read()


def write(rel, text):
    p = os.path.join(HERE, rel)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p, "w", encoding="utf-8") as f:
        f.write(text)


def esc(s):
    return html.escape(str(s), quote=True)


def esc_ic(s):
    """Escape text from source comments and turn Markdown `code` spans into code chips (no raw backticks)."""
    return re.sub(r"`([^`\n]+)`", r"<code>\1</code>", esc(s))


def trunc(s, n):
    s = re.sub(r"\s+", " ", (s or "").strip())
    if len(s) <= n:
        return s
    cut = s[: n - 1]
    sp = cut.rfind(" ")
    if sp > n * 0.6:  # end on a whole word, never mid-word
        cut = cut[:sp]
    return cut.rstrip(" ,;:·(—-") + "…"


TRACKED = set(subprocess.run(["git", "-C", REPO, "ls-files"], capture_output=True, text=True, check=True).stdout.split("\n"))

BRAND = json.loads(src("data/brand.json"))
STUDIO = BRAND["studio"]                    # the ONE place the app's name is set (src/data/brand.json)
STUDIO_CTA = f"Launch {STUDIO}"
STUDIO_OPEN = f"Open in {STUDIO}"

# =================================================================== facts
version = re.search(r'^version\s*=\s*"([^"]+)"', read("Cargo.toml"), re.M).group(1)
api = read("src/api.rs")
blk = api[api.index("pub fn list_scenario_kinds() -> Vec<ScenarioMeta>"): api.index("pub fn list_scenario_kinds_json")]
kinds = [[n, trunc(d.replace('\\"', '"').replace("\\\\", "\\"), 110)]
         for n, d in re.findall(r'ScenarioMeta \{ name: "([^"]+)", description: "((?:[^"\\]|\\.)*)"', blk)]
kind_desc = dict(kinds)
# Untruncated, for the docs' scenario reference (which has room for the whole sentence).
kind_desc_full = {n: re.sub(r"\s+", " ", re.sub(r"\\u\{([0-9a-fA-F]+)\}", lambda m: chr(int(m.group(1), 16)), d).replace('\\"', '"').replace("\\\\", "\\")).strip()
                  for n, d in re.findall(r'ScenarioMeta \{ name: "([^"]+)", description: "((?:[^"\\]|\\.)*)"', blk)}
# Bundled scenario files: git-tracked only, so a work-in-progress file never reaches the site.
files = sorted(os.path.basename(p)[:-5] for p in glob.glob(os.path.join(REPO, "scenarios/*.toml"))
               if not p.endswith(".suite.toml") and f"scenarios/{os.path.basename(p)}" in TRACKED)
# Every tracked scenario file (the study suite included), and the ones compiled into the engine.
all_files = sorted(os.path.basename(p)[:-5] for p in glob.glob(os.path.join(REPO, "scenarios/*.toml")) if f"scenarios/{os.path.basename(p)}" in TRACKED)
bundled = re.findall(r'bundled!\("([\w-]+)"\)', subprocess.run(["git", "-C", REPO, "show", "HEAD:src/bundled_scenarios.rs"], capture_output=True, text=True, check=True).stdout)
assert bundled and set(bundled) <= set(files), set(bundled) - set(files)
not_bundled = [f for f in all_files if f not in bundled]
file_kind, file_desc, file_toml, file_head = {}, {}, {}, {}
for f in files:
    t = read(f"scenarios/{f}.toml")
    file_toml[f] = t
    m = re.search(r'^\s*kind\s*=\s*"([^"]+)"', t, re.M)
    file_kind[f] = m.group(1) if m else "clock"
    for line in t.splitlines():
        if line.startswith("#") and line.strip("# ").strip():
            file_desc[f] = trunc(line.lstrip("# "), 140)
            break
    # The header comment's first paragraph, whole (docs scenario reference).
    head = []
    for line in t.splitlines():
        if not line.startswith("#"):
            break
        body = line.lstrip("#").strip()
        if not body:
            if head:
                break
            continue
        head.append(body)
    file_head[f] = " ".join(head)
vm = json.loads(read("web/data/verification-matrix.json"))
summ = vm["summary"]
assert summ["total"] == len(vm["rows"]) == summ["validated"] + summ["modelled"] + summ["partner_owned"]
caps = json.loads(read("web/capabilities.json"))
cards, standards = caps["capabilities"], caps["standards"]
# Public wording. The engine's capability data is written for the repository; two phrasings in it must
# not reach the public site or docs: an internal process word, and AI naming (content rule).
# tools/build_docs.mjs applies the same rules to the rendered docs (publicWording there); keep the two in step.
PUBLIC_WORDING = [(re.compile(r"\bfounder-gated\b"), "pending review"),
                  (re.compile(r"AI/ML \(AI: artificial intelligence; ML: machine learning\)"), "AI/ML"),
                  (re.compile(r"AI/ML, artificial intelligence / machine learning; "), "")]
_AIML = re.compile(r"(?<![\w/])AI/ML(?![\w/])( (?=[A-Za-z]))?")


def _aiml(m):
    pre = m.string[max(0, m.start() - 2):m.start()]
    word = "machine" if re.fullmatch(r"[a-z,&] ", pre) else "Machine"
    return f"{word}-learning " if m.group(1) else f"{word} learning"


def public_wording(v):
    if isinstance(v, str):
        for rx, to in PUBLIC_WORDING:
            v = rx.sub(to, v)
        return _AIML.sub(_aiml, v)
    if isinstance(v, list):
        return [public_wording(x) for x in v]
    if isinstance(v, dict):
        return {k: public_wording(x) for k, x in v.items()}
    return v


cards, standards = public_wording(cards), public_wording(standards)
# Ledger rows stay verbatim: each requirement name is pinned as an exact probe by the feature register
# (features/features.json), and the ledger is the engine's own record. The docs keep the matrix verbatim too.
kinds = public_wording(kinds)
kind_desc, kind_desc_full = dict(kinds), public_wording(kind_desc_full)
file_desc, file_head = public_wording(file_desc), public_wording(file_head)
# Abbreviations in the explorer and standards rows: each row gets a one-line key of the ones it uses.
ABBR = json.loads(src("data/abbr-public.json"))["abbr"]
_ABBR_RX = [(k, re.compile(r"(?<![A-Za-z0-9])" + re.escape(k) + r"(?![A-Za-z0-9])")) for k in sorted(ABBR, key=len, reverse=True)]


def abbr_used(*texts):
    t = " ".join(x for x in texts if x)
    hits = [(m.start(), k) for k, rx in _ABBR_RX for m in [rx.search(t)] if m]
    return [k for _, k in sorted(hits)]


def abbr_key(*texts):
    ks = abbr_used(*texts)
    if not ks:
        return ""
    return '<small class="abbr-key">' + " · ".join(f"<b>{esc(k)}</b> {esc(ABBR[k])}" for k in ks) + "</small>"
jam_thr = float(re.search(r'DEFAULT_TRACKING_THRESHOLD_DBHZ: f64 = ([\d.]+)', read("src/jamming.rs")).group(1))
mods = {}
for r in vm["rows"]:
    mods.setdefault(r.get("module") or "", []).append(r["status"])
kind_ledger = {k: [mods[k.replace("-", "_")].count("VALIDATED"), len(mods[k.replace("-", "_")])] for k, _ in kinds if mods.get(k.replace("-", "_"))}
# The MCP server's tools, from its source. src/data/sources.json may name a second checkout for it
# (mcp_repo) while the branch that adds tools is not merged into the site's engine checkout; the
# recorded session the pages replay (src/data/mcp-session.json) must list exactly these tools.
MCP_REPO = os.path.expanduser(os.environ.get("KSHANA_MCP_REPO") or SOURCES.get("mcp_repo") or REPO)
mcp_tools = re.findall(r"#\[tool\(\s*description\s*=\s*\"((?:[^\"\\]|\\.)*)\"\s*\)\]\s*fn (\w+)\(", open(os.path.join(MCP_REPO, "mcp/kshana-mcp/src/server.rs"), encoding="utf-8").read())
assert len(mcp_tools) >= 5, mcp_tools
_session_tools = sorted(t["name"] for t in json.loads(src("data/mcp-session.json"))["tools"])
assert _session_tools == sorted(n for _, n in mcp_tools), (f"the recorded MCP session lists {len(_session_tools)} tools, the server source {len(mcp_tools)}: "
                                                          "re-record it with tools/record_mcp_session.py (see src/data/sources.json: mcp_repo)")
server_json = json.loads(read("mcp/kshana-mcp/server.json"))
# Every scenario the Studio opens, from its own indexes: the bundled library and each optional
# group this build keeps. (A scenario the browser build cannot run is recorded by the native
# engine and listed in the same index.)
def _studio_runs(d):
    idx = json.load(open(os.path.join(PG_SRC, d, "recorded", "index.json"), encoding="utf-8"))
    return {f[:-5] for f, e in idx["runs"].items() if not e.get("error")}


PG_GROUPS = [g for g in (json.load(open(os.path.join(PG_SRC, "groups.json"), encoding="utf-8"))["groups"] if os.path.exists(os.path.join(PG_SRC, "groups.json")) else [])
             if os.path.isdir(os.path.join(PG_SRC, g)) and g not in PG_WITHHOLD]
recorded = _studio_runs("")
for _g in PG_GROUPS:
    recorded |= _studio_runs(_g)

# =================================================================== install channels (single source)
def cargo_name(path):
    return re.search(r'^\[package\][^\[]*?^name\s*=\s*"([^"]+)"', read(path), re.M | re.S).group(1)


crate = cargo_name("Cargo.toml")
mcp_crate = cargo_name("mcp/kshana-mcp/Cargo.toml")
mcp_version = re.search(r'^version\s*=\s*"([^"]+)"', read("mcp/kshana-mcp/Cargo.toml"), re.M).group(1)
py_name = re.search(r'^name\s*=\s*"([^"]+)"', read("pyproject.toml"), re.M).group(1)
npm_name = crate  # wasm-pack publishes the crate under its own name (README.npm.md: npm install kshana)
oci = next(p["identifier"] for p in server_json["packages"] if p.get("registryType") == "oci")
image = oci.split(":")[0]
jb = re.search(r"https://plugins\.jetbrains\.com/plugin/(\d+)-[a-z0-9-]+", read("README.md")).group(0)
jb_id = re.search(r"<id>([^<]+)</id>", read("ide/jetbrains/src/main/resources/META-INF/plugin.xml")).group(1)
mcp_readme = read("mcp/kshana-mcp/README.md")
cfg_blocks = re.findall(r"```json\n(\{\s*\"mcpServers\".*?)\n```", mcp_readme, re.S)
assert len(cfg_blocks) >= 2, "MCP client config snippets not found in the kshana-mcp README"
assert mcp_version == version, f"kshana-mcp {mcp_version} != kshana {version}"
assert oci.endswith(":" + version), f"server.json image {oci} is not tagged {version}"
channels = [
    {"id": "browser", "name": "Try in the browser", "audience": "anyone, nothing to install", "command": None, "pinned": None,
     "verify": None, "registry": None, "docs": "docs/studio.html", "note": "The engine runs locally as WebAssembly; nothing is uploaded."},
    {"id": "cli", "name": "Command line (Rust)", "audience": "engineers who want the kshana command", "command": f"cargo install {crate}",
     "pinned": f"cargo install {crate} --version {version}", "verify": f"{crate} --version", "registry": f"https://crates.io/crates/{crate}", "docs": "docs/rust.html#usage--cli"},
    {"id": "rust", "name": "Rust library", "audience": "Rust developers embedding the engine", "command": f"cargo add {crate}",
     "pinned": f"cargo add {crate}@{version}", "verify": f"cargo tree --depth 1 | grep {crate}", "registry": f"https://crates.io/crates/{crate}", "docs": f"https://docs.rs/{crate}"},
    {"id": "python", "name": "Python", "audience": "analysts and notebooks", "command": f"pip install {py_name}",
     "pinned": f"pip install {py_name}=={version}", "verify": f'python -c "import {py_name}; print({py_name}.version())"', "registry": f"https://pypi.org/project/{py_name}/", "docs": "docs/python-api.html"},
    {"id": "npm", "name": "JavaScript and WebAssembly", "audience": "web and Node.js developers", "command": f"npm install {npm_name}",
     "pinned": f"npm install {npm_name}@{version}", "verify": f"npm ls {npm_name}", "registry": f"https://www.npmjs.com/package/{npm_name}", "docs": "docs/npm.html"},
    {"id": "mcp", "name": "AI assistant (MCP server)", "audience": "anyone using an MCP-capable assistant", "command": f"cargo install {mcp_crate}",
     "pinned": f"cargo install {mcp_crate} --version {version}", "verify": f"cargo install --list | grep {mcp_crate}", "registry": f"https://crates.io/crates/{mcp_crate}", "docs": "docs/mcp.html",
     "configs": [c.strip() for c in cfg_blocks[:2]], "registry_name": server_json["name"]},
    {"id": "docker", "name": "Docker image", "audience": "no Rust toolchain; the MCP server in a container", "command": f"docker run --rm -i {image}",
     "pinned": f"docker run --rm -i {image}:{version}", "verify": f"docker image inspect {image}:{version} --format '{{{{.Id}}}}'", "registry": "https://github.com/ashfordeOU/kshana/pkgs/container/kshana-mcp", "docs": "docs/mcp.html#install"},
    {"id": "jetbrains", "name": "JetBrains IDE plugin", "audience": "IntelliJ, PyCharm and CLion users", "command": None, "pinned": None,
     "verify": None, "registry": jb, "docs": "docs/jetbrains.html", "note": f'Settings → Plugins → Marketplace → search "Kshana" (plugin ID {jb_id}). Right-click a scenario .toml and choose Run Kshana Scenario.'},
]
# Each channel's two links, named for what they open: where the package is published, then the
# guide for that channel. The browser channel has no registry: it opens the Studio itself.
LINK_LABELS = {
    "browser": (f"Open {STUDIO}", "Studio guide"),
    "cli": (f"crates.io: {crate}", "Command-line usage"),
    "rust": (f"crates.io: {crate}", "API reference on docs.rs"),
    "python": (f"PyPI: {py_name}", "Python API guide"),
    "npm": (f"npm: {npm_name}", "npm and WebAssembly guide"),
    "mcp": (f"crates.io: {mcp_crate}", "MCP server guide"),
    "docker": ("GitHub container registry", "MCP server guide: Docker"),
    "jetbrains": ("JetBrains Marketplace", "Plugin guide"),
}
for ch in channels:
    ch["registry_label"], ch["docs_label"] = LINK_LABELS[ch["id"]]
# Drift guard, part 1: each README must carry the same base commands (check.py repeats this).
readme_all = "".join(read(p) for p in ["README.md", "README.crates.md", "README.pypi.md", "README.npm.md"]) + mcp_readme
for ch in channels:
    if ch["command"]:
        base = ch["command"].replace(" --rm -i", " --rm -i")
        assert base in readme_all or base.replace("docker run --rm -i", "docker run -i") in readme_all, f"README does not carry: {base}"
write("channels.json", json.dumps({"version": version, "generated_from": "the Kshana repository's manifests, cross-checked against README*.md", "channels": channels}, indent=2, ensure_ascii=False))

# =================================================================== sectors, roles, ways
PRO = {
    "twin": "Pro: clock digital twins calibrated to your device's published Allan budget.",
    "trade": "Pro: architecture trade studies ranked on a figure of merit.",
    "pack": "Pro: audit-grade, reproducible evidence packs for a review.",
    "custom": "Custom: sensor and resilience models calibrated to your hardware.",
    "export": "Custom: export-controlled resilience work under the appropriate clearance and NDA (non-disclosure agreement).",
    "lic": "Commercial licence of the open core: closed or proprietary integration without AGPL obligations.",
    "train": "Custom study: training and consulting on quantum and classical PNT performance analysis.",
}
# Which paid work builds on which open kinds (public descriptions of Pro and the custom study).
EXTENDS = {
    ("Pro: calibrated clock twin", "editions.html#pro"): ["clock", "lunar-time-offset", "timetransfer", "quantum-time-transfer", "lunar-time-budget", "telecom-timing", "slot-timing"],
    ("Pro: ranked trade study", "editions.html#pro"): ["sweep", "sweep-nd", "combined-altpnt", "quantum-trade", "hybrid-optical-rf", "conflict-resilience"],
    ("Custom study", "editions.html#custom"): ["impairment-eval"],
}
# The Missions page source owns its nine sectors (tabs and panels); Home's teasers, the Missions
# sub-nav and the search index read them from there, so the list lives in one place.
def _sectors_from_missions():
    page = src("pages/missions.html")
    tabs = re.findall(r'<button role="tab" type="button" id="st-([\w-]+)" aria-controls="\1"[^>]*style="--c:var\((--[\w-]+)\)"><i></i>([^<]+)</button>', page)
    out = []
    for sid, c, name in tabs:
        m = re.search(r'role="tabpanel" id="' + re.escape(sid) + r'".*?<h3 class="m-q">(.*?)</h3>', page, re.S)
        assert m, f"missions.html: sector {sid} has no question"
        out.append({"id": sid, "n": html.unescape(name), "c": c, "q": html.unescape(re.sub(r"<[^>]+>", "", m.group(1)))})
    assert len(out) >= 8, f"missions.html: found {len(out)} sector tabs"
    return out


SECTORS = _sectors_from_missions()

ROLES = [
    ("Mission and systems engineer", "--orb", "Size a PNT architecture and see where it breaks.", ("Capability atlas", "capabilities.html#atlas"), "kshana scenarios/hybrid-pnt.toml", "trade"),
    ("PNT or GNSS specialist", "--tim", "Open the TOML, change the physics, check it against the ledger.", ("Validation ledger", "evidence.html#ledger"), "kshana --validate scenarios/integrity-raim.toml", "custom"),
    ("Test and evaluation lead", "--int", "Pin scenarios as regression tests and run a whole suite into one stamped study.", ("Ways to use it", "developers.html#ways"), "kshana --study scenarios/quantum-pnt-demonstrator.suite.toml", "pack"),
    ("Procurement or programme reviewer", "--itg", "Use the validation ledger as due-diligence evidence: every capability names its oracle and its test.", ("Verification matrix", "evidence.html#matrix"), "https://github.com/ashfordeOU/kshana", "pack"),
    ("Researcher or student", "--spf", "Reproduce a result to the seed, extend it and cite the exact engine version.", ("Research and citation", "evidence.html#cite"), "pip install kshana", None),
    ("Timing and network engineer", "--tim", "Check holdover against ITU-T masks before the GNSS antenna fails.", ("Timing wall", "capabilities.html#timing"), "kshana scenarios/telecom-prtc-holdover-24h.toml", "twin"),
    ("AI-assistant user", "--spf", "Let your assistant run the validated engine instead of guessing the maths.", ("MCP server", "developers.html#mcp"), "cargo install kshana-mcp", None),
    ("Embedded or product developer", "--nav", "Call the engine from Rust, Python or JavaScript inside your own product.", ("Install", "developers.html#install"), "cargo add kshana", "lic"),
]
WAYS = [
    ("browser", STUDIO, "--tim", "studio/index.html", "Any bundled scenario, run on your machine as WebAssembly; nothing installed, nothing uploaded. Tune it, compare runs, sweep a parameter and export a reproducible report."),
    ("cli", "Command line", "--int", f"cargo install {crate}", "kshana <scenario>.toml writes result.json, chart.svg and report.html"),
    ("rust", "Rust crate", "--nav", f"cargo add {crate}", "kshana::api::run_toml returns JSON, SVG, a summary and CSV (comma-separated values)"),
    ("python", "Python", "--tim", f"pip install {py_name}", "kshana.run and run_full, plus validate_toml and list_kinds"),
    ("npm", "npm · WebAssembly", "--itg", f"npm install {npm_name}", "run, summary and chart_svg in the browser or Node.js"),
    ("mcp", "MCP server", "--spf", f"cargo install {mcp_crate}", f"{len(mcp_tools)} tools an AI assistant can call, each a thin wrapper over the public API"),
    ("docker", "Docker image", "--orb", f"docker run --rm -i {image}", "The MCP server without a Rust toolchain, multi-architecture"),
    ("jetbrains", "JetBrains plugin", "--orb", jb, "Right-click a scenario .toml, Run Kshana Scenario, read the figures in a tool window"),
    ("embed", "Embed mode", "--spf", "studio/index.html?embed=1&scenario=integrity-raim&tab=timeseries", f"Only {STUDIO}, for a course page or a report, with the scenario and view preselected"),
    ("ci", "CI regression", "--int", "kshana --validate scenarios/integrity-raim.toml", "Output is deterministic from scenario, seed and engine version, so a committed result.json is a regression test (CI: continuous integration): rerun and diff it"),
    ("reports", "Study suite (free)", "--itg", "kshana --study scenarios/quantum-pnt-demonstrator.suite.toml", "kshana --study: a suite of scenarios into one stamped study.json and study.html, free. Not the paid custom study."),
]
GROUPS = [
    ("Earth GNSS and interference", "--int", ["jamming", "spectrum", "tracking-loop", "gnss-sim", "pvt", "spoof", "spoof-detect", "impairment-eval"]),
    ("Integrity and ARAIM", "--itg", ["integrity", "araim-reference-check"]),
    ("Alternative PNT and fusion", "--nav", ["gravity-map", "terrain-nav", "terrain-slam", "combined-altpnt", "inertial", "gnss-ins", "ins-trn-coast", "fusion", "hybrid", "hybrid-ukf", "hybrid-optical-rf"]),
    ("Clocks and timing", "--tim", ["clock", "sweep", "slot-timing", "telecom-timing", "timetransfer"]),
    ("Orbits and mission analysis", "--orb", ["orbit", "ephemeris", "oem-interop", "passes", "launch-window", "reentry", "eo-coverage", "attitude-budget", "space-packet", "space-weather", "link-budget", "aperture-duty-cycle", "constellation-design"]),
    ("Low Earth orbit (LEO) PNT", "--nav", ["leo-signal", "leo-pass", "leo-navmsg", "leo-pvt", "leo-ppp", "ntn-positioning", "leo-pnt-chain"]),
    ("Lunar and cislunar", "--spf", ["lunar-integrity", "lunar-time-offset", "lunar-vlbi", "lunar-joint-od-clock", "lunar-frame-realisation", "moonlight-service-volume", "lunar-differential-pnt", "lunar-interop-export", "lunar-time-budget", "cislunar-observability", "lunar-attack-surface", "realtime-frame-eop", "lunar-jamming", "lunar-vlbi-fim", "lunar-frame-campaign", "lunar-llr-datum", "lunar-beacon", "earth-gnss-lunar", "cislunar-arc-recovery"]),
    ("Mars and the solar system", "--int", ["mars-pnt", "body-pnt", "solar-system"]),
    ("Quantum sensors and trade studies", "--spf", ["quantum-anomaly-detect", "quantum-gnss-free-nav", "quantum-time-transfer", "quantum-trade", "conflict-resilience", "sweep-nd"]),
    ("Campaigns", "--itg", ["campaign"]),
]
# Every kind the engine lists sits in exactly one group: a new kind stops the build here until it is placed.
_grouped = [k for _, _, ks in GROUPS for k in ks]
assert len(_grouped) == len(set(_grouped)), "a kind is in two atlas groups"
assert {k for k, _ in kinds} == set(_grouped), f"atlas groups and the engine's kinds differ: ungrouped {sorted({k for k, _ in kinds} - set(_grouped))}, unknown {sorted(set(_grouped) - {k for k, _ in kinds})}"
# Every band below was checked in the engine source; hatched ones are named for context only.
BANDS = [
    {"id": "L1 · E1", "mhz": [1575.42], "con": "GPS · Galileo", "c": "--s-tim", "m": True, "src": "jamming.rs, tracking_loop.rs, gnss_sim.rs, ephemeris.rs, navsignal.rs",
     "use": "The C/A (coarse/acquisition) code carrier for jamming, tracking loops, measurement simulation, Doppler and positioning; Galileo E1 open service as a BOC(1,1) (binary offset carrier) modulation in the anti-jam model.",
     "jam": f"A jammer sets the jammer-to-signal ratio; processing gain converts it to an effective C/N₀ per satellite, and a satellite loses lock below {jam_thr:g} dB-Hz by default.",
     "files": ["jamming-demo", "tracking-loop", "small-uas-jammed-nav", "gnss-sim-raim", "spoof-detect", "ephemeris", "pvt-abmf", "earth-gnss-lunar"], "viz": "band-l1"},
    {"id": "L2", "mhz": [1227.6], "con": "GPS", "c": "--s-orb", "m": True, "src": "pvt.rs, ionex.rs",
     "use": "Dual-frequency L1/L2 ionosphere-free positioning on real IGS (International GNSS Service) station data, and IONEX delay scaling as 1/f².",
     "jam": "Not a jamming target in the bundled scenarios; used to remove the ionosphere.", "files": ["pvt-abmf"]},
    {"id": "L5 · E5a", "mhz": [1176.45], "con": "GPS · Galileo", "c": "--s-int", "m": True, "src": "sbas.rs, conflict_threat_params.rs",
     "use": "Dual-frequency L1/L5 ionosphere-free SBAS (satellite-based augmentation system) protection levels, and the wideband L5/E5a layer in layered-PNT conflict studies.",
     "jam": "Wideband, so harder to jam than L1 C/A, but still denied by a jammer on the same band.", "files": ["conflict-resilience"]},
    {"id": "S-band 2.4 GHz", "mhz": [2400], "con": "Lunar augmented forward signal", "c": "--s-spf", "m": True, "src": "lunar_service.rs, lunar_jamming",
     "use": "The lunar augmented forward signal carrier for lunar service volumes and lunar-surface jamming.",
     "jam": "Per-satellite jammer-to-signal ratio for a lunar surface user, and the lunar signal-security attack surface.", "files": ["lunar-jamming", "lunar-attack-surface", "moonlight-service-volume"], "viz": "band-lunar-s"},
    {"id": "S · X · Ka", "mhz": [2300, 8420, 32000], "con": "Deep Space Network", "c": "--s-itg", "m": True, "src": "linkbudget.rs, radiometric.rs",
     "use": "Deep-space link budgets in the DSN (Deep Space Network) allocations: S near 2.3 GHz, X near 8.4 GHz and Ka near 32 GHz.",
     "jam": "Link margin, not jamming, is the question here.", "files": ["link-budget"]},
    {"id": "Optical 1550 nm", "mhz": None, "con": "Optical link", "c": "--s-orb", "m": True, "src": "optical_linkbudget.rs",
     "use": "The 1550 nm optical link for optical PNT and inter-satellite time transfer. This is the optical telecom C-band, not RF C-band.",
     "jam": "Weather and pointing, not RF jamming, limit it.", "files": ["hybrid-optical-rf", "timetransfer"]},
    {"id": "G1 · G2", "mhz": [1602, 1246], "con": "GLONASS", "c": "--s-nav", "m": False, "use": "GLONASS broadcast ephemerides are parsed from RINEX (Receiver Independent Exchange Format), but its FDMA (frequency-division multiple access) signals are not modelled.", "jam": "Not modelled.", "files": []},
    {"id": "B1 · B2", "mhz": [1561.098, 1207.14], "con": "BeiDou", "c": "--s-nav", "m": False, "use": "BeiDou ephemerides are parsed from RINEX; its signals are not modelled.", "jam": "Not modelled.", "files": []},
    {"id": "E6", "mhz": [1278.75], "con": "Galileo", "c": "--s-spf", "m": False, "use": "Galileo E6 is not modelled.", "jam": "Not modelled.", "files": []},
]
for b in BANDS:
    for f in b["files"]:
        assert f in files, f"band {b['id']} names {f}"

GROUPS_BY_NAME = [(n, ks) for n, _, ks in GROUPS]
GLOSS = json.loads(src("data/glossary.json"))  # site-only abbreviations; rendered on the Docs glossary page, never on marketing pages
# Published research: generated by src/tools/gen_publications.py from the arXiv API (title, authors,
# date, version, abstract) and the engine checkout (the capability that reproduces each paper).
_pubs_json = json.loads(src("data/publications.json"))
PUBS, PUB_ENGINE = _pubs_json["papers"], _pubs_json["engine"]
assert PUBS and all(p["reproduce"]["command"] for p in PUBS), "publications.json: rerun src/tools/gen_publications.py"

# =================================================================== charts (real engine output)
PAGES = [  # (key, nav label, <title> at most 60 characters, source, meta description at most 155: check.py rule 23)
    ("index", "Home", "Kshana: open-source GNSS jamming and spoofing simulator", "home.html",
     "Open-source GNSS jamming and spoofing simulator for PNT (positioning, navigation and timing) resilience: holdover, RAIM, lunar navigation, in your browser."),
    ("missions", "Missions", "Missions: GNSS and PNT resilience by sector · Kshana", "missions.html",
     "GNSS jamming, spoofing and clock holdover scenarios for ten sectors, from defence, aviation and maritime to telecom timing and lunar navigation."),
    ("capabilities", "Capabilities", "Capabilities: GNSS, clocks, orbits, Moon and Mars · Kshana", "capabilities.html",
     "GNSS signal bands and jamming, clock holdover, timing masks, integrity monitoring (RAIM), orbits and lunar navigation: every scenario kind on one map."),
    ("evidence", "Evidence", "Evidence: validation against external oracles · Kshana", "evidence.html",
     "Which Kshana capabilities are validated against independent oracles and which are modelled: verification matrix, validation ledger, papers and citation."),
    ("developers", "Developers", "Install Kshana: CLI, Rust, Python, npm and MCP server", "developers.html",
     "Install Kshana as a command-line tool, Rust crate, Python or npm package, Model Context Protocol (MCP) server, container or JetBrains plugin."),
    ("editions", "Editions", "Editions: open source and Kshana Pro · Kshana", "editions.html",
     "Kshana is free and open source under the AGPL-3.0 licence. Kshana Pro adds a design optimiser, uncertainty and sensitivity studies and a mission dossier."),
]
NAV = [("missions", "Missions", "missions.html"), ("capabilities", "Capabilities", "capabilities.html"), ("evidence", "Evidence", "evidence.html"),
       ("research", "Research", "evidence.html#research"), ("developers", "Developers", "developers.html"), ("docs", "Docs", "docs/index.html"), ("editions", "Editions", "editions.html")]

raw_pages = {key: src(f"pages/{part}") for key, _, _, part, _ in PAGES}
FIG_RE = re.compile(r'<figure class="viz([^"]*)" data-viz="([\w-]+)" data-w="(\d+)" data-h="(\d+)"(?: data-variant="([\w-]+)")?></figure>')
reqs = {}
for key, text in raw_pages.items():
    for m in FIG_RE.finditer(text):
        reqs[(m.group(2), int(m.group(3)), int(m.group(4)), m.group(5) or "")] = 1
# Generated blocks use charts too (Pro deliverables, bands, journey).
for b in BANDS:
    if b.get("viz"):
        reqs[(b["viz"], 760, 0, "jammed")] = 1
EXTRA_CHARTS = [("pro-twin", 420, 230, ""), ("pro-trade", 420, 230, ""), ("clk-adev", 760, 360, ""), ("tw-mtie", 560, 300, ""), ("tw-mtie", 560, 300, "tdev"), ("clk-slot", 420, 240, ""),
                ("jr-earth", 520, 0, ""), ("jr-moon", 520, 240, ""), ("jr-mars", 520, 240, ""), ("clk-hold-a", 560, 280, ""), ("clk-hold-b", 560, 280, "")]
for r in EXTRA_CHARTS:
    reqs[r] = 1
req_list = [{"id": i, "w": w, "h": h, "variant": v} for (i, w, h, v) in reqs]
json.dump(req_list, open(os.path.join(SCRATCH, "viz-requests.json"), "w"))

# Real coastlines for 2-D maps: Natural Earth 1:110m land, public domain, rounded to 0.1°.
land = json.load(open(os.path.join(HERE, "assets", "geo", "ne_110m_land.geojson")))
rings = []
for feat in land["features"]:
    g = feat["geometry"]
    polys = g["coordinates"] if g["type"] == "MultiPolygon" else [g["coordinates"]]
    for poly in polys:
        ring = [[round(x, 1), round(y, 1)] for x, y in poly[0]]
        ded = [ring[0]] + [p for i, p in enumerate(ring[1:], 1) if p != ring[i - 1]]
        if len(ded) >= 4:
            rings.append(ded)
json.dump(rings, open(os.path.join(SCRATCH, "land.json"), "w"), separators=(",", ":"))
os.makedirs(os.path.join(HERE, "data"), exist_ok=True)
json.dump(rings, open(os.path.join(HERE, "data", "land.json"), "w"), separators=(",", ":"))

VIZ_OUT = os.path.join(SCRATCH, "viz.json")
subprocess.run(["node", os.path.join(TOOLS, "build_viz.mjs"), PG_SRC, VIZ_OUT, os.path.join(SCRATCH, "viz-requests.json"), os.path.join(SCRATCH, "land.json")], check=True)
viz = json.load(open(VIZ_OUT))
CHARTS, SVGS = viz["charts"], viz["svgs"]


# Every deep link the Studio answers, from the Studio's own generator (tools_links.mjs: the
# scenario catalogue, each recorded result through the Studio's view code, and the native
# engine's recordings): per scenario, the views it has and the link a site visual of that run
# uses (its replay where it has one, else its most telling view). Nothing here is typed.
_links_json = os.path.join(SCRATCH, "studio-links.json")
subprocess.run(["node", os.path.join(PG_SRC, "tools_links.mjs"), "--out", os.path.join(SCRATCH, "STUDIO-DEEP-LINKS.md"), "--json", _links_json], check=True, stdout=subprocess.DEVNULL)
STUDIO_LINKS = {r["name"]: r for r in json.load(open(_links_json, encoding="utf-8")) if r["dir"] not in PG_WITHHOLD}
assert set(STUDIO_LINKS) == recorded, f"the Studio's link list and its recordings disagree: {sorted(set(STUDIO_LINKS) ^ recorded)[:6]}"
json.dump({n: {"tabs": r["tabs"], "best": r["links"]["best"]} for n, r in sorted(STUDIO_LINKS.items())}, open(os.path.join(SCRATCH, "studio-views.json"), "w", encoding="utf-8"), indent=0)


def pg_link(root, file, tab):
    assert file in recorded, f"{file} has no recording in the Studio"
    assert tab in STUDIO_LINKS[file]["tabs"], f"{file} has no {tab} view in the Studio (it has {', '.join(STUDIO_LINKS[file]['tabs'])})"
    return f"{root}studio/index.html?scenario={file}&amp;tab={tab}"


def studio_best(root, file):
    """The Studio view that reproduces a run drawn on the site (HTML-escaped href)."""
    assert file in STUDIO_LINKS, f"{file} is not in the Studio"
    return esc(f"{root}studio/" + STUDIO_LINKS[file]["links"]["best"])


def studio_open(root, file):
    """The site's one "Open in <Studio>" link component, for a run the Studio reproduces."""
    return f'<a class="prov-open" href="{studio_best(root, file)}">{esc(STUDIO_OPEN)}<svg aria-hidden="true"><use href="#i-ext"/></svg></a>'


def file_href(root, f):
    """Where a scenario file's chip opens: its recorded run in the Studio, or, for a file the Studio
    does not run, its entry in the docs' scenario reference (kind, description and the TOML)."""
    return pg_link(root, f, "overview") if f in recorded else f"{root}docs/scenario-reference.html#{f}"


def prov_html(root, cid, lead=None, dark=False, svg=False):
    """Provenance line for a chart or a table: engine version, scenario file(s), seed, and the
    routes to reproduce and export it: the exact run in the Studio, its report export, the plotted
    data as CSV (from data/series.json) and, for charts, the SVG as drawn."""
    c = CHARTS[cid]
    seed = f" · seed {c['seed']}" if c.get("seed") is not None else " · deterministic, no seed"
    files_txt = " + ".join(f"<code>{esc(f)}.toml</code>" for f in c.get("files", [c["file"]]))
    lead_txt = f"<b>{esc(lead)}:</b> " if lead else ""
    tools = (f'<span class="prov-tools"><a class="prov-x" href="{pg_link(root, c["file"], "exports")}" title="Open the run\'s exports in {esc(STUDIO)}: reproducible HTML report, JSON, CSV">Report</a>'
             f'<button class="prov-x" type="button" data-dl-csv="{esc(cid)}" title="Download the plotted data as comma-separated values"><abbr title="comma-separated values">CSV</abbr></button>'
             + (f'<button class="prov-x" type="button" data-dl-svg title="Download this chart as scalable vector graphics"><abbr title="scalable vector graphics">SVG</abbr></button>' if svg else "")
             + "</span>")
    return (f'<span class="prov-src">{lead_txt}Engine v{esc(c["engine"])} · {files_txt}{seed}</span>'
            f'<a class="prov-open" href="{pg_link(root, c["file"], c["tab"])}">{esc(STUDIO_OPEN)}<svg aria-hidden="true"><use href="#i-ext"/></svg></a>{tools}')


def fig_html(root, cid, w, h, variant="", cls="", label=None):
    c = CHARTS[cid]
    svg = SVGS[f"{cid}|{w}|{h}|{variant}"]
    title = label or (c["model"].get("title") if isinstance(c.get("model"), dict) else None) or ""
    wide = " viz-wide" if w >= 700 else ""
    cap = "" if "viz-bare" in cls else f'<figcaption class="prov">{prov_html(root, cid, svg=True)}</figcaption>'
    return (f'<figure class="viz{esc(cls)}{wide}" data-viz="{cid}" data-variant="{esc(variant)}">'
            f'<div class="viz-plot" role="img" aria-label="{esc(title)}">{svg}</div>{cap}</figure>')


# =================================================================== small real-data graphics (Python-drawn)
def _fin(v):
    return isinstance(v, (int, float)) and math.isfinite(v)


def _fmt(v):
    if not _fin(v):
        return "–"
    a = abs(v)
    if a == 0:
        return "0"
    if a >= 1e6 or a < 1e-3:
        return f"{v:.1e}"
    if a >= 1000:
        return f"{v:,.0f}"
    return f"{v:.3g}"


def spark_svg(cid, w=360, h=128, logy=None, abs_y=False, budget=None, max_series=3):
    """A compact chart of a recorded run: the real series (no smoothing beyond build_viz's
    thinning), min and max labels, the threshold or budget and the outage band. Lines carry
    pathLength=1 so site.css can draw them in when they scroll into view."""
    c = CHARTS[cid]
    m = c["model"]
    if c["kind"] == "mask":
        series = [{"label": "MTIE", "color": "var(--s-tim)", "points": m["mtie"]}, {"label": "TDEV", "color": "var(--s-spf)", "points": m["tdev"]}]
        logx, logy = True, True
    else:
        series = [s for s in m["series"] if len(s["points"]) > 1][:max_series]
        logx = bool(m.get("logX"))
        logy = bool(m.get("logY")) if logy is None else logy
    fx = (lambda v: math.log10(v)) if logx else (lambda v: v)
    fy = (lambda v: math.log10(abs(v))) if logy else ((lambda v: abs(v)) if abs_y else (lambda v: v))
    ok = lambda p: _fin(p[0]) and _fin(p[1]) and (p[0] > 0 or not logx) and (p[1] != 0 or not logy)
    good = [p for s in series for p in s["points"] if ok(p)]
    x0, x1 = min(fx(p[0]) for p in good), max(fx(p[0]) for p in good)
    ys = [fy(p[1]) for p in good]
    thr = budget if budget is not None else (m.get("threshold") if c["kind"] != "mask" else None)
    if _fin(thr) and (thr > 0 or not logy):
        ys.append(fy(thr))
    y0, y1 = min(ys), max(ys)
    if not logy:
        y0 = min(0, y0)
    pad = (y1 - y0) * 0.08 or 1
    y1 += pad
    if logy:
        y0 -= pad
    x1 = x1 if x1 > x0 else x0 + 1
    # Legend rows first (they wrap before the max label on the right), then the plot below them.
    unit = m.get("unit", "ns" if c["kind"] == "mask" else "")
    # Widths leave room for WCAG 1.4.12 text spacing (0.12 em letters, 0.16 em words): about 7.4
    # units a character at the 10 px spark size, so a spaced legend never runs into the max label.
    top_ = 10 ** y1 if logy else y1 - pad
    max_lbl = f'max {_fmt(top_)} {unit}{", log" if logy else ""}'
    maxw = 14 + len(max_lbl) * 7.4
    leg, lx, ly = [], 8, 12
    for s_ in series:
        wl = 22 + len(str(s_["label"])) * 7.4
        if lx + wl > w - maxw and lx > 8:
            lx, ly = 8, ly + 13
        leg.append((lx, ly, s_))
        lx += wl
    ml, mr, mt, mb = 8, 8, ly + 10, 18
    # Spoofing and timing colours fall together under protanopia/deuteranopia: dash the spoofing one
    # when both are drawn (same rule as the Studio's lineChartSvg).
    has_tim = any("--s-tim" in str(s_["color"]) for s_ in series)
    dashed = lambda s_: has_tim and "--s-spf" in str(s_["color"])
    px = lambda v: ml + (fx(v) - x0) / (x1 - x0) * (w - ml - mr)
    py = lambda v: mt + (1 - (fy(v) - y0) / (y1 - y0)) * (h - mt - mb)
    o = [f'<svg class="chart spark" viewBox="0 0 {w} {h}" width="{w}" height="{h}" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="{esc(m.get("title") or c["file"])}">',
         f'<rect class="c-bg" width="{w}" height="{h}"/>']
    for a, b in (m.get("outages") or []) if c["kind"] != "mask" else []:
        xa, xb = px(max(a, 1e-9) if logx else a), px(b)
        o.append(f'<rect class="c-outage" x="{xa:.1f}" y="{mt}" width="{max(0, xb - xa):.1f}" height="{h - mt - mb}"/>')
    for k in (1, 2, 3):
        y = mt + k * (h - mt - mb) / 4
        o.append(f'<line class="c-grid" x1="{ml}" y1="{y:.1f}" x2="{w - mr}" y2="{y:.1f}"/>')
    if _fin(thr) and (thr > 0 or not logy):
        y = py(thr)
        o.append(f'<line class="c-thr" x1="{ml}" y1="{y:.1f}" x2="{w - mr}" y2="{y:.1f}"/>')
    for s in series:
        p = [q for q in s["points"] if ok(q)]
        if len(p) < 2:
            continue
        pts_ = " ".join(f"{px(q[0]):.1f},{py(q[1]):.1f}" for q in p)
        if dashed(s):
            o.append(f'<polyline class="c-line c-line-d sp-line-d" stroke-dasharray="6 4" style="stroke:{s["color"]}" points="{pts_}"/>')
        else:
            o.append(f'<polyline class="c-line sp-line" pathLength="1" style="stroke:{s["color"]}" points="{pts_}"/>')
    for lx, ly, s in leg:
        sw = f'<path d="M0 -2.5h4M7 -2.5h3" style="stroke:{s["color"]}" stroke-width="3" fill="none"/>' if dashed(s) else f'<rect width="10" height="3" y="-4" style="fill:{s["color"]}"/>'
        o.append(f'<g class="c-legend" transform="translate({lx:.0f},{ly})">{sw}<text x="14" y="0">{esc(s["label"])}</text></g>')
    top, bot = (10 ** y1 if logy else y1 - pad), (10 ** y0 if logy else y0)
    o.append(f'<text class="c-tick" x="{w - mr}" y="12" text-anchor="end">max {esc(_fmt(top))} {esc(unit)}{", log" if logy else ""}</text>')
    xa, xb = (10 ** x0 if logx else x0), (10 ** x1 if logx else x1)
    o.append(f'<text class="c-tick" x="{ml}" y="{h - 4}">{esc(_fmt(xa))}</text><text class="c-tick" x="{w - mr}" y="{h - 4}" text-anchor="end">{esc(_fmt(xb))} {"s, τ" if logx else "s"}</text>')
    o.append("</svg>")
    return "".join(o)


def register_svg(cid, w, h, variant, svg):
    SVGS[f"{cid}|{w}|{h}|{variant}"] = svg
    return (cid, w, h, variant)


def lunar_time_svg(w=420, h=190):
    f = CHARTS["tw-lunar"]["figures"]
    sp, kn, tot, lo, hi = f["self_potential_us_per_day"], f["kinetic_us_per_day"], f["secular_rate_us_per_day"], f["band_low"], f["band_high"]
    ml, mr = 118, 16
    top = max(sp, hi) * 1.05
    X = lambda v: ml + (v - min(0, kn)) / (top - min(0, kn)) * (w - ml - mr)
    o = [f'<svg class="chart" viewBox="0 0 {w} {h}" width="{w}" height="{h}" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Lunar time rate: the engine\'s terms against the published band">',
         f'<rect class="c-bg" width="{w}" height="{h}"/>']
    rows = [("self-potential", sp, "var(--s-spf)"), ("kinetic", kn, "var(--s-nav)"), ("total rate", tot, "var(--s-tim)")]
    for i, (lab, v, col) in enumerate(rows):
        y = 18 + i * 26
        x0, x1 = sorted((X(0), X(v)))
        o.append(f'<text class="c-axis" x="{ml - 8}" y="{y + 11}" text-anchor="end">{esc(lab)}</text>')
        o.append(f'<rect class="sp-bar" x="{x0:.1f}" y="{y}" width="{max(1.5, x1 - x0):.1f}" height="14" rx="3" style="fill:{col}"/>')
        inside = x1 > w - 70
        o.append(f'<text class="c-tick{" on-bar" if inside else ""}" x="{(x1 - 6 if inside else x1 + 6):.1f}" y="{y + 11}" text-anchor="{"end" if inside else "start"}">{v:+.2f}</text>')
    o.append(f'<line class="c-grid" x1="{X(0):.1f}" y1="12" x2="{X(0):.1f}" y2="94"/>')
    # Zoomed strip: the published band and where the engine's total lands inside it.
    zl, zh = math.floor(min(lo, tot) - 1), math.ceil(max(hi, tot) + 1)
    Z = lambda v: ml + (v - zl) / (zh - zl) * (w - ml - mr)
    y = 128
    o.append(f'<text class="c-axis" x="{ml - 8}" y="{y + 11}" text-anchor="end">zoomed</text>')
    o.append(f'<rect class="c-grid-band" x="{Z(lo):.1f}" y="{y - 4}" width="{Z(hi) - Z(lo):.1f}" height="22" rx="4"/>')
    o.append(f'<line class="c-grid" x1="{ml}" y1="{y + 7}" x2="{w - mr}" y2="{y + 7}"/>')
    o.append(f'<circle class="c-mark" cx="{Z(tot):.1f}" cy="{y + 7}" r="6" style="stroke:var(--s-tim);fill:var(--s-tim)"/>')
    for t in range(zl, zh + 1):
        o.append(f'<text class="c-tick" x="{Z(t):.1f}" y="{y + 34}" text-anchor="middle">{t}</text>')
    o.append(f'<text class="c-note" x="{Z(lo) + 4:.1f}" y="{y - 8}">published band {lo:g} to {hi:g}</text>')
    o.append(f'<text class="c-axis" x="{ml + (w - ml - mr) / 2:.0f}" y="{h - 4}" text-anchor="middle">µs per day, Moon clock against Earth\'s geoid</text>')
    o.append("</svg>")
    return "".join(o)




def fig_value(cid, path, digits):
    v = CHARTS[cid]
    for k in path.split("."):
        v = v[k] if not isinstance(v, list) else v[int(k)]
    if isinstance(v, (int, float)):
        return f"{v:.{int(digits)}f}" if digits else f"{v:g}"
    return esc(v)


def expand(text, root):
    text = FIG_RE.sub(lambda m: fig_html(root, m.group(2), int(m.group(3)), int(m.group(4)), m.group(5) or "", m.group(1)), text)
    text = re.sub(r'<p class="prov([^"]*)" data-prov="([\w-]+)"(?: data-prov-lead="([^"]+)")?></p>',
                  lambda m: f'<p class="prov{m.group(1)}">{prov_html(root, m.group(2), m.group(3))}</p>', text)
    text = re.sub(r"\{\{FIG ([\w-]+) ([\w.]+) (\d+)\}\}", lambda m: fig_value(m.group(1), m.group(2), m.group(3)), text)
    return text


# =================================================================== generated blocks
def code_btn(cmd, dark=False):
    return (f'<button class="cmd{" dark" if dark else ""}" type="button" data-copy="{esc(cmd)}" aria-label="Copy: {esc(cmd)}">'
            f'<span class="pr">$</span><span class="cmd-t">{esc(cmd)}</span><span class="cp"><svg width="14" height="14" aria-hidden="true"><use href="#i-copy"/></svg></span></button>')


def installer(root, compact=False):
    tabs, panels = [], []
    order = ["browser", "cli", "python", "npm", "mcp", "docker", "rust", "jetbrains"] if not compact else ["browser", "cli", "python", "npm", "mcp"]
    for i, cid in enumerate(order):
        ch = next(c for c in channels if c["id"] == cid)
        tabs.append(f'<button role="tab" type="button" id="it-{"c" if compact else "f"}-{cid}" aria-controls="ip-{"c" if compact else "f"}-{cid}" data-ch="{cid}" aria-selected="{"true" if i == 0 else "false"}" tabindex="{0 if i == 0 else -1}">{esc(ch["name"])}</button>')
        body = [f'<p class="ip-aud">For {esc(ch["audience"])}.</p>']
        if ch["command"]:
            body.append(f'<div class="ip-cmd" data-latest="{esc(ch["command"])}" data-pinned="{esc(ch["pinned"])}">{code_btn(ch["command"])}</div>')
            if ch.get("verify"):
                body.append(f'<p class="ip-verify"><span>Then check it</span><code>{esc(ch["verify"])}</code></p>')
        if cid == "browser":
            body.append(f'<div class="ip-cmd"><a class="btn btn-ink" href="{root}studio/index.html">{esc(STUDIO_CTA)} <svg aria-hidden="true"><use href="#i-arrow"/></svg></a></div>')
        if ch.get("note"):
            body.append(f'<p class="ip-note">{esc(ch["note"])}</p>')
        if cid == "mcp" and not compact:
            body.append(f'<p class="ip-note">Registry name <code>{esc(ch["registry_name"])}</code>. Register it with your client, from the kshana-mcp README:</p>')
            body.append('<div class="ip-configs">' + "".join(
                f'<div class="cfg"><div class="cfg-h"><span>{"Local binary" if j == 0 else "Docker image"}</span><button class="tbtn" type="button" data-copy="{esc(cfg)}">Copy</button></div><pre><code>{esc(cfg)}</code></pre></div>'
                for j, cfg in enumerate(ch["configs"])) + "</div>")
        reg = ch["registry"] or "studio/index.html"
        reg = reg if reg.startswith("http") else root + reg
        docs = ch["docs"] if ch["docs"].startswith("http") else root + ch["docs"]
        body.append(f'<p class="ip-links"><a href="{esc(reg)}">{esc(ch["registry_label"])}</a><a href="{esc(docs)}">{esc(ch["docs_label"])}</a></p>')
        panels.append(f'<div class="ip" role="tabpanel" id="ip-{"c" if compact else "f"}-{cid}" aria-labelledby="it-{"c" if compact else "f"}-{cid}"{"" if i == 0 else " hidden"}>{"".join(body)}</div>')
    pin = (f'<div class="pin-toggle" role="group" aria-label="Version"><button type="button" data-pin="latest" aria-pressed="true">Latest</button>'
           f'<button type="button" data-pin="pinned" aria-pressed="false">Pin v{esc(version)}</button></div>')
    return (f'<div class="installer{" compact" if compact else ""}" data-installer>'
            f'<div class="inst-top"><div class="inst-tabs" role="tablist" aria-label="Choose how to install">{"".join(tabs)}</div>{pin}</div>'
            f'{"".join(panels)}</div>')


def sector_teasers(root):
    return "\n".join(f'<a class="st-card" href="{root}missions.html#{s["id"]}" style="--c:var({s["c"]})"><i></i><b>{esc(s["n"])}</b><span>{esc(trunc(s["q"], 96))}</span></a>' for s in SECTORS)


def roles(root):
    out = []
    for n, c, p, (el, href), cmd, cue in ROLES:
        snip = (f'<a class="tbtn" href="{esc(cmd)}">Open</a>' if cmd.startswith("http") else f'<button class="tbtn" type="button" data-copy="{esc(cmd)}">Copy</button>')
        cue_t = PRO[cue] if cue else ("Free: the whole engine, citable by DOI." if "pip" in cmd else f"Free: {len(mcp_tools)} MCP tools, from run_scenario to export_oem.")
        out.append(f'<article class="role" style="--c:var({c})"><h3>{esc(n)}</h3><p>{esc(p)}</p><div class="entry">Start at <a href="{root}{href}">{esc(el)}</a></div>'
                   f'<div class="snip"><code>{esc(cmd)}</code>{snip}</div><div class="cue"><b>Free today:</b> everything in the open engine. {esc(cue_t)}</div></article>')
    return "\n".join(out)







def matrix_cells():
    S = {"VALIDATED": "v", "MODELLED": "m", "PARTNER": "p"}
    return "".join(f'<i class="c-{S[r["status"]]}" title="{esc(trunc(r["capability"].replace("`", ""), 90))} · {r["status"].title()}"></i>' for r in vm["rows"])




def pub_bibtex(p):
    """A paper's BibTeX entry (arXiv's own fields), as highlighted HTML."""
    auth = " and ".join(f"{a.split()[-1]}, {' '.join(a.split()[:-1])}" for a in p["authors"])
    # the title is braced inside quotes (keeps its capitals; a doubled brace would read as a page placeholder)
    rows = [("title", '"{' + p["title"] + '}"'), ("author", auth), ("year", p["published"][:4]), ("eprint", p["arxiv"]),
            ("archivePrefix", "arXiv"), ("primaryClass", p["category"]), ("doi", p["doi"]), ("url", f"https://arxiv.org/abs/{p['arxiv']}")]
    w = max(len(k) for k, _ in rows)
    body = ",\n".join(f'  <span class="k">{k}</span>{" " * (w - len(k))} = {esc(v) if v.startswith(chr(34)) else "{" + esc(v) + "}"}' for k, v in rows)
    return f'<span class="k">@misc</span>{{<span class="v">{esc(p["bibkey"])}</span>,\n{body}\n}}'


def pub_date(d):
    y, m, dd = d.split("-")
    return f"{int(dd)} {['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'][int(m) - 1]} {y}"


def pub_cmd(c):
    return (f'<button class="cmd pub-cmd" type="button" data-copy="{esc(c)}" aria-label="Copy: {esc(c)}"><span class="pr">$</span>'
            f'<span class="cmd-t">{esc(c)}</span><span class="cp"><svg width="14" height="14" aria-hidden="true"><use href="#i-copy"/></svg></span></button>')


# Research areas: an inline line icon, a name and an accent (theme tokens: --acc for text and the
# card's rule, --acc-s for the chart line on the dark chart panel). css/evidence.css .pub-<area>.
PUB_AREAS = {
    "lunar": ("Lunar navigation", '<path d="M15.5 4.2A8 8 0 1 0 19.8 15 6.4 6.4 0 0 1 15.5 4.2Z"/><path d="M3 18.5c3.5 1.9 12.3 1.4 18-4.6" stroke-dasharray="2 2.4"/>'),
    "timing": ("Timing integrity", '<circle cx="12" cy="13" r="7.5"/><path d="M12 9v4.2l2.8 1.8M9.5 2.8h5M12 2.8v2.7"/>'),
    "resilience": ("Resilience scoring", '<path d="M12 3 5 6v5.2c0 4.3 2.9 8 7 9.8 4.1-1.8 7-5.5 7-9.8V6Z"/><path d="m8.8 12.2 2.3 2.3 4.3-4.6"/>'),
    "rf": ("Radio-frequency interference", '<path d="M12 13v8M9.5 21h5"/><circle cx="12" cy="11" r="1.6"/><path d="M8.3 7.3a5.2 5.2 0 0 0 0 7.4M15.7 7.3a5.2 5.2 0 0 1 0 7.4M5.5 4.5a9.2 9.2 0 0 0 0 13M18.5 4.5a9.2 9.2 0 0 1 0 13"/>'),
}


def _nice(v):
    """A round axis maximum at or above v, and its tick step."""
    e = 10 ** math.floor(math.log10(v))
    for m in (1, 1.2, 1.5, 2, 2.5, 3, 4, 5, 6, 8, 10):
        if m * e >= v:
            top = m * e
            break
    step = {1: .2, 1.2: .4, 1.5: .5, 2: .5, 2.5: .5, 3: 1, 4: 1, 5: 1, 6: 2, 8: 2, 10: 2}[round(top / e, 1)] * e
    return top, step


def _fmt(v, unit=""):
    if unit == "%":
        return f"{v:g}%"
    if unit == "m":
        return f"{v:,.0f} m" if v >= 100 else f"{v:.1f} m"
    return f"{v:g}"


def _ns(v):
    for d, u in ((1e6, "ms"), (1e3, "µs"), (1, "ns")):
        if v >= d:
            x = v / d
            return f"{x:.2f} {u}" if u == "ms" and x < 10 and abs(x - round(x)) > 1e-9 else f"{x:.0f} {u}"
    return f"{v:g} ns"


def pub_chart(p, W=360, H=160):
    """The paper's headline result as an inline SVG in the site's chart style (the dark chart panel,
    c-* classes). Every number is the engine's (publications.json: figure); axes are drawn to scale."""
    f = p["figure"]
    L, R, T, B = 12, 16, 12, 30
    out = [f'<rect class="c-bg" width="{W}" height="{H}"/>']
    if f["type"] == "hbar":
        items, unit, log = f["items"], f.get("unit", ""), f["scale"] == "log"
        vals = [i["value"] for i in items]
        if log:
            lo, hi = 10 ** math.floor(math.log10(min(vals))), 10 ** math.ceil(math.log10(max(vals)))
            X = lambda v: L + (math.log10(v) - math.log10(lo)) / (math.log10(hi) - math.log10(lo)) * (W - L - R)
            ticks = [10 ** k for k in range(int(math.log10(lo)), int(math.log10(hi)) + 1)]
            x0 = L
        else:
            hi, step = _nice(max(vals) * 1.08)
            X = lambda v: L + v / hi * (W - L - R)
            ticks = [round(k * step, 6) for k in range(int(round(hi / step)) + 1)]
            x0 = X(0)
        row = (H - T - B) / len(items)
        inline = len(items) > 3  # many bars: the label sits left of its bar, on the same line
        if inline:
            L0 = L
            L = L + 118
            if log:
                X = lambda v: L + (math.log10(v) - math.log10(lo)) / (math.log10(hi) - math.log10(lo)) * (W - L - R)
                x0 = L
            else:
                X = lambda v: L + v / hi * (W - L - R)
                x0 = X(0)
        for t in ticks:
            anc = "start" if X(t) < L + 12 else "end" if X(t) > W - R - 12 else "middle"
            out.append(f'<line class="c-grid" x1="{X(t):.1f}" x2="{X(t):.1f}" y1="{T}" y2="{H - B}"/>'
                       f'<text class="c-tick" x="{X(t):.1f}" y="{H - B + 14}" text-anchor="{anc}">{esc(_fmt(t, unit) if not log else (f"{t:,.0f} m" if t >= 1 else f"{t:g} m"))}</text>')
        for k, it in enumerate(items):
            y = T + k * row
            x1 = X(it["value"])
            val = _fmt(it["value"], unit)
            inside = x1 - x0 > (W - L - R) * .72
            if inline:
                bh = min(12, row * .55)
                by = y + (row - bh) / 2
                out.append(f'<text class="c-axis" x="{L0}" y="{by + bh * .8:.1f}">{esc(it["label"])}</text>')
            else:
                bh = min(14, row * .34)
                by = y + row * .52
                out.append(f'<text class="c-axis" x="{L}" y="{y + row * .34:.1f}">{esc(it["label"])}</text>')
            out.append(f'<rect class="pb" x="{x0:.1f}" y="{by:.1f}" width="{max(x1 - x0, 1.5):.1f}" height="{bh:.1f}" rx="2"><title>{esc(it["label"])}: {esc(val)}{(" · " + esc(it["note"])) if it.get("note") else ""}</title></rect>'
                       f'<text class="{"pv pv-in" if inside else "pv"}" x="{(x1 - 5) if inside else (x1 + 5):.1f}" y="{by + bh * .8:.1f}" text-anchor="{"end" if inside else "start"}">{esc(val)}</text>')
        if log:
            out.append(f'<text class="c-note" x="{W - R}" y="{T + 8}" text-anchor="end">log scale</text>')
    elif f["type"] == "tpl":
        L = 44
        pts, refs = f["points"], f["refs"]
        lo, hi = 10, 10 ** math.ceil(math.log10(max(r["value"] for r in refs) * 1.5))
        xmax = max(x for x, _ in pts)
        X = lambda v: L + v / xmax * (W - L - R)
        Y = lambda v: T + (math.log10(hi) - math.log10(v)) / (math.log10(hi) - math.log10(lo)) * (H - T - B)
        for k in range(int(math.log10(lo)), int(math.log10(hi)) + 1):
            out.append(f'<line class="c-grid" x1="{L}" x2="{W - R}" y1="{Y(10 ** k):.1f}" y2="{Y(10 ** k):.1f}"/><text class="c-tick" x="{L - 5}" y="{Y(10 ** k) + 3.5:.1f}" text-anchor="end">{_ns(10 ** k)}</text>')
        for x in [0] + [x for x, _ in pts]:
            if x in (xmax, 10, 30):
                out.append(f'<text class="c-tick" x="{X(x):.1f}" y="{H - B + 14}" text-anchor="middle">{x:g} s</text>')
        pull, claim = refs
        out.append(f'<line class="c-thr" x1="{L}" x2="{W - R}" y1="{Y(pull["value"]):.1f}" y2="{Y(pull["value"]):.1f}"/>'
                   f'<text class="c-thr-t" x="{W - R}" y="{Y(pull["value"]) + 13:.1f}" text-anchor="end">served error {_ns(pull["value"])}, receiver unaware</text>'
                   f'<line class="pref" x1="{L}" x2="{W - R}" y1="{Y(claim["value"]):.1f}" y2="{Y(claim["value"]):.1f}"/>'
                   f'<text class="pref-t" x="{W - R}" y="{Y(claim["value"]) + 12:.1f}" text-anchor="end">accuracy it claimed ≤ {_ns(claim["value"])}</text>')
        d = " ".join(f'{"M" if i == 0 else "L"}{X(x):.1f},{Y(v):.1f}' for i, (x, v) in enumerate(pts))
        out.append(f'<path class="c-line pd" pathLength="1" d="{d}"/>')
        for i, (x, v) in enumerate(pts):
            out.append(f'<circle class="pp" cx="{X(x):.1f}" cy="{Y(v):.1f}" r="2.6"><title>Timing Protection Level {v:g} ns at {x:g} s</title></circle>')
        (x_a, v_a), (x_b, v_b) = pts[0], pts[-1]
        out.append(f'<text class="pv" x="{X(x_a) + 4:.1f}" y="{Y(v_a) - 7:.1f}">protection level {v_a:g} ns</text>'
                   f'<text class="pv" x="{X(x_b) - 2:.1f}" y="{Y(v_b) - 7:.1f}" text-anchor="end">{v_b:g} ns</text>')
    elif f["type"] == "scatter":
        L, B = 40, 30
        pts = f["points"]
        vs = [v for q in pts for v in q]
        lo = math.floor(min(vs) * 20) / 20
        hi = math.ceil(max(vs) * 20) / 20
        X = lambda v: L + (v - lo) / (hi - lo) * (W - L - R)
        Y = lambda v: T + (hi - v) / (hi - lo) * (H - T - B)
        k = lo
        while k <= hi + 1e-9:
            out.append(f'<line class="c-grid" x1="{X(k):.1f}" x2="{X(k):.1f}" y1="{T}" y2="{H - B}"/><line class="c-grid" x1="{L}" x2="{W - R}" y1="{Y(k):.1f}" y2="{Y(k):.1f}"/>'
                       + (f'<text class="c-tick" x="{X(k):.1f}" y="{H - B + 13}" text-anchor="middle">{k:.2f}</text>' if k > lo + 1e-9 else "")
                       + f'<text class="c-tick" x="{L - 5}" y="{Y(k) + 3.5:.1f}" text-anchor="end">{k:.2f}</text>')
            k = round(k + .05, 4)
        out.append(f'<line class="c-eq" x1="{X(lo):.1f}" y1="{Y(lo):.1f}" x2="{X(hi):.1f}" y2="{Y(hi):.1f}"/>')
        out.append('<g class="pp">' + "".join(f'<circle cx="{X(a):.1f}" cy="{Y(b):.1f}" r="1.9"/>' for a, b in pts) + "</g>")
        out.append(f'<text class="pv" x="{L + 8}" y="{T + 14}">R² {f["r2"]:.2f}, detector left out</text>'
                   f'<text class="c-note" x="{W - R}" y="{H - 3}" text-anchor="end">predicted →</text><text class="c-note" x="{L + 8}" y="{T + 28}">↑ actual</text>')
    return (f'<svg class="chart pub-svg" viewBox="0 0 {W} {H}" role="img" aria-label="{esc(f["axis"])}" xmlns="http://www.w3.org/2000/svg">' + "".join(out) + "</svg>")


def pub_figure(p, featured=False):
    f, r = p["figure"], p["reproduce"]
    st = {"VALIDATED": ("v", "Validated"), "MODELLED": ("m", "Modelled"), "PARTNER": ("p", "Partner")}[f["label"]]
    src_ = (f'<code>{esc(os.path.basename(f["source"]))}</code>' if f["source"].endswith(".toml") else f'<code>{esc(f["source"])}</code>')
    seed = f"seed {esc(str(f['seed']))}" if f.get("seed") not in (None, "") else "deterministic, no seed"
    studio = studio_open("", r["studio"]) if "studio" in r else ""
    return (f'<figure class="pub-fig" data-anim><div class="pub-plot">{pub_chart(p, 520, 156)}</div>'
            f'<figcaption class="prov pub-prov"><span class="st st-{st[0]}" title="{esc(f["capability"])}: verification-matrix label">{st[1]}</span><span>Engine v{esc(PUB_ENGINE["version"])} · {src_} · {seed}</span>{studio}</figcaption>'
            '</figure>')


def pubs():
    """Evidence #research: one card per paper live on arXiv, with a figure from a real engine run and
    layered detail (key findings, method, commands, citation) in native disclosures."""
    out = []
    for n, p in enumerate(PUBS):
        a, r = p["arxiv"], p["reproduce"]
        slug = a.replace(".", "-")
        featured = n == 0 and len(PUBS) % 2 == 1  # css/evidence.css spans this card (odd count)
        area, icon = PUB_AREAS[p["area"]]
        if "kind" in r:
            what = f'Runs the scenario kind <code>{esc(r["kind"])}</code> (<code>{esc(r["file"])}</code>) with the installed engine.'
            studio = studio_open("", r["studio"])
        else:
            what = f'Regenerates the study artifact from <code>{esc(r["example"])}</code>, in a clone of the repository.'
            studio = ""
        cmds = "".join(pub_cmd(c) for c in r["command"].split("\n"))
        ver = f' · version {p["version"]}' if p["version"] > 1 else ""
        finds = "".join(f"<li>{esc(x)}</li>" for x in p["findings"])
        out.append(f'''<article class="glass pub pub-{p["area"]} rv" id="pub-{slug}">
  <div class="pub-a"><p class="pub-meta"><span class="pub-area"><svg class="pub-ic" viewBox="0 0 24 24" aria-hidden="true">{icon}</svg>{esc(area)}</span><span><time datetime="{esc(p["published"])}">{pub_date(p["published"])}</time> · arXiv:{esc(a)}{ver}</span></p>
  <h3 class="card-h pub-t">{esc(p["title"])}</h3>
  <p class="pub-sum">{esc(p["summary"])} {abbr_key(p["title"], p["summary"]).replace("<small class=\"abbr-key\">", "<small class=\"abbr-key pub-ab\">")}</p>
  <p class="pub-links"><a class="tbtn" href="https://arxiv.org/abs/{esc(a)}">Abstract <svg aria-hidden="true"><use href="#i-ext"/></svg></a><a class="tbtn" href="https://arxiv.org/pdf/{esc(a)}">PDF <svg aria-hidden="true"><use href="#i-ext"/></svg></a><a class="pub-doi" href="https://doi.org/{esc(p["doi"])}">DOI {esc(p["doi"])}</a></p></div>
  {pub_figure(p, True) if featured else ""}
  <div class="pub-foot">
  {"" if featured else '<details class="pub-more pub-figd"><summary>Figure</summary>' + pub_figure(p) + "</details>"}
  <details class="pub-more"><summary>Key findings</summary><ul class="pub-find">{finds}</ul></details>
  <details class="pub-more"><summary>How Kshana reproduces it</summary><p class="pub-how">{esc(p["method"])}</p><p class="pub-how"><b>The figure.</b> {esc(p["figure"]["caption"])}</p></details>
  <details class="pub-more"><summary>Reproduce it</summary><div class="pub-rp"><p class="pub-rp-h">{what}</p>{cmds}{studio}</div></details>
  <details class="pub-more"><summary>Cite it</summary><div class="pub-bib"><div class="bib-h"><span>BibTeX · {esc(p["bibkey"])}</span><button class="tbtn" type="button" data-copy-from="bib-{slug}">Copy BibTeX</button></div><pre id="bib-{slug}">{pub_bibtex(p)}</pre></div></details>
  </div>
</article>''')
    return "\n".join(out)


def mcp_tools_html():
    # the description is read by screen readers only (Developers shows the names as chips), so it is kept whole
    return "\n".join(f'<div class="tool"><code>{esc(n)}</code><p>{esc_ic(re.sub(r"\s+", " ", d.replace(chr(92) + chr(34), chr(34))).strip())}</p></div>' for d, n in mcp_tools)


def mcp_demo(root):
    c = CHARTS["mcp-imu"]
    f = c["figures"]
    toml_head = "\n".join(file_toml["imu-deadreckoning"].splitlines()[:6])
    return f'''<div class="chat">
  <div class="chat-h"><span>What an assistant sends and receives</span><span class="real-tag">Real engine output</span></div>
  <div class="msg tool"><div class="tn"><i></i>request · run_scenario</div><pre><code>{{ "toml": "{esc(toml_head.replace(chr(10), "\\n"))}\\n…" }}</code></pre></div>
  <div class="msg res"><div class="tn"><i></i>response · first content block, the summary</div><p class="mono">{esc(c["summary"])}</p></div>
  <div class="msg res"><div class="tn"><i></i>response · from the result JSON</div>
    <table class="mini"><tr><th>Figure</th><th>{esc(f["classical"]["id"])}</th><th>{esc(f["quantum"]["id"])}</th></tr>
    {"".join(f'<tr><td>{esc(k)}</td><td>{f["classical"]["fom"][k]:.4g}</td><td>{f["quantum"]["fom"][k]:.4g}</td></tr>' for k in ("pos_rms_m", "pos_p95_m", "holdover_s") if isinstance(f["classical"]["fom"].get(k), (int, float)))}
    </table></div>
  <p class="prov">{prov_html(root, "mcp-imu")}</p>
</div>'''



def nav_links(page, root):
    return "\n".join(f'      <a href="{root}{href}"{" aria-current=\"page\"" if key == page else ""}>{label}</a>' for key, label, href in NAV)


IMAGERY_CREDIT = ("Imagery: NASA Visible Earth (Blue Marble, Black Marble), NASA Scientific Visualization Studio (CGI Moon Kit), NASA/JPL (Mars), "
                  "NASA, JPL and USGS mission mosaics (moons and Pluto); Sun, Mercury, Venus, Jupiter, Saturn, Uranus and Neptune maps: Solar System Scope, Creative Commons Attribution (CC BY) 4.0; "
                  "Deimos map: Oleg-Pluton, Creative Commons Attribution-ShareAlike (CC BY-SA) 3.0; coastlines: Natural Earth.")


cff = read("CITATION.cff")
cff_title = re.search(r'^title:\s*"([^"]+)"', cff, re.M).group(1)
cff_doi = re.search(r'^doi:\s*(\S+)', cff, re.M).group(1)
cff_family = re.search(r'family-names:\s*(\S+)', cff).group(1)
cff_given = re.search(r'given-names:\s*(\S+)', cff).group(1)
cff_url = re.search(r'^url:\s*"([^"]+)"', cff, re.M).group(1)
cff_lic = re.search(r'^license:\s*(\S+)', cff, re.M).group(1)
BIBTEX = (f'<span class="k">@software</span>{{<span class="v">kshana</span>,\n  <span class="k">title</span>   = {{{esc(cff_title)}}},\n  <span class="k">author</span>  = {{{esc(cff_family)}, {esc(cff_given)}}},\n'
          f'  <span class="k">version</span> = {{{version}}},\n  <span class="k">year</span>    = {{2026}},\n  <span class="k">doi</span>     = {{{esc(cff_doi)}}},\n  <span class="k">url</span>     = {{{esc(cff_url)}}},\n  <span class="k">license</span> = {{{esc(cff_lic)}}}\n}}')
APA = f'{esc(cff_family)}, {esc(cff_given[0])}. (2026). <i>{esc(cff_title)}</i> (Version {version}). Ashforde OÜ. https://doi.org/{esc(cff_doi)}'


def lunar_classes():
    rows = CHARTS["clk-lunar-classes"]["figures"]
    return ('<thead><tr><th scope="col">Clock</th><th scope="col">σ<sub>y</sub>(1 s)</th><th scope="col">Time error after a day</th><th scope="col">Noise type</th></tr></thead><tbody>'
            + "".join(f'<tr><td>{esc(r["clock"])}</td><td class="num">{r["sigma_y_one_s"]:.2e}</td><td class="num">{r["x_one_day_s"] * 1e9:.3g} ns</td><td>{esc(r["noise_type"])}</td></tr>' for r in rows) + "</tbody>")


def telecom_verdicts():
    m = CHARTS["tw-mtie"]["model"]
    out = [f'<span class="verdict {"pass" if x["verdict"] == "PASS" else "fail"}"><b>{esc(x["verdict"])}</b>{esc(x["title"] or x["id"])}</span>' for x in m["masks"]]
    e = m.get("envelope")
    if e:
        out.append(f'<span class="verdict {"pass" if e["verdict"] == "PASS" else "fail"}"><b>{esc(e["verdict"])}</b>Holdover time-error envelope, {esc(e["source"])}</span>')
    return "".join(out)


def fmt_dur(s):
    return f"{s / 3600:.1f} h" if s >= 5400 else f"{s / 60:.0f} min"


def telecom_budgets():
    b = CHARTS["tw-telecom-budgets"]["figures"]["budgets"]
    return ('<thead><tr><th scope="col">Budget</th><th scope="col">Limit</th><th scope="col">In holdover</th></tr></thead><tbody>'
            + "".join(f'<tr><td>{esc(x["name"])}<br><small>{esc(x["source"])}</small></td><td class="num">{x["max_abs_te_ns"]:g} ns</td><td>{"exceeded after " + fmt_dur(x["time_to_exceed_s"]) if x["exceeded"] else "held for the whole run"}</td></tr>' for x in b) + "</tbody>")


def slot_text():
    f = CHARTS["slot-result"]["figures"]
    r, sl = f["result"], f["slot"]
    return (f'After its last GNSS fix the oscillator stays inside the {sl["guard_ns"]:g} ns slot guard for {r["breach_after_sync_s"]:.0f} s, so it needs a fresh fix about every '
            f'{r["resync_interval_s"]:.0f} s, {r["fixes_per_day"]:.0f} times a day. The dominant term at the breach is {esc(r["dominant_term"])}.')


def tag_list(fs):
    return "".join(f'<a class="file-chip" href="{file_href("", f)}"><code>{esc(f)}</code><small>{esc(file_kind[f])}</small></a>' for f in fs if f in files)


LUNAR_KINDS = dict(GROUPS_BY_NAME)["Lunar and cislunar"]


C = 2 * 3.141592653589793 * 170
EXTRA_SCALARS = {
    "{{LUNAR_CLASSES}}": lunar_classes(), "{{TELECOM_OSC}}": esc(CHARTS["tw-telecom-budgets"]["figures"]["oscillator"]["model"]),
    "{{TELECOM_VERDICTS}}": telecom_verdicts(), "{{TELECOM_BUDGETS}}": telecom_budgets(), "{{SLOT_TEXT}}": slot_text(),
    "{{JR_EARTH}}": tag_list(["orbit-sgp4-gps", "orbit-gnss-challenged", "orbit-molniya", "slot-timing-ocxo-leo", "passes", "ephemeris"]),
    "{{JR_MOON}}": tag_list([f for f in files if file_kind[f] in LUNAR_KINDS]),
    "{{JR_MARS}}": tag_list([f for f in files if file_kind[f] == "mars-pnt"]),
    "{{BIBTEX}}": BIBTEX, "{{APA}}": APA,
    "{{RING_V}}": f'{C * summ["validated"] / summ["total"]:.1f}', "{{RING_P}}": f'{C * summ["partner_owned"] / summ["total"]:.1f}',
    "{{RING_P_OFF}}": f'{C * (summ["validated"] + summ["modelled"]) / summ["total"]:.1f}',
    "{{MCP_CRATE}}": esc(mcp_crate), "{{MCP_REG}}": esc(server_json["name"]), "{{JB_URL}}": esc(jb),
}


# =================================================================== catalogues (visible, from source)
def clock_catalogue():
    cs = read("src/clock_state.rs")
    ids = re.findall(r'ClockClass::(\w+) => "([a-z]+)",', cs[cs.index("pub fn id(self)"):])
    adev = dict(re.findall(r'ClockClass::(\w+) => ([\d.e-]+),', cs[cs.index("pub fn adev_1s(self)"): cs.index("pub const ALL: [ClockClass")]))
    names = {"csac": "chip-scale atomic clock", "uso": "ultra-stable quartz oscillator", "dsac": "deep space atomic clock", "tcxo": "temperature-compensated crystal oscillator",
             "ocxo": "oven-controlled crystal oscillator", "rafs": "rubidium atomic frequency standard"}
    rows = "".join(f'<tr><td><code>{esc(i)}</code></td><td>{esc(names.get(i, ""))}</td><td class="num">{float(adev[c]):.1e}</td></tr>' for c, i in ids if c in adev)
    models = {}
    for f in files:
        for m in re.finditer(r'^\[(?:[\w.]+\.)?(clock[\w]*)\]\s*\n(?:[^\[]*?\n)?id\s*=\s*"([^"]+)"\s*\nprovenance\s*=\s*"([^"]+)"', file_toml[f], re.M):
            models.setdefault(m.group(2), (f, m.group(3)))
    mrows = "".join(f'<tr><td><code>{esc(i)}</code></td><td>{esc(trunc(pv.split(":")[0].split(";")[0], 80))}</td><td><a class="file-chip" href="{file_href("", f)}"><code>{esc(f)}</code></a></td></tr>' for i, (f, pv) in sorted(models.items()))
    assert len(ids) == 6 and len(models) >= 5, (ids, models)
    return (f'<div class="cat"><h3 class="card-h">Clock classes the engine selects by id</h3><div class="tbl-wrap"><table class="mini"><thead><tr><th scope="col">Class id</th><th scope="col">Clock</th><th scope="col">σ<sub>y</sub>(1 s)</th></tr></thead><tbody>{rows}</tbody></table></div>'
            f'<p class="src-note">From <code>src/clock_state.rs</code>. The <code>clock</code> scenario kind runs any of them against a spec.</p></div>'
            f'<div class="cat"><h3 class="card-h">Clock models in the bundled scenarios</h3><div class="tbl-wrap"><table class="mini"><thead><tr><th scope="col">Model id</th><th scope="col">What it is</th><th scope="col">Scenario</th></tr></thead><tbody>{mrows}</tbody></table></div>'
            f'<p class="src-note">Each model id and its provenance are read from the scenario files.</p></div>')


def timing_catalogue():
    tt = read("src/telecom_timing.rs")
    pre = re.findall(r'OscillatorPreset \{\s*id: "([^"]+)",\s*class: "([^"]+)",\s*model: "([^"]+)"', tt)
    masks = re.findall(r'TimingMask \{\s*id: "([^"]+)",\s*title: "([^"]+)"', tt)
    assert len(pre) == 4 and len(masks) >= 10, (pre, masks)
    prow = "".join(f'<tr><td><code>{esc(i)}</code></td><td>{esc(c)}</td><td>{esc(m)}</td></tr>' for i, c, m in pre)
    mrow = "".join(f'<tr><td><code>{esc(i)}</code></td><td>{esc(t)}</td></tr>' for i, t in masks)
    return (f'<div class="cat-grid"><div class="cat"><h3 class="card-h">Oscillator presets</h3><div class="tbl-wrap"><table class="mini"><thead><tr><th scope="col">Preset id</th><th scope="col">Class</th><th scope="col">Device</th></tr></thead><tbody>{prow}</tbody></table></div></div>'
            f'<div class="cat"><h3 class="card-h">ITU-T masks and budgets</h3><div class="tbl-wrap"><table class="mini"><thead><tr><th scope="col">Mask id</th><th scope="col">Title</th></tr></thead><tbody>{mrow}</tbody></table></div></div></div>'
            f'<p class="src-note">Both tables are read from <code>src/telecom_timing.rs</code>; the <code>telecom-timing</code> kind checks a clock against any mask.</p>')


def repro_stamp():
    rp = CHARTS["pro-repro"]["figures"]
    return (f'<div class="repro"><dl><dt>scenario_hash</dt><dd><code>{esc(rp["scenario_hash"])}</code></dd><dt>seed</dt><dd><code>{esc(rp["seed"])}</code></dd>'
            f'<dt>engine_version</dt><dd><code>{esc(rp["engine_version"])}</code></dd><dt>schema_version</dt><dd><code>{esc(rp["schema_version"])}</code></dd></dl>'
            f'<p class="prov">{prov_html("", "pro-repro")}</p></div>')


def cli_reference():
    main_rs = read("src/main.rs")
    usage = re.search(r'const USAGE: &str = "(.*?)";', main_rs, re.S).group(1)
    lines = [re.sub(r"^(usage|\s+or):\s*", "", l).strip() for l in usage.split("\n")]
    outs = []
    for ext in re.findall(r'with_extension\("([a-z.]+)"\)', main_rs):
        if ext not in outs: outs.append(ext)
    # Which flags the binary takes (its own match arms), so a sentence below never names a flag it lacks.
    flags = set(re.findall(r'^\s*"(--[a-z0-9-]+)"(?:\s*\|\s*"[^"]+")*\s*=>', main_rs, re.M))
    every = [o for o in outs if not o.endswith(("sp3", "omm", "oem")) and not o.startswith(("animation.", "frames", "study."))]
    anim = [o for o in outs if o.startswith(("animation.", "frames"))]
    fmts = re.search(r"--export <([a-z|]+)>", usage)
    parts = [f'A run writes {", ".join(f"<code>{esc(o)}</code>" for o in every)} next to the scenario']
    for fl in ("--export-sp3", "--export-omm", "--export-oem", "--eop", "--study"):
        assert fl in flags, f"src/main.rs no longer takes {fl}"
    parts.append('<code>--export-sp3</code>, <code>--export-omm</code> and <code>--export-oem</code> write orbit files')
    if "--animate" in flags and anim:
        parts.append(f'<code>--animate</code> also writes the run as something that moves ({", ".join(f"<code>{esc(o)}</code>" for o in anim)}: an animated SVG, a self-contained HTML player or a folder of numbered frames), '
                     'with <code>--animate-fps</code> and <code>--animate-duration</code> for the playback rate and length')
    if "--export" in flags and fmts:
        names = {"czml": "CZML (Cesium Language)", "kml": "KML (Keyhole Markup Language)", "geojson": "GeoJSON (geographic JSON)", "stk": "an STK (Systems Tool Kit) ephemeris file", "sigmf": "SigMF (Signal Metadata Format)"}
        got = [f for f in fmts.group(1).split("|") if f not in ("all", "list")]
        assert all(f in names for f in got), f"--export format without a spelled-out name: {got}"
        parts.append(f'<code>--export</code> writes the scenario\'s geometry or its synthesised signal for other tools: {", ".join(names[f] for f in got)}')
    if "--import-route" in flags:
        parts.append('<code>--import-route</code> reads a route drawn as a GeoJSON line')
    parts.append('<code>--eop</code> injects Earth-orientation parameters from a finals2000A file')
    parts.append('<code>--study</code> runs a free suite of scenarios into <code>study.json</code> and <code>study.html</code>')
    parts.append('<code>kshana kinds --json</code> lists every kind with its fields')
    parts.append('<code>kshana example</code> prints a scenario to start from')
    return (f'<div class="cli"><h3 class="card-h">Command-line reference, from the binary\'s own usage text</h3><pre class="cli-pre"><code>{esc(chr(10).join(lines))}</code></pre>'
            f'<p class="src-note">{"; ".join(parts)}. Results can travel in the Kshana Interchange Format (KIF) envelope. The MCP server exposes the orbit exports as tools.</p></div>')

def ss_bodies():
    """The bodies of the recorded solar-system run, by name, as the picker's content before the page
    script draws its thumbnails (js/capabilities.mjs replaces it). Names come from the run's result."""
    bodies = json.load(open(os.path.join(HERE, "assets", "capabilities", "capabilities.json"), encoding="utf-8"))["solar"]["bodies"]
    return "".join(f'<span class="ss-nojs" role="option" aria-selected="false">{esc(b["name"])}</span>' for b in bodies)


EXTRA_SCALARS["{{SS_BODIES}}"] = ss_bodies()
EXTRA_SCALARS.update({"{{REPRO_STAMP}}": repro_stamp(), "{{CLOCK_CATALOGUE}}": clock_catalogue(), "{{TIMING_CATALOGUE}}": timing_catalogue(), "{{CLI_REFERENCE}}": cli_reference()})

# =================================================================== shared filter-list toolbar
def fl_bar(label, placeholder, facets=(), extra=""):
    """Toolbar for a [data-flist] block (site.js): a search box and single-choice facet groups.
    facets: [(key, aria label, [(value, text), ...]), ...]; value "" means all."""
    segs = "".join(
        f'<div class="fl-seg" role="group" aria-label="{esc(al)}" data-fl-key="{k}">'
        + "".join(f'<button type="button" data-v="{esc(v)}" aria-pressed="{"true" if not v else "false"}">{t}</button>' for v, t in opts) + "</div>"
        for k, al, opts in facets)
    return (f'<div class="fl-bar">{segs}<label class="fl-q"><span class="vh">{esc(label)}</span>'
            f'<svg aria-hidden="true"><use href="#i-search"/></svg><input type="search" data-fl-q placeholder="{esc(placeholder)}"></label>{extra}'
            f'<output class="fl-count" data-fl-count aria-live="polite"></output></div>')


def fl_foot(total, noun, limit):
    more = f'<button class="fl-more" type="button" data-fl-more>Show all {total} {esc(noun)}</button>' if limit else ""
    return f'<p class="fl-empty" data-fl-empty hidden>Nothing matches that filter.</p>{more}'


# =================================================================== band atlas (#17)
def _stack(marks, lo, hi):
    """Label rows for a frequency axis. A box first slides off any other mark's leader line when
    it can; boxes that still cover another mark's leader are placed last, so they sit above it
    and no leader crosses a box. Each box takes the lowest row where it touches no other box."""
    xs = [m["x"] for m in marks]
    for mk in marks:
        wdt = len(mk["text"]) * 6.7 + 16
        left = min(max(mk["x"] - wdt / 2, lo), hi - wdt)
        for ob in sorted(x for x in xs if x != mk["x"]):
            if left - 4 <= ob <= left + wdt + 4:
                cand = [c for c in ((ob - 6 - wdt) if ob > mk["x"] else None, (ob + 6) if ob < mk["x"] else None) if c is not None and lo <= c <= hi - wdt]
                if cand:
                    left = cand[0]
        mk.update(left=left, wdt=wdt, covers=sum(1 for x in xs if x != mk["x"] and left - 4 <= x <= left + wdt + 4))
    rows = []
    for mk in sorted(marks, key=lambda m: (m["covers"], m["x"])):
        a, b = mk["left"], mk["left"] + mk["wdt"]
        r = next((i for i, iv in enumerate(rows) if all(b + 8 <= l or a >= r_ + 8 for l, r_ in iv)), None)
        if r is None:
            rows.append([])
            r = len(rows) - 1
        rows[r].append((a, b))
        mk["row"] = r
    return max(1, len(rows))


def _band_axis_svg(marks, lo, hi, scale, ticks, title, w=1000, bracket=None, tail=None):
    """One frequency axis with stacked labels and leader lines. marks: {x (MHz), text, c, band}."""
    ml, mr = 24, 24 if not tail else 250
    X = lambda f: ml + scale(f) * (w - ml - mr)
    for mk in marks:
        mk["x"] = X(mk["f"])
    nrows = _stack(marks, ml, w - mr)
    row_h, top = 24, 30
    axis_y = top + nrows * row_h + 18
    h = axis_y + 44
    o = [f'<svg class="band-svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">',
         f'<rect class="c-bg" width="{w}" height="{h}" rx="0"/>',
         f'<text class="c-note" x="{ml}" y="18">{esc(title)}</text>',
         f'<line class="bs-axis" x1="{ml}" y1="{axis_y}" x2="{w - mr}" y2="{axis_y}"/>']
    for n, (f, lab) in enumerate(ticks):
        x = X(f)
        anc = "start" if n == 0 else ("end" if n == len(ticks) - 1 else "middle")
        o.append(f'<line class="c-grid" x1="{x:.1f}" y1="{axis_y - 4}" x2="{x:.1f}" y2="{axis_y + 4}"/><text class="c-tick" x="{x - 4 if anc == "start" else x + 4 if anc == "end" else x:.1f}" y="{axis_y + 18}" text-anchor="{anc}">{esc(lab)}</text>')
    if bracket:
        xa, xb = X(bracket[0]), X(bracket[1])
        o.append(f'<path class="bs-bracket" d="M{xa:.1f},{axis_y + 26} v6 H{xb:.1f} v-6"/><text class="c-note" x="{max(xa, ml):.1f}" y="{axis_y + 42}">{esc(bracket[2])}</text>')
    for mk in marks:
        ly = top + (nrows - 1 - mk["row"]) * row_h
        cx = mk["left"] + mk["wdt"] / 2
        o.append(f'<g class="bm" data-band="{mk["band"]}" style="--c:var({mk["c"]})">'
                 f'<polyline class="bm-lead" points="{mk["x"]:.1f},{axis_y - 5} {mk["x"]:.1f},{ly + 26} {cx:.1f},{ly + 20}"/>'
                 f'<circle class="bm-dot" cx="{mk["x"]:.1f}" cy="{axis_y}" r="4.5"/>'
                 f'<rect class="bm-box" x="{mk["left"]:.1f}" y="{ly}" width="{mk["wdt"]:.1f}" height="20" rx="6"/>'
                 f'<text class="bm-t" x="{cx:.1f}" y="{ly + 14}" text-anchor="middle">{esc(mk["text"])}</text></g>')
    if tail:
        x = w - mr + 24
        o.append(f'<g class="bm" data-band="{tail["band"]}" style="--c:var({tail["c"]})"><rect class="bm-box" x="{x}" y="{axis_y - 30}" width="{mr - 36}" height="44" rx="8"/>'
                 f'<text class="bm-t" x="{x + 12}" y="{axis_y - 12}">{esc(tail["text"])}</text><text class="c-tick" x="{x + 12}" y="{axis_y + 6}">{esc(tail["sub"])}</text></g>')
    o.append("</svg>")
    return "".join(o)


def bands_block(root):
    shown = [b for b in BANDS if b["m"]]
    dropped = [b for b in BANDS if not b["m"]]
    chips, dets = [], []
    for i, b in enumerate(shown):
        fq = " · ".join((f"{f/1000:g} GHz" if f >= 2000 else f"{f:g} MHz") for f in b["mhz"]) if b["mhz"] else "1550 nm, about 193 THz"
        chips.append(f'<button role="tab" type="button" id="bt-{i}" aria-controls="bd-{i}" aria-selected="{"true" if i == 0 else "false"}" tabindex="{0 if i == 0 else -1}" style="--c:var({b["c"]})"><i></i>{esc(b["id"])}<small>{esc(fq)}</small></button>')
        tags = "".join(f'<a class="file-chip" href="{file_href(root, f)}"><code>{esc(f)}</code></a>' for f in b["files"]) or '<span class="none">No bundled scenario.</span>'
        chart = f'<div class="bd-chart"><p class="m-chart-l">Effective C/N₀ by satellite, from a real run</p>{fig_html(root, b["viz"], 760, 0, "jammed")}</div>' if b.get("viz") else ""
        src_t = f'<p class="bd-src">Engine source: {esc(b["src"])}</p>' if b.get("src") else ""
        dets.append(f'<div class="bd" role="tabpanel" id="bd-{i}" aria-labelledby="bt-{i}"{"" if i == 0 else " hidden"}><div class="bd-grid"><div><h3>{esc(b["id"])} · {esc(b["con"])}</h3><div class="fq">{esc(fq)} · modelled</div><p>{esc(b["use"])}</p>{src_t}</div>'
                    f'<div><span class="lbl">What interference does</span><p>{esc(b["jam"])}</p><div class="tags">{tags}</div></div></div>{chart}</div>')
    # Overview on a log axis (1 to 40 GHz) and a zoomed L-band inset, labels stacked with leaders.
    lg = lambda f: (math.log10(f) - math.log10(1000)) / (math.log10(40000) - math.log10(1000))
    over, inset = [], []
    for i, b in enumerate(shown):
        if not b["mhz"]:
            continue
        names = b["id"].split(" · ") if len(b["mhz"]) > 1 else [b["id"]]
        for f, nm in zip(b["mhz"], names):
            if 1150 <= f <= 1610:
                inset.append({"f": f, "text": f"{b['id']} · {f:g} MHz", "c": b["c"], "band": f"bt-{i}"})
            else:
                lab = {"S-band 2.4 GHz": "Lunar S · 2.4 GHz"}.get(b["id"], f"DSN {nm} · {f / 1000:g} GHz" if b["con"] == "Deep Space Network" else nm)
                over.append({"f": f, "text": lab, "c": b["c"], "band": f"bt-{i}"})
    opt = next((i, b) for i, b in enumerate(shown) if not b["mhz"])
    ov = _band_axis_svg(over, 1000, 40000, lg, [(1000, "1 GHz"), (2000, "2"), (5000, "5"), (10000, "10"), (20000, "20"), (40000, "40 GHz")],
                        "Every band the engine models · log scale · DSN: Deep Space Network", bracket=(1176.45, 1575.42, "L-band, zoomed below"),
                        tail={"band": f"bt-{opt[0]}", "c": opt[1]["c"], "text": "Optical · 1550 nm", "sub": "about 193 THz · off scale"})
    lin = lambda f: (f - 1150) / (1610 - 1150)
    zi = _band_axis_svg(inset, 1150, 1610, lin, [(t, f"{t}") for t in range(1150, 1601, 50)], "L-band inset · MHz, linear")
    nm = ", ".join(f'{esc(b["con"])} {esc(b["id"].replace(" · ", " and "))}' for b in dropped)
    note = (f'<p class="ba-nm"><b>Not modelled:</b> {nm}. GLONASS (Russia\'s Global Navigation Satellite System) and BeiDou ephemerides are parsed from RINEX (Receiver Independent Exchange Format) files, '
            f'but none of these signals is modelled, so they are left off the map.</p>') if dropped else ""
    return (f'<div class="ba-maps"><div class="ba-map">{ov}</div><div class="ba-map">{zi}</div></div>'
            f'<div class="ba-chips" role="tablist" aria-label="Signal bands the engine models">{"".join(chips)}</div>{"".join(dets)}{note}')


# =================================================================== validation ledger (#21)
LEDGER_DOMAINS = [
    ("lunar", "Lunar and cislunar", ["lunar", "cislunar", "moon", "lunanet"]),
    ("quantum", "Quantum sensors", ["quantum", "cold-atom", "atom"]),
    ("clock", "Clocks and timing", ["allan", "clock", "telecom", "slot", "timing", "mtie", "time transfer", "holdover", "oscillator"]),
    ("integrity", "Integrity", ["raim", "integrity", "sbas", "protection level"]),
    ("orbit", "Orbits, frames and geometry", ["sgp4", "orbit", "propagat", "sp3", "rinex", "frame", "maneuver", "lambert", "ephemer", "igrf", "precession", "dop"]),
    ("nav", "Inertial, fusion and alternative PNT", ["inertial", "imu", "hybrid", "fusion", "terrain", "gravity", "map-match", "odometr"]),
    ("signal", "Signals and interference", ["jam", "spoof", "navsignal", "signal", "tracking", "gnss", "interference"]),
]


def ledger_domain(r):
    t = " ".join([r.get("module") or "", r["requirement"], r["capability"]]).lower()
    for key, name, keys in LEDGER_DOMAINS:
        if any(k in t for k in keys):
            return key, name
    return "other", "Mission analysis, assurance and other"


def mod_wbr(t):
    """A module list breaks only after an underscore, a slash, "::", a comma or a space, never inside an
    identifier: a piece with a hyphen (a file name) is kept whole."""
    parts = [x for x in re.split(r"(?<=[_/,\s])|(?<=::)", esc(t)) if x]
    return "<wbr>".join(f'<span class="nw">{x}</span>' if "-" in x else x for x in parts)


def ledger_block():
    S = {"VALIDATED": ("v", "Validated"), "MODELLED": ("m", "Modelled"), "PARTNER": ("p", "Partner-owned")}
    rows, doms = [], {}
    for r in vm["rows"]:
        key, name = ledger_domain(r)
        s, lbl = S[r["status"]]
        d = doms.setdefault(key, {"name": name, "v": 0, "m": 0, "p": 0})
        d[s] += 1
        t = (r.get("test_links") or [{}])[0].get("path") or ""
        rows.append(f'<details class="lg-row" data-fl-item data-s="{s}" data-g="{key}"><summary><span class="st st-{s}">{lbl}</span><span class="lg-req">{esc_ic(r["requirement"])}</span><code class="lg-mod">{mod_wbr((r.get("module") or "").replace("`", ""))}</code></summary>'
                    f'<dl class="lg-body"><dt>Capability</dt><dd>{esc_ic(r["capability"])}</dd><dt>Oracle</dt><dd>{esc_ic(r.get("oracle") or "None: labelled " + lbl)}</dd>'
                    f'<dt>Tests</dt><dd><code>{esc(trunc((r.get("tests") or t or "").replace("`", ""), 220))}</code></dd></dl></details>')
    order = [k for k, _, _ in LEDGER_DOMAINS] + ["other"]
    mx = max(sum(d[x] for x in "vmp") for d in doms.values())
    summ_rows = []
    for k in order:
        if k not in doms:
            continue
        d = doms[k]
        n = d["v"] + d["m"] + d["p"]
        seg = "".join(f'<i class="c-{x}" style="width:{d[x] / mx * 100:.2f}%"></i>' for x in "vmp" if d[x])
        summ_rows.append(f'<button type="button" class="lg-dom" data-fl-set="g" data-v="{k}" aria-pressed="false"><span class="lg-dn">{esc(d["name"])}</span><span class="lgx-bar">{seg}</span><span class="lg-dc"><b>{d["v"]}</b> of {n} validated</span></button>')
    total = len(vm["rows"])
    bar = fl_bar("Search the ledger", "Search requirement, oracle, module or test",
                 [("s", "Filter by status", [("", f"All {total}"), ("v", f"Validated {summ['validated']}"), ("m", f"Modelled {summ['modelled']}"), ("p", f"Partner-owned {summ['partner_owned']}")])])
    return (f'<div class="lg-compact" data-flist data-fl-scroll>'
            f'<div class="lg-summary" role="group" aria-label="Filter by domain"><p class="lg-sh">By domain: bar length is rows, colour is status. Pick one to filter.</p>{"".join(summ_rows)}'
            f'<div class="lg-key"><span><i class="c-v"></i>Validated</span><span><i class="c-m"></i>Modelled</span><span><i class="c-p"></i>Partner-owned</span></div></div>'
            f'<div class="lg-list">{bar}<div class="lg-scroll" tabindex="0" role="region" aria-label="Validation ledger, {total} rows, scrollable" data-fl-region><div class="lg-rows">{"".join(rows)}</div>{fl_foot(total, "rows", False)}</div>'
            f'<p class="lgx-foot">Generated from the engine\'s verification matrix and pinned by a test. Scroll the list, or open a row for its capability, oracle and tests. '
            f'The full table, every column at once, is in the docs: <a href="docs/capability-reference.html#matrix">Capability reference</a>.</p></div></div>')


def ledger_summary():
    """One line of the ledger for other pages: counts and a link."""
    return (f'<a class="lg-strip" href="evidence.html#ledger"><span class="lgx-bar">'
            + "".join(f'<i class="c-{x}" style="width:{summ[k] / summ["total"] * 100:.2f}%"></i>' for x, k in (("v", "validated"), ("m", "modelled"), ("p", "partner_owned")))
            + f'</span><span><b>{summ["validated"]}</b> validated · <b>{summ["modelled"]}</b> Modelled · <b>{summ["partner_owned"]}</b> partner-owned, of {summ["total"]}</span></a>')


# =================================================================== capability explorer (#24)
def explorer(root):
    groups = []
    for c in cards:
        if c.get("group") and c["group"] not in groups:
            groups.append(c["group"])
    rows = []
    for c in cards:
        run = (c.get("run") or "").replace(".toml", "")
        s = "v" if c["status"] == "validated" else "m"
        act = f'<a class="tbtn" href="{pg_link(root, run, "overview")}">Run {esc(run)} in {esc(STUDIO)}</a>' if run in recorded else ""
        q = " ".join([c["name"], c["domain"], c.get("group") or "", c["summary"], c.get("proof") or "", run]
                     + [ABBR[k] for k in abbr_used(c["name"], c["domain"], c.get("proof"), c["summary"])]).lower()
        rows.append(f'<details class="cx-row" data-fl-item data-s="{s}" data-g="{esc(c.get("group") or "")}" data-q="{esc(q)}"><summary>'
                    f'<span class="st st-{s}">{"Validated" if s == "v" else "Modelled"}</span><b class="cx-n">{esc(c["name"])}</b><span class="cx-d">{esc(c["domain"])}</span>'
                    f'{"<span class=cx-p>" + esc(c["proof"]) + "</span>" if c.get("proof") else ""}</summary>'
                    f'<div class="cx-body"><p>{esc_ic(c["summary"])}</p>{abbr_key(c["name"], c["domain"], c.get("proof"), c["summary"])}{act}</div></details>')
    nv = sum(1 for c in cards if c["status"] == "validated")
    bar = fl_bar("Search the capabilities", "Search capability, domain or proof",
                 [("s", "Filter by status", [("", f"All {len(cards)}"), ("v", f"Validated {nv}"), ("m", f"Modelled {len(cards) - nv}")]),
                  ("g", "Filter by group", [("", "Every group")] + [(g, esc(g)) for g in groups])])
    return f'<div class="cx" data-flist data-fl-limit="10">{bar}<div class="cx-rows">{"".join(rows)}</div>{fl_foot(len(cards), "capabilities", True)}</div>'


# =================================================================== scenario atlas as a kind matrix (#25, #27)
def atlas(root):
    placed, out = set(), []

    def st(k):
        r = kind_ledger.get(k)
        return "u" if not r else ("v" if r[0] > 0 else "m")
    for name, c, ks in GROUPS + [("Other", "--ink-3", None)]:
        fs = [f for f in files if (file_kind[f] in ks if ks else f not in placed)]
        if not ks:
            fs += [f for f in all_files if f not in files]
        if not fs:
            continue
        placed.update(fs)
        by_kind = {}
        for f in fs:
            by_kind.setdefault(file_kind.get(f, "study suite"), []).append(f)
        kblocks = []
        for k, kf in by_kind.items():
            s = st(k) if k != "study suite" else "u"
            chips = "".join(
                f'<a class="km-f" data-fl-item data-s="{s}" data-q="{esc((f + " " + k + " " + file_desc.get(f, "")).lower())}" '
                f'href="{file_href(root, f) if f in files else root + "docs/scenario-reference.html#study-suites"}" '
                f'title="{esc(file_desc.get(f, "A study suite: kshana --study runs it"))}">{esc(f)}</a>' for f in kf)
            kblocks.append(f'<span class="km-k" data-fl-grp><code class="km-kind"><i class="{s}"></i>{esc(k)}</code>{chips}</span>')
        out.append(f'<div class="km-row" data-fl-grp style="--c:var({c})"><h3 class="km-h"><i></i>{esc(name)}<span>{len(fs)}</span></h3><div class="km-body">{"".join(kblocks)}</div></div>')
    notes = []
    for (label, href), ks in EXTENDS.items():
        hit = [k for k in ks if k in {file_kind[f] for f in files}]
        if hit:
            notes.append(f'<li><b>{esc(label)}</b> builds on {", ".join(f"<code>{esc(k)}</code>" for k in hit)}. <a href="{root}{href}">See what it adds</a></li>')
    bar = fl_bar("Filter scenarios", "Filter by file, kind or words",
                 [("s", "Filter by ledger status", [("", "Every kind"), ("v", '<i class="dot v"></i>Validated rows'), ("m", '<i class="dot m"></i>Modelled'), ("u", '<i class="dot u"></i>Not matched')])])
    return (f'<div class="km" data-flist>{bar}<div class="km-rows">{"".join(out)}</div>{fl_foot(len(all_files), "files", False)}'
            f'<ul class="km-pro">{"".join(notes)}</ul>'
            f'<p class="km-key"><span><i class="dot v"></i>Kind has validated ledger rows</span><span><i class="dot m"></i>Kind\'s rows are Modelled</span><span><i class="dot u"></i>Not matched to a ledger module by name</span></p></div>')


# =================================================================== standards (#26)
def standards_grid():
    rows = "".join(
        f'<li class="sd{" ok" if s.get("proof") else ""}" data-fl-item data-s="{"ok" if s.get("proof") else "im"}" data-q="{esc(" ".join([s["name"], s["note"]] + [ABBR[k] for k in abbr_used(s["name"], s["note"])]).lower())}">'
        f'<i aria-hidden="true"></i><b>{esc(s["name"])}</b><span>{esc(s["note"])}</span><em>{"Checked" if s.get("proof") else "Implemented"}</em>{abbr_key(s["name"], s["note"])}</li>' for s in standards)
    nok = sum(1 for s in standards if s.get("proof"))
    facets = [("s", "Filter by check", [("", f"All {len(standards)}"), ("ok", f"Checked against a reference {nok}"), ("im", f"Implemented, not yet checked {len(standards) - nok}")])] if 0 < nok < len(standards) else []
    bar = fl_bar("Search the standards", "Search standard or format", facets)
    return f'<div class="sd-wrap" data-flist>{bar}<ul class="sd-list">{rows}</ul>{fl_foot(len(standards), "standards", False)}</div>'


# =================================================================== ways to use it (#28)
WAY_GROUPS = [("Try", ["browser", "embed"]), ("Run", ["cli", "ci", "reports"]), ("Code", ["rust", "python", "npm"]), ("Assistants and tools", ["mcp", "docker", "jetbrains"])]


def ways(root):
    tabs, panels = [], []
    by_id = {w[0]: w for w in WAYS}
    i = 0
    grp_html = []
    for gname, ids in WAY_GROUPS:
        btns = []
        for wid in ids:
            _, n, c, cmd, what = by_id[wid]
            btns.append(f'<button role="tab" type="button" id="wt-{wid}" aria-controls="wp-{wid}" aria-selected="{"true" if i == 0 else "false"}" tabindex="{0 if i == 0 else -1}" style="--c:var({c})"><i></i>{esc(n)}</button>')
            if cmd.startswith("http") or cmd.startswith("studio/"):
                href = cmd if cmd.startswith("http") else root + cmd
                cell = f'<a class="btn btn-ink" href="{esc(href)}">{esc(STUDIO_CTA) if cmd.startswith("studio/") else "Open the page"} <svg aria-hidden="true"><use href="#i-arrow"/></svg></a>'
                if "embed=1" in cmd:
                    cell = f'<a class="btn btn-ghost" href="{esc(href)}">Open an embedded view <svg aria-hidden="true"><use href="#i-arrow"/></svg></a><code class="wy-url">{esc(cmd)}</code>'
            else:
                cell = code_btn(cmd)
            panels.append(f'<div class="wy-p" role="tabpanel" id="wp-{wid}" aria-labelledby="wt-{wid}"{"" if i == 0 else " hidden"}><h3 class="card-h">{esc(n)}</h3><p>{esc(what)}</p><div class="wy-cmd">{cell}</div></div>')
            i += 1
        grp_html.append(f'<div class="wy-g" role="presentation"><span class="wy-gl" aria-hidden="true">{esc(gname)}</span>{"".join(btns)}</div>')
    return f'<div class="wy" data-ks-tabs><div class="wy-tabs" role="tablist" aria-label="Ways to use Kshana">{"".join(grp_html)}</div><div class="wy-panels">{"".join(panels)}</div></div>'


# =================================================================== the engine at work (#23, #11)
def engine_bento(root):
    def tier(k):
        r = kind_ledger.get(k)
        if not r:
            return None
        return ("v", f"{r[0]} of {r[1]} ledger rows validated") if r[0] else ("m", "Modelled")
    fh, fi, fq = CHARTS["eng-fom-hold"]["figures"], CHARTS["eng-fom-itg"]["figures"], CHARTS["eng-fom-quantum"]["figures"]
    fc = CHARTS["fail-nav"]["figures"]
    mm = CHARTS["tw-mtie"]["model"]
    bud = CHARTS["tw-telecom-budgets"]["figures"]["budgets"]
    npass = sum(1 for x in mm["masks"] if x["verdict"] == "PASS")
    first = min((x for x in bud if x["exceeded"]), key=lambda x: x["time_to_exceed_s"], default=None)
    hmin = lambda s: fmt_dur(s) if s >= 60 else f"{s:g} s"
    tiles = [
        ("Holdover", "--tim", "clock", "fail-tim", {"abs_y": True, "logy": True},
         " · ".join(f'<b>{hmin(c["holdover_s"])}</b> {esc(c["id"])}' for c in fh["clocks"]),
         f'GNSS is lost and each clock free-runs; the figure is how long it stays inside the {fh["threshold_ns"]:g} ns guard.'),
        ("Integrity", "--itg", "integrity", "fail-itg", {},
         f'<b>{fi["samples_available"]}</b> of {fi["samples_total"]} epochs available',
         f'Protection levels against a {fi["al_h_m"]:g} m horizontal and {fi["al_v_m"]:g} m vertical alert limit.'),
        ("MTIE", "--tim", "telecom-timing", "tw-mtie", {},
         f'<b>{npass}</b> of {len(mm["masks"])} telecom masks pass' + (f' · the {first["max_abs_te_ns"]:g} ns budget is exceeded after <b>{fmt_dur(first["time_to_exceed_s"])}</b>' if first else ""),
         "Maximum time interval error (MTIE) and time deviation (TDEV) of a telecom clock in holdover, against the masks of the ITU-T (International Telecommunication Union, Telecommunication Standardization Sector)."),
        ("Coasting", "--nav", "gnss-ins", "fail-nav", {},
         " · ".join(f'<b>{c["holdover_s"]:g} s</b> {esc(c["id"])}' for c in fc["imus"]),
         f'Seconds each inertial measurement unit stays inside the {fc["budget_m"]:g} m position budget after GNSS is lost, on a log scale.'),
        ("Quantum", "--spf", "inertial", "eng-quantum", {"logy": True, "abs_y": True},
         " · ".join(f'<b>{hmin(c["holdover_s"])}</b> {esc(c["id"])}' for c in fq["imus"]),
         f'A cold-atom quantum inertial sensor against a navigation-grade quartz one: time inside the {fq["threshold_m"]:g} m budget.'),
    ]
    out = []
    for i, (k, c, kind, cid, o, fig, txt) in enumerate(tiles):
        tr = tier(kind)
        pill = f'<span class="st st-{tr[0]}">{esc(tr[1])}</span>' if tr else '<a class="eb-lg" href="evidence.html#ledger">Ledger status</a>'
        w, h = (520, 170) if i < 2 else (380, 160)
        kk = register_svg(cid, w, h, "spark", spark_svg(cid, w, h, logy=o.get("logy"), abs_y=o.get("abs_y", False)))
        out.append(f'<article class="eb{" eb-wide" if i < 2 else ""}" style="--c:var({c})" data-anim><div class="eb-k"><i></i>{esc(k)}<code>{esc(kind)}</code>{pill}</div>'
                   f'<p class="eb-fig">{fig}</p><p class="eb-t">{esc(txt)}</p>{fig_html(root, kk[0], w, h, "spark", " viz-spark")}</article>')
    return f'<div class="eb-grid">{"".join(out)}</div>'


# =================================================================== the shared Pro band (Home, Editions)
def pro_band(root, sid, with_cols=True):
    val, tot = summ["validated"], summ["total"]
    cols = [
        ("open", "--s-itg", "Open core · free · AGPL-3.0", "Why everyone needs it",
         [f"Anyone who takes position or time from GNSS is exposed: jamming, spoofing and signal loss reach vehicles, networks, grids and spacecraft alike.",
          f"{val} of {tot} capabilities agree with independent external oracles; the rest are labelled Modelled, out loud.",
          f"All {len(kinds)} scenario kinds, free with no feature gate, deterministic from scenario, seed and engine version.",
          "The same core under a commercial licence, for closed integration."],
         (f'{root}studio/index.html', STUDIO_CTA, "btn-white"), (f'{root}developers.html#install', "Install the engine")),
        ("pro", "--s-spf", "Kshana Pro · under contract", "Why Pro, even more",
         ["The Pareto front of a design space, with every front design as a scenario the free engine re-runs.",
          "Uncertainty and sensitivity over thousands of runs.",
          "A one-command mission dossier with a verification matrix and a PDF (Portable Document Format) file.",
          "Audit-grade, reproducible evidence packs; clock digital twins calibrated to your device's published Allan budget; architecture trade studies ranked on a figure of merit; model-based systems-engineering (MBSE) and programme tooling."],
         ("mailto:contact@ashforde.org?subject=Kshana%20Pro%20evaluation", "Request a Pro evaluation", "btn-white"), (f'{root}editions.html#pro', "What Pro includes")),
        ("custom", "--s-nav", "Custom study · done for you", "Custom study: the answer, not the tool",
         ["A Modelled PNT-resilience study of your system, for example an optimism-gap audit. Not a certification.",
          "Sensor and resilience models calibrated to your hardware.",
          "Export-controlled work under the appropriate clearance and NDA (non-disclosure agreement).",
          "Integration, training and consulting."],
         ("mailto:contact@ashforde.org?subject=Kshana%20study%20request", "Request a study", "btn-white"), (f'{root}editions.html#custom-why', "Why and who it is for")),
    ]
    for _x in cols[1][4][:3]:   # the Pro column's headline lines are the README's "What you get" words (pq below)
        pq(_x[0].lower() + _x[1:].rstrip("."))
    for _x in cols[1][4][3].rstrip(".").split("; "):   # ...and so is each part of the fourth
        pq(_x[0].lower() + _x[1:])
    # Edition markers the feature register reads: -free, -pro, and a "plan" whose heading says custom study.
    PB_CLASS = {"open": "pb-free", "pro": "pb-pro", "custom": "plan pb-custom"}
    col_html = "".join(
        f'<div class="pb-col {PB_CLASS[k]}" style="--c:var({c})"><span class="pb-tag"><i></i>{esc(tag)}</span><h3>{esc(h)}</h3><ul>{"".join(f"<li>{esc(x)}</li>" for x in items)}</ul>'
        f'<div class="pb-go"><a class="btn {cta[2]}" href="{esc(cta[0])}">{esc(cta[1])} <svg aria-hidden="true"><use href="#i-arrow"/></svg></a><a class="pb-more" href="{esc(sec[0])}">{esc(sec[1])}</a></div></div>'
        for k, c, tag, h, items, cta, sec in cols)
    # The Pro wording is quoted from the public README of the docs checkout (then the main checkout).
    # PRO_MBSE must stay word for word in it; the three "builds on" lines must each be stated there.
    def _plain(s):
        return re.sub(r"\s+", " ", re.sub(r"[*`]", "", s))
    pro_readmes = [_plain(open(os.path.join(r, "README.md"), encoding="utf-8").read()) for r in dict.fromkeys([DOCS_REPO, REPO])]
    def quoted(s):
        assert any(_plain(s) in r for r in pro_readmes), f"Pro wording is not in the public README: {s[:90]}"
        return s
    # The register row (features/gen_features.py) quotes the Editions paragraph in full; the card
    # quotes the shorter "Support & professional services" line and lists what is read beside it.
    does = quoted("model-based systems-engineering (MBSE) and programme tooling that checks a programme's requirements against runs of the open engine "
                  "and produces a verification cross-reference matrix (VCRM), a Systems Modeling Language version 2 (SysML v2) model, "
                  "a change-impact report and an offline evidence pack.")
    on = quoted("re-runs nothing and reads only the engine's published outputs")
    PRO_MBSE = f"Pro adds {does} What it builds on: it {on}, listed here."
    quoted("Clock digital twins use the open engine's clock model and its Allan-deviation estimator")
    quoted("Trade studies run every architecture option as an open-engine scenario, unchanged, and rank the options on a figure of merit read from each run's result.")
    quoted("Evidence packs record, for each run, the scenario hash the open engine stamps on its result")
    # What the tooling reads, in the README's words. Each file is checked to exist in a checkout.
    def _has(path=None, ext=None):
        for r in dict.fromkeys([DOCS_REPO, REPO]):
            if path and os.path.exists(os.path.join(r, path)): return True
            if ext and f'with_extension("{ext}")' in open(os.path.join(r, "src/main.rs"), encoding="utf-8").read(): return True
        return False
    reads = [("result.json", "each run's result.json (", "the fields a criterion names, their units, the reproducibility stamp of scenario hash, seed, engine version and schema version, and the run's label and figure tiers", _has(ext="result.json")),
             ("report.json", "its report.json (", "kind, reproducibility record and capability labels", _has(ext="report.json")),
             ("scenario file", "the scenario file (", "its hash, kind, seed and parameters", os.path.isdir(os.path.join(REPO, "scenarios"))),
             ("field-units schema", "the field-units schema (", "each field's unit, provenance and evidence tier", _has(path="docs/field-units-schema.json")),
             ("verification matrix", "the verification matrix (", "each capability's VALIDATED, MODELLED or PARTNER label", _has(path="web/data/verification-matrix.json"))]
    assert all(r[3] for r in reads), f"an open-engine output the Pro card names is missing: {[r[0] for r in reads if not r[3]]}"
    plug = ('<div class="pb-plug"><p class="pb-plug-h">The open engine\'s outputs it reads</p><ul class="pb-reads">'
            + "".join(f'<li><code>{esc(n)}</code><span>{esc(quoted(lead + what + ")")[len(lead):-1])}</span></li>' for n, lead, what, _ in reads)
            + '</ul><p class="prov"><span class="prov-src">Quoted from the public README, engine v' + esc(version) + '</span>'
            + f'<a class="prov-open" href="{root}developers.html#ways">What a run writes<svg aria-hidden="true"><use href="#i-ext"/></svg></a></p></div>')
    items = [
        ("A calibrated clock twin", "--s-tim", "Pro calibrates a clock digital twin to your device's published Allan budget. What it builds on: the open engine's clock model and its Allan-deviation estimator.", fig_html(root, "pro-twin", 420, 230, "", " viz-dark")),
        ("A ranked trade study", "--s-nav", "Pro ranks architecture options on a figure of merit. What it builds on: every option is an open-engine scenario, run unchanged, and the figure of merit is read from each run's result.", fig_html(root, "pro-trade", 420, 230, "", " viz-dark")),
        ("An audit-grade evidence pack", "--s-itg", "Pro packages the evidence a review asks for, reproducibly. What it builds on: the scenario hash the open engine stamps on each run's result, and the open verification matrix's Validated, Modelled or Partner label for each capability.",
         f'<div class="evi"><div class="evi-n"><b>{val}</b><span>of {tot} capabilities validated against independent oracles</span></div><div class="evi-bar"><i style="width:{val / tot * 100:.1f}%"></i></div><p class="prov"><span class="prov-src">Verification matrix, engine v{esc(version)}</span><a class="prov-open" href="{root}evidence.html#ledger">Open the ledger<svg aria-hidden="true"><use href="#i-ext"/></svg></a></p></div>'),
        ("Systems-engineering and programme tooling", "--s-spf", PRO_MBSE, plug),
    ]
    deliv = "\n".join(f'<article class="dv" style="--c:var({c})"><div class="dv-k"><i></i>You walk away with</div><h3>{esc(t)}</h3><p>{esc(d)}</p>'
                      f'<div class="dv-viz">{v}</div><p class="dv-honest">{"The image is the open engine. Pro extends it." if "<figure" in v or 'class="evi"' in v else "Described from the public README. Ask us for a walkthrough."}</p></article>' for t, c, d, v in items)
    # Home draws the band on the page ground (home.css), so the nav keeps its light tone over it there
    nav_dark = "" if sid == "pro-band" else " data-nav-dark"
    return f'''<section class="pro-band" id="{sid}"{nav_dark}>
  <div class="wrap">
    <div class="pro-head">
      <span class="eyebrow eyebrow-d" style="--c:var(--s-spf)"><i></i>Open core, Kshana Pro, custom study</span>
      <h2 class="h2 h2-d">Free for everyone. Pro when the answer has to stand up in a review.</h2>
      <p class="lede lede-d">{esc(pq("Kshana Pro is a proprietary overlay that depends on the open core and never forks it, available under contract."))} No prices on a web page: every paid route starts with a message to <a href="mailto:contact@ashforde.org">contact@ashforde.org</a>.</p>
    </div>
    {f'<div class="pb-cols">{col_html}</div>' if with_cols else ""}
    {pro_strip(root)}
  </div>
</section>'''


# =================================================================== Kshana Pro: the same engine, amplified
# Editions #pro, the Home Pro band's strip and the Home hero line. Every chart is drawn from real
# Kshana Pro output (src/data/pro-power.json, written by src/tools/gen_pro_power.py from the files
# `kshana-pro design`, `kshana-pro uncertainty` and `kshana-pro mission build` wrote). Every sentence
# about Pro is quoted from the public docs/PRO.md or README.md of the open repository: pq() stops
# the build when a quote is no longer there.
PRO_POWER = json.loads(src("data/pro-power.json"))


def _pro_public_text():
    """docs/PRO.md from the engine checkout once it carries it, else the public-text checkout
    named in pro-power.json, at the commit recorded there (committed text only)."""
    for r in dict.fromkeys([REPO, DOCS_REPO]):
        p = os.path.join(r, "docs", "PRO.md")
        if os.path.exists(p):
            return open(p, encoding="utf-8").read()
    pt = PRO_POWER["public_text"]
    out = subprocess.run(["git", "-C", os.path.expanduser(pt["checkout"]), "show", f"{pt['commit']}:{pt['file']}"], capture_output=True, text=True)
    assert out.returncode == 0, f"the public Pro text {pt['file']} is not at {pt['checkout']} {pt['commit'][:10]}"
    return out.stdout


def _pplain(s):
    s = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", s)
    return re.sub(r"\s+", " ", re.sub(r"[*`]", "", s)).replace("’", "'")


_PRO_PUBLIC = [_pplain(_pro_public_text())] + [_pplain(open(os.path.join(r, "README.md"), encoding="utf-8").read()) for r in dict.fromkeys([DOCS_REPO, REPO])]
# ...and the README of the public-text checkout, whose Editions section carries the same Pro text
_pt = PRO_POWER["public_text"]
_ptr = subprocess.run(["git", "-C", os.path.expanduser(_pt["checkout"]), "show", f"{_pt['commit']}:README.md"], capture_output=True, text=True)
if _ptr.returncode == 0:
    _PRO_PUBLIC.append(_pplain(_ptr.stdout))


PQ_USED = []


def pq(s):
    """A sentence about Pro, quoted from the public text; the build stops if it is not there."""
    assert any(_pplain(s) in t for t in _PRO_PUBLIC), f"Pro wording is not in the public docs/PRO.md or README.md: {s[:100]}"
    PQ_USED.append(s)
    return s


def pn(x):
    """A Pro figure as the site prints it, which must appear, printed the same way, in the public
    docs/PRO.md (a number the page shows as text has a public line); the build stops otherwise."""
    t = str(x)
    assert re.search(r"(?<![\d.])" + re.escape(t) + r"(?![\d])", _PRO_PUBLIC[0]), f"Pro figure {t} is not printed in the public docs/PRO.md"
    return t


def _n(x, d=4):
    """A figure as the Pro output wrote it (trailing zeros dropped), with thousands separators."""
    if isinstance(x, (int,)) or float(x).is_integer():
        return f"{int(x):,}"
    return f"{float(x):,.{d}f}".rstrip("0").rstrip(".")


def _short(h):
    """A hash as the site prints it (its first 8 hex digits); the full hash must be printed in
    the public docs/PRO.md, so every identifier the page shows has a public line."""
    assert re.fullmatch(r"[0-9a-f]{64}", h) and h in _PRO_PUBLIC[0], f"Pro hash {h[:16]} is not printed in the public docs/PRO.md"
    return h[:8] + "…"


PRO_KINDS = sorted({d["base_kind"] for d in PRO_POWER["design"]}), sorted({PRO_POWER["band"]["base_kind"], PRO_POWER["sensitivity"]["base_kind"]}), sorted({r["kind"] for r in PRO_POWER["dossier"]["runs"]})
_known_kinds = {k for k, _ in kinds}
for _ks in PRO_KINDS:
    assert set(_ks) <= _known_kinds, f"a Pro worked example ran a kind the engine does not list: {set(_ks) - _known_kinds}"
EXTENDS[("Pro: design optimiser", "editions.html#pro-design")] = PRO_KINDS[0]
EXTENDS[("Pro: uncertainty and sensitivity", "editions.html#pro-uncertainty")] = PRO_KINDS[1]
EXTENDS[("Pro: mission dossier", "editions.html#pro-dossier")] = PRO_KINDS[2]
# the campaign watch demonstration runs the open campaign kind; the coexistence study reads leo-signal and constellation-design runs
_cw_kind = re.search(r'^kind = "([\w-]+)"', open(os.path.join(REPO, "scenarios", PRO_POWER["watch"]["base_scenario"] + ".toml"), encoding="utf-8").read(), re.M).group(1)
for _ks in ([_cw_kind], PRO_POWER["coexist"]["member_kinds"]):
    assert set(_ks) <= _known_kinds, f"a Pro worked example ran a kind the engine does not list: {set(_ks) - _known_kinds}"
EXTENDS[("Pro: campaign watch", "editions.html#pro-watch")] = [_cw_kind]
EXTENDS[("Pro: spectrum coexistence", "editions.html#pro-coexist")] = PRO_POWER["coexist"]["member_kinds"]


# ------------------------------------------------------------------ charts (SVG, drawn here)
def front_svg(d, idx, w=560, h=330, mini=False):
    """Every design the optimiser ran (faint), the Pareto front (line and points), its knee and
    the reference design, for one design study."""
    L, R, T, B = (8, 8, 8, 8) if mini else (46, 16, 34, 46)
    sats = sorted({p[0] for p in d["evaluated"]})
    x0, x1 = sats[0] - 1, sats[-1] + 1
    X = lambda v: L + (v - x0) / (x1 - x0) * (w - L - R)
    Y = lambda v: T + (1 - v / 100) * (h - T - B)
    o = [f'<svg viewBox="0 0 {w} {h}" role="img" aria-label="{esc(d["title"])}: {len(d["evaluated"])} designs run, a Pareto front of {len(d["front"])} designs">']
    if not mini:
        for v in (0, 25, 50, 75, 100):
            o.append(f'<line class="g" x1="{L}" x2="{w - R}" y1="{Y(v):.1f}" y2="{Y(v):.1f}"/><text x="{L - 8}" y="{Y(v) + 4:.1f}" text-anchor="end">{v}</text>')
        for s in sats:
            o.append(f'<text x="{X(s):.1f}" y="{h - B + 18}" text-anchor="middle">{_n(s)}</text>')
        o.append(f'<line class="ax" x1="{L}" x2="{w - R}" y1="{h - B}" y2="{h - B}"/>')
        o.append(f'<text class="t-ax t-lab" x="{(L + w - R) / 2:.0f}" y="{h - 6}" text-anchor="middle">satellites in the constellation</text>')
        o.append(f'<text class="t-ax t-lab" x="{L - 36}" y="14" text-anchor="start">availability over the region, %</text>')
    seen = set()
    for s, a in d["evaluated"]:
        k = (s, round(a, 1))
        if k in seen:
            continue
        seen.add(k)
        o.append(f'<circle class="ev" cx="{X(s):.1f}" cy="{Y(a):.1f}" r="{1.6 if mini else 2.3}"/>')
    pts = sorted({(f["sat"], f["av"]) for f in d["front"]})
    path = "M" + " L".join(f"{X(s):.1f},{Y(a):.1f}" for s, a in pts)
    o.append(f'<path class="fa" d="{path} L{X(pts[-1][0]):.1f},{Y(0):.1f} L{X(pts[0][0]):.1f},{Y(0):.1f} Z" opacity=".5"/>')
    o.append(f'<path class="fl" d="{path}"/>')
    kn = d["knee"]
    o.append(f'<circle class="knee" cx="{X(kn["sat"]):.1f}" cy="{Y(kn["av"]):.1f}" r="{6 if mini else 11}"/>')
    rf = d["reference"]
    if not mini:
        rx, ry = X(rf["sat"]), Y(rf["av"])
        o.append(f'<rect class="ref" x="{rx - 5:.1f}" y="{ry - 5:.1f}" width="10" height="10" transform="rotate(45 {rx:.1f} {ry:.1f})"/>')
        o.append(f'<text class="t-lab" x="{rx:.1f}" y="{ry + 24:.1f}" text-anchor="middle">reference</text>')
        kx, ky = X(kn["sat"]), Y(kn["av"])
        o.append(f'<text class="t-a t-lab" x="{kx + 15:.1f}" y="{ky + 18:.1f}">knee</text>')
    for i, (s, a) in enumerate(pts):
        ids = [f["id"] for f in d["front"] if (f["sat"], f["av"]) == (s, a)]
        if mini:
            continue
        o.append(f'<circle class="pw-pt" tabindex="0" role="button" data-pw-pt="{idx}:{i}" cx="{X(s):.1f}" cy="{Y(a):.1f}" r="6" '
                 f'aria-label="{pn(_n(s))} satellites, {pn(_n(a))} % availability: front design{"s" if len(ids) > 1 else ""} {", ".join(ids)}"/>')
    o.append("</svg>")
    return "".join(o)


def band_svg(b, w=560, h=340, mini=False):
    """The confidence band of a time series over every realisation (5th to 95th percentile), its
    median, the base run and the limit; below it, the share of realisations meeting the limit."""
    L, R, T = (8, 8, 8) if mini else (46, 16, 30)
    H1 = h - 8 if mini else 214
    S0, S1 = 258, 300
    t = [x / 60 for x in b["t"]]
    ym = max(max(abs(v) for v in b["p05"]), max(abs(v) for v in b["p95"]), b["limit"]) * 1.12
    ym = math.ceil(ym / 10) * 10
    X = lambda v: L + v / t[-1] * (w - L - R)
    Y = lambda v: T + (1 - (v + ym) / (2 * ym)) * (H1 - T)
    step = max(1, len(t) // 120)
    ii = list(range(0, len(t), step)) + ([len(t) - 1] if (len(t) - 1) % step else [])
    o = [f'<svg viewBox="0 0 {w} {H1 + 8 if mini else h}" role="img" aria-label="{esc(b["title"])}: the 5th to 95th percentile of {_n(b["n"])} runs against the limit of {_n(b["limit"])} {b["unit"]}">']
    if not mini:
        for v in (-ym, -b["limit"], 0, b["limit"], ym):
            o.append(f'<line class="g" x1="{L}" x2="{w - R}" y1="{Y(v):.1f}" y2="{Y(v):.1f}"/><text x="{L - 8}" y="{Y(v) + 4:.1f}" text-anchor="end">{_n(v)}</text>')
        o.append(f'<text class="t-ax t-lab" x="{L - 36}" y="12" text-anchor="start">time error, {b["unit"]}</text>')
    band = " L".join(f"{X(t[i]):.1f},{Y(b['p95'][i]):.1f}" for i in ii) + " L" + " L".join(f"{X(t[i]):.1f},{Y(b['p05'][i]):.1f}" for i in reversed(ii))
    o.append(f'<path class="band" d="M{band} Z"/>')
    for v in (b["limit"], -b["limit"]):
        o.append(f'<line class="lim" x1="{L}" x2="{w - R}" y1="{Y(v):.1f}" y2="{Y(v):.1f}"/>')
    o.append('<path class="nom" d="M' + " L".join(f"{X(t[i]):.1f},{Y(b['nominal'][i]):.1f}" for i in ii) + '"/>')
    o.append('<path class="med" d="M' + " L".join(f"{X(t[i]):.1f},{Y(b['p50'][i]):.1f}" for i in ii) + '"/>')
    if not mini:
        o.append(f'<text class="t-r t-lab" x="{L + 6}" y="{Y(b["limit"]) - 6:.1f}" text-anchor="start">limit ±{_n(b["limit"])} {b["unit"]}</text>')
        SY = lambda v: S0 + (1 - v) * (S1 - S0)
        o.append(f'<line class="g" x1="{L}" x2="{w - R}" y1="{SY(1):.1f}" y2="{SY(1):.1f}"/><line class="g" x1="{L}" x2="{w - R}" y1="{SY(0):.1f}" y2="{SY(0):.1f}"/>')
        o.append(f'<text x="{L - 8}" y="{SY(1) + 4:.1f}" text-anchor="end">100%</text><text x="{L - 8}" y="{SY(0) + 4:.1f}" text-anchor="end">0%</text>')
        o.append(f'<text class="t-ax t-lab" x="{L}" y="{S0 - 10}">share of runs within the limit</text>')
        o.append('<path class="share" d="M' + " L".join(f"{X(t[i]):.1f},{SY(b['share'][i]):.1f}" for i in ii) + '"/>')
        for m in range(0, int(t[-1]) + 1, 10):
            o.append(f'<text x="{X(m):.1f}" y="{S1 + 18}" text-anchor="middle">{m}</text>')
        o.append(f'<text class="t-ax t-lab" x="{(L + w - R) / 2:.0f}" y="{h - 4}" text-anchor="middle">minutes of holdover</text>')
        o.append(f'<g class="pw-scrub" data-pw-scrub style="display:none"><line class="scrub" x1="0" x2="0" y1="{T}" y2="{S1}"/><circle class="scrub-d" data-d="p" r="3.5"/><circle class="scrub-d" data-d="s" r="3.5"/></g>')
        o.append(f'<rect data-pw-hit x="{L}" y="{T}" width="{w - L - R}" height="{S1 - T}" fill="transparent" style="cursor:crosshair"/>')
    o.append("</svg>")
    return "".join(o), {"L": L, "R": R, "T": T, "H1": H1, "S0": S0, "S1": S1, "w": w, "ym": ym}


def sens_svg(out, names, w=560):
    """The total Sobol' index of each input for one output, with its 95 % interval."""
    rows = out["inputs"]
    h = 18 + len(rows) * 54
    L, R = 8, 64
    X = lambda v: L + max(0.0, min(1.0, v)) * (w - L - R)
    o = [f'<svg viewBox="0 0 {w} {h}" role="img" aria-label="What drives {esc(out["name"])}: total Sobol\' index of each input">']
    for j, x in enumerate(rows):
        y = 16 + j * 54
        o.append(f'<text class="t-ax" x="{L}" y="{y}">{esc(names[x["key"]])}</text>')
        o.append(f'<rect class="bar-bg" x="{L}" y="{y + 8}" width="{w - L - R}" height="14" rx="3"/>')
        o.append(f'<rect class="bar{"" if j == 0 else " b2"}" x="{L}" y="{y + 8}" width="{max(1.5, X(x["total"]) - L):.1f}" height="14" rx="3"/>')
        if x["hi"] - x["lo"] > 0.004:
            o.append(f'<line class="ci" x1="{X(x["lo"]):.1f}" x2="{X(x["hi"]):.1f}" y1="{y + 15}" y2="{y + 15}"/>')
        o.append(f'<text class="t-v" x="{w - R + 8}" y="{y + 20}">{x["total"]:.2f}</text>')
    o.append("</svg>")
    return "".join(o)


def front_mini_hero(d, w=58, h=24):
    """The hero line's mark: the design the optimiser picked, flying.

    The knee of the real front (d["knee"]): its planes, satellites per plane and phasing are
    read from the study, drawn around the Moon, with the satellites moving along their
    orbits. Under reduced motion it holds still (css/pro-power.css, .pro-mini)."""
    kn = next(f for f in d["front"] if f["id"] == d["knee"]["id"])
    v = kn["v"]
    planes, total, phasing = int(v["planes"]), int(v["total"]), int(v.get("phasing", 0))
    per = max(total // planes, 1)
    cx, cy, rx, ry, dur = w / 2, h / 2, w / 2 - 3, h / 2 - 5, 9.0
    orbit = f"M{cx - rx:.1f},{cy:.1f} A{rx:.1f},{ry:.1f} 0 1,0 {cx + rx:.1f},{cy:.1f} A{rx:.1f},{ry:.1f} 0 1,0 {cx - rx:.1f},{cy:.1f}"
    o = [f'<svg class="pro-mini" viewBox="0 0 {w} {h}" aria-hidden="true">',
         '<defs><radialGradient id="pro-mini-moon" cx=".38" cy=".35" r=".75">'
         '<stop offset="0" style="stop-color:var(--ink)"/><stop offset="1" style="stop-color:var(--ink-3)"/>'
         '</radialGradient></defs>']
    tilts = [(-18 + 36 * k / max(planes - 1, 1)) if planes > 1 else 0 for k in range(planes)]
    for k, t in enumerate(tilts):  # far half of every orbit behind the Moon
        o.append(f'<g transform="rotate({t:.1f} {cx:.1f} {cy:.1f})"><path class="ob" d="M{cx - rx:.1f},{cy:.1f} A{rx:.1f},{ry:.1f} 0 0,1 {cx + rx:.1f},{cy:.1f}"/></g>')
    o.append(f'<circle class="mn" cx="{cx:.1f}" cy="{cy:.1f}" r="{h / 2 - 5.5:.1f}"/>')
    for k, t in enumerate(tilts):
        o.append(f'<g transform="rotate({t:.1f} {cx:.1f} {cy:.1f})"><path class="of" d="M{cx - rx:.1f},{cy:.1f} A{rx:.1f},{ry:.1f} 0 0,0 {cx + rx:.1f},{cy:.1f}"/>')
        for j in range(per):
            delay = -dur * (j / per + k * phasing / total)
            o.append(f'<circle class="st" r="1.35" style="offset-path:path(\'{orbit}\');animation-delay:{delay:.2f}s"/>')
        o.append('</g>')
    o.append('</svg>')
    return "".join(o)


# ------------------------------------------------------------------ the words (quoted)
PW_LEDE = pq("Open Kshana runs one scenario and tells you what it gives. Kshana Pro answers the questions a programme asks next, over the same engine and the same scenario files: which design to fly, how sure the answer is, and whether the mission meets its requirements in a form a review board can check.")
PW_PRINCIPLES = [
    ("A strict superset of open Kshana.", pq("Every open scenario kind runs in Pro unchanged, and an open scenario runs in Pro with no licence at all.")),
    ("No new physics.", pq("Every Pro number comes from runs of the open engine, so a Pro figure is as trustworthy as the open scenario kind it came from, and never more.")),
    ("Reproducible in the free engine.", pq("Anyone with open Kshana can re-run one and get the same figure.")),
    ("Honest labels carried through.", pq("Every Pro result is labelled MODELLED") + ". " + pq("Every figure keeps the tier the open engine gave it") + " (Validated, Modelled or Partner), never a higher one."),
]
for _t, _ in PW_PRINCIPLES:
    pq(_t[:-1])


def _prov(r, cmd, runs, eng, ov, digest, files, lead=""):
    links = "".join(f'<a href="{r}{esc(h)}">{esc(t)}</a>' for t, h in files)
    return (f'<p class="pw-prov"><span>{esc(lead) + " · " if lead else ""}Kshana Pro {esc(ov)} on open engine {esc(eng)} · <b>{esc(cmd)}</b> · {_n(runs)} engine runs · run digest {esc(_short(digest))}</span>{links}</p>')


# ------------------------------------------------------------------ three more answers (tabs)
# Campaign watch, spectrum coexistence and the on-premises job service: one switcher, each panel
# in the pattern of the three sections above (free gives / Pro adds / a real artefact). Every
# artefact is drawn from the files the Pro commands wrote (pro-power.json: watch, coexist, jobs);
# every number printed as text goes through pn(), so it is printed the same way in docs/PRO.md.
CW_GLYPH = {"regressed": ("▼", "regressed"), "improved": ("▲", "improved"), "unchanged": ("·", "unchanged"),
            "within-tolerance": ("≈", "moved within its tolerance"), "no-baseline": ("○", "first run: becomes the baseline")}


def watch_win(r):
    W = PRO_POWER["watch"]
    runs, figs = W["runs"], W["figures"]
    cn0 = next(f for f in figs if f["id"].startswith("carrier-to-noise density ratio (C/N0) at the end of the jamming phase"))
    trk = next(f for f in figs if f["id"] == "satellites tracking at the end of the jamming phase")
    spf = next(f for f in figs if f["id"] == "spoofing detected (phase length)")
    floor = cn0["limits"][0]["value"]

    def readout(i):
        ru = runs[i]
        head = f'<b>Revision {esc(ru["name"])}</b> · '
        tail = f' · <span class="{"ok" if ru["verdict"] == "pass" else "bad"}">{"pass" if ru["verdict"] == "pass" else "fail"}, exit code {ru["exit"]}</span>'
        moved = [(f, f["status"][i]) for f in figs if f["status"][i] not in ("unchanged", "no-baseline")]
        if i == 0:
            return head + "the open campaign as published: every watched figure becomes the baseline" + tail
        if not moved and ru["name"].endswith("withdrawn"):
            return head + "the regressing revision withdrawn: every watched figure back where the last passing run had it" + tail
        if not moved:
            return head + "every watched figure unchanged against the last passing run" + tail
        parts = []
        if cn0["status"][i] == "regressed" and trk["status"][i] == "regressed":
            j0 = runs[i - 1]["jammer_dbw"]
            c0, c1 = pn(f"{cn0['value'][i - 1]:.2f}"), pn(f"{cn0['value'][i]:.2f}")
            assert sum(f["failed"][i] for f in figs) == 3 and pq("three watches regressed, the requirement was not met, and the run exited 1")
            parts.append(f'jammer raised from {pn(_n(j0))} to {pn(_n(ru["jammer_dbw"]))} dBW (decibels relative to one watt): C/N0 at the end of the jamming phase '
                         f'{c0} → {c1} dB-Hz (decibel-hertz), below the {pn(_n(floor))} dB-Hz floor, requirement {esc(cn0["requirement"][i])}; '
                         f'satellites tracking {pn(_n(trk["value"][i - 1]))} → {pn(_n(trk["value"][i]))}; three watches regressed')
        elif spf["status"][i] == "improved":
            parts.append(f'a faster spoofer detected at {pn(_n(spf["value"][i]))} s instead of {pn(_n(spf["value"][i - 1]))} s: reported as an improvement')
            parts += [f'{esc(f["id"])}: {CW_GLYPH[st][1]}' for f, st in moved if f is not spf]
        else:
            parts += [f'{esc(f["id"])}: {CW_GLYPH[st][1]}' for f, st in moved]
        return head + "; ".join(parts) + tail

    reads = [readout(i) for i in range(len(runs))]
    sel = next(i for i, ru in enumerate(runs) if ru["exit"] != 0)
    cols = "".join(f'<button type="button" class="cw-col{" fail" if ru["verdict"] != "pass" else ""}" data-cw-col="{i}" aria-pressed="{"true" if i == sel else "false"}" '
                   f'aria-label="Revision {esc(ru["name"])}: {ru["verdict"]}, exit code {ru["exit"]}"><b>r{i + 1}</b><span>{"pass" if ru["verdict"] == "pass" else "fail"}</span></button>'
                   for i, ru in enumerate(runs))
    rows = "".join(f'<div class="cw-row" role="row"><span class="cw-f" role="rowheader">{esc(f["id"])}</span>'
                   + "".join(f'<span class="cw-c {st}{" on" if i == sel else ""}" role="cell" data-cw-cell="{i}" title="{esc(CW_GLYPH[st][1])}" aria-label="r{i + 1}: {esc(CW_GLYPH[st][1])}">{CW_GLYPH[st][0]}</span>'
                             for i, st in enumerate(f["status"])) + '</div>' for f in figs)
    legend = "".join(f'<span><i class="cw-c {k}">{g}</i>{esc(t)}</span>' for k, (g, t) in CW_GLYPH.items())
    P_ = W["pack"]
    assert P_["verdicts"] == ["PASS", "PASS"] and P_["changed"] == 0
    pack = (f'The bundled campaign pack, {pn(_n(P_["watches"]))} watched figures over three open campaigns, run twice: pass both times, '
            f'{pn(_n(P_["moved"]))} of {pn(_n(P_["figures"]))} figures moved, history of {pn(_n(P_["records"]))} records intact.')
    H = W["history"]
    return (f'<figure class="pw-win" id="pwWatch"><div class="pw-bar"><span>Campaign watch · <b>six revisions, one history</b></span><span class="pw-r"><span class="pw-tag">Kshana Pro output</span></span></div>'
            f'<div class="cw" data-reads="{esc(json.dumps(reads))}"><div class="cw-head"><span class="cw-f">Watched figure · revision</span><div class="cw-cols" role="group" aria-label="Revision">{cols}</div></div>'
            f'<div class="cw-grid" role="table" aria-label="Verdict of each watched figure at each revision">{rows}</div></div>'
            f'<div class="pw-legend cw-legend">{legend}</div>'
            f'<p class="pw-read" id="pwWatchRead" aria-live="polite">{reads[sel]}</p>'
            f'<p class="pw-prov"><span>{esc(pack)}</span></p>'
            f'<p class="pw-prov"><span>Kshana Pro {esc(W["overlay"])} on open engine {esc(W["engine"])} · <b>kshana-pro ci</b> · demonstration revisions of <code>{esc(W["base_scenario"])}.toml</code> · '
            f'history {_n(H["records"])} records, chain intact, last hash {esc(_short(H["last"]))} · {esc(W["label"].split(" - ")[0])}</span>{studio_open(r, W["base_scenario"])}</p></figure>')


CX_RAMP = ["rgba(195,166,255,.12)", "rgba(195,166,255,.22)", "rgba(195,166,255,.32)", "rgba(195,166,255,.42)",
           "rgba(195,166,255,.54)", "rgba(195,166,255,.66)", "rgba(195,166,255,.8)", "#C3A6FF"]


def coexist_win(r):
    C = PRO_POWER["coexist"]
    lim = C["limit"]
    g3 = lambda x: pn(f"{x:.3g}")
    worst = max((p for p in C["plans"] if p["worst"] is not None), key=lambda p: p["worst"])
    wv = max(range(len(worst["caused"])), key=lambda j: worst["caused"][j]["loss"] or 0)
    names = {v["id"]: v["name"] for v in C["victims"]}

    def cell_read(p, c):
        s = f'<b>{esc(p["label"])} → {esc(names[c["victim"]])}</b> · '
        if not c["overlap"]:
            return s + "no overlap in frequency: no interference"
        if p is worst and c is worst["caused"][wv]:
            return s + (f'worst loss caused {g3(c["loss"])} dB (decibel), at a cell with {pn(_n(c["in_view"]))} of its satellites in view: '
                        f'a margin of {g3(c["margin"])} dB to the {pn(_n(lim))} dB limit · <span class="ok">compatible</span>')
        if c["loss"] == p["worst"]:
            return s + f'at most {g3(c["loss"])} dB, with {pn(_n(c["in_view"]))} in view · <span class="ok">compatible</span>'
        return s + f'within the {pn(_n(lim))} dB limit · <span class="ok">compatible</span>'
    reads, rows = {}, []
    for i, p in enumerate(C["plans"]):
        cells = []
        for j, c in enumerate(p["caused"]):
            reads[f"{i}:{j}"] = cell_read(p, c)
            on = p is worst and j == wv
            if c["overlap"]:
                w = max(2.5, c["loss"] / lim * 100)
                cells.append(f'<button type="button" class="cx-c ov{" on" if on else ""}" data-cx="{i}:{j}" aria-label="{esc(p["label"])} to {esc(names[c["victim"]])}: within the limit"><b class="cx-v">{esc(names[c["victim"]])}</b><span class="cx-t"><i style="width:{w:.1f}%"></i></span><span>within limit</span></button>')
            else:
                cells.append(f'<button type="button" class="cx-c no" data-cx="{i}:{j}" aria-label="{esc(p["label"])} to {esc(names[c["victim"]])}: no overlap in frequency"><b class="cx-v">{esc(names[c["victim"]])}</b><span>no overlap</span></button>')
        rows.append(f'<div class="cx-row"><span class="cx-p">{esc(p["label"])}{"<em>ranked front</em>" if p["front"] else ""}</span>{"".join(cells)}</div>')
    head = '<div class="cx-row cx-h"><span class="cx-p">Plan · signal</span>' + "".join(f'<span>{esc(v["name"])}</span>' for v in C["victims"]) + '</div>'
    assert [p["label"] for p in C["plans"] if p["front"]] == C["front"]
    # the world map of the worst loss one plan causes (per cell of the worldwide grid)
    M = C["map"]
    lat, lon, cells = M["lat"], M["lon"], M["cells"]
    Wd, Hd = 720, 360
    cw, ch = Wd / len(lon), Hd / len(lat)
    o = [f'<svg viewBox="0 0 {Wd} {Hd}" shape-rendering="crispEdges" role="img" aria-label="World map of the worst loss the plan {esc(M["plan"])} causes, per cell: every cell within the limit">',
         f'<rect class="cx-sea" x="0" y="0" width="{Wd}" height="{Hd}"/>']
    order = sorted(range(len(lat)), key=lambda k: -lat[k])
    for yi, k in enumerate(order):
        row, x0, lv0 = cells[k], 0, None
        for xi in range(len(row) + 1):
            lv = None if xi == len(row) or row[xi] is None else min(7, int(row[xi] * 8))
            if xi == len(row) or lv != lv0:
                if lv0 is not None:
                    o.append(f'<rect fill="{CX_RAMP[lv0]}" x="{x0 * cw:.0f}" y="{yi * ch:.0f}" width="{(xi - x0) * cw:.0f}" height="{ch:.0f}"/>')
                x0, lv0 = xi, lv
    for ring in rings:
        pts = [f"{(x + 180) / 360 * Wd:.0f},{(90 - y) / 180 * Hd:.0f}" for x, y in ring]
        pts = [q for i, q in enumerate(pts) if i == 0 or q != pts[i - 1]]
        if len(pts) >= 3:
            o.append('<path class="cx-land" d="M' + " L".join(pts) + 'Z"/>')
    o.append("</svg>")
    mapsvg = "".join(o)
    ramp = "".join(f'<i style="--c:{c}"></i>' for c in CX_RAMP)
    rx = max(p["received_max"] for p in C["plans"] if p["received_max"] is not None and p["label"].startswith("L band"))
    return (f'<figure class="pw-win" id="pwCoexist"><div class="pw-bar"><span>Spectrum coexistence · <b>{_n(len(C["plans"]))} example signal plans</b></span><span class="pw-r"><span class="pw-tag">Kshana Pro output</span></span></div>'
            f'<div class="pw-seg" role="group" aria-label="View"><button type="button" aria-pressed="true" data-cx-view="0">Plans against signals</button><button type="button" aria-pressed="false" data-cx-view="1">World map of the worst loss</button></div>'
            f'<div class="pw-panel" data-cx-panel="0"><div class="cx" data-reads="{esc(json.dumps(reads))}" role="group" aria-label="Loss each plan causes to each signal, against the limit">{head}{"".join(rows)}</div>'
            f'<p class="pw-read" id="pwCoexistRead" aria-live="polite">{reads[f"{C["plans"].index(worst)}:{wv}"]}</p></div>'
            f'<div class="pw-panel" data-cx-panel="1" hidden><div class="pw-plot cx-map">{mapsvg}</div>'
            f'<div class="pw-legend cx-legend"><span>0 dB</span><span class="cx-ramp">{ramp}</span><span>{g3(M["max"])} dB, the worst cell · limit {pn(_n(lim))} dB</span></div>'
            f'<p class="pw-read">Worst loss the plan <b>{esc(M["plan"])}</b> causes at each cell of the worldwide grid, over the signals it overlaps: an upper bound, every satellite in view at the maximum received power.</p></div>'
            f'<p class="pw-prov"><span><b>{pn(_n(C["summary"]["plans_compatible"]))} of {pn(_n(C["summary"]["plans"]))}</b> plans compatible · the GNSS signals cost the L-band plans at most {g3(rx)} dB</span></p>'
            f'<p class="pw-prov"><span>Kshana Pro {esc(C["overlay"])} on open engine {esc(C["engine"])} · <b>kshana-pro coexist</b> · {pn(_n(C["runs"]))} engine runs ({", ".join(f"<code>{esc(k)}</code>" for k in C["member_kinds"])}) · '
            f'scenario hash {esc(_short(C["scenario_hash"]))} · MODELLED upper bound · the {pn(_n(lim))} dB limit was chosen for the example only, not a regulatory criterion · '
            f'C/A: coarse/acquisition; AltBOC: alternative binary offset carrier</span></p></figure>')


def jobs_win(r):
    S = PRO_POWER["jobs"]["steps"]
    st = {s["req"]: s for s in S}
    a, b, c, d, e, f_, g, k, n1, n2 = S
    assert a["code"] == 202 and a["state"] == "queued" and b["state"] == "succeeded" and b["attempts"] == 1 and c["same"] and not c["created"]
    assert d["verified"] and d["intact"] and d["mismatches"] == 0 and not f_["external"] and g["attempts"] == 1
    assert k["ledger"]["verified"] and k["after"]["succeeded"] == 3 and n1["queued_after"] == 0 and n2["state"] == "queued"
    rerun = [j for j in k["jobs"] if j["attempts"] > 1]
    assert len(rerun) == 1 and all(j["state"] == "succeeded" for j in k["jobs"])
    items = [
        ("POST /jobs", "a link-budget scenario", a["code"], f'accepted and queued · job {esc(_short(a["job"]))}'),
        ("GET /jobs/{id}", "", b["code"], f'succeeded on its first attempt · kind <code>{esc(b["kind"])}</code>'),
        ("POST /jobs", "the same request again", c["code"], "the same job, already finished: nothing run twice"),
        ("GET /ledger/verify", "", d["code"], f'verified, chain intact · {pn(_n(d["records"]))} records · {pn(_n(d["delivered"]))} delivered job · no mismatch'),
        ("GET /openapi.json", "", e["code"], f'OpenAPI {pn(e["openapi"][:3])} · {pn(_n(e["paths"]))} paths, from the service\'s own route table'),
        ("GET /", "the job board", f_["code"], "loads nothing from outside"),
        ("restart", "then GET /jobs/{id}", g["code"], "the finished job still shows one attempt"),
        ("kill -9", "while a job runs, then restart", None, f'the unfinished job ran again and succeeded; the finished job{"s were" if k["before"]["succeeded"] != 1 else " was"} not run again · ledger {pn(_n(k["ledger"]["records"]))} records, {pn(_n(k["ledger"]["delivered_jobs"]))} delivered jobs, verified'),
        ("no licence: POST /jobs", "a Pro scenario", n1["code"], "refused on submission: nothing queued"),
        ("no licence: POST /jobs", "an open scenario", n2["code"], "accepted and queued"),
    ]
    li = "".join(f'<li class="{"bad" if (code or 0) >= 400 else ""}"><span class="sj-n">{i + 1:02d}</span><span class="sj-rq"><b>{esc(m)}</b>{(" <em>" + esc(w) + "</em>") if w else ""}</span>'
                 f'<span class="sj-code">{code if code else "—"}</span><span class="sj-rs">{t}</span></li>' for i, (m, w, code, t) in enumerate(items))
    return (f'<figure class="pw-win" id="pwJobs"><div class="pw-bar"><span>On-premises job service · <b>one recorded session</b></span><span class="pw-r"><span class="pw-tag">Kshana Pro output</span></span></div>'
            f'<ol class="sj" aria-label="Requests to the service and what it answered, in order">{li}</ol>'
            f'<p class="pw-prov"><span>Kshana Pro on open engine {esc(PRO_POWER["watch"]["engine"])} · <b>kshana-pro serve --data-dir</b>, {pn("two")} workers (the restart test: one) · '
            f'HTTP status codes as the service returned them · a local run on public scenario files with the development entitlement, not a customer deployment</span></p></figure>')


def pro_trio(r):
    """The three newer Pro answers behind one switcher (Editions #pro-watch, #pro-coexist, #pro-jobs)."""
    def panel(aid, i, key, q, free, pro, how, nots, win):
        return (f'<article class="pw-amp pw-tp" id="{aid}" role="tabpanel" aria-labelledby="{aid}-t" data-pw3p="{i}"{"" if i == 0 else " hidden"}>'
                f'<div><h3 class="pw-q">{esc(q)}</h3>'
                f'<ul class="pw-ladder"><li class="fr"><span class="pw-ed">Free</span><span>{esc(free)}</span></li><li class="pr"><span class="pw-ed">Pro</span><span>{esc(pro)}</span></li></ul>'
                f'<p class="pw-how">{esc(how)}</p><p class="pw-not"><b>What it does not do.</b> {esc(nots)}</p></div>{win}</article>')
    ps = [
        ("pro-watch", "Campaign watch", pq("Did the last change make a figure we care about worse?"),
         pq("A free user runs a campaign and reads the result."),
         pq("A Pro user names the figures that matter in a set of scenarios, says what worse means for each, and gets a pipeline that fails when a change to a scenario, a model or the engine makes one of them worse, or stops meeting its requirement."),
         pq("It extends the scenario regression check.") + " " + pq("A Monte Carlo figure is judged with statistics instead of a tolerance")
         + ", so " + pq("a new set of random seeds is not called a regression and a doubled spread is.") + " "
         + pq("Every run is appended to a history in which each record carries the hash of the one before, and a check reports whether that chain is intact."),
         pq("A watch proves that a MODELLED figure stayed stable or moved; it does not prove the figure is true.") + " " + pq("A figure with no stated direction is reported and never fails a run."),
         watch_win(r)),
        ("pro-coexist", "Spectrum coexistence", pq("Will our signal plan interfere with GNSS, or suffer from it?"),
         pq("A free user checks one satellite's signal against one GNSS signal."),
         pq("A Pro user tests one or more candidate signal plans, each a signal design and a constellation, against every open-service GNSS signal the engine models, with every satellite of the plan in view at once, and gets a verdict per plan and signal against a limit the study states."),
         pq("Both directions are tested: the loss of C/N0 each plan causes to each GNSS signal, and the loss the GNSS signals cause to the plan.") + " "
         + pq("Pro adds only the sum over the satellites in view at each cell of a worldwide grid and the ranking of the plans.") + " " + pq("The limit and its source are inputs; no limit is built in."),
         pq("The result is an upper bound: every satellite in view is taken at the maximum received power, with no power that varies with elevation and no receive-antenna pattern.") + " "
         + pq("It is not a coordination filing and not a regulatory finding."),
         coexist_win(r)),
        ("pro-jobs", "On-premises job service", pq("Can the team queue studies on its own network and prove what was delivered?"),
         pq("A free user runs one scenario at a time on one machine."),
         pq("A Pro team queues studies on its own network, lets them run while no one watches, and can show afterwards exactly which result was delivered for which request."),
         pq("It extends the on-premises service") + ": " + pq("Pro can run as a small") + " HTTP (Hypertext Transfer Protocol) "
         + pq("service on your own network: it reports its health, its version and the kinds it can run, and runs a scenario posted to it.") + " "
         + pq("A job's identifier is the SHA-256 of the operation and the request, so the same request is the same job and is not run twice.") + " "
         + pq("After a stop or a crash, every unfinished job is queued again and no finished job is run again.") + " "
         + pq("Without a licence it serves the open engine only and refuses every Pro feature."),
         pq("One request per connection, with no streaming: a client polls.") + " " + pq("One licence for the whole service and one shared access token, not user management.") + " "
         + pq("No cancellation, time limit, priority or retention policy."),
         jobs_win(r)),
    ]
    tabs = "".join(f'<button type="button" role="tab" id="{a}-t" aria-controls="{a}" aria-selected="{"true" if i == 0 else "false"}"{"" if i == 0 else " tabindex=\"-1\""} data-pw3="{i}">'
                   f'{esc(k)}</button>' for i, (a, k, q, *_x) in enumerate(ps))
    return (f'<div class="pw-trio" id="pro-ops"><div class="pw-trio-h"><span class="pw-k"><i></i>Three more answers</span>'
            f'<div class="pw-trio-tabs" role="tablist" aria-label="More Kshana Pro answers">{tabs}</div></div>'
            + "".join(panel(a, i, *x) for i, (a, *x) in enumerate(ps)) + '</div>')


def pro_power(r):
    P = PRO_POWER
    d0 = P["design"][0]
    # ---- 1 design optimiser
    fronts = "".join(f'<div class="pw-plot pw-panel" data-pw-region="{i}"{"" if i == 0 else " hidden"}>{front_svg(d, i)}</div>' for i, d in enumerate(P["design"]))
    seg = "".join(f'<button type="button" aria-pressed="{"true" if i == 0 else "false"}" data-pw-region-btn="{i}">{esc(d["region"])}</button>' for i, d in enumerate(P["design"]))
    kn = next(f for f in d0["front"] if f["id"] == d0["knee"]["id"])
    design_read = (f'<b>Front design {esc(kn["id"])} · knee</b> · {pn(_n(kn["sat"]))} satellites, {pn(_n(kn["av"]))} % availability · '
                   f'{pn(_n(kn["v"]["planes"]))} planes at {pn(_n(kn["v"]["altitude_km"]))} km, {pn(_n(kn["v"]["inclination_deg"]))}° · '
                   f're-run in the free engine: <span class="ok">same SHA-256 {esc(_short(kn["sha"]))}</span>')
    caps = "".join(
        f'<span data-pw-region-cap="{i}"{" hidden" if i else ""}>{_n(d["runs"])} designs, all run ({esc(d["region"].lower())}): '
        f'a front of {len(d["front"])} designs at {d["front_points"]} points; {sum(f["free"] for f in d["front"])} of {len(d["front"])} '
        f're-run in the free engine with the same result hash. {esc(d["label"].split(" - ")[0])}.</span>'
        for i, d in enumerate(P["design"]))
    design_win = (f'<figure class="pw-win" id="pwFront"><div class="pw-bar"><span>Pareto front · <b>{esc(d0["title"].split(":")[0])}</b></span><span class="pw-r"><span class="pw-tag">Kshana Pro output</span></span></div>'
                  f'<div class="pw-seg" role="group" aria-label="Region scored">{seg}</div>{fronts}'
                  f'<div class="pw-legend"><span><i class="dot" style="--c:rgba(238,240,250,.45)"></i>every design run</span><span><i style="--c:var(--pw-p)"></i>Pareto front</span><span><i class="dot" style="--c:var(--pw-a)"></i>knee</span><span><i class="dia" style="--c:var(--pw-b)"></i>reference: {esc(d0["reference"]["label"])}</span></div>'
                  f'<p class="pw-read" id="pwFrontRead" aria-live="polite">{design_read}</p>'
                  f'<figcaption class="pw-prov" id="pwFrontCap">{caps}</figcaption>'
                  + _prov(r, "kshana-pro design", d0["engine_runs"], d0["engine"], d0["overlay"], d0["digest"], []) + '</figure>')
    # ---- 2 uncertainty
    b, s = P["band"], P["sensitivity"]
    bsvg, geo = band_svg(b)
    # GPS, LEO and dB-Hz are spelled out where this section first uses them on the page
    names = {i["key"]: i["name"].replace("GPS signal", "Global Positioning System (GPS) signal", 1) for i in s["inputs"]}
    unit1 = {"dB-Hz": "dB-Hz (decibel-hertz)"}
    sens_panels = "".join(f'<div class="pw-plot pw-panel" data-pw-out="{j}"{"" if j == 0 else " hidden"}>{sens_svg(o, names)}'
                          f'<p class="pw-read">{esc(o["name"])}: mean {pn(_n(o["mean"]))} {esc(unit1.get(o["unit"], o["unit"]))}, 5th to 95th percentile {pn(_n(o["p05"]))} to {pn(_n(o["p95"]))} {esc(o["unit"])} · top driver <b>{esc(names[o["inputs"][0]["key"]])}</b>, total index {pn(format(o["inputs"][0]["total"], ".2f"))}</p></div>'
                          for j, o in enumerate(s["outputs"]))
    s_title = s["title"].replace("LEO-PNT chain", "Low-Earth-orbit (LEO) PNT chain", 1)
    outseg = "".join(f'<button type="button" aria-pressed="{"true" if j == 0 else "false"}" data-pw-out-btn="{j}">{esc(o["name"].split(",")[0])}{", " + esc(o["name"].split(", ")[1]) if ", " in o["name"] else ""}</button>' for j, o in enumerate(s["outputs"]))
    fb = b["final"]
    uq_win = (f'<figure class="pw-win" id="pwUq"><div class="pw-bar"><span>Uncertainty and sensitivity</span><span class="pw-r"><span class="pw-tag">Kshana Pro output</span></span></div>'
              f'<div class="pw-seg" role="tablist" aria-label="Uncertainty views"><button type="button" role="tab" aria-selected="true" id="pwUqT0" aria-controls="pwUqP0" data-pw-tab="0">Confidence band</button><button type="button" role="tab" aria-selected="false" tabindex="-1" id="pwUqT1" aria-controls="pwUqP1" data-pw-tab="1">What drives the result</button></div>'
              f'<div class="pw-panel" role="tabpanel" id="pwUqP0" aria-labelledby="pwUqT0" data-pw-tabp="0"><div class="pw-plot" id="pwBand">{bsvg}</div>'
              f'<div class="pw-legend"><span><i class="box" style="--c:var(--pw-b2)"></i>5th to 95th percentile of {_n(b["n"])} runs</span><span><i style="--c:var(--pw-b)"></i>median</span><span><i style="--c:rgba(238,240,250,.75)"></i>base run</span><span><i style="--c:var(--pw-g)"></i>share within the limit</span></div>'
              f'<p class="pw-read" id="pwBandRead" aria-live="polite">After {pn(_n(b["t"][-1] / 60))} minutes: <b>{pn(_n(fb["met"]))} of {pn(_n(fb["of"]))} runs</b> within ±{pn(_n(b["limit"]))} {esc(b["unit"])}, a probability of {pn(_n(fb["p"]))} (95 % interval {pn(_n(fb["lo"]))} to {pn(_n(fb["hi"]))})</p>'
              + _prov(r, "kshana-pro uncertainty", b["n"], b["engine"], b["overlay"], b["digest"], [], f'{b["title"]} · input {b["input"]["name"]}{" (the clock" + chr(39) + "s white-frequency-noise level)" if b["input"]["name"] == "q_wf" else ""}: {b["input"]["dist"]}, ASSUMED · {b["label"].split(":")[0]}') + '</div>'
              f'<div class="pw-panel" role="tabpanel" id="pwUqP1" aria-labelledby="pwUqT1" data-pw-tabp="1" hidden><div class="pw-seg pw-seg2" role="group" aria-label="Output">{outseg}</div>{sens_panels}'
              + _prov(r, "kshana-pro uncertainty", s["n"], s["engine"], s["overlay"], s["digest"], [], f'{s_title} · Sobol\' indices, {s["cost"]} · {len(s["inputs"])} of {len(s["inputs"])} input uncertainties ASSUMED · MODELLED') + '</div></figure>')
    # ---- 3 mission dossier
    D = P["dossier"]
    sm = D["summary"]
    tiles = "".join(f'<div style="--c:{c}"><b>{_n(v)}</b><span>{t}</span></div>' for v, t, c in (
        (sm["requirements"], "requirements", "var(--pw-ink)"), (sm["met"], "met", "var(--pw-g)"), (sm["not_met"], "not met", "var(--pw-r)"),
        (sm["not_verified"], "not verified", "var(--pw-a)"), (sm["open_items"], "open items", "var(--pw-p)")))
    rows = []
    for m in D["matrix"]:
        vcls = m["verdict"].replace(" ", "-")
        basis = f'<span class="pw-chip basis">{esc(m["basis"])}</span>' if m["basis"] else ""
        val = f'{_n(m["value"])} {esc(m["unit"] or "")}'.strip() if m["value"] is not None else "no run traced"
        rows.append(f'<li><details><summary><span class="id">{esc(m["id"])}</span><span class="crit">{esc(m["criterion"] or "no machine-checkable criterion")}</span>'
                    f'<span class="vd"><span class="pw-chip {vcls}">{esc(m["verdict"])}</span>{basis}</span></summary>'
                    f'<div class="det"><p>{esc(m["text"])}</p><p>Method: {esc(m["method"])} · run {esc(m["run"] or "none")} · value read: {val}</p>'
                    + (f'<code>{esc(m["detail"])}</code>' if m["detail"] else "")
                    + (f'<p>Open item: {esc(", ".join(m["open"]))}</p>' if m["open"] else "") + '</div></details></li>')
    pdf = r + P["files"]["dossier_pdf"]
    pages = "".join(f'<a href="{esc(pdf)}#page={i + 1}" aria-label="Page {i + 1} of the generated dossier (PDF)"><img src="{r}{esc(p)}" alt="Page {i + 1} of the generated mission dossier" width="600" height="848"></a>' for i, p in enumerate(P["files"]["dossier_pages"]))
    dossier_win = (f'<figure class="pw-win" id="pwDossier"><div class="pw-bar"><span>Mission dossier · <b>{esc(D["title"])}</b></span><span class="pw-r"><span class="pw-tag">Kshana Pro output</span></span></div>'
                   f'<div class="pw-tiles">{tiles}</div><ul class="pw-mx" aria-label="Verification matrix: open a row for the check the dossier made">{"".join(rows)}</ul>'
                   f'<div class="pw-pages">{pages}<p>{_n(D["pages"])} pages, one command. <a href="{esc(pdf)}">Open the generated PDF</a> (Portable Document Format)</p></div>'
                   + f'<p class="pw-prov"><span>Kshana Pro {esc(D["overlay"])} on open engine {esc(", ".join(D["engine"]))} · <b>kshana-pro mission build</b> · {_n(len(D["runs"]))} runs and one trade study · {_n(sm["met_on_modelled_basis"])} met, all on a MODELLED or PARTNER basis · dossier SHA-256 {esc(_short(D["sha"]))} · {esc(D["build"])} build</span></p></figure>')

    def amp(aid, key, q, free, pro, how, nots, win):
        return (f'<article class="pw-amp" id="{aid}"><div><span class="pw-k"><i></i>{esc(key)}</span><h3 class="pw-q">{esc(q)}</h3>'
                f'<ul class="pw-ladder"><li class="fr"><span class="pw-ed">Free</span><span>{esc(free)}</span></li><li class="pr"><span class="pw-ed">Pro</span><span>{esc(pro)}</span></li></ul>'
                f'<p class="pw-how">{esc(how)}</p><p class="pw-not"><b>What it does not do.</b> {esc(nots)}</p></div>{win}</article>')

    amps = [
        amp("pro-design", "Design optimiser", pq("Which design should we fly?"),
            pq("A free user scores one design they wrote by hand."),
            pq("A Pro user states the design space (each variable a scenario setting with its allowed values), the constraints and two or three objectives, and gets the whole Pareto front: the designs that no other design beats on every objective at once."),
            pq("The base scenario can be of any open kind, so the same tool designs a constellation around the Earth, the Moon or Mars, or trades a link budget.") + " "
            + pq("Every design is one run of the open engine.") + " Each front design is written out as a plain open scenario file, with the Secure Hash Algorithm (SHA-256) hash of the result the free engine gives for it.",
            pq("It has no cost model, so a variable no objective penalises drifts to the edge of its range.") + " " + pq("Constellation studies score geometry only: no signal power, satellite health or terrain."),
            design_win),
        amp("pro-uncertainty", "Uncertainty and sensitivity", pq("How sure are we, and what drives the result?"),
            pq("A free user gets one number, or a spread over random seeds."),
            pq("A Pro user states how uncertain each input is and gets confidence bands, the probability of meeting a limit, and a ranking of the inputs that drive the result."),
            pq("It works on any open scenario.") + " " + pq("Inputs are ranked by Sobol' sensitivity indices or by standardised regression coefficients, and a ranking that is not reliable is flagged as such.") + " " + pq("A long study can be stopped and resumed."),
            pq("Inputs are treated as independent.") + " " + pq("More runs narrow the sampling error, not the model error: a MODELLED kind stays MODELLED."),
            uq_win),
        amp("pro-dossier", "Mission dossier", pq("Does the mission meet its requirements, and can we hand that over?"),
            pq("A free user gets one report per run."),
            pq("A Pro user gets one mission dossier in one command: requirements checked against real runs, a verification matrix with its open items, and a") + " PDF (Portable Document Format) " + pq("in which every number can be re-derived with the free engine."),
            pq("A requirement met on a MODELLED or PARTNER basis is flagged.") + " " + pq("Requirements to be verified by test, inspection or demonstration stay open items however the run went."),
            pq("Simulation evidence counts as analysis and does not replace a test, an inspection or a demonstration.") + " " + pq("A dossier is not a certification, a qualification or an acceptance by any authority."),
            dossier_win),
    ]
    pq("A free user gets one report per run. A Pro user gets one mission dossier in one command: requirements checked against real runs, a verification matrix with its open items, and a PDF in which every number can be re-derived with the free engine.")
    # ---- the rest of Pro
    reads_list = "".join(f"<li><code>{esc(n)}</code><span>{esc(w)}</span></li>" for n, w in (
        ("result.json", pq("the fields a criterion names, their units, the reproducibility stamp of scenario hash, seed, engine version and schema version, and the run's label and figure tiers")),
        ("report.json", pq("kind, reproducibility record and capability labels")), ("scenario file", pq("its hash, kind, seed and parameters")),
        ("field-units schema", pq("each field's unit, provenance and evidence tier")), ("verification matrix", pq("each capability's VALIDATED, MODELLED or PARTNER label"))))
    more = [
        ("pro-trace", "Requirements and traceability", "Kshana Pro's model-based systems-engineering (MBSE) " + pq("and programme tooling checks a programme's requirements against runs of the open engine: it imports requirements as comma-separated values (CSV), or as a documented subset of the Requirements Interchange Format (ReqIF) or of Systems Modeling Language version 2 (SysML v2) text") + ", and " + pq("produces a verification cross-reference matrix (VCRM), a SysML v2 model, a change-impact report and an offline evidence pack.") + " " + pq("It re-runs and recomputes nothing.")
         , f'<details><summary>The open engine\'s outputs it reads</summary><ul class="pb-reads">{reads_list}</ul></details>'),
        ("pro-trade", "Trade studies", pq("A trade study runs every architecture option as an unchanged open-engine scenario, ranks the options on a figure of merit read from each run's result, and writes an evidence pack that records each option's scenario hash, so any figure in it can be reproduced by re-running that scenario."), ""),
        ("pro-twin", "Clock digital twin", pq("A clock twin is calibrated to a device's published Allan-deviation budget, then checked by estimating the twin's own Allan deviation with the open engine's estimator and comparing the two within a stated tolerance.") + " " + pq("A twin of your own device needs your characterisation data and is built under contract and a non-disclosure agreement."), ""),
        ("pro-study", "Study dossier", pq("One scenario becomes a reproducible technical note") + " in HTML (HyperText Markup Language) and JSON (JavaScript Object Notation): " + pq("scope, method, results with a tier on every figure, limitations, and the exact scenario text run."), ""),
    ]
    pq("It writes an HTML report and exits with a non-zero code when a figure regresses, so a continuous-integration pipeline can stop on it.")
    more_html = "".join(f'<article id="{i}"><h4>{esc(t)}</h4><p>{esc(p)}</p>{x}</article>' for i, t, p, x in more)
    terms = [
        ("Pro needs a licence.", pq("Without one, Pro runs open scenarios only and refuses every Pro feature with a message naming what is required.")),
        ("Some work needs a signed customer.", pq("A twin of your own device needs your characterisation data and is built under contract and a non-disclosure agreement.")),
        ("Nothing Pro produces is a certification.", pq("Every result is modelled, and says so.")),
    ]
    for t, _ in terms:
        pq(t)
    # work for a signed customer that the public text places with the custom study, not with Pro
    cs_terms = [("Models of your hardware.", "Sensor or " + pq("resilience models calibrated to your hardware") + " are built only under contract, with your data."),
                ("Export-controlled resilience models are not publicly offered.", pq("Such work is done only under the appropriate clearance and a non-disclosure agreement."))]
    pq("Export-controlled resilience models are not publicly offered")
    terms_html = ('<div class="pw-tl"><ul>' + "".join(f"<li><b>{esc(t)}</b> {esc(x)}</li>" for t, x in terms) + '</ul>'
                  + '<div class="plan pw-cs"><h4 class="pw-cs-h">Done as a custom study</h4><ul>'
                  + "".join(f"<li><b>{esc(t)}</b> {esc(x)}</li>" for t, x in cs_terms) + '</ul></div></div>')
    return f'''<section class="pro-band pw" id="pro" data-nav-dark aria-labelledby="pwH">
  <div class="wrap">
    <div class="pw-head">
      <span class="eyebrow eyebrow-d" style="--c:var(--pw-p)"><i></i>Kshana Pro · the same engine, amplified</span>
      <h2 class="h2 h2-d" id="pwH">Free runs one scenario. Pro answers the programme's question.</h2>
      <p class="lede lede-d">{esc(PW_LEDE)}</p>
    </div>
    <ul class="pw-principles">{"".join(f"<li><b>{esc(t)}</b><span>{esc(x)}</span></li>" for t, x in PW_PRINCIPLES)}</ul>
    {"".join(amps)}
    {pro_trio(r)}
    <div class="pw-dh" id="pro-more"><h3 class="pb-dh">Also in Kshana Pro</h3></div>
    <div class="pw-more">{more_html}</div>
    <div class="pw-terms">{terms_html}<div class="pw-go"><a class="btn btn-white" href="mailto:contact@ashforde.org?subject=Kshana%20Pro%20evaluation">Request a Pro evaluation <svg aria-hidden="true"><use href="#i-arrow"/></svg></a><a class="pb-more" href="{r}docs/licensing.html">Licensing guide</a></div></div>
    <p class="pw-prov pw-src">The charts in this section are drawn from the files the Pro commands wrote for the worked examples that ship with Pro: modelled results, not statements about any real mission. The wording is quoted from the public Kshana Pro page of the open repository.</p>
  </div>
</section>'''


def pro_strip(r):
    """Home: three real Pro artefacts in the Pro band, each a route to its Editions section."""
    P = PRO_POWER
    d0, b, D = P["design"][0], P["band"], P["dossier"]
    sm = D["summary"]
    bsvg, _ = band_svg(b, 300, 130, mini=True)
    tiles = "".join(f'<div style="--c:{c}"><b>{_n(v)}</b><span>{t}</span></div>' for v, t, c in (
        (sm["met"], "met", "var(--pw-g)"), (sm["not_met"], "not met", "var(--pw-r)"), (sm["open_items"], "open items", "var(--pw-p)")))
    items = [
        ("pro-design", "Design optimiser", pq("Which design should we fly?"), front_svg(d0, 0, 300, 150, mini=True),
         f'<b>{_n(d0["runs"])} designs</b>, all run: the Pareto front and its knee'),
        ("pro-uncertainty", "Uncertainty and sensitivity", pq("How sure are we, and what drives the result?"), bsvg,
         f'<b>{_n(b["n"])} runs</b>: a confidence band against a limit'),
        ("pro-dossier", "Mission dossier", pq("Does the mission meet its requirements, and can we hand that over?"), f'<div class="pw-tiles">{tiles}</div>',
         f'<b>{_n(sm["requirements"])} requirements</b> checked against real runs, {_n(D["pages"])}-page PDF (Portable Document Format)'),
    ]
    return ('<div class="pw-dh"><h3 class="pb-dh">Same engine, amplified: real Kshana Pro output</h3><a href="' + r + 'editions.html#pro">What Pro includes</a></div><div class="pw pw-strip">'
            + "".join(f'<a class="pw-tile" href="{r}editions.html#{a}"><span class="pw-k"><i></i>{esc(k)}</span><h3>{esc(q)}</h3>{v}<p class="pw-cap">{c}</p></a>' for a, k, q, v, c in items)
            + "</div>")


PRO_HERO_MINI = front_mini_hero(PRO_POWER["design"][0])
PRO_HERO_TEXT = f'Same engine, amplified: {_n(PRO_POWER["design"][0]["runs"])} designs, one Pareto front'
EXTRA_SCALARS.update({"{{PRO_HERO_MINI}}": PRO_HERO_MINI, "{{PRO_HERO_TEXT}}": esc(PRO_HERO_TEXT)})


# =================================================================== plotted data for the CSV buttons
def series_export():
    out = {}
    for cid, c in CHARTS.items():
        k, m = c.get("kind"), c.get("model")
        head = [f"# {c['file']}.toml · engine v{c['engine']}" + (f" · seed {c['seed']}" if c.get("seed") is not None else "") + " · points as plotted on kshana.dev; the full run exports from Kshana Studio"]
        if k == "line":
            rows = [["series", "x", "y"]] + [[s["label"], p[0], p[1]] for s in m["series"] for p in s["points"]]
            head.append(f"# x: {m.get('xLabel', '')} · y: {m.get('yLabel', '')}")
        elif k == "heat":
            rows = [["t_s", "satellite", "cn0_nominal_dbhz", "cn0_effective_dbhz", "js_db", "status"]]
            for t, row in zip(m["times"], m["cells"]):
                for prn, x in row.items():
                    rows.append([t, prn, x["cn0_nominal_dbhz"], x["cn0_effective_dbhz"], x["js_db"], x["status"]])
        elif k == "ground":
            rows = [["lat_deg", "lon_deg", "visible"]] + [[p["lat"], p["lon"], p.get("visible")] for p in m["points"]]
        elif k == "mask":
            rows = [["metric", "tau_s", "value_ns"]] + [["mtie", *p] for p in m["mtie"]] + [["tdev", *p] for p in m["tdev"]]
        else:
            flat = []

            def walk(pfx, v):
                if isinstance(v, dict):
                    for kk, vv in v.items():
                        walk(f"{pfx}.{kk}" if pfx else kk, vv)
                elif isinstance(v, list) and v and isinstance(v[0], (dict, list)):
                    for i, vv in enumerate(v):
                        walk(f"{pfx}.{i}", vv)
                elif not isinstance(v, list) or len(v) < 400:
                    flat.append([pfx, json.dumps(v) if isinstance(v, list) else v])
            walk("", c.get("figures"))
            rows = [["field", "value"]] + flat
        out[cid] = {"file": f"{cid}.csv", "head": head, "rows": rows}
    return out


LUNAR_TIME_FIG = register_svg("tw-lunar", 420, 190, "rate", lunar_time_svg())
EXTRA_SCALARS["{{LUNAR_TIME}}"] = fig_html("", "tw-lunar", 420, 190, "rate", " viz-dark", "Lunar time rate: the engine's terms against the published band")

# Generated blocks. Page sources place them with {{NAME}}; every one is real engine or repository
# data and carries its own behaviour through site.js (data-flist, data-ks-tabs, data-dl-*).
BLOCKS = {
    "{{SECTOR_TEASERS}}": sector_teasers, "{{ROLES}}": roles, "{{WAYS}}": ways,
    "{{ATLAS}}": atlas, "{{EXPLORER}}": explorer, "{{BANDS}}": bands_block, "{{MCP_DEMO}}": mcp_demo,
    "{{ENGINE_BENTO}}": engine_bento,
    "{{PRO_BAND_HOME}}": lambda r: pro_band(r, "pro-band"), "{{PRO_BAND_EDITIONS}}": pro_power,
    "{{INSTALLER}}": lambda r: installer(r), "{{INSTALLER_COMPACT}}": lambda r: installer(r, True),
}
STATIC_BLOCKS = {"{{LEDGER}}": ledger_block, "{{LEDGER_SUMMARY}}": ledger_summary, "{{MATRIX_CELLS}}": matrix_cells, "{{STANDARDS}}": standards_grid,
                 "{{PUBS}}": pubs, "{{MCP_TOOLS}}": mcp_tools_html}
# ONE engine: every recorded run the site draws must come from the checkout the counts come from
# (src/data/sources.json: repo), at its current commit. A recording left over from another build
# stops the build here and names the recorder to re-run. {{BUILD}} is that commit.
HEAD_FULL = subprocess.run(["git", "-C", REPO, "rev-parse", "HEAD"], capture_output=True, text=True, check=True).stdout.strip()
# The build id shown on the pages: the first seven characters of the commit, the same fixed length
# every recorder in src/tools stores. (git's own --short grows with the repository: acd1c51 became
# acd1c51b on 2026-09-30 and made every recording look stale.)
HEAD = HEAD_FULL[:7]


def _mjs_default(rel):
    s = src(rel)
    return json.loads(s[s.index("export default ") + len("export default "):].rstrip().rstrip(";"))


_cap_eng = json.loads(open(os.path.join(HERE, "assets", "capabilities", "capabilities.json"), encoding="utf-8").read())["engine"]
_recorded_by = {
    "src/tools/record_runs.py": json.load(open(os.path.join(SRC, "data", "runs", "index.json")))["engine"],
    "src/tools/gen_waterfall.py": json.load(open(os.path.join(HERE, "assets", "waterfall", "home-waterfall.json")))["engine"],
    "src/tools/gen_capabilities.py": _cap_eng,
    "src/tools/gen_missions_runs.py": _mjs_default("js/missions-runs.mjs")["engine"],
    "src/tools/record_cli_session.py": json.loads(src("data/cli-session.json"))["engine"],
    "src/tools/gen_evidence_data.py": _mjs_default("js/evidence-data.mjs")["engine"],
    "src/tools/gen_publications.py": PUB_ENGINE,
}
_stale = [f"{tool}: recorded at {e.get('commit')} v{e.get('version')}" for tool, e in _recorded_by.items() if e.get("commit") != HEAD or e.get("version") != version]
if _stale:
    sys.exit(f"recorded runs are not from the site's engine checkout ({REPO} at {HEAD}, v{version}); re-run:\n  " + "\n  ".join(_stale))
assert _cap_eng["kinds"] == len(kinds), f"the engine binary lists {_cap_eng['kinds']} kinds, src/api.rs {len(kinds)}: rebuild the binary"
SCALARS = {"{{VER}}": version, "{{KINDS}}": str(len(kinds)), "{{BUILD}}": esc(HEAD), "{{FILES}}": str(len(bundled)), "{{ALLFILES}}": str(len(all_files)),
           "{{NOTBUNDLED}}": ", ".join(f"<code>{esc(f)}</code>" for f in not_bundled), "{{TOTAL}}": str(summ["total"]),
           "{{VAL}}": str(summ["validated"]), "{{MOD}}": str(summ["modelled"]), "{{PAR}}": str(summ["partner_owned"]),
           "{{NCAPS}}": str(len(cards)), "{{NSTD}}": str(len(standards)), "{{NTOOLS}}": str(len(mcp_tools)), "{{JAMTHR}}": f"{jam_thr:g}",
           "{{STUDIO}}": esc(STUDIO), "{{STUDIO_CTA}}": esc(STUDIO_CTA), "{{STUDIO_OPEN}}": esc(STUDIO_OPEN)}
SCALARS["{{NPUBS}}"] = str(len(PUBS))  # papers live on arXiv (src/data/publications.json)

def _orbit_extract(f):
    rec = json.load(open(os.path.join(PG_SRC, "recorded", f + ".json")))
    j = json.loads(rec["json"]) if isinstance(rec["json"], str) else rec["json"]
    return {"geometry": {k: v for k, v in j["geometry"].items() if not isinstance(v, (list, dict))}, "toml": rec["toml"]}


# Page data for the few interactive modules (everything else is static HTML).
PAGE_DATA = {
    # "index" (Home) is set with the Observatory Home block below.
    "capabilities": {"hold": {k: CHARTS[k] for k in ("clk-hold-a", "clk-hold-b", "clk-slot")},
                     # "One block, a different answer": the two recorded orbit runs' geometry figures and TOML.
                     "orbits": {f: _orbit_extract(f) for f in re.findall(r'class="cz-col" data-run="([\w-]+)"', raw_pages["capabilities"])}},
}
PAGE_SCRIPTS = {
    "index": '<script type="importmap">{"imports":{"three":"https://cdn.jsdelivr.net/npm/three@0.160.0/build/three.module.js"}}</script>\n<script type="module" src="js/home.mjs"></script>',
    "missions": '<script type="importmap">{"imports":{"three":"https://cdn.jsdelivr.net/npm/three@0.160.0/build/three.module.js"}}</script>\n<script type="module" src="js/missions.mjs"></script>',
    "capabilities": '<script type="importmap">{"imports":{"three":"https://cdn.jsdelivr.net/npm/three@0.160.0/build/three.module.js"}}</script>\n<script type="module" src="js/capabilities.mjs"></script>',
    "evidence": '<script type="module" src="js/evidence.mjs"></script>',
    "developers": '<script type="module" src="js/developers.mjs"></script>',
    "editions": "",
}
# =================================================================== Home on the Observatory system (2026-09-28)
# Every number drawn here is read from a recorded engine run: the Studio's recordings (CHARTS),
# the Missions footprint runs (FOOTPRINT in src/js/missions.mjs), or the integration-engine runs
# in src/data/runs/ (written by src/tools/record_runs.py, which runs the engine; the build never does).
RUNS_DIR = os.path.join(SRC, "data", "runs")
RUNS_IDX = json.load(open(os.path.join(RUNS_DIR, "index.json")))
HOME_RUNS = json.load(open(os.path.join(RUNS_DIR, "home.json")))
CAMP, CONST = HOME_RUNS["campaign"], HOME_RUNS["constellation"]
ENG = RUNS_IDX["engine"]
assert ENG["version"] == version, f"recorded runs are engine {ENG['version']}, the site is {version}: re-run src/tools/record_runs.py"
assert CONST["residual_deg"] < 0.2, "constellation orbit reconstruction residual too large"

# Publish each run: the scenario (served as text), the engine's result and its own report.
_runs_out = os.path.join(HERE, "assets", "runs")
if os.path.isdir(_runs_out):
    shutil.rmtree(_runs_out)
for _name in RUNS_IDX["runs"]:
    _d = os.path.join(_runs_out, _name)
    os.makedirs(_d, exist_ok=True)
    shutil.copyfile(os.path.join(RUNS_DIR, _name, "scenario.toml"), os.path.join(_d, _name + ".toml.txt"))
    shutil.copyfile(os.path.join(RUNS_DIR, _name, "result.json"), os.path.join(_d, "result.json"))
    shutil.copyfile(os.path.join(RUNS_DIR, _name, "report.html"), os.path.join(_d, "report.html"))


def prov_run(root, name, lead=None):
    """Provenance for an integration-engine run: version, build, scenario file, seed, and the routes
    to check and reproduce it (the engine's own report, its result JSON, the TOML and the command)."""
    r = RUNS_IDX["runs"][name]
    seed = f" · seed {r['seed']}" if r["seed"] is not None else " · deterministic, no seed"
    base = f"{root}assets/runs/{name}/"
    lead_txt = f"<b>{esc(lead)}:</b> " if lead else ""
    return (f'<span class="prov-src">{lead_txt}Engine v{esc(ENG["version"])}, build <code>{esc(ENG["commit"])}</code> · <code>{esc(name)}.toml</code>{seed}</span>'
            f'{studio_open(root, name)}'
            f'<span class="prov-tools"><a class="prov-x" href="{base}report.html" title="The engine\'s own report of this run">Report</a>'
            f'<a class="prov-x" href="{base}result.json" title="The engine\'s result, unmodified">JSON</a>'
            f'<a class="prov-x" href="{base}{esc(name)}.toml.txt" title="The scenario file that ran">TOML</a>'
            f'<button class="prov-x" type="button" data-copy="kshana {esc(name)}.toml" title="Copy the command that reproduces it">Copy command</button></span>')


def hms(s):
    s = int(round(s))
    return f"T+{s // 3600:02d}:{s % 3600 // 60:02d}:{s % 60:02d}"


# ------------------------------------------------------------------ campaign facts (all read from the run)
_t, _ch = CAMP["t"], CAMP["ch"]
_ph = {p["name"]: p for p in CAMP["phases"]}


def _at(ts, key):
    i = max(k for k, x in enumerate(_t) if x <= ts)
    return _ch[key][i]


def _in(name, key):
    p = _ph[name]
    return [v for x, v in zip(_t, _ch[key]) if p["t0"] <= x < p["t1"] and v is not None]


def _first(pred, t0=0.0):
    return next((x for k, x in enumerate(_t) if x >= t0 and pred(k)), None)


def _jam(name):
    return next(r["jammer_dbw"] for r in _ph[name]["runs"] if r["kind"] == "jamming")


def _mask(name):
    return next(r["mask_deg"] for r in _ph[name]["runs"] if r["kind"] == "jamming")


_rf = CAMP["engine_events"][0]
_guard_t = _first(lambda k: _ch["time_error_ns"][k] > _ch["guard_ns"][k])
_ins_end = _in("holdover", "position_error_m")[-1]
_pl_ia = _in("integrity-alarm", "protection_level_m")
_evtxt = [e[3] for e in CAMP["events"]]
FACTS = {
    "{{CAMP_FILE}}": CAMP["file"], "{{CAMP_SEED}}": str(CAMP["seed"]), "{{CAMP_DUR}}": hms(CAMP["duration_s"]),
    "{{CAMP_NPH}}": str(len(CAMP["phases"])), "{{CAMP_NRUNS}}": str(sum(len(p["runs"]) for p in CAMP["phases"])),
    "{{CAMP_STEP}}": f"{CAMP['step_s']:g}",
    "{{CONST_FILE}}": CONST["file"], "{{CONST_N}}": str(CONST["total"]), "{{CONST_RES}}": f"{CONST['residual_deg']:g}",
    "{{CONST_SYS}}": ", ".join(c["name"] for c in CONST["constellations"][:-1]) + " and " + CONST["constellations"][-1]["name"],
    "{{CONST_MASK}}": f"{CONST['mask_deg']:g}", "{{CONST_VIS}}": f"{CONST['global']['mean_visible']:.0f}",
    "{{ENGINE_COMMIT}}": ENG["commit"],
    "{{S_TRK}}": f"{_at(10, 'tracking'):.0f}", "{{S_CN0}}": f"{_at(10, 'cn0_dbhz'):.1f}", "{{S_PL0}}": f"{min(_in('nominal', 'protection_level_m')):.1f}",
    "{{S_PL1}}": f"{max(_in('nominal', 'protection_level_m')):.1f}", "{{S_AL}}": f"{_at(10, 'alert_limit_m'):g}",
    "{{S_JAM1}}": f"{_jam('jamming'):g}", "{{S_CN0J}}": f"{_at(_ph['jamming']['t0'], 'cn0_dbhz'):.1f}", "{{S_FLOOR}}": f"{_at(10, 'cn0_floor_dbhz'):g}",
    "{{S_RF_T}}": hms(_rf["t_s"]), "{{S_DET_S}}": f"{_ph['spoofing']['t1'] - _ph['spoofing']['t0']:g}",
    "{{S_OFF}}": f"{_at(_ph['spoofing']['t1'] - CAMP['step_s'], 'time_error_ns'):.1f}",
    "{{S_RATE}}": re.search(r"Time push ([\d.]+) ns/s", next(e for e in _evtxt if e.startswith("Time push"))).group(1),
    "{{S_JAMKM}}": re.search(r"dBW, ([\d.]+) km away", next(e for e in _evtxt if "km away" in e)).group(1),
    "{{S_JAM2}}": f"{_jam('holdover'):g}", "{{S_CARRY}}": f"{_ph['holdover']['carried']['time_error_ns']:.1f}",
    "{{S_GUARD}}": f"{_at(10, 'guard_ns'):g}", "{{S_GUARD_T}}": hms(_guard_t), "{{S_GUARD_AFTER}}": f"{(_guard_t - _ph['holdover']['t0']) / 60:.0f}",
    "{{S_INS_KM}}": f"{_ins_end / 1000:.1f}", "{{S_MASK2}}": f"{_mask('integrity-alarm'):g}", "{{S_TRK2}}": f"{_at(_ph['integrity-alarm']['t0'], 'tracking'):.0f}",
    "{{S_PLMAX}}": f"{max(_pl_ia):.0f}", "{{S_TEMAX}}": f"{max(_in('holdover', 'time_error_ns')):.1f}",
    "{{PROV_CAMP}}": prov_run("", CAMP["file"]), "{{PROV_CONST}}": prov_run("", CONST["file"]),
    "{{PROV_CAMP_L}}": prov_run("", CAMP["file"], "Campaign"), "{{PROV_CONST_L}}": prov_run("", CONST["file"], "Orbits"),
}
PHASE_C = {"nominal": "--lime", "jamming": "--amber", "spoofing": "--magenta", "holdover": "--cyan", "integrity-alarm": "--coral", "recovery": "--lime"}
PHASE_N = {"nominal": "Nominal", "jamming": "Jamming", "spoofing": "Spoofing", "holdover": "Holdover", "integrity-alarm": "Integrity alarm", "recovery": "Recovery"}


def tl_scrub_labels():
    tot = CAMP["duration_s"]
    return "".join(f'<span style="--c:var({PHASE_C[p["name"]]});flex-basis:{(p["t1"] - p["t0"]) / tot * 100:.3f}%" data-i="{i}" data-n="{i + 1}" title="{esc(PHASE_N[p["name"]])}"><b class="nm">{esc(PHASE_N[p["name"]])}</b></span>' for i, p in enumerate(CAMP["phases"]))


def tl_scrub_fill():
    tot = CAMP["duration_s"]
    return "linear-gradient(90deg," + ",".join(f"var({PHASE_C[p['name']]}) {p['t0'] / tot * 100:.2f}% {p['t1'] / tot * 100:.2f}%" for p in CAMP["phases"]) + ")"


def tl_side():
    out = []
    for i, p in enumerate(CAMP["phases"]):
        runs = "".join(f'<li><code>{esc(r["kind"])}</code>' + (f'<small>seed {r["seed"]}</small>' if r["seed"] is not None else "") + "</li>" for r in p["runs"])
        out.append(f'<li class="scn" data-i="{i}" style="--c:var({PHASE_C[p["name"]]})"><span class="scn-h"><i></i>{esc(PHASE_N[p["name"]])}<small>{hms(p["t0"])}</small></span><ul>{runs}</ul></li>')
    return "".join(out)


FACTS["{{TL_LABELS}}"] = tl_scrub_labels()
FACTS["{{TL_FILL}}"] = tl_scrub_fill()
FACTS["{{TL_SIDE}}"] = tl_side()


# ------------------------------------------------------------------ bento visuals (Observatory tiles, real data)
def _lin(v0, v1, p0, p1):
    return lambda v: p0 + (v - v0) / ((v1 - v0) or 1) * (p1 - p0)


def _log(v0, v1, p0, p1):
    a, b = math.log10(v0), math.log10(v1)
    return lambda v: p0 + (math.log10(max(v, 1e-30)) - a) / ((b - a) or 1) * (p1 - p0)


def _pts(pts, X, Y):
    return " ".join(f"{X(x):.1f},{Y(y):.1f}" for x, y in pts)


def ob_svg(w, h, label, body):
    return (f'<svg class="ob-svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="{esc(label)}">{body}</svg>')


def ob_plot(svg):
    return f'<div class="ob-plot">{svg}</div>'


def _series(cid, label):
    return next(s["points"] for s in CHARTS[cid]["model"]["series"] if s["label"] == label)


def _fmt_s(s):
    if s >= 3600:
        return f"{s / 3600:g} h"
    if s >= 120:
        return f"{s / 60:g} min"
    return f"{s:g} s"


def _fmt_ns(v):
    if v >= 1000:
        return f"{v / 1000:.3g} µs"
    if v >= 1:
        return f"{v:.3g} ns"
    if v >= 1e-3:
        return f"{v * 1000:.3g} ps"
    return f"{v * 1e6:.3g} fs"


def _footprint():
    """FOOTPRINT from src/js/missions.mjs, the one place the per-cell runs are stored."""
    t = src("js/missions.mjs")
    i0 = t.index("const FOOTPRINT = {")
    blk = t[i0: t.index("};", i0)]
    g = lambda k: re.search(r"\b" + k + r":\s*([^,\n]+)", blk).group(1).strip().strip('"')
    arr = lambda k: [int(x) for x in re.search(r"\b" + k + r":\s*\[([^\]]+)\]", blk).group(1).split(",")]
    ship = [float(x) for x in re.search(r"ship:\s*\[([^\]]+)\]", blk).group(1).split(",")]
    return {"engine": g("engine"), "file": g("file"), "jlat": float(g("jlat")), "jlon": float(g("jlon")), "ship": ship,
            "step": int(g("step")), "nx": int(g("nx")), "ny": int(g("ny")), "js10": arr("js10"), "av": arr("av")}


FP = _footprint()


def svg_footprint():
    F = FP
    NX, NY = 2 * F["nx"] + 1, 2 * F["ny"] + 1
    W, H = 440, 210
    cell = min(246 / NX, (H - 10) / NY)
    mw, mh = cell * NX, cell * NY
    oy = (H - mh) / 2
    o = ['<defs><pattern id="obDots" width="12" height="12" patternUnits="userSpaceOnUse"><circle cx="1" cy="1" r="1" class="ob-dot"/></pattern></defs>',
         f'<rect width="{W}" height="{H}" fill="url(#obDots)"/>']
    jsv = [x / 10 for x in F["js10"]]
    lo, hi = min(jsv), 60.0
    k = 0
    for j in range(-F["ny"], F["ny"] + 1):
        for i in range(-F["nx"], F["nx"] + 1):
            js, av = jsv[k], F["av"][k] / 1000
            x, y = (i + F["nx"]) * cell, oy + (F["ny"] - j) * cell
            cls = "lost" if av <= 0 else ("part" if av < 1 else "held")
            op = 0.16 + 0.64 * max(0.0, min(1.0, (js - lo) / (hi - lo)))
            d = math.hypot(i, j) / math.hypot(F["nx"], F["ny"])
            o.append(f'<rect class="fp {cls}" x="{x:.2f}" y="{y:.2f}" width="{cell + .3:.2f}" height="{cell + .3:.2f}" style="--o:{op:.2f};--d:{d * 900:.0f}ms"/>')
            k += 1
    cx, cy = F["nx"] * cell + cell / 2, oy + F["ny"] * cell + cell / 2
    o.append('<g class="ring-anim">' + "".join(f'<circle class="pulse" cx="{cx:.1f}" cy="{cy:.1f}" r="{mh * .5:.1f}"/>' for _ in range(3)) + "</g>")
    o.append(f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="4" class="ob-jam"/>')
    kmLat, kmLon = 111.2, 111.32 * math.cos(math.radians(F["jlat"]))
    sx = cx + (F["ship"][1] - F["jlon"]) * kmLon / F["step"] * cell
    sy = cy - (F["ship"][0] - F["jlat"]) * kmLat / F["step"] * cell
    o.append(f'<path d="M{sx - 5:.1f} {sy + 4:.1f}L{sx + 5:.1f} {sy + 4:.1f}L{sx:.1f} {sy - 6:.1f}Z" class="ob-ship"/>')
    lost = sum(1 for a in F["av"] if a <= 0)
    part = sum(1 for a in F["av"] if 0 < a < 1000)
    n = len(F["av"])
    x0 = mw + 22
    o.append(f'<text x="{x0}" y="{oy + 12:.0f}" class="ob-k">{n} engine runs</text><text x="{x0}" y="{oy + 28:.0f}" class="ob-s">{F["step"]} km cells around the jammer</text>')
    for r_, (c, t_) in enumerate((("lost", f"{lost} lost every satellite"), ("part", f"{part} lost some epochs"), ("held", f"{n - lost - part} held tracking"))):
        yy = oy + 56 + r_ * 20
        o.append(f'<rect class="fp {c} key" x="{x0}" y="{yy - 9:.0f}" width="10" height="10" rx="2"/><text x="{x0 + 16}" y="{yy:.0f}" class="ob-s">{esc(t_)}</text>')
    o.append(f'<text x="{x0}" y="{oy + 134:.0f}" class="ob-s">darker = higher mean J/S</text><text x="{x0}" y="{oy + 150:.0f}" class="ob-s">peak {max(jsv):.1f} dB at the mast</text>')
    o.append(f'<text x="{x0}" y="{oy + 166:.0f}" class="ob-s">▲ the scenario\'s ship</text>')
    return ob_svg(W, H, "Jamming footprint: mean jammer-to-signal ratio and tracking on a grid of receiver positions around the jammer", "".join(o))


def svg_spoof():
    W, H = 440, 180
    cid = "sec-defence"
    off = _series(cid, "classical offset")
    bc = _series(cid, "classical bound")[0][1]
    bq = _series(cid, "quantum bound")[0][1]
    x1 = off[-1][0]
    ymax = max(max(p[1] for p in off), bc) * 1.14
    X, Y = _lin(0, x1, 12, W - 10), _lin(0, ymax, H - 22, 12)
    tq = next(p for p in off if p[1] > bq)
    tc = next(p for p in off if p[1] > bc)
    cap = max(p for p in off if p[1] == 0)
    o = [f'<line class="ob-grid" x1="0" x2="{W}" y1="{Y(ymax * k / 4):.1f}" y2="{Y(ymax * k / 4):.1f}"/>' for k in (1, 2, 3)]
    o.append(f'<line class="ob-thr amber" x1="12" x2="{W - 10}" y1="{Y(bc):.1f}" y2="{Y(bc):.1f}"/><text x="14" y="{Y(bc) - 5:.1f}" class="ob-s" style="fill:var(--amber)">classical clock\'s detection bound {bc:.3g} ns</text>')
    o.append(f'<polyline class="ob-tr" pathLength="1" style="stroke:var(--lime)" points="{X(0):.1f},{Y(0):.1f} {X(x1):.1f},{Y(0):.1f}"/>')
    o.append(f'<polyline class="ob-tr d2" pathLength="1" style="stroke:var(--magenta)" points="{_pts([p for p in off if p[0] >= cap[0]], X, Y)}"/>')
    o.append(f'<circle cx="{X(cap[0]):.1f}" cy="{Y(0):.1f}" r="5" class="ob-cap-halo"/><circle cx="{X(cap[0]):.1f}" cy="{Y(0):.1f}" r="5" class="ob-cap"/><text x="{X(cap[0]) - 8:.1f}" y="{Y(0) - 8:.1f}" text-anchor="end" class="ob-s">capture</text>')
    for p, col in ((tq, "--cyan"), (tc, "--amber")):
        o.append(f'<g class="ob-det" style="--c:var({col})"><circle cx="{X(p[0]):.1f}" cy="{Y(p[1]):.1f}" r="9"/><path d="M{X(p[0]) - 3.5:.1f} {Y(p[1]):.1f}l2.5 2.5 5-5"/></g>')
    o.append(f'<text x="{X(tq[0]) + 4:.1f}" y="{Y(0) - 30:.1f}" class="ob-s" style="fill:var(--cyan)">quantum clock flags it at {tq[0]:g} s (bound {bq:.3g} ns)</text><line x1="{X(tq[0]):.1f}" x2="{X(tq[0]):.1f}" y1="{Y(0) - 27:.1f}" y2="{Y(0) - 11:.1f}" class="ob-lead"/>')
    # below and right of the detection mark, clear of the bound label that runs along the line
    o.append(f'<text x="{W - 10}" y="{Y(tc[1]) + 22:.1f}" text-anchor="end" class="ob-s" style="fill:var(--amber)">classical clock flags it at {tc[0]:g} s</text>')
    o.append(f'<text x="{W - 12}" y="{Y(0) - 6:.1f}" text-anchor="end" class="ob-s" style="fill:var(--lime)">true time</text>')
    o.append(f'<text x="{X(off[-1][0]) - 4:.1f}" y="{Y(off[-1][1]) - 8:.1f}" text-anchor="end" class="ob-s" style="fill:var(--magenta)">spoofed time</text>')
    o.append(f'<text x="12" y="{H - 5}" class="ob-s">0 s</text><text x="{W - 10}" y="{H - 5}" text-anchor="end" class="ob-s">{x1:g} s · offset {off[-1][1]:g} ns</text>')
    return ob_svg(W, H, "A spoofer's time offset against each clock's detection bound, with the moment each monitor flags it", "".join(o))


def svg_clocks():
    W, H = 640, 200
    ser = CHARTS["sec-cni"]["model"]["series"]
    thr = CHARTS["sec-cni"]["model"]["threshold"]
    x1 = max(p[0] for s in ser for p in s["points"])
    classes = CHARTS["clk-lunar-classes"]["figures"]
    pos = [p[1] for s in ser for p in s["points"] if p[1] > 0] + [c["x_one_day_s"] * 1e9 for c in classes]
    lo, hi = 10 ** math.floor(math.log10(min(pos))), 10 ** math.ceil(math.log10(max(pos)))
    ml, mr = 46, 170
    X, Y = _lin(0, x1, ml, W - mr), _log(lo, hi, H - 22, 12)
    o = [f'<line class="ob-axis" x1="{ml}" y1="10" x2="{ml}" y2="{H - 22}"/><line class="ob-axis" x1="{ml}" y1="{H - 22}" x2="{W - mr}" y2="{H - 22}"/>']
    step = 2 if math.log10(hi / lo) > 6 else 1
    for e in range(int(round(math.log10(lo))), int(round(math.log10(hi))) + 1, step):
        y = Y(10 ** e)
        o.append(f'<line class="ob-grid" x1="{ml}" x2="{W - mr}" y1="{y:.1f}" y2="{y:.1f}"/><text x="{ml - 6}" y="{y + 3:.1f}" text-anchor="end" class="ob-s">{_fmt_ns(10 ** e)}</text>')
    o.append(f'<line class="ob-thr amber" x1="{ml}" x2="{W - mr}" y1="{Y(thr):.1f}" y2="{Y(thr):.1f}"/><text x="{W - mr - 6}" y="{Y(thr) + 13:.1f}" text-anchor="end" class="ob-s" style="fill:var(--amber)">guard {thr:g} ns</text>')
    cols = {"csac-sa45s p95": "--coral", "csac-sa45s median": "--amber", "optical-sr-lattice p95": "--cyan", "optical-sr-lattice median": "--lime"}
    for d, s in enumerate(ser):
        pts = [p for p in s["points"] if p[1] > 0]
        col = cols.get(s["label"], "--cyan")
        # 95th percentiles dashed, medians solid: the pair stays apart without colour (colour-vision check)
        if "p95" in s["label"]:
            o.append(f'<polyline class="ob-tr ob-dash d{d + 1}" style="stroke:var({col})" points="{_pts(pts, X, Y)}"/>')
        else:
            o.append(f'<polyline class="ob-tr d{d + 1}" pathLength="1" style="stroke:var({col})" points="{_pts(pts, X, Y)}"/>')
        lab = s["label"].replace("csac-sa45s", "CSAC").replace("optical-sr-lattice", "optical lattice")
        o.append(f'<text x="{X(pts[-1][0]) - 4:.1f}" y="{Y(pts[-1][1]) + (-6 if "p95" in s["label"] else 13):.1f}" text-anchor="end" class="ob-s" style="fill:var({col})">{esc(lab)}</text>')
    o.append(f'<text x="{ml}" y="{H - 6}" class="ob-s">GNSS lost · 0 s</text><text x="{W - mr}" y="{H - 6}" text-anchor="end" class="ob-s">{_fmt_s(x1)}</text>')
    cx = W - mr + 30
    o.append(f'<line class="ob-axis" x1="{cx}" y1="12" x2="{cx}" y2="{H - 22}"/><text x="{cx - 8}" y="{H - 6}" class="ob-s">after one day</text>')
    names = {"optical-master": "optical master", "passive-h-maser": "hydrogen maser", "rafs": "RAFS", "mini-rafs": "mini-RAFS"}
    ly_prev = -1e9
    for c in sorted(classes, key=lambda c: -c["x_one_day_s"]):
        v = c["x_one_day_s"] * 1e9
        y = Y(v)
        ly = max(y + 3, ly_prev + 13)  # keep labels 13 px apart; the dot stays at the value
        ly_prev = ly
        o.append(f'<g class="ob-cls"><circle cx="{cx}" cy="{y:.1f}" r="3.5"/><text x="{cx + 9}" y="{ly:.1f}" class="ob-s">{esc(names.get(c["clock"], c["clock"]))} · {_fmt_ns(v)}</text></g>')
    return ob_svg(W, H, "Clock time error after GNSS is lost: median and 95th percentile of 200 Monte Carlo runs for two clocks, and the one-day error of four oscillator classes", "".join(o))


def svg_cone():
    W, H = 280, 150
    fig = CHARTS["fail-nav"]["figures"]
    bud = fig["budget_m"]
    a = [p for p in _series("fail-nav", "tactical-imu") if p[0] >= 0]
    b = [p for p in _series("fail-nav", "bmi088-flight-controller") if p[0] >= 0]
    hold = next(i["holdover_s"] for i in fig["imus"] if i["id"] == "bmi088-flight-controller")
    x1 = max(p[0] for p in a)
    lim = bud * 1.3
    X, Y = _lin(0, x1, 16, W - 8), _lin(-lim, lim, H - 18, 12)
    cone = lambda pts: (f'M{X(0):.1f} {Y(0):.1f} ' + " ".join(f"L{X(x):.1f} {Y(min(e, lim)):.1f}" for x, e in pts) + " "
                        + " ".join(f"L{X(x):.1f} {Y(-min(e, lim)):.1f}" for x, e in reversed(pts)) + " Z")
    o = [f'<rect x="{X(0):.1f}" y="{Y(bud):.1f}" width="{X(x1) - X(0):.1f}" height="{Y(-bud) - Y(bud):.1f}" class="ob-band"/>',
         f'<line class="ob-thr" x1="{X(0):.1f}" x2="{X(x1):.1f}" y1="{Y(bud):.1f}" y2="{Y(bud):.1f}"/><line class="ob-thr" x1="{X(0):.1f}" x2="{X(x1):.1f}" y1="{Y(-bud):.1f}" y2="{Y(-bud):.1f}"/>',
         f'<path d="{cone(b)}" class="ob-cone amber"/>', f'<path d="{cone(a)}" class="ob-cone cyan"/>',
         f'<line x1="{X(0):.1f}" x2="{X(x1):.1f}" y1="{Y(0):.1f}" y2="{Y(0):.1f}" class="ob-tr" pathLength="1" style="stroke:var(--cyan)"/>',
         f'<line x1="{X(hold):.1f}" x2="{X(hold):.1f}" y1="10" y2="{H - 18}" class="ob-mark"/>',
         f'<circle cx="{X(0):.1f}" cy="{Y(0):.1f}" r="4.5" class="ob-lost"/>',
         f'<text x="2" y="{H - 4}" class="ob-s">GNSS lost</text><text x="{W - 8}" y="22" text-anchor="end" class="ob-s ob-halo" style="fill:var(--amber)">flight-controller IMU out at {hold:.1f} s</text>',
         f'<text x="{W - 8}" y="{H - 4}" text-anchor="end" class="ob-s">{x1:g} s · budget ±{bud:g} m</text>']
    return ob_svg(W, H, "Position-error cone after GNSS is lost for two inertial measurement units, against the position budget", "".join(o))


def svg_slot():
    W, H = 280, 150
    s = _series("clk-slot", "oscillator (datasheet)")
    thr = CHARTS["clk-slot"]["model"]["threshold"]
    x1, ymax = s[-1][0], max(p[1] for p in s) * 1.08
    X, Y = _lin(0, x1, 6, W - 6), _lin(0, ymax, H - 18, 10)
    ex = next((p[0] for p in s if p[1] > thr), None)
    o = [f'<rect x="0" y="{Y(thr):.1f}" width="{W}" height="{Y(0) - Y(thr):.1f}" class="ob-band"/>',
         f'<line class="ob-thr amber" x1="0" x2="{W}" y1="{Y(thr):.1f}" y2="{Y(thr):.1f}"/>',
         f'<polyline class="ob-tr" pathLength="1" style="stroke:var(--lime)" points="{_pts(s, X, Y)}"/>',
         f'<text x="6" y="{Y(thr) + 14:.1f}" class="ob-s" style="fill:var(--amber)">slot guard {thr:g} ns</text>']
    if ex is not None:
        o.append(f'<line x1="{X(ex):.1f}" x2="{X(ex):.1f}" y1="8" y2="{H - 18}" class="ob-exit"/><text x="{X(ex) - 5:.1f}" y="18" text-anchor="end" class="ob-s" style="fill:var(--coral)">exit at {ex:.0f} s</text>')
    o.append(f'<text x="6" y="{H - 4}" class="ob-s">last fix</text><text x="{W - 6}" y="{H - 4}" text-anchor="end" class="ob-s">{x1:.0f} s</text>')
    return ob_svg(W, H, "Predicted time error since the last fix against the slot guard", "".join(o))


def svg_raim():
    W, H = 280, 150
    ia = _ph["integrity-alarm"]
    vals = [(x, _ch["protection_level_m"][k], _ch["alert_limit_m"][k]) for k, x in enumerate(_t) if ia["t0"] <= x < ia["t1"]][::3]
    al = next(v[2] for v in vals if v[2] is not None)
    ymax = max(v[1] or 0 for v in vals) * 1.08
    bw = (W - 8) / len(vals)
    Y = _lin(0, ymax, H - 18, 14)
    o = ['<defs><pattern id="obHatch" width="5" height="5" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><line x1="0" y1="0" x2="0" y2="5" class="ob-hatch"/></pattern></defs>',
         f'<line class="ob-thr" x1="0" x2="{W}" y1="{Y(al):.1f}" y2="{Y(al):.1f}"/>']
    for i, (x, pl, _) in enumerate(vals):
        bx = 4 + i * bw
        if pl is None:
            o.append(f'<rect class="ob-nopl" x="{bx + 1:.1f}" y="{Y(ymax * .9):.1f}" width="{bw - 2:.1f}" height="{Y(0) - Y(ymax * .9):.1f}" rx="2" fill="url(#obHatch)"/>')
        else:
            o.append(f'<rect class="ob-bar{" over" if pl > al else ""}" x="{bx + 1:.1f}" y="{Y(pl):.1f}" width="{bw - 2:.1f}" height="{Y(0) - Y(pl):.1f}" rx="2" style="--i:{i}"/>')
    o.append(f'<text x="4" y="10" class="ob-s" style="fill:var(--coral)">dashed: alert limit {al:g} m</text>')
    o.append(f'<text x="4" y="{H - 4}" class="ob-s">{hms(ia["t0"])} · 30 s epochs</text>')
    return ob_svg(W, H, "Vertical protection level at each epoch of the integrity-alarm phase, against the alert limit", "".join(o))


def svg_masks():
    W, H = 280, 150
    m = CHARTS["tw-mtie"]["model"]
    mt, td = [p for p in m["mtie"] if p[1] > 0], [p for p in m["tdev"] if p[1] > 0]
    prtc = next(k for k in m["masks"] if k["id"] == "prtc-a")
    c = next(c for c in prtc["checks"] if c["metric"] == "mtie")
    allp = mt + td
    xlo, xhi = min(p[0] for p in allp), max(p[0] for p in allp)
    lo = 10 ** math.floor(math.log10(min(p[1] for p in allp)))
    hi = 10 ** math.ceil(math.log10(max([p[1] for p in allp] + [c["limit_ns"]])))
    X, Y = _log(xlo, xhi, 8, W - 8), _log(lo, hi, H - 18, 12)
    o = [f'<line class="ob-thr" x1="8" x2="{W - 8}" y1="{Y(c["limit_ns"]):.1f}" y2="{Y(c["limit_ns"]):.1f}"/>',
         f'<polyline class="ob-tr" pathLength="1" style="stroke:var(--cyan)" points="{_pts(mt, X, Y)}"/>',
         f'<polyline class="ob-tr d2" pathLength="1" style="stroke:var(--magenta)" points="{_pts(td, X, Y)}"/>']
    wt = min(max(c["worst_tau_s"], xlo), xhi)
    o.append(f'<circle cx="{X(wt):.1f}" cy="{Y(c["value_ns"]):.1f}" r="4" class="ob-fail"/>')
    o.append(f'<text x="10" y="{Y(c["limit_ns"]) + 13:.1f}" class="ob-s" style="fill:var(--coral)">PRTC-A MTIE limit {c["limit_ns"]:g} ns · {c["verdict"]}</text>')
    o.append(f'<text x="{W - 10}" y="{Y(mt[-1][1]) + 14:.1f}" text-anchor="end" class="ob-s" style="fill:var(--cyan)">MTIE</text><text x="{W - 10}" y="{Y(td[-1][1]) + 14:.1f}" text-anchor="end" class="ob-s" style="fill:var(--magenta)">TDEV</text>')
    o.append(f'<text x="8" y="{H - 4}" class="ob-s">τ {xlo:g} s</text><text x="{W - 8}" y="{H - 4}" text-anchor="end" class="ob-s">{xhi:g} s, log</text>')
    return ob_svg(W, H, "Maximum time interval error and time deviation of the holdover run against the PRTC-A limit", "".join(o))


def svg_ground():
    W, H = 280, 150
    pts = CHARTS["fail-orb"]["model"]["points"]
    X, Y = _lin(-180, 180, 0, W), _lin(90, -90, 0, H)
    segs, cur = [], []
    for a, b in zip(pts, pts[1:] + [None]):
        cur.append(a)
        if b is None or abs(b["lon"] - a["lon"]) > 180:
            segs.append(cur)
            cur = []
    d = " ".join("M" + " L".join(f"{X(p['lon']):.1f} {Y(p['lat']):.1f}" for p in s) for s in segs if len(s) > 1)
    longest = max(segs, key=len)
    path = "M" + " L".join(f"{X(p['lon']):.1f} {Y(p['lat']):.1f}" for p in longest)
    vis = [p for p in pts if p.get("visible")]
    o = [f'<image href="assets/planets/earth-day-256.jpg" x="0" y="0" width="{W}" height="{H}" preserveAspectRatio="none" class="ob-map"/>',
         f'<path d="{d}" class="ob-track"/>']
    o += [f'<circle cx="{X(p["lon"]):.1f}" cy="{Y(p["lat"]):.1f}" r="1.7" class="ob-vis"/>' for p in vis[::2]]
    o.append(f'<circle r="4" class="ob-sat" style="offset-path:path(\'{path}\')"/>')
    return ob_svg(W, H, "Ground track of the engine's SGP4 propagation of a real two-line element set, on NASA Blue Marble imagery", "".join(o))


def svg_moon():
    W, H = 280, 150
    s1, s2 = _series("jr-moon", "horizontal (HPL)"), _series("jr-moon", "vertical (VPL)")
    al = CHARTS["jr-moon"]["model"]["threshold"]
    x1 = s1[-1][0]
    ymax = max(p[1] for p in s1 + s2) * 1.12
    X, Y = _lin(0, x1, 128, W - 6), _lin(0, ymax, H - 20, 14)
    o = ['<image href="assets/planets/moon-disc-320.webp" x="2" y="14" width="114" height="114" class="ob-moon"/>',
         f'<line class="ob-thr" x1="128" x2="{W - 6}" y1="{Y(al):.1f}" y2="{Y(al):.1f}"/>',
         f'<polyline class="ob-tr" pathLength="1" style="stroke:var(--cyan)" points="{_pts(s1, X, Y)}"/>',
         f'<polyline class="ob-tr d2" pathLength="1" style="stroke:var(--amber)" points="{_pts(s2, X, Y)}"/>',
         f'<text x="130" y="{Y(al) - 5:.1f}" class="ob-s" style="fill:var(--coral)">alert limit {al:g} m</text>',
         f'<text x="{W - 6}" y="{Y(s2[-1][1]) - 6:.1f}" text-anchor="end" class="ob-s" style="fill:var(--amber)">VPL</text><text x="{W - 6}" y="{Y(s1[-1][1]) + 13:.1f}" text-anchor="end" class="ob-s" style="fill:var(--cyan)">HPL</text>',
         f'<text x="128" y="{H - 5}" class="ob-s">0</text><text x="{W - 6}" y="{H - 5}" text-anchor="end" class="ob-s">{x1 / 3600:g} h</text>']
    return ob_svg(W, H, "Lunar protection levels from the LunaNet advanced RAIM run against the alert limit, beside the Moon's near side", "".join(o))


def svg_sweep():
    W, H = 280, 150
    q, c = _series("sec-research", "quantum"), _series("sec-research", "classical")
    xs = [p[0] for p in q + c]
    ys = [p[1] for p in q + c]
    X, Y = _log(min(xs), max(xs), 16, W - 10), _log(min(ys) / 1.4, max(ys) * 1.6, H - 20, 12)
    o = [f'<line class="ob-axis" x1="16" y1="{H - 20}" x2="{W - 10}" y2="{H - 20}"/><line class="ob-axis" x1="16" y1="10" x2="16" y2="{H - 20}"/>',
         f'<polyline class="ob-link" points="{_pts(c, X, Y)}"/>']
    o += [f'<circle cx="{X(x):.1f}" cy="{Y(y):.1f}" r="3.2" class="ob-pt cl" style="--i:{i}"/>' for i, (x, y) in enumerate(c)]
    o += [f'<circle cx="{X(x):.1f}" cy="{Y(y):.1f}" r="3.6" class="ob-pt qu" style="--i:{i}"/>' for i, (x, y) in enumerate(q)]
    o.append(f'<text x="{W - 10}" y="{Y(q[-1][1]) - 9:.1f}" text-anchor="end" class="ob-s" style="fill:var(--magenta)">quantum · {q[-1][1]:g} s</text>')
    o.append(f'<text x="{W - 10}" y="{Y(c[-1][1]) + 16:.1f}" text-anchor="end" class="ob-s">classical · {c[0][1]:g} → {c[-1][1]:g} s</text>')
    o.append(f'<text x="18" y="{H - 5}" class="ob-s">classical clock noise, log</text><text x="20" y="20" class="ob-s">holdover, log</text>')
    return ob_svg(W, H, "Holdover time of a quantum and a classical clock across a sweep of the classical clock's noise level", "".join(o))


NOT_ICON = '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M5.6 5.6l12.8 12.8"/></svg>'


def home_bento(root):
    def fig(cid, w, h, fn):
        register_svg(cid, w, h, "ob", fn())
        return fig_html(root, cid, w, h, "ob", " viz-ob")
    chips = "".join(f'<span class="chip{" hi" if i == 0 else ""}">{esc(t)}</span>' for i, t in enumerate(
        ["WebAssembly", f"cargo install {crate}", f"pip install {py_name}", f"npm install {npm_name}", "JetBrains Marketplace", f"cargo install {mcp_crate}", "ghcr.io image", "CCSDS OEM", "SP3"]))
    F = FP
    fp_prov = (f'<p class="prov"><span class="prov-src">Engine v{esc(F["engine"])} · <code>{esc(F["file"])}.toml</code> · seed 1 · receiver moved per cell</span>'
               f'<a class="prov-open" href="{root}studio/index.html?scenario={esc(F["file"])}&amp;tab=signal">{esc(STUDIO_OPEN)}<svg aria-hidden="true"><use href="#i-ext"/></svg></a>'
               f'<a class="prov-open" href="{root}missions.html#shows">Every cell, on Missions<svg aria-hidden="true"><use href="#i-ext"/></svg></a></p>')
    tiles = [
        ("s3", "jam", "--amber", "Interference", "GNSS jamming and interference",
         "A broadband jammer on a mast and a receiver moved over a grid around it: every cell is its own run of the engine's <code>jamming</code> kind. Shade is the mean J/S (jammer-to-signal ratio); red cells lost every satellite.",
         f'<div class="vis">{ob_plot(svg_footprint())}</div>{fp_prov}'),
        ("s3", None, "--magenta", "Deception", "Spoofing detection",
         "A spoofer captures the receiver and walks its time off at a steady rate. Each clock's integrity monitor has its own detection bound: the quantum clock flags the lie almost at once, the classical one only once the offset has grown.",
         f'<div class="vis">{fig("sec-defence", 440, 180, svg_spoof)}</div>'),
        ("s4", None, "--cyan", "Clocks", "Clock holdover across oscillator classes",
         "From the moment GNSS disappears: the median and 95th percentile of 200 Monte Carlo runs for a chip-scale atomic clock (CSAC) and an optical lattice clock, and the one-day error of four more classes: an optical master clock, a hydrogen maser, a rubidium atomic frequency standard (RAFS) and its miniature version (mini-RAFS).",
         f'<div class="vis">{fig("sec-cni", 640, 200, svg_clocks)}</div><p class="prov">{prov_html(root, "clk-lunar-classes", "One-day column")}</p>'),
        ("s2", None, "--cyan", "Dead reckoning", "GNSS/INS coasting",
         "When GNSS drops, the INS (inertial navigation system) carries on and its error cone opens. Two inertial measurement units (IMUs), one budget.",
         f'<div class="vis">{fig("fail-nav", 280, 150, svg_cone)}</div>'),
        ("s2", None, "--lime", "Timing", "Slot timing",
         "How long a clock stays inside its guard interval after the last fix: the question time-slotted networks care about.",
         f'<div class="vis">{fig("clk-slot", 280, 150, svg_slot)}</div>'),
        ("s2", None, "--coral", "Integrity", "RAIM and ARAIM",
         "Protection levels (PL) from receiver autonomous integrity monitoring (RAIM) and its advanced form (ARAIM), checked against the alert limit: here, the chained mission's integrity-alarm phase. Hatched bars are epochs where no protection level could be formed.",
         f'<div class="vis">{ob_plot(svg_raim())}</div><p class="prov">{prov_run(root, CAMP["file"])}</p>'),
        ("s2", None, "--cyan", "Telecom", "Timing masks",
         "MTIE (maximum time interval error) and TDEV (time deviation) of a 24-hour holdover against ITU-T (International Telecommunication Union, Telecommunication Standardization Sector) limits, here the PRTC-A (primary reference time clock, class A) one.",
         f'<div class="vis">{fig("tw-mtie", 280, 150, svg_masks)}</div>'),
        ("s2", None, "--cyan", "Astrodynamics", "Orbits: SGP4 and numerical",
         "SGP4 (Simplified General Perturbations 4) propagation of a real two-line element set, drawn on real imagery; CCSDS OEM and SP3 ephemerides in and out.",
         f'<div class="vis">{fig("fail-orb", 280, 150, svg_ground)}</div>'),
        ("s2", None, "--cyan", "Cislunar", "Lunar PNT",
         "Positioning, navigation and timing around the Moon: horizontal and vertical protection levels (HPL, VPL) from ARAIM on a lunar navigation service.",
         f'<div class="vis">{fig("jr-moon", 280, 150, svg_moon)}</div>'),
        ("s2", None, "--magenta", "Emerging sensors", "Quantum-sensor trade studies",
         "Weigh a quantum clock against a classical one before anyone buys hardware: holdover across a sweep of the classical clock's noise.",
         f'<div class="vis">{fig("sec-research", 280, 150, svg_sweep)}</div>'),
        ("s3", None, "--cyan", "Runs everywhere", "Browser, terminal, notebook, IDE, assistant",
         f"The same Rust engine ships as WebAssembly for {esc(STUDIO)}, a CLI (command-line interface), Python and JavaScript packages, a JetBrains Marketplace plugin for your IDE (integrated development environment) and an MCP (Model Context Protocol) server.",
         f'<div class="vis"><div class="chips">{chips}</div><p class="chips-note">CCSDS OEM: Consultative Committee for Space Data Systems Orbit Ephemeris Message. SP3: Standard Product 3 orbit format.</p></div>'),
        ("s3", None, "--coral", "Scope, stated plainly", "What Kshana is not",
         "Knowing the edges matters as much as the features.",
         f'<div class="vis not-list"><div class="not">{NOT_ICON}<div>Not an RF signal simulator<span>It does not generate radio-frequency (RF) waveforms.</span></div></div>'
         f'<div class="not">{NOT_ICON}<div>Not a GNSS receiver<span>It simulates what happens to PNT; it does not track real signals.</span></div></div>'
         f'<div class="not">{NOT_ICON}<div>Not a flight product<span>A study and trade-off instrument, every answer labelled Validated or Modelled.</span></div></div></div>'),
    ]
    out = []
    for span, tid, c, kick, h3, p, vis in tiles:
        idattr = f' id="{tid}"' if tid else ""
        out.append(f'<article class="glass tile {span} rv"{idattr} style="--c:var({c})"><div class="kicker"><i></i>{esc(kick)}</div><h3>{esc(h3)}</h3><p>{p}</p>{vis}</article>')
    return '<div class="bento" id="fails">' + "".join(out) + "</div>"


BLOCKS["{{HOME_BENTO}}"] = home_bento
EXTRA_SCALARS.update(FACTS)
SCALARS["{{CLI_CMD}}"] = f"cargo install {crate}"

# Page data for Home: the two runs the hero, the timeline and the story replay, and the MCP session.
PAGE_DATA["index"] = {
    "camp": {k: CAMP[k] for k in ("file", "seed", "step_s", "duration_s", "t", "ch", "phases", "events", "receiver")},
    "orb": {k: CONST[k] for k in ("file", "radius_km", "rotation_rate_deg_s", "mask_deg", "sats", "constellations", "total")},
    "engine": {"version": ENG["version"], "commit": ENG["commit"]},
    "phaseNames": PHASE_N, "phaseColors": PHASE_C,
    "facts": {k.strip("{}"): v for k, v in FACTS.items() if k.startswith("{{S_")},
    "mcp": json.loads(src("data/mcp-session.json")),
}
# Developers #mcp plays the same recorded session through the same module (js/mcp-player.mjs).
PAGE_DATA["developers"] = {"mcp": PAGE_DATA["index"]["mcp"]}

# ------------------------------------------------------------------ R5 hero visuals (2026-09-29)
# Developers hero: the terminal replays src/data/cli-session.json, real `kshana` commands and their
# real output (src/tools/record_cli_session.py runs the engine; the build never does).
_cli = json.loads(src("data/cli-session.json"))
assert _cli["engine"]["version"] == version, f"cli-session.json is engine {_cli['engine']['version']}, the site is {version}: re-run src/tools/record_cli_session.py"
PAGE_DATA["developers"]["cli"] = _cli
# Evidence hero: one cell per capability of the engine's verification matrix, in ledger order.
_VM_S = {"VALIDATED": "v", "MODELLED": "m", "PARTNER": "p"}
PAGE_DATA["evidence"] = {"matrix": {"version": version, "summary": summ, "source": vm.get("generated_from", ""),
                                    "rows": [{"r": r["requirement"], "c": trunc(r["capability"], 150), "s": _VM_S[r["status"]],
                                              "o": trunc(r.get("oracle") or "", 170), "m": r.get("module") or ""} for r in vm["rows"]]}}
# Editions hero: the open engine's kinds (src/api.rs, grouped as on Capabilities), each kind's ledger
# status, and which kinds Kshana Pro and the custom study build on (EXTENDS above).
_grp_of = {k: i for i, (_, _, ks) in enumerate(GROUPS) for k in ks}
PAGE_DATA["editions"] = {"version": version, "summary": summ, "groups": [[g, c] for g, c, _ in GROUPS],
                         "kinds": [[k, _grp_of.get(k, -1)] + (kind_ledger.get(k) or [0, 0]) for k, _ in kinds],
                         "extends": [[lab, href, ks] for (lab, href), ks in EXTENDS.items()]}
PAGE_SCRIPTS["editions"] = '<script type="module" src="js/editions.mjs"></script>'
# Kshana Pro section (#pro): the readouts of the front designs and the band scrubber (js/pro-power.mjs)
PAGE_DATA["editions"]["pro"] = {"design": [{"region": d["region"], "points": [
    # a result hash is shown only when the public docs/PRO.md prints it (the whole-Moon knee); the others read "same as the index"
    {"sat": s_, "av": a_, "designs": [{"id": f["id"], "knee": f["knee"], "v": f["v"], "sha": f["sha"] if f["sha"] in _PRO_PUBLIC[0] else None, "free": f["free"]} for f in d["front"] if (f["sat"], f["av"]) == (s_, a_)]}
    for s_, a_ in sorted({(f["sat"], f["av"]) for f in d["front"]})]} for d in PRO_POWER["design"]],
    "band": {k: PRO_POWER["band"][k] for k in ("t", "p05", "p50", "p95", "share", "limit", "unit", "n")}}
_, PAGE_DATA["editions"]["pro"]["band"]["geo"] = band_svg(PRO_POWER["band"])
PAGE_SCRIPTS["editions"] += '\n<script type="module" src="js/pro-power.mjs"></script>'
# (the Evidence and Developers instruments live in evidence.mjs and developers.mjs; the capability
#  console and the solar-system hero share cap-hero.mjs, loaded first so deep links open their tab)
PAGE_SCRIPTS["capabilities"] = PAGE_SCRIPTS["capabilities"].replace('<script type="module" src="js/capabilities.mjs">', '<script type="module" src="js/cap-hero.mjs"></script>\n<script type="module" src="js/capabilities.mjs">')
# js/developers-data.mjs (the no-page-data fallback) must not drift from the recording.
_gd = subprocess.run([sys.executable, os.path.join(SRC, "tools", "gen_developers_data.py"), "--check"], capture_output=True, text=True)
if _gd.returncode:
    sys.exit("js/developers-data.mjs is stale: run src/tools/gen_developers_data.py\n" + _gd.stdout + _gd.stderr)
# Evidence: the bento's data module must match the engine tree, and the page's static figures and
# Studio links must match the module. The check builds a small Rust probe, so it needs cargo and the
# engine tree; without either the build says so and carries on (the committed module still ships).
_ev_tree = os.path.expanduser(os.environ.get("KSHANA_ENGINE_TREE") or REPO)
if shutil.which("cargo") and os.path.isdir(os.path.join(_ev_tree, "src")):
    _ge = subprocess.run([sys.executable, os.path.join(SRC, "tools", "gen_evidence_data.py"), "--check"], capture_output=True, text=True)
    if _ge.returncode:
        sys.exit("js/evidence-data.mjs or the Evidence page is stale: run src/tools/gen_evidence_data.py\n" + _ge.stdout + _ge.stderr)
else:
    print(f"note: Evidence data check skipped (needs cargo and the engine tree at {_ev_tree})")


shell = src("shell.html")


def render_page(key, title, main, desc, root="", scripts="", head_extra="", page_data=None):
    out = shell
    data = f'<script type="application/json" id="kpage">{json.dumps(page_data, ensure_ascii=False, separators=(",", ":")).replace("</", "<\\/")}</script>\n' if page_data else ""
    for k, v in {"{{TITLE}}": esc(title), "{{DESC}}": esc(desc), "{{PAGE}}": key, "{{ROOT}}": root, "{{NAV_LINKS}}": nav_links(key, root),
                 "{{MAIN}}": main, "{{SCRIPTS}}": data + scripts, "{{HEAD_EXTRA}}": head_extra, "{{IMAGERY_CREDIT}}": IMAGERY_CREDIT}.items():
        out = out.replace(k, v)
    for k, v in SCALARS.items():
        out = out.replace(k, v)
    left = re.findall(r"\{\{[A-Z_ ]+[^}]*\}\}", out)
    if left:
        sys.exit(f"{key}: unreplaced placeholders {set(left)}")
    return out


written = []
for key, label, title, part, desc in PAGES:
    main = raw_pages[key]
    for k, fn in BLOCKS.items():
        if k in main:
            main = main.replace(k, fn(""))
    for k, fn in STATIC_BLOCKS.items():
        if k in main:
            main = main.replace(k, fn())
    for k, v in EXTRA_SCALARS.items():
        main = main.replace(k, v)
    # {{STUDIO_OPEN_LINK <scenario>}}: the "Open in <Studio>" link to the view that reproduces a run;
    # {{STUDIO_VIEW <scenario>}}: that view's address alone. A scenario the Studio lacks stops the build.
    main = re.sub(r"\{\{STUDIO_OPEN_LINK ([\w-]+)\}\}", lambda m: studio_open("", m.group(1)), main)
    main = re.sub(r"\{\{STUDIO_VIEW ([\w-]+)\}\}", lambda m: studio_best("", m.group(1)), main)
    main = expand(main, "")
    css_name = "home" if key == "index" else key
    _pw_css = '<link rel="stylesheet" href="css/pro-power.css">' if key in ("index", "editions") else ""
    page = render_page(key, title, main, desc, "", PAGE_SCRIPTS[key], f'<link rel="stylesheet" href="css/{css_name}.css">' + _pw_css, PAGE_DATA.get(key))
    write(f"{key}.html", page)
    written.append(f"{key}.html")

# Plotted data behind every chart's CSV button (one file, fetched on demand by site.js).
json.dump(series_export(), open(os.path.join(HERE, "data", "series.json"), "w"), ensure_ascii=False, separators=(",", ":"))

# =================================================================== static files
shutil.copyfile(os.path.join(SRC, "tokens.css"), os.path.join(HERE, "tokens.css"))
# (The Studio's published copy gets the same tokens.css below, after the sync; the build never
# writes into the Studio folder itself.)
shutil.copyfile(os.path.join(SRC, "site.css"), os.path.join(HERE, "site.css"))
shutil.copyfile(os.path.join(SRC, "theme.css"), os.path.join(HERE, "theme.css"))
os.makedirs(os.path.join(HERE, "css"), exist_ok=True)
for p in glob.glob(os.path.join(SRC, "css", "*.css")):
    shutil.copyfile(p, os.path.join(HERE, "css", os.path.basename(p)))
shutil.copyfile(os.path.join(SRC, "site.js"), os.path.join(HERE, "site.js"))
os.makedirs(os.path.join(HERE, "js"), exist_ok=True)
for p in glob.glob(os.path.join(SRC, "js", "*.mjs")):
    shutil.copyfile(p, os.path.join(HERE, "js", os.path.basename(p)))
# The Studio view of every run, for the page scripts that write a provenance line at run time
# (generated: build.py from the Studio's tools_links.mjs; never edit js/studio-links.mjs).
write("js/studio-links.mjs",
      "// Generated by build.py from the Studio's own link list (tools_links.mjs). Do not edit.\n"
      "// scenario name -> the Studio view that reproduces the run, relative to the Studio folder.\n"
      "const LINKS = " + json.dumps({n: r["links"]["best"] for n, r in sorted(STUDIO_LINKS.items())}, ensure_ascii=False, separators=(",", ":")) + ";\n"
      "const VIEWS = " + json.dumps({n: r["tabs"] for n, r in sorted(STUDIO_LINKS.items())}, ensure_ascii=False, separators=(",", ":")) + ";\n"
      "const stem = (f) => String(f).replace(/\\.toml$/, \"\");\n"
      "/// The address of the Studio view that reproduces `scenario` (a file name, with or without .toml),\n"
      "/// or of its `tab` when one is named and the run has it; null when the Studio does not carry the run.\n"
      "/// A clean address from the site root (/studio/?scenario=...), whatever page asks: `root` is kept\n"
      "/// for the callers' signature only.\n"
      "export function studioHref(root, scenario, tab) {\n"
      "  const n = stem(scenario);\n"
      "  if (!LINKS[n]) return null;\n"
      "  return \"/studio/\" + (tab && VIEWS[n].includes(tab) ? `?scenario=${n}&tab=${tab}` : LINKS[n].replace(/^index\\.html/, \"\"));\n"
      "}\n"
      "export const studioHas = (scenario, tab) => !!LINKS[stem(scenario)] && (!tab || VIEWS[stem(scenario)].includes(tab));\n")

# =================================================================== docs
DOCS_CFG = os.path.join(SCRATCH, "docs-config.json")
json.dump({
    "repo": DOCS_REPO, "out": os.path.join(HERE, "docs"), "manifest": os.path.join(SCRATCH, "docs-manifest.json"), "shell": shell, "version": version,
    "tracked": sorted(t for t in subprocess.run(["git", "-C", DOCS_REPO, "ls-files"], capture_output=True, text=True, check=True).stdout.split("\n") if t),
    # Paths GitHub already serves (origin/main as last fetched): only these get "on GitHub" links.
    "remote": subprocess.run(["git", "-C", DOCS_REPO, "ls-tree", "-r", "--name-only", "origin/main"], capture_output=True, text=True).stdout.split(),
    "nav": [[k, l, h] for k, l, h in NAV], "studioDir": "studio", "scalars": SCALARS, "imagery": IMAGERY_CREDIT, "studio": STUDIO, "siteGlossary": GLOSS,
    "scenarios": [{"file": f, "kind": file_kind[f], "desc": file_head.get(f) or file_desc.get(f, ""), "kindDesc": kind_desc_full.get(file_kind[f]) or kind_desc.get(file_kind[f], ""), "toml": file_toml[f],
                   "status": ("v" if kind_ledger.get(file_kind[f], [0])[0] else ("m" if file_kind[f] in kind_ledger else "u")), "recorded": f in recorded,
                   "ledger": kind_ledger.get(file_kind[f])} for f in files],
    "suites": [{"file": f, "desc": next((trunc(l.lstrip("# "), 160) for l in read(f"scenarios/{f}.toml").splitlines() if l.startswith("#") and l.strip("# ").strip()), "")} for f in all_files if f not in files],
    "cards": cards, "standards": standards, "ledger": [{"req": r["requirement"], "cap": r["capability"], "status": r["status"], "oracle": r.get("oracle") or "", "module": r.get("module") or ""} for r in vm["rows"]],
}, open(DOCS_CFG, "w"), ensure_ascii=False)
if os.path.isdir(os.path.join(HERE, "docs")):
    shutil.rmtree(os.path.join(HERE, "docs"))
subprocess.run(["node", os.path.join(TOOLS, "build_docs.mjs"), DOCS_CFG], check=True)
docs_index = json.load(open(os.path.join(HERE, "docs", "search-docs.json")))

# =================================================================== clean URLs
# The host serves every page without its extension (/evidence is evidence.html, /docs/ is
# docs/index.html), so every link to a page is written that way, from the site root: "/" for Home,
# /evidence#research, /docs/changelog, /studio/?scenario=... (never index.html or .html).
# Pages, scripts and data keep writing ordinary relative file links; clean_url() is the one place
# that turns them into addresses, applied to every published page and both search indexes at the
# end of the build. Old .html addresses keep working on the host. check.py rule 20 holds it.
def clean_url(u, from_rel):
    """A link as written in the published file `from_rel` -> its clean address from the site root.
    Anything that is not a link to a page of this site (another host, mail, a bare #fragment, an
    asset) comes back unchanged."""
    if not u or re.match(r"^(?:[a-z][a-z0-9+.-]*:|//|#|\{\{)", u, re.I):
        return u
    cut = min([i for i in (u.find("?"), u.find("#")) if i >= 0] or [len(u)])
    path, rest = u[:cut], u[cut:]
    if not path:
        return u
    full = path if path.startswith("/") else posixpath.normpath(posixpath.join("/" + posixpath.dirname(from_rel), path)) + ("/" if path.endswith("/") else "")
    if full.endswith("/index.html") or full == "/index.html":
        full = full[: -len("index.html")]
    elif full.endswith(".html"):
        full = full[: -len(".html")]
    elif not full.endswith("/"):
        return u  # an asset (stylesheet, script, image, data): left as written
    return full + rest


_HREF = re.compile(r'(<(?:a|area|link|iframe)\b[^>]*?\s(?:href|src)=")([^"]*)(")', re.I)
def clean_page(rel, text):
    return _HREF.sub(lambda m: m.group(1) + clean_url(m.group(2), rel) + m.group(3), text)


# =================================================================== search index for the palette
search = []
SECT = {
    "index": [("Home", "index.html"), ("Mission timeline: jamming, spoofing, holdover, integrity alarm", "index.html#timeline"), ("Capabilities, each from a real run", "index.html#capabilities"),
              ("GNSS jamming footprint", "index.html#jam"), ("AI assistant: a recorded kshana-mcp session", "index.html#assistant"), ("Spoofing scroll story", "index.html#story"), ("Open core, Pro and custom study", "index.html#pro-band")],
    "missions": [("Missions by sector", "missions.html#sectors"), ("Who it is for", "missions.html#who")] + [(s["n"], f"missions.html#{s['id']}") for s in SECTORS],
    "capabilities": [("The engine at work: five recorded runs", "capabilities.html#runs"), ("Scenario atlas", "capabilities.html#atlas"), ("Capability explorer", "capabilities.html#explorer"), ("Band atlas", "capabilities.html#bands"), ("Clock ladder", "capabilities.html#clocks"), ("Timing wall", "capabilities.html#timing"), ("Earth to Moon to Mars", "capabilities.html#journey")],
    "evidence": [("Validated results", "evidence.html#results"), ("Verification matrix", "evidence.html#matrix"), ("Validation ledger", "evidence.html#ledger"), ("Honesty labels", "evidence.html#labels"), ("Standards grid", "evidence.html#standards"), ("Research", "evidence.html#research"), ("Cite the engine", "evidence.html#cite-engine"), ("Glossary", "docs/glossary.html")],
    "developers": [("Install", "developers.html#install"), ("Ways to use it", "developers.html#ways"), ("MCP server", "developers.html#mcp"), ("JetBrains plugin", "developers.html#jetbrains")],
    "editions": [("Editions: free and Pro", "editions.html"), ("Kshana Pro", "editions.html#pro"), ("Custom work and studies", "editions.html#custom"), ("Talk to us", "editions.html#contact")],
}
for pg, items in SECT.items():
    for t, h in items:
        search.append({"t": t, "h": h, "k": "Section"})
search.append({"t": "Papers", "h": "evidence.html#research", "k": "Section", "d": "Research built on the open engine"})
search.append({"t": "arXiv", "h": "evidence.html#research", "k": "Section", "d": f"{len(PUBS)} papers on arXiv built on the open engine"})
for p in PUBS:  # each paper, by its title
    search.append({"t": p["title"], "h": f"evidence.html#pub-{p['arxiv'].replace('.', '-')}", "k": "Paper", "d": p["summary"]})
for f in files:
    search.append({"t": f + ".toml", "h": f"studio/index.html?scenario={f}&tab=overview" if f in recorded else f"docs/scenario-reference.html#{f}", "k": "Scenario", "d": file_desc.get(f, "")})
for k, d in kinds:
    search.append({"t": k, "h": f"docs/scenario-reference.html#kind-{k}", "k": "Kind", "d": d})
search.extend(docs_index)
for e in search:
    e["h"] = clean_url(e["h"], "index.html")
json.dump(search, open(os.path.join(HERE, "data", "search.json"), "w"), ensure_ascii=False, separators=(",", ":"))

# =================================================================== sync the Studio (published at /studio/)
PG_OUT = os.path.join(HERE, "studio")
if os.path.isdir(PG_OUT):
    shutil.rmtree(PG_OUT)
# Development and recording tools, notes and type stubs never ship.
EXCLUDE = {"selfcheck.mjs", "tools_record.mjs", "tools_native.mjs", "tools_links.mjs", "tools_bundle.mjs", "PARITY.md", "PROGRESS.md", "README.md"}
# Folders of review material (screenshots, gate scripts and their reports) never ship either.
EXCLUDE_DIRS = {"REVIEW"}
_all_groups = json.load(open(os.path.join(PG_SRC, "groups.json"), encoding="utf-8"))["groups"] if os.path.exists(os.path.join(PG_SRC, "groups.json")) else []
_withheld = [g for g in _all_groups if g in PG_WITHHOLD]
for root_dir, dirs, fnames in os.walk(PG_SRC):
    rel = os.path.relpath(root_dir, PG_SRC)
    parts = [] if rel == "." else rel.split(os.sep)
    if parts and (parts[0] + "/" in _withheld or parts[0] in EXCLUDE_DIRS):
        continue
    # The recorded runs and the native engine's recordings ship as packs (written below), and the
    # scenario files as one JSON: the preview host limits the number of files (check.py rule 12).
    if parts and parts[-1] in ("recorded", "native"):
        continue
    for fn in fnames:
        if fn in EXCLUDE or fn.startswith("tools_") or fn.endswith(".test.mjs") or fn.endswith(".d.ts") or fn.startswith(".") or fn.endswith(".orig"):
            continue
        if rel == "scenarios":
            continue
        dst = os.path.join(PG_OUT, rel, fn)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.copyfile(os.path.join(root_dir, fn), dst)
shutil.copyfile(os.path.join(SRC, "tokens.css"), os.path.join(PG_OUT, "tokens.css"))  # one palette: the site's
_bundle_cmd = ["node", os.path.join(PG_SRC, "tools_bundle.mjs"), "--to", PG_OUT]
if _withheld:
    _bundle_cmd += ["--without", ",".join(_withheld)]
    json.dump({**json.load(open(os.path.join(PG_SRC, "groups.json"), encoding="utf-8")), "groups": [g for g in _all_groups if g not in _withheld]},
              open(os.path.join(PG_OUT, "groups.json"), "w", encoding="utf-8"), ensure_ascii=False, indent=1)
subprocess.run(_bundle_cmd, check=True)
# The Studio's data files are the engine checkout's generated copies, never the Studio folder's
# own (which may be older): the app cannot then state a count the site does not.
for _name, _from in (("capabilities.json", "web/capabilities.json"), ("verification-matrix.json", "web/data/verification-matrix.json"),
                     ("card-matrix-map.json", "web/data/card-matrix-map.json"), ("oracle-references.json", "web/data/oracle-references.json"),
                     ("standards-matrix-map.json", "web/data/standards-matrix-map.json")):
    assert os.path.exists(os.path.join(PG_OUT, "data", _name)), f"the Studio no longer reads data/{_name}"
    shutil.copyfile(os.path.join(REPO, _from), os.path.join(PG_OUT, "data", _name))
_pg_matrix = json.load(open(os.path.join(PG_OUT, "data", "verification-matrix.json"), encoding="utf-8"))["summary"]
assert _pg_matrix["total"] == summ["total"] and _pg_matrix["validated"] == summ["validated"], "the Studio's ledger is not the site's"
json.dump({"version": version, "studio": STUDIO, "channels": channels}, open(os.path.join(PG_OUT, "channels.json"), "w"), ensure_ascii=False, indent=1)
# The Studio's name, from brand.json: its <title> (which app.js reads for document.title) and every
# data-studio-name / data-studio-short element in the published copy.
for _page in ("index.html", os.path.join("advanced", "index.html")):  # the Simple view and the Advanced view
    if not os.path.exists(os.path.join(PG_OUT, _page)):
        assert _page != "index.html", "the Studio has no index.html"
        continue
    _pg = open(os.path.join(PG_OUT, _page), encoding="utf-8").read()
    _pg = re.sub(r"<title>[^<]*</title>", f"<title>{esc(STUDIO)}</title>", _pg, count=1)
    _pg = re.sub(r"(data-studio-name>)[^<]*(<)", lambda m: m.group(1) + esc(STUDIO) + m.group(2), _pg)
    _pg = re.sub(r"(data-studio-short>)[^<]*(<)", lambda m: m.group(1) + esc(re.sub(r"^Kshana\s+", "", STUDIO)) + m.group(2), _pg)
    # Links the Studio's source carries to the old single-page kshana.dev: on this site the validation
    # ledger is on the evidence page, and the embed's "Open in" link opens this Studio itself.
    _pg = _pg.replace('href="https://kshana.dev/#ledger"', 'href="../evidence.html#ledger"')
    _pg = re.sub(r'(<p class="embed-only"><a href=")https://kshana\.dev(")', r'\1index.html\2', _pg)
    assert "https://kshana.dev" not in _pg, "the Studio still links to kshana.dev"
    open(os.path.join(PG_OUT, _page), "w", encoding="utf-8").write(_pg)

# Every published page links pages by their clean address (see clean_url above). Engine run
# reports under assets/ are the engine's own files, copied as they are; the Studio's own scripts
# and pages other than its index stay as the Studio writes them.
for _root, _dirs, _fns in os.walk(HERE):
    _rel = os.path.relpath(_root, HERE)
    _top = _rel.split(os.sep)[0]
    if _top in ("src", ".build", "node_modules", "assets"):
        continue
    for _fn in _fns:
        _p = os.path.normpath(os.path.join(_rel, _fn)).replace(os.sep, "/")
        if not _fn.endswith(".html") or (_top == "studio" and _p != "studio/index.html"):
            continue
        _t = open(os.path.join(HERE, _p), encoding="utf-8").read()
        _c = clean_page(_p, _t)
        if _c != _t:
            open(os.path.join(HERE, _p), "w", encoding="utf-8").write(_c)

# =================================================================== search engines and agents (0.29.2)
# Everything a crawler, a search engine or an agent reads, generated here from the same data the
# pages are built from; nothing below is typed by hand per page:
#   * per page: a unique <title> and meta description, canonical, Open Graph and social-card tags,
#     and schema.org JSON-LD (SoftwareApplication + SoftwareSourceCode + Organization + WebSite on
#     Home, ScholarlyArticle per paper on Evidence, BreadcrumbList on every docs page, a
#     WebApplication on the Studio);
#   * <noscript> text for the sections a page script draws (key text and numbers, from the data
#     those scripts read), and a static description plus a scenario list on the Studio page;
#   * /robots.txt, /sitemap.xml (lastmod from git), /llms.txt and /llms-full.txt, the IndexNow key
#     file, and .build/indexnow.json (the body of the one ping to send after a deploy);
#   * playground/index.html: the Studio's old address, a redirect to /studio/ that keeps the query
#     and the fragment (the port writes the repository's own copy from web/tools/legacy-urls.json).
# check.py rules 21 to 25 hold all of it. The port (web/tools/port_site.py) carries these tags and
# files over as they are; it no longer writes its own.
ORIGIN = "https://kshana.dev"
SEO_OPEN, SEO_CLOSE = "<!-- seo:head -->", "<!-- /seo:head -->"
SEO_BODY_OPEN, SEO_BODY_CLOSE = "<!-- seo:body -->", "<!-- /seo:body -->"
REPO_URL = re.search(r'^repository\s*=\s*"([^"]+)"', read("Cargo.toml"), re.M).group(1).rstrip("/")
OG_IMAGE = f"{ORIGIN}/og-card.png"  # web/og-card.png, rendered by the repository's tools/gen_og_card.py
OG_ALT = (f"Kshana, a PNT (positioning, navigation and timing) resilience simulator. "
          f"{summ['validated']} of {summ['total']} capabilities validated against external oracles.")
INDEXNOW_KEY = json.loads(src("data/indexnow.json"))["key"]
assert re.fullmatch(r"[0-9a-f]{32}", INDEXNOW_KEY), "src/data/indexnow.json: the key must be 32 lowercase hex characters"
_git_date = lambda repo, *path: subprocess.run(["git", "-C", repo, "log", "-1", "--format=%cs", "HEAD", "--", *path], capture_output=True, text=True, check=True).stdout.strip()
SITE_LASTMOD = _git_date(REPO)
DOCS_LASTMOD = _git_date(DOCS_REPO)


def canonical_url(rel):
    """A published page's clean address on kshana.dev ("/" for Home, /docs/ for the docs index)."""
    if rel == "index.html":
        return ORIGIN + "/"
    if rel.endswith("/index.html"):
        return f"{ORIGIN}/{rel[:-len('index.html')]}"
    return f"{ORIGIN}/{rel[:-len('.html')]}"


def plain(fragment):
    """HTML -> one line of plain text."""
    t = re.sub(r"<(script|style|svg|template)\b.*?</\1>", " ", fragment, flags=re.S | re.I)
    t = " ".join(html.unescape(re.sub(r"<[^>]+>", " ", t)).split())
    return re.sub(r"\s+([,.;:!?)])", r"\1", re.sub(r"([(])\s+", r"\1", t))


def clip(text, n):
    """At most n characters: whole sentences when that keeps most of it, else whole words and an ellipsis."""
    text = " ".join(text.split())
    if len(text) <= n:
        return text
    cut = text[:n]
    end = max(cut.rfind(". "), cut.rfind("? "), cut.rfind("! "))
    if end >= n * 0.55:
        return cut[:end + 1]
    return cut[:n - 1].rsplit(" ", 1)[0].rstrip(",;:—-– ") + "…"


def ld_script(obj):
    # one line, with the spaces scripts/check-version-sync.sh greps for ("softwareVersion": "x.y.z")
    return '<script type="application/ld+json">' + json.dumps(obj, ensure_ascii=False, separators=(", ", ": ")).replace("</", "<\\/") + "</script>"


ORG = {"@type": "Organization", "@id": f"{ORIGIN}/#org", "name": "Ashforde OÜ", "url": "https://ashforde.org", "email": "contact@ashforde.org",
       "logo": f"{ORIGIN}/favicon.svg"}
APP_ID, SRC_ID = f"{ORIGIN}/#app", f"{ORIGIN}/#source"
INSTALL = [c["command"] for c in channels if c["command"]]
SAME_AS = [REPO_URL, f"https://crates.io/crates/{crate}", f"https://pypi.org/project/{py_name}/", f"https://www.npmjs.com/package/{npm_name}", f"https://doi.org/{cff_doi}"]
LICENSE_URL = f"https://spdx.org/licenses/{cff_lic}.html"
assert cff_lic == "AGPL-3.0-only", f"CITATION.cff licence is {cff_lic}; the structured data says AGPL-3.0-only"
HOME_LD = {"@context": "https://schema.org", "@graph": [
    ORG,
    {"@type": "WebSite", "@id": f"{ORIGIN}/#website", "url": ORIGIN + "/", "name": "Kshana", "inLanguage": "en", "publisher": {"@id": ORG["@id"]},
     # site.js opens the search palette with ?q=<terms> already typed (any page)
     "potentialAction": {"@type": "SearchAction", "target": {"@type": "EntryPoint", "urlTemplate": ORIGIN + "/?q={search_term_string}"}, "query-input": "required name=search_term_string"}},
    {"@type": "SoftwareApplication", "@id": APP_ID, "name": "Kshana", "url": ORIGIN + "/", "softwareVersion": version,
     "applicationCategory": "DeveloperApplication", "applicationSubCategory": "GNSS and PNT resilience simulation",
     "operatingSystem": "Linux, macOS, Windows; any modern browser (WebAssembly)", "license": LICENSE_URL, "isAccessibleForFree": True,
     "offers": {"@type": "Offer", "price": "0", "priceCurrency": "EUR", "availability": "https://schema.org/InStock"},
     "downloadUrl": f"https://crates.io/crates/{crate}", "installUrl": f"{ORIGIN}/developers#install", "softwareHelp": f"{ORIGIN}/docs/",
     "description": "", "keywords": "GNSS jamming simulator, GNSS spoofing, PNT resilience, holdover, RAIM, lunar navigation, open source",
     "author": {"@id": ORG["@id"]}, "publisher": {"@id": ORG["@id"]}, "sameAs": SAME_AS},
    {"@type": "SoftwareSourceCode", "@id": SRC_ID, "name": "Kshana", "codeRepository": REPO_URL, "programmingLanguage": "Rust", "version": version,
     "license": LICENSE_URL, "runtimePlatform": ["Rust", "Python", "Node.js", "WebAssembly"], "targetProduct": {"@id": APP_ID},
     "identifier": {"@type": "PropertyValue", "propertyID": "DOI", "value": cff_doi}, "sameAs": f"https://doi.org/{cff_doi}", "author": {"@id": ORG["@id"]}},
]}

# Titles (at most 60 characters) and descriptions (at most 155) of the site pages: PAGES above.
PAGE_SEO = {key: (title, desc) for key, _, title, _, desc in PAGES}
# The Studio's <title> stays its bare name (app.js reads it as the app's name); the social cards
# carry the longer title.
STUDIO_SEO = (STUDIO,
              "Run GNSS jamming, spoofing, holdover and integrity scenarios in your browser. The open-source engine runs locally as WebAssembly; nothing is uploaded.")


def docs_seo(rel, text):
    """A docs page's title and description, from the page itself: its heading and first paragraph."""
    prose = re.search(r'<div class="prose">(.*?)</article>', text, re.S)
    body = prose.group(1) if prose else text
    h1 = re.search(r"<h1\b[^>]*>(.*?)</h1>", body, re.S)
    head = plain(h1.group(1)) if h1 else re.sub(r"\s*·\s*Kshana docs$", "", html.unescape(re.search(r"<title>([^<]*)</title>", text).group(1)))
    head = re.sub(r"^Kshana\s+[—–-]\s+", "", head).replace("&", "and").strip()
    first = re.split(r"\s+[—–]\s+|:\s+|\s+\(", head)[0].strip()
    title = next((t for t in (f"{head} · Kshana docs", f"{head} · Kshana", f"{first} · Kshana docs", f"{first} · Kshana", clip(head, 60)) if len(t) <= 60), clip(head, 60))
    # prose, not a row of badges or of links
    paras = [plain(p) for p in re.findall(r"<p\b[^>]*>(.*?)</p>", body, re.S)
             if "<img" not in p and "img-alt" not in p and len(plain("".join(re.findall(r"<a\b.*?</a>", p, re.S)))) < 0.5 * max(1, len(plain(p)))]
    good = [p for p in paras if len(p) >= 60 and not re.match(r"(?i)(this file is generated|generated by|edit the source|> |kshana · )", p)]
    descs = [clip(p, 155) for p in good] + [clip(f"{head}: {p}", 155) for p in good[:1]]
    descs.append(clip(f"{head}. Kshana documentation for the open-source PNT (positioning, navigation and timing) resilience simulator.", 155))
    return title, [d for d in descs if len(d) >= 50], head


STUDIO_OG_TITLE = f"{STUDIO}: run GNSS and PNT scenarios in your browser"


def head_block(rel, title, desc, lds=()):
    url = canonical_url(rel)
    t, d, a = esc(STUDIO_OG_TITLE if rel == "studio/index.html" else title), esc(desc), esc(OG_ALT)
    lines = [SEO_OPEN, f'<link rel="canonical" href="{url}">',
             '<meta property="og:type" content="website">', '<meta property="og:site_name" content="Kshana">', '<meta property="og:locale" content="en_GB">',
             f'<meta property="og:url" content="{url}">', f'<meta property="og:title" content="{t}">', f'<meta property="og:description" content="{d}">',
             f'<meta property="og:image" content="{OG_IMAGE}">', '<meta property="og:image:width" content="1200">', '<meta property="og:image:height" content="630">',
             '<meta property="og:image:type" content="image/png">', f'<meta property="og:image:alt" content="{a}">',
             '<meta name="twitter:card" content="summary_large_image">', f'<meta name="twitter:title" content="{t}">', f'<meta name="twitter:description" content="{d}">',
             f'<meta name="twitter:image" content="{OG_IMAGE}">', f'<meta name="twitter:image:alt" content="{a}">']
    lines += [ld_script(x) for x in lds]
    return "\n".join(lines + [SEO_CLOSE]) + "\n"


def set_head(rel, text, title, desc, lds=()):
    """Write a page's title, description and head block (idempotent: an earlier block is replaced)."""
    text = re.sub(re.escape(SEO_OPEN) + r".*?" + re.escape(SEO_CLOSE) + r"\n?", "", text, flags=re.S)
    text, n1 = re.subn(r"<title>[^<]*</title>", f"<title>{esc(title)}</title>", text, count=1)
    text, n2 = re.subn(r'<meta name="description" content="[^"]*">', f'<meta name="description" content="{esc(desc)}">', text, count=1)
    assert n1 == 1 and n2 == 1, f"{rel}: no <title> or meta description to set"
    assert text.count("</head>") == 1, f"{rel}: expected one </head>"
    return text.replace("</head>", head_block(rel, title, desc, lds) + "</head>", 1)


# ---------------------------------------------------------------- <noscript> text for script-drawn sections
def ns(inner, cls="crawl-text"):
    return f'<noscript><div class="{cls}">{inner}</div></noscript>'


def wf_text():
    """The L-band waterfall and the campaign it drives (home-waterfall.mjs draws both from this file)."""
    W = json.load(open(os.path.join(HERE, "assets", "waterfall", "home-waterfall.json"), encoding="utf-8"))
    S, C, ph = W["spectrum"], W["campaign"], W["campaign"]["phases"]
    out = []
    for b in W["bands"]:
        st = b["st"]
        lost = st.find("X")
        rec = st.find("L", lost) if lost >= 0 else -1
        if lost < 0:
            out.append(f"{b['name']} tracks throughout, lowest {b['min']:g} dB-Hz.")
        else:
            out.append(f"{b['name']} loses lock at {W['t'][lost]:g} s" + (f" and recovers at {W['t'][rec]:g} s" if rec >= 0 else " and stays lost") + f", lowest {b['min']:g} dB-Hz.")
    jam = "; ".join(f"{j['name']} at {j['fc']:,.2f} MHz from {j['on']:g} s" + (f" to {j['off']:g} s" if j.get("off") is not None else " to the end") for j in W["jammers"])
    return ns(f"<p>Recorded run of <code>{esc(S['file'])}.toml</code> (seed {S['seed']}, engine v{esc(W['engine']['version'])}): {len(W['bands'])} GNSS bands, "
              f"{len(W['jammers'])} jammers over {W['duration_s']:g} s, tracking floor {W['thr']:g} dB-Hz. Jammers: {esc(jam)}.</p>"
              f"<p>{esc(' '.join(out))}</p>"
              f"<p>The campaign it drives, <code>{esc(C['file'])}.toml</code> (seed {C['seed']}): {esc(C['title'])}. "
              f"Holdover for {ph[1]['t1'] - ph[1]['t0']:g} s with the clock free-running, worst time error {ph[1]['te_max_ns']:g} ns against a {C['guard_ns']:g} ns guard; "
              f"then a Galileo-only fallback with a protection level of {ph[2]['pl_min_m']:g} to {ph[2]['pl_max_m']:g} m, under the {C['al_m']:g} m alert limit.</p>")


def mcp_text():
    """The recorded kshana-mcp (Model Context Protocol server) session the assistant player replays."""
    M = json.loads(src("data/mcp-session.json"))
    tools = "".join(f"<li><code>{esc(t['name'])}</code>: {esc(t['does'])}</li>" for t in M["tools"])
    return ns(f"<p>A recorded session with the kshana-mcp server (Model Context Protocol, engine v{esc(M['server']['version'])}): the assistant lists the "
              f"{len(M['tools'])} tools, validates <code>{esc(M['file'])}</code> ({esc(M['validate'])}) and runs it: {esc(M['summary'])}.</p><ul>{tools}</ul>")


def cli_text():
    """The recorded command-line session the developers terminal replays."""
    C = json.loads(src("data/cli-session.json"))
    rows = "".join(f"<li><code>$ {esc(s['cmd'])}</code>: {esc(' '.join(s['out'])[:240])}</li>" for s in C["steps"])
    return ns(f"<p>A recorded command-line session, engine v{esc(C['engine']['version'])} (build {esc(C['engine']['commit'])}):</p><ul>{rows}</ul>")


def evidence_text():
    F = _mjs_default("js/evidence-data.mjs")["facts"]
    return {
        "ev-raim": f"<p>{F['raim_n']} of {F['raim_n']} RAIM (receiver autonomous integrity monitoring) threshold cases match SciPy inside the band. Seed {F['seed_raim']}.</p>",
        "ev-cowell": f"<p>Numerical (Cowell) orbit propagation against Orekit over a 24-hour arc: worst difference {F['cowell_worst']} m in force tiers 1 to 5.</p>",
        "ev-hold": f"<p>Holdover prediction on a measured caesium clock record: predicted over measured between {F['hold_lo']} and {F['hold_hi']}, inside the pre-set factor of {F['hold_bar']}.</p>",
    }


def solar_text():
    sol = json.load(open(os.path.join(HERE, "assets", "capabilities", "capabilities.json"), encoding="utf-8"))["solar"]
    per = lambda b: f"{b['period_d'] / 365.25:.1f} years" if b["period_d"] > 1000 else f"{b['period_d']:.1f} days"
    items = "".join(f"<li>{esc(b['name'])}: {'Validated' if b['label'] == 'VALIDATED' else 'Modelled'}, {b['dist_au']:.2f} au from the Sun at the epoch, orbital period {per(b)}.</li>"
                    for b in sol["bodies"] if b.get("period_d"))
    return ns(f"<p>Every body in the engine's solar-system run (<code>{esc(sol['file'])}.toml</code>, engine v{esc(sol['engine_version'])}), at the epoch {esc(str(sol['epoch']))}:</p><ul>{items}</ul>")


MODES_TXT = [("int", "Interference"), ("spf", "Spoofing"), ("tim", "Timing"), ("orb", "Geometry"), ("itg", "Integrity"), ("nav", "Coasting")]


def matrix_text(t):
    """The Missions threat matrix (missions.mjs draws it from the sector cards on the page), as a table."""
    rows = []
    for m in re.finditer(r'<button role="tab"[^>]*aria-controls="([^"]+)"[^>]*>(.*?)</button>', t, re.S):
        pid, name = m.group(1), plain(m.group(2))
        pm = re.search(r'<div[^>]*\bid="' + re.escape(pid) + r'"[^>]*>(.*?)(?=<div[^>]*role="tabpanel"|</section>)', t, re.S)
        cards = re.findall(r'<(?:a|article) class="sc[^"]*"[^>]*data-mode="(\w+)"[^>]*>.*?<code class="sc-f">([\w-]+)</code>', pm.group(1) if pm else "", re.S)
        rows.append((name, {k: [f for mode, f in cards if mode == k] for k, _ in MODES_TXT}))
    assert len(rows) >= 8, f"missions: the threat-matrix text found only {len(rows)} sectors"
    head = "".join(f"<th scope=\"col\">{n}</th>" for _, n in MODES_TXT)
    body = "".join(f"<tr><th scope=\"row\">{esc(n)}</th>" + "".join(f"<td>{len(by[k])}</td>" for k, _ in MODES_TXT) + "</tr>" for n, by in rows)
    files = "".join(f"<li>{esc(n)}: " + "; ".join(f"{lab}: {', '.join(by[k])}" for k, lab in MODES_TXT if by[k]) + "</li>" for n, by in rows)
    return ns(f'<table><caption>Scenario files by sector (rows) and failure mode (columns)</caption><thead><tr><th scope="col">Sector</th>{head}</tr></thead><tbody>{body}</tbody></table><ul>{files}</ul>')


def put_after(rel, text, anchor_re, block):
    """Insert block right after the first match of anchor_re (a whole opening tag or element)."""
    m = re.search(anchor_re, text, re.S)
    assert m, f"{rel}: no anchor for the crawlable text ({anchor_re[:60]})"
    return text[:m.end()] + block + text[m.end():]


def crawl_text(rel, text):
    if "crawl-text" in text:
        return text
    if rel == "index.html":
        text = put_after(rel, text, r'<p[^>]*id="lbSummary"[^>]*>.*?</p>', wf_text())
        text = put_after(rel, text, r'<div[^>]*id="mcpStage"[^>]*>', mcp_text())
    elif rel == "capabilities.html":
        text = put_after(rel, text, r'<[a-z]+[^>]*id="cwSummary"[^>]*>.*?</[a-z]+>', wf_text())
        text = put_after(rel, text, r'<ul[^>]*id="chList"[^>]*>.*?</ul>', solar_text())
    elif rel == "developers.html":
        text = put_after(rel, text, r'<div[^>]*id="mcpStage"[^>]*>', mcp_text())
        text = put_after(rel, text, r'<[a-z]+[^>]*id="dtTerm"[^>]*>', cli_text())
    elif rel == "evidence.html":
        for cid, p in evidence_text().items():
            text = put_after(rel, text, r'<[a-z]+[^>]*\bid="' + cid + r'"[^>]*>', ns(p))
    elif rel == "missions.html":
        text = put_after(rel, text, r'<div class="mm-body" data-mm>', matrix_text(text))
    return text


# ---------------------------------------------------------------- the pages
seo_pages = {}  # rel -> (title, description), for the sitemap, llms.txt and check.py
for key, _, _, _, _ in PAGES:
    rel = f"{key}.html"
    t = open(os.path.join(HERE, rel), encoding="utf-8").read()
    title, desc = PAGE_SEO[key]
    lds = []
    if key == "index":
        HOME_LD["@graph"][2]["description"] = desc
        lds.append(HOME_LD)
    elif key == "evidence":
        lds.append({"@context": "https://schema.org", "@graph": [
            {"@type": "ScholarlyArticle", "@id": f"https://arxiv.org/abs/{p['arxiv']}", "headline": clip(p["title"], 110), "name": p["title"],
             "author": [{"@type": "Person", "name": a} for a in p["authors"]], "datePublished": p["published"], "dateModified": p.get("updated") or p["published"],
             "url": f"https://arxiv.org/abs/{p['arxiv']}", "sameAs": f"https://doi.org/{p['doi']}", "abstract": p["abstract"], "description": p["summary"],
             "identifier": [{"@type": "PropertyValue", "propertyID": "arXiv", "value": p["arxiv"]}, {"@type": "PropertyValue", "propertyID": "DOI", "value": p["doi"]}],
             "isBasedOn": {"@id": SRC_ID}, "mainEntityOfPage": f"{ORIGIN}/evidence#pub-{p['arxiv'].replace('.', '-')}"} for p in PUBS]})
    if key != "index":
        lds.append({"@context": "https://schema.org", "@type": "BreadcrumbList", "itemListElement": [
            {"@type": "ListItem", "position": 1, "name": "Home", "item": ORIGIN + "/"}, {"@type": "ListItem", "position": 2, "name": dict((k, l) for k, l, *_ in PAGES)[key], "item": canonical_url(rel)}]})
    t = crawl_text(rel, set_head(rel, t, title, desc, lds))
    open(os.path.join(HERE, rel), "w", encoding="utf-8").write(t)
    seo_pages[rel] = (title, desc)

docs_titles = {}
for p in sorted(glob.glob(os.path.join(HERE, "docs", "*.html"))):
    rel = "docs/" + os.path.basename(p)
    t = open(p, encoding="utf-8").read()
    if rel == "docs/index.html":
        title, head = "Docs: guides, tutorials and reference · Kshana", "Docs"
        desc = "Kshana documentation: tutorials, scenario reference, validation status, the Python, Rust and npm interfaces, and the MCP (Model Context Protocol) server."
    else:
        title, descs, head = docs_seo(rel, t)
        # the first candidate no other page uses (two READMEs can open with the same paragraph)
        desc = next(d for d in descs if d not in {x for _, x in seo_pages.values()})
    crumbs = [("Home", ORIGIN + "/"), ("Docs", ORIGIN + "/docs/")] + ([] if rel == "docs/index.html" else [(head, canonical_url(rel))])
    ld = {"@context": "https://schema.org", "@type": "BreadcrumbList", "itemListElement": [{"@type": "ListItem", "position": i + 1, "name": n, "item": u} for i, (n, u) in enumerate(crumbs)]}
    docs_titles[rel] = (title, desc)
    seo_pages[rel] = (title, desc)
    open(p, "w", encoding="utf-8").write(set_head(rel, t, title, desc, [ld]))
# A title or description two pages share: the second gets its page's own name added (check.py rule 23).
_by_title, _by_desc = {}, {}
for rel in sorted(seo_pages):
    title, desc = seo_pages[rel]
    if title in _by_title or desc in _by_desc:
        sys.exit(f"{rel}: its title or description repeats {_by_title.get(title) or _by_desc.get(desc)} ({title!r} / {desc[:60]!r}); give the page its own wording (docs: its heading or first paragraph)")
    _by_title[title], _by_desc[desc] = rel, rel

# The Studio: a static description and a crawlable list of what it runs (it is a script-drawn app).
_studio_rel = "studio/index.html"
_sp = open(os.path.join(HERE, _studio_rel), encoding="utf-8").read()
_scen = "".join(f'<li><a href="/studio/{esc(STUDIO_LINKS[f]["links"]["best"]).replace("index.html", "", 1)}">{esc(f)}</a>: {esc(file_desc.get(f, ""))}</li>'
                for f in sorted(STUDIO_LINKS) if f in file_desc)
_studio_body = (f'{SEO_BODY_OPEN}<noscript><section class="crawl-text"><h1>{esc(STUDIO)}</h1><p>{esc(STUDIO)} runs Kshana, the open-source PNT (positioning, navigation and timing) '
                f'resilience simulator, in your browser: the engine runs locally as WebAssembly, so nothing is installed and nothing is uploaded. Pick a GNSS (global navigation '
                f'satellite system) jamming, spoofing, clock holdover, integrity or lunar navigation scenario, tune it, replay it, compare runs, sweep a parameter and download a '
                f'reproducible report. It needs JavaScript.</p><h2>Scenarios it runs ({len(STUDIO_LINKS)})</h2><ul>{_scen}</ul>'
                f'<p><a href="/">Kshana home</a> · <a href="/docs/studio">Studio guide</a> · <a href="/developers#install">Install the engine</a></p></section></noscript>{SEO_BODY_CLOSE}')
_sp = re.sub(re.escape(SEO_BODY_OPEN) + r".*?" + re.escape(SEO_BODY_CLOSE), "", _sp, flags=re.S)
_sp, _n = re.subn(r"(<body\b[^>]*>)", lambda m: m.group(1) + "\n" + _studio_body, _sp, count=1)
assert _n == 1, "the Studio page has no <body>"
_sp = set_head(_studio_rel, _sp, *STUDIO_SEO, [{"@context": "https://schema.org", "@type": "WebApplication", "@id": f"{ORIGIN}/studio/#app", "name": STUDIO, "url": ORIGIN + "/studio/",
               "applicationCategory": "DeveloperApplication", "operatingSystem": "Any modern browser (WebAssembly)", "browserRequirements": "Requires JavaScript and WebAssembly",
               "softwareVersion": version, "isAccessibleForFree": True, "license": LICENSE_URL, "offers": {"@type": "Offer", "price": "0", "priceCurrency": "EUR"},
               "isPartOf": {"@id": APP_ID}, "publisher": {"@id": ORG["@id"]}, "description": STUDIO_SEO[1]}])
open(os.path.join(HERE, _studio_rel), "w", encoding="utf-8").write(_sp)
seo_pages[_studio_rel] = STUDIO_SEO

# The Studio's old address. The single-page site and every release up to 0.29.1 linked
# /playground/ and /playground/index.html?scenario=...&tab=...; this page sends them on to /studio/
# with the query and the fragment unchanged. (The repository's copy is written by the port from
# web/tools/legacy-urls.json and loads web/legacy-redirects.js; the behaviour is the same.)
os.makedirs(os.path.join(HERE, "playground"), exist_ok=True)
for _old in glob.glob(os.path.join(HERE, "playground", "*")):
    shutil.rmtree(_old) if os.path.isdir(_old) else os.remove(_old)
write("playground/index.html", f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>{esc(STUDIO)} has moved to /studio/</title>
<meta name="robots" content="noindex">
<link rel="canonical" href="{ORIGIN}/studio/">
<script>location.replace("/studio/" + location.search + location.hash);</script>
<noscript><meta http-equiv="refresh" content="0; url=/studio/"></noscript>
</head>
<body>
<p>{esc(STUDIO)} is now at <a href="/studio/">/studio/</a>.</p>
</body>
</html>
""")

# robots.txt: everything is open to every crawler; the major search and answer engines are named
# so a site-wide rule elsewhere can never shut them out by accident.
CRAWLERS = ["Googlebot", "Bingbot", "GPTBot", "OAI-SearchBot", "ChatGPT-User", "ClaudeBot", "Claude-User", "Claude-SearchBot", "PerplexityBot",
            "Google-Extended", "Applebot", "Applebot-Extended", "CCBot"]
write("robots.txt", "# kshana.dev: every page is open to every crawler, search engine and agent.\n# Generated by the site build. An index of the site for agents: /llms.txt\n\n"
      + "".join(f"User-agent: {c}\nAllow: /\n\n" for c in CRAWLERS) + "User-agent: *\nAllow: /\n\n" + f"Sitemap: {ORIGIN}/sitemap.xml\n")

# sitemap.xml: every indexable page by its clean address; lastmod is the date of the commit the page
# was built from (a docs page: its Markdown source's last commit in the docs checkout).
_docs_src = {f"docs/{pg['slug']}.html": pg["source"] for pg in json.load(open(os.path.join(SCRATCH, "docs-manifest.json"), encoding="utf-8"))["pages"]}
def _lastmod(rel):
    if rel.startswith("docs/"):
        s = _docs_src.get(rel)
        return (_git_date(DOCS_REPO, s) if s and not s.startswith("@") else "") or DOCS_LASTMOD
    return SITE_LASTMOD
_sm_pages = sorted(seo_pages, key=lambda r: (r != "index.html", r.startswith("docs/"), r.count("/"), r))
write("sitemap.xml", '<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n'
      + "".join(f"  <url><loc>{canonical_url(r)}</loc><lastmod>{_lastmod(r)}</lastmod></url>\n" for r in _sm_pages) + "</urlset>\n")

# IndexNow: the key file a search engine fetches to confirm the ping came from this site, and the
# body of that ping (every sitemap address), sent once after a deploy (PORT-RUNBOOK.md: After a deploy).
write(f"{INDEXNOW_KEY}.txt", INDEXNOW_KEY + "\n")
json.dump({"host": "kshana.dev", "key": INDEXNOW_KEY, "keyLocation": f"{ORIGIN}/{INDEXNOW_KEY}.txt", "urlList": [canonical_url(r) for r in _sm_pages]},
          open(os.path.join(SCRATCH, "indexnow.json"), "w", encoding="utf-8"), indent=1)


# llms.txt (https://llmstxt.org): an H1, a one-paragraph summary, then sections of links.
def _md_link(rel, title=None):
    t, d = seo_pages[rel]
    return f"- [{title or re.sub(r' · Kshana( docs)?$', '', t)}]({canonical_url(rel)}): {d}"
_home_desc = PAGE_SEO["index"][1]
_docs_groups = {}
for pg in json.load(open(os.path.join(SCRATCH, "docs-manifest.json"), encoding="utf-8"))["pages"]:
    _docs_groups.setdefault(pg["group"], []).append(f"docs/{pg['slug']}.html")
llms = [f"# Kshana", "", f"> {_home_desc} Version {version}, {cff_lic} licence, by Ashforde OÜ. "
        f"{summ['validated']} of {summ['total']} capabilities are validated against external oracles; the rest are labelled Modelled or partner-owned.", "",
        "Kshana is a deterministic Rust engine for PNT (positioning, navigation and timing) resilience studies: GNSS (global navigation satellite system) jamming and "
        "spoofing, clock holdover, RAIM (receiver autonomous integrity monitoring) and protection levels, orbits, low-Earth-orbit navigation, and navigation at the Moon "
        f"and Mars. Every scenario is a TOML file; every run is reproducible from its scenario, seed and engine version. It is not an RF (radio-frequency) signal simulator, "
        f"not a GNSS receiver and not a flight product.", "",
        "## Site", ""] + [_md_link(f"{k}.html") for k, *_ in PAGES] + [_md_link("studio/index.html", STUDIO), "",
        "## Install", ""] + [f"- {c['name']}: `{c['command']}` ({c['registry']})" for c in channels if c["command"]] + [
        f"- Source code: {REPO_URL}", f"- Cite the software: DOI [{cff_doi}](https://doi.org/{cff_doi})", "",
        "## Research", ""] + [f"- [{p['title']}](https://arxiv.org/abs/{p['arxiv']}): arXiv {p['arxiv']}. {p['summary']}" for p in PUBS] + [""]
for g, rels in _docs_groups.items():
    llms += [f"## Docs: {g}", ""] + [_md_link(r) for r in rels if r in seo_pages] + [""]
llms += ["## Optional", "", f"- [Full text of the site and docs]({ORIGIN}/llms-full.txt): every page above as plain text, generated with the site",
         f"- [Sitemap]({ORIGIN}/sitemap.xml)", f"- [Docs index]({ORIGIN}/docs/)", ""]
write("llms.txt", "\n".join(llms))


class _Text(HTMLParser):
    """Built page HTML -> readable plain text: headings as Markdown, list items as dashes, code kept."""
    SKIP = {"script", "style", "svg", "template", "button", "select", "nav", "aside", "header", "footer"}
    BLOCK = {"p", "div", "section", "article", "li", "tr", "table", "ul", "ol", "dl", "dt", "dd", "figure", "figcaption", "pre", "blockquote", "br", "h1", "h2", "h3", "h4", "h5", "h6", "summary", "details"}

    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.out, self.skip, self.pre = [], 0, 0

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if self.skip or tag in self.SKIP or a.get("aria-hidden") == "true" or "vh" in (a.get("class") or "").split():
            if tag not in ("br", "img", "input", "meta", "link", "hr", "source", "wbr", "use", "path", "circle", "rect", "line"):
                self.skip += 1
            return
        if tag in self.BLOCK:
            self.out.append("\n")
        if tag in ("h1", "h2", "h3", "h4", "h5", "h6"):
            self.out.append("#" * int(tag[1]) + " ")
        elif tag == "li":
            self.out.append("- ")
        elif tag == "pre":
            self.pre += 1
            self.out.append("```\n")

    def handle_endtag(self, tag):
        if self.skip:
            if tag not in ("br", "img", "input", "meta", "link", "hr", "source", "wbr", "use", "path", "circle", "rect", "line"):
                self.skip -= 1
            return
        if tag == "pre":
            self.pre -= 1
            self.out.append("\n```")
        if tag in self.BLOCK:
            self.out.append("\n")
        elif tag in ("td", "th"):
            self.out.append(" | ")

    def handle_data(self, data):
        if not self.skip:
            self.out.append(data if self.pre else re.sub(r"\s+", " ", data))

    def text(self):
        t = "".join(self.out)
        t = "\n".join(l.rstrip() if l.startswith("    ") or l.startswith("```") else l.strip() for l in t.split("\n"))
        return re.sub(r"\n{3,}", "\n\n", t).strip()


def page_text(rel):
    t = open(os.path.join(HERE, rel), encoding="utf-8").read()
    m = re.search(r'<article class="docs-main">(.*?)</article>', t, re.S) or re.search(r"<main\b[^>]*>(.*?)</main>", t, re.S)
    body = re.sub(r'<div class="docs-meta">.*?</div>', "", m.group(1), count=1, flags=re.S)
    body = re.sub(r"</?noscript>", "", body)  # the crawlable fallbacks are part of the text
    p = _Text(); p.feed(body); p.close()
    return p.text()


full = [f"# Kshana: full text of kshana.dev", "", f"Generated by the site build from the published pages, engine v{version}. Index: {ORIGIN}/llms.txt", ""]
for rel in [f"{k}.html" for k, *_ in PAGES] + ["docs/index.html"] + [r for g in _docs_groups.values() for r in g if r in seo_pages]:
    full += ["", "=" * 72, f"URL: {canonical_url(rel)}", f"Title: {seo_pages[rel][0]}", "=" * 72, "", page_text(rel)]
write("llms-full.txt", "\n".join(full) + "\n")


# =================================================================== publish list
pub = []
for root_dir, dirs, fnames in os.walk(HERE):
    rel = os.path.relpath(root_dir, HERE)
    if rel.split(os.sep)[0] in ("src", ".build", "node_modules") or rel.startswith("assets" + os.sep + "geo"):
        continue
    for fn in fnames:
        if rel == "." and fn in ("build.py", "check.py", "COVERAGE.md", "PUBLISH.txt", ".DS_Store"):
            continue
        if fn == ".DS_Store":
            continue
        pub.append(os.path.normpath(os.path.join(rel, fn)))
pub.sort()
write("PUBLISH.txt", "\n".join(pub) + "\n")
total = sum(os.path.getsize(os.path.join(HERE, p)) for p in pub)
print(f"built {len(written)} pages + docs · v{version} · {len(kinds)} kinds · {len(all_files)} files, {len(bundled)} bundled · {summ['validated']}/{summ['modelled']}/{summ['partner_owned']} of {summ['total']} · "
      f"{len(CHARTS)} charts · {len(pub)} files to publish, {total / 1e6:.1f} MB")
for p in written:
    print(f"  {p} {os.path.getsize(os.path.join(HERE, p)):,} bytes")
# Every sentence about Pro the pages quote (pq), for the claims table: .build/pro-quotes.json
json.dump({"public_text": PRO_POWER["public_text"], "quotes": list(dict.fromkeys(PQ_USED))}, open(os.path.join(SCRATCH, "pro-quotes.json"), "w", encoding="utf-8"), indent=1, ensure_ascii=False)
