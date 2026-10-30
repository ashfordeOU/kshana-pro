#!/usr/bin/env python3
"""Build the public Kshana capability dossier v0.40.0 as one self-contained HTML file.

Usage: python3 build.py <path to a kshana checkout at tag v0.40.0> [--toc toc.json]

Every count is read from the checkout at build time (the ledger, the kind pages, the command and
surface tables); every figure is a file from the checkout, embedded as a data URI. Prose that
quotes the repository is taken from README.md, CHANGELOG.md and docs/, with programme names
replaced by neutral wording (see NEUTRAL below).
"""
import base64, html, json, os, re, sys, collections

SRC = os.path.abspath(sys.argv[1])
HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "Kshana-capability-dossier-v0.40.0.html")
TOC = {}
if "--toc" in sys.argv:
    TOC = json.load(open(sys.argv[sys.argv.index("--toc") + 1]))

VERSION = "0.40.0"
DATE = "October 2026"
ADVISORY = ("Advisory: analysis output from a research and engineering tool, not type-approved navigation "
            "equipment and not a safety-of-life service; it must not be the sole basis for any navigation or "
            "safety decision.")
ADVISORY_SHORT = ("Advisory: analysis output from a research and engineering tool, not type-approved navigation "
                  "equipment and not a safety-of-life service.")


def rd(p):
    return open(os.path.join(SRC, p), encoding="utf-8").read()


def b64(p, mime):
    return "data:%s;base64,%s" % (mime, base64.b64encode(open(os.path.join(SRC, p), "rb").read()).decode())


# ---------------------------------------------------------------- neutral wording
# Programme names in the repository's prose are replaced; code identifiers (in backticks) are kept.
NEUTRAL = [
    (r"Moonlight\s*/\s*LCNS-class", "lunar-relay-class"),
    (r"Moonlight/LCNS", "lunar-relay"),
    (r"LCNS\s*/\s*9:2 NRHO", "lunar-relay / 9:2 NRHO"),
    (r"LCNS/NRHO", "lunar-relay/NRHO"),
    (r"LCNS-class", "lunar-relay-class"),
    (r"\bLCNS\b", "lunar-relay"),
    (r"NovaMoon-class", "reference-station-class"),
    (r"\(NovaMoon referenced only as a sys[^)]*\)", ""),
    (r"\bNovaMoon\b", "a lunar reference-station concept"),
    (r"\bMoonlight\b", "lunar relay service"),
    (r"(?i)not affiliated with ESA", "not affiliated with any agency"),
    (r"Celeste IOD preset", "LEO-PNT demonstration preset"),
    (r"Celeste IOD", "a LEO-PNT demonstration"),
    (r"\bCeleste\b", "any LEO-PNT demonstration system"),
    (r"Orion-lander style", "crew-vehicle-to-lander"),
    (r"ESA NAVISP \*?Quantum Wayfarer\*? target", "published programme target"),
    (r"the Gateway and CAPSTONE orbit family", "the CAPSTONE orbit family"),
    (r"a public NASA concept-of-operations slide deck", "a public concept-of-operations slide deck"),
    (r"\bDr T\. S\. Kelso\b,?", ""),
    (r"If you need first-principles cold-atom\s+interferometer error budgets.*?to collaborate\.", ""),
    (r"not a real Gateway ephemeris", "not a real flown ephemeris"),
]


def neutral(s):
    for a, b in NEUTRAL:
        s = re.sub(a, b, s)
    return s


def esc(s):
    return html.escape(s, quote=False)


def inline(s):
    """Markdown inline to HTML: code spans, bold, italics, links (as text)."""
    out = []
    s = re.sub(r"!\[[^\]]*\]\([^)]*\)", "", s)
    s = re.sub(r"\[([^\]]+)\]\([^)]*\)", r"\1", s)
    parts = re.split(r"(`[^`]*`)", s)
    for p in parts:
        if p.startswith("`") and p.endswith("`") and len(p) >= 2:
            out.append("<code>%s</code>" % esc(p[1:-1]))
            continue
        p = neutral(p)
        p = re.sub(r"<[^>]+>", "", p)               # stray inline HTML
        p = re.sub(r"!\[[^\]]*\]\([^)]*\)", "", p)  # images
        p = re.sub(r"\[([^\]]+)\]\([^)]*\)", r"\1", p)
        p = esc(p)
        p = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", p)
        p = re.sub(r"(?<![\w*])\*(?!\s)([^*]+?)\*(?!\w)", r"<em>\1</em>", p)
        out.append(p)
    return "".join(out)


def trunc(s, n):
    if len(s) <= n:
        return s
    cut = s[:n]
    # never end inside a code span
    if cut.count("`") % 2:
        cut = cut[: cut.rfind("`")]
    cut = cut[: cut.rfind(" ")] if " " in cut else cut
    return cut.rstrip(" ,;:(") + " …"


def md_table(lines, cls="tbl", maxcell=None):
    rows = [[c.strip() for c in l.strip().strip("|").split("|")] for l in lines if l.strip()]
    head, body = rows[0], [r for r in rows[2:]]
    h = ['<table class="%s"><thead><tr>' % cls]
    h += ["<th>%s</th>" % inline(c) for c in head]
    h.append("</tr></thead><tbody>")
    for r in body:
        h.append("<tr>" + "".join("<td>%s</td>" % label_chips(inline(trunc(c, maxcell) if maxcell else c)) for c in r) + "</tr>")
    h.append("</tbody></table>")
    return "".join(h)


def label_chips(h):
    h = re.sub(r"(?<![\w-])(VALIDATED|MODELLED|PARTNER)(?![\w-])", r'<span class="lbl \1">\1</span>', h)
    h = re.sub(r"(?<![\w-])(NOT MET|NOT ASSESSABLE)(?![\w-])", r'<span class="lbl NOTMET">\1</span>', h)
    return h


def md_blocks(text):
    """A small block renderer for the repository's Markdown: headings, paragraphs, lists, tables, code."""
    out, lines, i = [], text.split("\n"), 0
    while i < len(lines):
        l = lines[i]
        s = l.strip()
        if not s or s.startswith("<") or s.startswith("<!--"):
            i += 1
            continue
        if s.startswith("```"):
            j = i + 1
            buf = []
            while j < len(lines) and not lines[j].strip().startswith("```"):
                buf.append(lines[j])
                j += 1
            out.append('<pre class="code">%s</pre>' % esc("\n".join(buf)))
            i = j + 1
            continue
        if s.startswith("|"):
            j = i
            while j < len(lines) and lines[j].strip().startswith("|"):
                j += 1
            out.append(md_table(lines[i:j]))
            i = j
            continue
        m = re.match(r"(#{2,4})\s+(.*)", s)
        if m:
            lvl = min(len(m.group(1)) + 1, 5)
            out.append("<h%d>%s</h%d>" % (lvl, inline(m.group(2)), lvl))
            i += 1
            continue
        if re.match(r"[-*] ", s):
            items = []
            while i < len(lines) and lines[i].strip():
                t = lines[i].strip()
                if re.match(r"[-*] ", t):
                    items.append(t[2:])
                elif items:
                    items[-1] += " " + t
                i += 1
            out.append("<ul>%s</ul>" % "".join("<li>%s</li>" % label_chips(inline(x)) for x in items))
            continue
        if s.startswith(">"):
            buf = []
            while i < len(lines) and lines[i].strip().startswith(">"):
                buf.append(lines[i].strip()[1:].strip())
                i += 1
            out.append("<blockquote>%s</blockquote>" % label_chips(inline(" ".join(buf))))
            continue
        buf = []
        while i < len(lines) and lines[i].strip() and not lines[i].strip().startswith(("|", "```", "#", "- ", "<", ">")):
            buf.append(lines[i].strip())
            i += 1
        if buf:
            out.append("<p>%s</p>" % label_chips(inline(" ".join(buf))))
        else:
            i += 1
    return "\n".join(out)


def section(md, heading, level="##"):
    """Text of a Markdown section, up to the next heading of the same or higher level."""
    pat = re.compile(r"^%s %s\s*$" % (re.escape(level), re.escape(heading)), re.M)
    m = pat.search(md)
    if not m:
        raise SystemExit("section not found: %s" % heading)
    rest = md[m.end():]
    stop = re.search(r"^#{1,%d} " % len(level), rest, re.M)
    return rest[: stop.start()] if stop else rest


# ---------------------------------------------------------------- data from the checkout
README = rd("README.md")
CHANGELOG = rd("CHANGELOG.md")
LEDGER = json.load(open(os.path.join(SRC, "web/data/verification-matrix.json")))
ROWS = LEDGER["rows"]
SUM = LEDGER["summary"]
assert SUM["total"] == len(ROWS)
# stable ids from docs/VERIFICATION-MATRIX.md (same order as the JSON)
vm = rd("docs/VERIFICATION-MATRIX.md")
IDS = re.findall(r"^\| (VM-[0-9a-f]{8}) \|", vm, re.M)
assert len(IDS) == len(ROWS), (len(IDS), len(ROWS))
for r, i in zip(ROWS, IDS):
    r["id"] = i

KINDS_MD = rd("docs/KINDS.md")
FAMILIES = []  # (name, slug)
for m in re.finditer(r"^### \[(.+?)\]\(kinds/(.+?)\.md\)", KINDS_MD, re.M):
    FAMILIES.append((m.group(1), m.group(2)))
KIND_LEDGER = {}
for m in re.finditer(r"^\| \[`([a-z0-9-]+)`\]\(kinds/by-kind/.*?\) \| \[(.+?)\]\(.*?\) \| (.+?) \| `(.+?)` \|$", KINDS_MD, re.M):
    KIND_LEDGER[m.group(1)] = dict(family=m.group(2), ledger=m.group(3), example=m.group(4))
N_KINDS = len(KIND_LEDGER)
assert N_KINDS == len(os.listdir(os.path.join(SRC, "docs/kinds/by-kind")))

# the README's family table: what each family answers
FAM_ANSWER = {}
for m in re.finditer(r"^\| \*\*\[(.+?)\]\(docs/kinds/.+?\)\*\* \| (.+?) \| .*? \| `(.+?)` \|$", README, re.M):
    FAM_ANSWER[m.group(1)] = (m.group(2), m.group(3))

FAMILY_KINDS = collections.OrderedDict()
FAMILY_TOPICS = {}
for name, slug in FAMILIES:
    md = rd("docs/kinds/%s.md" % slug)
    ks = re.findall(r"^\| \[`([a-z0-9-]+)`\]\(by-kind/.+?\) \| (.+?) \| (.+?) \|$", md, re.M)
    FAMILY_KINDS[name] = [(k, d, l) for k, d, l in ks]
    t = re.search(r"^Topic pages: (.+?)\. Run one", md, re.M)
    FAMILY_TOPICS[name] = re.findall(r"\[([A-Z0-9-]+)\]", t.group(1)) if t else []
assert sum(len(v) for v in FAMILY_KINDS.values()) == N_KINDS


def parse_kind(k):
    md = rd("docs/kinds/by-kind/%s.md" % k)
    secs = dict(re.findall(r"^## (.+?)\n(.*?)(?=^## |\Z)", md, re.M | re.S))
    what = [p.strip() for p in secs["What it does"].strip().split("\n\n") if p.strip()]
    desc = what[0]
    summ = ""
    for p in what[1:]:
        m = re.match(r"One run of `(.+?)` summarises as: (.*)", p, re.S)
        if m:
            summ = (m.group(1), m.group(2).strip())
    inputs = secs["Inputs"]
    req = re.search(r"- Required fields: (.*)", inputs).group(1)
    opt = re.search(r"- Optional fields: (.*)", inputs)
    opt = opt.group(1) if opt else "none"
    run = re.search(r"```sh\n(.*?)\n```", secs["Run it"], re.S).group(1).strip()
    gives = []
    for l in secs["What it gives"].split("\n"):
        m = re.match(r"\| `(.+?)` \| (.*?) \| (.*?) \|$", l)
        if m and not m.group(1).startswith("evidence."):
            gives.append(m.groups())
    ev = secs["Evidence"].strip().split("\n")
    ev_head = ev[0].split(":")[0]
    bullets = re.findall(r"^- \*\*(VALIDATED|MODELLED|PARTNER)\*\* (.*)$", secs["Evidence"], re.M)
    more = re.search(r"^- and (\d+) more in the matrix", secs["Evidence"], re.M)
    lims = re.findall(r"^- (.*)$", secs["Limitations"], re.M)
    return dict(kind=k, desc=desc, summ=summ, req=req, opt=opt, run=run, gives=gives, ev_head=ev_head,
                bullets=bullets, more=int(more.group(1)) if more else 0, lims=lims)


KIND_DATA = {k: parse_kind(k) for k in KIND_LEDGER}
# a kind whose page carries no engine chart: the figure the 0.38 figure script drew for its example study
KIND_CHART = {"lunar-infrastructure": "docs/figures/0.38/lunar-tie-light.svg"}

# commands and surfaces
CMD_MD = rd("docs/COMMANDS.md")
CMD_ROWS = [l for l in CMD_MD.split("\n") if l.startswith("| `kshana")]
SURF_MD = rd("docs/ALL-SURFACES.md")
SURF_ROWS = [l for l in SURF_MD.split("\n") if l.startswith("| ") and not l.startswith("| Surface") and not l.startswith("|---")]
DOM_MD = rd("docs/CAPABILITY-DOMAINS.md")
DOM_ROWS = [l for l in DOM_MD.split("\n") if l.startswith("| **")]
N_TOOLS = int(re.search(r"Each of its (\d+) tools", README).group(1))
LANGS = re.search(r"beside English: (.+?)\. English is the source", rd("docs/i18n.md"), re.S).group(1).replace("\n", " ")
LANG_LIST = [x.strip() for x in LANGS.split(",")]
N_SCEN = len([f for f in os.listdir(os.path.join(SRC, "scenarios")) if f.endswith(".toml")])
assert len(LANG_LIST) == 17, LANG_LIST

# ledger tallies
by_status = collections.Counter(r["status"] for r in ROWS)
by_oracle = collections.defaultdict(collections.Counter)
for r in ROWS:
    by_oracle[r["oracle_kind"]][r["status"]] += 1
assert by_status["VALIDATED"] == SUM["validated"] and by_status["MODELLED"] == SUM["modelled"]

# figures
def svgfile(p):
    return b64(p, "image/svg+xml")


def fig(p, cap, src=None, cls="fig", mime="image/svg+xml", dark=False):
    src = src or p
    return ('<figure class="%s%s"><img src="%s" alt="%s"><figcaption>%s<span class="src">Source: <code>%s</code></span></figcaption></figure>'
            % (cls, " darkfig" if dark else "", b64(p, mime), esc(re.sub("<[^>]+>", "", cap)), cap, esc(src)))


# ---------------------------------------------------------------- page furniture
SEC = []  # (id, number, title, part)


def chapter(cid, num, title, lede, part=None):
    SEC.append((cid, num, title, part))
    return ('<section class="chapter" id="%s"><div class="chap-head"><div class="chap-num">CHAPTER %s</div>'
            '<h1>%s</h1><p class="lede">%s</p></div>' % (cid, num, title, lede))


def part_page(n, title, lede, items):
    lis = "".join("<li><span>%s</span> %s</li>" % (a, b) for a, b in items)
    return ('<section class="part"><div class="part-inner"><div class="part-n">%s</div><div class="part-k">PART %s</div>'
            '<h1>%s</h1><p>%s</p><ol class="part-list">%s</ol></div>'
            '<p class="part-adv">%s</p></section>' % (n, n, title, lede, lis, ADVISORY_SHORT))


def endsec():
    return '<p class="sec-adv">%s</p></section>' % ADVISORY_SHORT


def stat(n, label, cls=""):
    return '<div class="stat %s"><b>%s</b><span>%s</span></div>' % (cls, n, label)


def bar_svg(data, title, w=640, rowh=18):
    """Horizontal stacked bars (VALIDATED / MODELLED / PARTNER) drawn from ledger counts."""
    keys = [("VALIDATED", "#2F6E0A"), ("MODELLED", "#4F6FB0"), ("PARTNER", "#A8247F")]
    mx = max(sum(c.values()) for _, c in data)
    lab = 230
    h = 40 + rowh * len(data)
    o = ['<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 %d %d" class="chart" role="img" aria-label="%s">' % (w, h, esc(title))]
    x = lab
    for k, c in keys:
        o.append('<rect x="%d" y="6" width="10" height="10" fill="%s"/><text x="%d" y="15" class="lg">%s</text>' % (x, c, x + 14, k))
        x += 110
    for i, (name, c) in enumerate(data):
        y = 30 + i * rowh
        o.append('<text x="%d" y="%d" text-anchor="end" class="ax">%s</text>' % (lab - 8, y + 12, esc(name)))
        x = lab
        for k, col in keys:
            v = c.get(k, 0)
            if v:
                ww = (w - lab - 50) * v / mx
                o.append('<rect x="%.1f" y="%d" width="%.1f" height="%d" fill="%s"/>' % (x, y + 2, ww, rowh - 5, col))
                x += ww
        o.append('<text x="%.1f" y="%d" class="ax">%d</text>' % (x + 4, y + 12, sum(c.values())))
    o.append("</svg>")
    return "".join(o)


# ---------------------------------------------------------------- the document
H = []

# cover --------------------------------------------------------------------
fam_chips = "".join('<span class="chip">%s</span>' % esc(n) for n, _ in FAMILIES)
H.append("""
<section class="cover">
 <div class="cover-top"><img class="logo" src="%(logo)s" alt="Kshana"><div class="brand">ASHFORDE<sup>®</sup><br><small>CAPABILITY DOSSIER · PUBLIC</small></div></div>
 <div class="cover-k">CAPABILITY DOSSIER · ENGINE V%(v)s · %(date)s</div>
 <h1 class="cover-h">Every PNT figure,<br><em>with its evidence.</em></h1>
 <p class="cover-sa">Kshana · <span lang="sa">क्षण</span> · <i>the precise instant</i></p>
 <p class="cover-p">What the open Kshana engine computes for positioning, navigation and timing on Earth, in orbit, at the Moon
 and in deep space; how a run works; every scenario kind with an example command and its headline numbers; every command and
 surface; and how every figure is checked, labelled VALIDATED, MODELLED or PARTNER exactly as the machine-checked ledger has it,
 with the misses shown.</p>
 <div class="cover-stats">%(stats)s</div>
 <div class="chips">%(chips)s</div>
 <div class="cover-adv"><b>Advisory statement.</b> %(adv)s Kshana transmits nothing; its outputs are research and engineering analysis,
 not certifications, approvals or endorsements.</div>
 <div class="cover-foot"><span>ASHFORDE® OÜ · Kshana v%(v)s · Public</span><span>kshana.dev</span></div>
</section>
""" % dict(logo=svgfile("docs/assets/readme/kshana-logo-dark.svg"), v=VERSION, date=DATE.upper(), adv=ADVISORY, chips=fam_chips,
           stats=stat(SUM["total"], "rows in the machine-checked verification ledger")
           + stat(SUM["validated"], "VALIDATED against an independent external oracle", "v")
           + stat(SUM["modelled"], "MODELLED: internally checked, and labelled so", "m")
           + stat(SUM["partner_owned"], "PARTNER: owned by a hardware partner", "p")
           + stat(N_KINDS, "scenario kinds in %d families" % len(FAMILIES))
           + stat(len(LANG_LIST), "languages beside English (machine-translated)")))

# about --------------------------------------------------------------------
H.append("""
<section class="about">
 <div class="about-h"><h1>Kshana capability dossier</h1><div class="brand dark">ASHFORDE<sup>®</sup><br><small>Ashforde OÜ · Estonia</small></div></div>
 <p class="meta">Engine v%(v)s · public edition · %(date)s</p>
 <h3>About this document</h3>
 <p>This dossier describes the open-source Kshana engine as released at version %(v)s (tag <code>v%(v)s</code>, released
 2026-10-29). Every number in it is taken from the public repository at that release: the README, the CHANGELOG, the pages
 under <code>docs/</code>, the verification ledger <code>web/data/verification-matrix.json</code>, the kind pages under
 <code>docs/kinds/</code> (each written from one run of the engine) and the figures the engine and the repository's tools
 drew. Counts (ledger rows, kinds, families, commands, surfaces, languages, tools) are computed from those files when this
 document is built, never typed. Each figure names its source file. Appendix C lists every file used.</p>
 <p>Where a capability is labelled VALIDATED, the ledger row says so; everything else is labelled MODELLED or PARTNER, as the
 ledger does. A bar that a pre-registered comparison did not meet is shown as <span class="lbl NOTMET">NOT MET</span>, never
 softened. This is the third public edition; it supersedes the editions for v0.37.0 and v0.38.0.</p>
 <h3>Advisory statement</h3>
 <div class="adv-box"><p><b>As stated in the engine's own outputs.</b> %(adv)s</p>
 <p>For vessel outputs (receiver trust, training streams): advisory software, not type-approved navigation equipment
 (IEC 61108, IEC 61162); the operator remains responsible for the navigation of the vessel.</p>
 <p>Nothing in this document is a certification, a type approval or a claim of conformance with any programme's
 requirements. Public trajectories (Artemis I, CAPSTONE, LRO) appear only as comparison references; the lunar mission examples
 are "Artemis-class" and illustrative. Kshana is not affiliated with or endorsed by any agency.</p></div>
 <h3>Licence</h3>
 <p>The engine is dual-licensed: the GNU Affero General Public License, version 3 only (AGPL-3.0-only), or a commercial licence
 from Ashforde® OÜ for closed integration into proprietary products. The whole engine, all %(nk)d scenario kinds, has no
 feature gate. Third-party names in this document are used only to identify public data, standards and software; they are
 named in the repository and imply no endorsement.</p>
 <h3>Trademarks</h3>
 <p>ASHFORDE® is a registered trademark of Ashforde OÜ. "Kshana" and its marks are trademarks of Ashforde® OÜ: the licence
 covers the code, not the name.</p>
 <h3>Contact</h3>
 <table class="kv"><tr><td>Web</td><td>kshana.dev · ashforde.org</td></tr>
 <tr><td>Source</td><td>github.com/ashfordeOU/kshana (tag v%(v)s)</td></tr>
 <tr><td>Mail</td><td>contact@ashforde.org (commercial licences, Kshana Pro evaluations, custom studies)</td></tr>
 <tr><td>Cite</td><td>DOI 10.5281/zenodo.20528627 (concept record of every release)</td></tr></table>
 <div class="where"><b>Where to start.</b> New to Kshana: chapters 1 to 4. Looking for a capability: Part II, one chapter per
 family, one entry per kind. Judging how far to trust a figure: chapters 24 to 26. Buying or partnering: chapter 29.</div>
</section>
""" % dict(v=VERSION, date=DATE, adv=ADVISORY, nk=N_KINDS))

TOC_SLOT = len(H)
H.append("")  # contents, filled at the end

# ===================================================================== PART I
H.append(part_page("I", "Kshana at a glance",
                   "What the engine is, how a run works, how it is built, and what changed in the last three releases.",
                   [("1", "Executive summary"), ("2", "What Kshana is, and is not"), ("3", "How a run works"),
                    ("4", "Architecture"), ("5", "What is new in 0.40, 0.39 and 0.38")]))

# 1 executive summary
fam_rows = []
for name, slug in FAMILIES:
    ks = FAMILY_KINDS[name]
    c = collections.Counter()
    for r in ROWS:
        pass
    fam_rows.append(name)
v_kinds = sum(1 for k, d in KIND_LEDGER.items() if "VALIDATED" in d["ledger"])
H.append(chapter("ch1", "01", "Executive summary",
                 "Kshana is an open-source simulation and analysis engine for PNT resilience: it replays jamming, spoofing and "
                 "clock holdover when GNSS fails, and says how long a system keeps time and position inside its budget, with the "
                 "evidence for every figure."))
H.append("""
<div class="stats">%s</div>
<div class="cols2">
<div><h3>A neutral referee for PNT figures</h3>
<p>Kshana (Sanskrit <span lang="sa">क्षण</span>, the precise instant) is one Rust engine. It runs in a terminal, a notebook,
an IDE, an AI assistant or a browser, and every run is reproducible bit for bit from the scenario, the seed and the engine
version. It answers questions such as: how long does this clock hold 20 ns after GNSS is lost; how far does a jammer reach; would
a position push be caught; does the geometry meet the alert limits; can a lander at the lunar south pole get a trustworthy fix and
time.</p>
<p>Every capability carries one of three labels in a machine-checked ledger of %d rows: <span class="lbl VALIDATED">VALIDATED</span>
(%d rows: an independent external oracle agrees), <span class="lbl MODELLED">MODELLED</span> (%d rows: internally consistent, and
said out loud) or <span class="lbl PARTNER">PARTNER</span> (%d rows: a hardware partner owns it). Continuous integration makes it
impossible to call a capability VALIDATED without an independent external oracle, and every run's figures carry the tier of the
weakest ledger row they rest on.</p>
<h3>Scope</h3>
<p>Kshana is a study and trade-off instrument: not radio-frequency signal-generator hardware (it drives no radio and transmits
nothing; its signal-level output is baseband IQ files for software receivers), not a hardware or certified GNSS receiver and not a
flight product. Timing and holdover come first, because that domain is the best validated.</p></div>
<div><h3>One engine, from the signal to the Moon</h3>
<p>A scenario is a small TOML file naming a kind and its parameters. The engine runs it deterministically and writes a result, a
chart and a report that says how to reproduce the run and how far to trust each figure. The %d kinds are filed under %d families:
clocks and timing; inertial, fusion and quantum sensors; GPS-denied alternative PNT; constellations and geometry; GNSS
measurements, positioning and integrity; jamming, spoofing and the spectrum; low-Earth-orbit navigation; lunar and cislunar;
space-user navigation; optical PNT; mission analysis and space operations; mission threads; and campaigns that compose all of
them.</p>
<p>The same engine is reached from the command line, the Rust library, Python, JavaScript and WebAssembly, Kshana Studio in the
browser, an MCP server with %d tools for AI assistants, a JetBrains plugin, Signal K and OpenCPN on a boat, ROS 2, GNSS-SDR and
<code>gnss_lib_py</code> adapters, and systemd, Docker and Grafana deploy kits.</p>
<h3>New in 0.40</h3>
<p>A mission-level error budget from launch to entry (the <code>mission-thread</code> kind and four lunar phase kinds), an
evidence label on every figure and table a run writes, measured clock logs in eight formats, a data-partner submission check, an
estimated emitter region from ADS-B or AIS reports, the 0.40 benchmark re-campaign, and seventeen languages beside English. The
ledger grew from 388 to %d rows; no row was promoted to VALIDATED, and every miss is pinned by a test (chapter 26).</p></div>
</div>
""" % (stat(SUM["total"], "ledger rows, machine-checked") + stat(SUM["validated"], "VALIDATED, independent external oracle", "v")
       + stat(SUM["modelled"], "MODELLED, labelled so on every figure", "m") + stat(SUM["partner_owned"], "PARTNER, hardware partner owns it", "p")
       + stat(N_KINDS, "scenario kinds") + stat(len(FAMILIES), "kind families") + stat(len(CMD_ROWS), "command-line entries")
       + stat(N_TOOLS, "MCP tools for AI assistants"),
       SUM["total"], SUM["validated"], SUM["modelled"], SUM["partner_owned"], N_KINDS, len(FAMILIES), N_TOOLS, SUM["total"]))
H.append(fig("docs/assets/figures/validation-breakdown.svg",
             "The ledger at v0.40.0: %d VALIDATED, %d MODELLED, %d PARTNER of %d rows." % (SUM["validated"], SUM["modelled"], SUM["partner_owned"], SUM["total"])))
fam_tbl = ['<table class="tbl"><thead><tr><th>Family</th><th>What it answers</th><th>Kinds</th><th>Kinds with a VALIDATED row</th></tr></thead><tbody>']
for name, slug in FAMILIES:
    ks = FAMILY_KINDS[name]
    nv = sum(1 for k, _, _ in ks if "VALIDATED" in KIND_LEDGER[k]["ledger"])
    fam_tbl.append("<tr><td><b>%s</b></td><td>%s</td><td>%d</td><td>%d</td></tr>" % (esc(name), inline(FAM_ANSWER[name][0]), len(ks), nv))
fam_tbl.append("</tbody></table>")
H.append("<h3>The %d families</h3>%s<p class='note'>A kind counts as having a VALIDATED row when <code>docs/KINDS.md</code> lists "
         "at least one VALIDATED ledger row naming it; %d of %d kinds do. The rest are MODELLED (some by the label their result "
         "carries), as the ledger says.</p>" % (len(FAMILIES), "".join(fam_tbl), v_kinds, N_KINDS))
H.append(endsec())

# 2 what it is / is not
isnot = section(README, "What it is / is not", "###")
H.append(chapter("ch2", "02", "What Kshana is, and is not",
                 "Owning the scope is deliberate: Kshana sits next to mission-analysis and signal tools and exchanges files with them; "
                 "it does not replace them."))
H.append(md_blocks(isnot))
H.append("""<h3>In one table</h3><table class="tbl"><thead><tr><th>Kshana is</th><th>Kshana is not</th></tr></thead><tbody>
<tr><td>a deterministic engine spanning orbit geometry, inertial navigation, GNSS/INS fusion, integrity, clocks and timing</td><td>flight hardware or a flight product</td></tr>
<tr><td>a study and trade-off instrument with a label on every figure</td><td>a certified avionics product, a type approval or a certification</td></tr>
<tr><td>an analysis software receiver for recorded or generated baseband IQ</td><td>a hardware or certified GNSS receiver; it drives no radio and transmits nothing</td></tr>
<tr><td>a performance-simulation layer above GMAT and Orekit, exchanging SP3, CCSDS OMM/OEM, CZML, KML, GeoJSON, STK and SigMF files</td><td>a replacement for MATLAB/Simulink, STK, GMAT or Orekit</td></tr>
<tr><td>a referee that scores receiver logs and timing tests against stated bars</td><td>a ranking of receivers or products; a met bar says a modelled scenario met a stated bar</td></tr>
<tr><td>quantum-sensor performance models from published error models</td><td>a quantum-payload design or a full atom-interferometry physics engine</td></tr>
</tbody></table>
<h3>Advisory, everywhere</h3><p>The advisory statement is printed by the engine on its reports, by the Studio on every page, by
the MCP server and by the vessel integrations. It is kept, word for word in meaning, in all seventeen translations, together with
the verdict words and the evidence tiers.</p>""")
H.append(endsec())

# 3 how a run works
toml = rd("scenarios/clock-holdover.toml")
toml = re.sub(r'provenance = ".*"', 'provenance = "…"   # source of the noise figure, shortened here', toml)
H.append(chapter("ch3", "03", "How a run works",
                 "A scenario is a TOML file: a kind, a seed and the parameters. The engine runs it and writes the result, a chart and a "
                 "report beside it, and the exports you ask for."))
H.append('<div class="cols2"><div>')
H.append("""<h3>Scenario, seed, engine version</h3>
<p>The engine reads one TOML document. Its <code>kind</code> selects one of the %d scenario kinds (a file without one is the
original clock-holdover pack); its <code>seed</code> fixes every random draw; the remaining tables set the parameters, each with the
unit the schema states. <code>kshana kinds --json</code> lists every kind with its required and optional fields, and
<code>kshana --validate</code> lints a scenario without running it.</p>
<p><b>Reproducible by construction:</b> scenario + seed + engine version give identical bits. Every result is stamped with the
scenario hash, the seed and the engine version; <code>scripts/check-reproducible.sh</code> enforces it and continuous integration
compares SHA-256 goldens on Linux, macOS and Windows. Quantum and classical runs use independent seeds so their noise is
uncorrelated.</p>
<h3>The bundled example</h3>
<p>This bundled scenario runs 2 h: 10 min of GNSS, then about 1.8 h with GNSS denied. It asks how long a strontium optical lattice
clock and a chip-scale atomic clock (CSAC) each hold time to within 20 ns.</p>
<pre class="code">kshana example clock-holdover &gt; clock-holdover.toml
kshana clock-holdover.toml</pre>
<pre class="code out">scenario 5ba83a232b94 | quantum holdover 6600s p95 1.20e-4ns integrity 1.000 security n/a (no attack) | classical holdover 2610s p95 19.7ns integrity 1.000 security n/a (no attack)
wrote clock-holdover.result.json, clock-holdover.chart.svg, clock-holdover.report.html, and clock-holdover.report.json</pre>
<p>The optical clock holds the 20 ns budget for the whole 6600 s outage; the CSAC holds it for 2610 s. The same numbers come back
from Python (<code>kshana.run</code>) and from WebAssembly (<code>run</code>), version 0.40.0, 6600 and 2610.</p>
</div><div>
<h3>The scenario file</h3><pre class="code">%s</pre>
<h3>What a run writes</h3>
<table class="tbl"><tbody>
<tr><td><code>*.result.json</code></td><td>every figure of merit with its unit, the reproducibility stamp and the <code>evidence</code> key</td></tr>
<tr><td><code>*.chart.svg</code></td><td>the chart, with a line under the plot naming the tier and the ledger rows</td></tr>
<tr><td><code>*.report.html</code>, <code>*.report.json</code></td><td>the report: method, figures, limitations and capability labels</td></tr>
<tr><td><code>*.table.csv</code></td><td>the byte-stable reproducibility table, with <code># evidence:</code> comment lines in front</td></tr>
<tr><td>exports on request</td><td>SP3, CCSDS OMM and OEM, CZML, KML, GeoJSON, STK <code>.e</code>, SigMF; animations</td></tr>
</tbody></table>
</div></div>""" % (N_KINDS, esc(toml.strip())))
H.append("<h3>The evidence label on every figure (new in 0.40)</h3>" + md_blocks(section(README, "The evidence label on every figure", "###")))
H.append(fig("docs/assets/readme/flow-pipeline-light.svg", "From a scenario file to a stamped result, chart and report."))
H.append(endsec())

# 4 architecture
H.append(chapter("ch4", "04", "Architecture",
                 "One open engine sits at the centre, and every surface runs it. The same scenario file gives the same bytes on each."))
arch = section(README, "Architecture")
arch_inner = re.search(r"\*\*One engine, many front doors\.\*\*(.*?)</details>", arch, re.S).group(0).replace("</details>", "")
H.append(fig("docs/assets/readme/architecture-light.svg", "The engine and its surfaces. Kshana Pro sits on top as a separate overlay that depends on the engine and never forks it."))
H.append('<div class="cols2"><div>%s</div><div>' % md_blocks(arch_inner))
H.append("""<h3>The determinism guarantee</h3>
<ul><li><b>Same scenario, same seed, same engine version: same bits.</b> No wall-clock, no unseeded randomness, no
network access in a run.</li>
<li><b>Same engine everywhere.</b> The command line, the library, Python, WebAssembly, the Studio, the MCP server and the plugins
all converge on one <code>api::run_toml</code> dispatch.</li>
<li><b>Machine files never depend on the language.</b> A run in another language executes the scenario twice: once in English for
the JSON, CSV and report data, once in the chosen language for the person.</li>
<li><b>Labels are read, not typed.</b> The tier and ledger ids on every figure come from <code>src/verification.rs</code>; a test
fails if a figure has no label or a label disagrees with the matrix.</li></ul>
</div></div>""")
for f, c in [("system-overview", "One dispatch, one engine, reproducible output."),
             ("engine-flow", "The inner loop of a run."),
             ("module-map", "Where the code lives."),
             ("surfaces", "Every surface, from the same engine."),
             ("trust-flow", "From a receiver log to a trust score and what uses it."),
             ("lab-round-trip", "Scenario to signed pack; Kshana never produces or replays a signal."),
             ]:
    H.append(fig("docs/assets/diagrams/%s.svg" % f, c, cls="fig diag"))
H.append(endsec())

# 5 what's new
H.append(chapter("ch5", "05", "What is new in 0.40, 0.39 and 0.38",
                 "Three releases in October 2026: 0.38 made the neutral referee measurable, 0.39 changed no result, 0.40 added the "
                 "mission thread, evidence labels, measured clocks and seventeen languages."))
new40 = section(README, "What's new in 0.40")
H.append("<h2>0.40.0 · released 2026-10-29</h2>" + md_blocks(new40))
H.append("""<h3>Security and toolchain, in 0.40</h3>
<ul><li><code>p256</code>, <code>p384</code> and <code>p521</code> 0.13 to 0.14 (with <code>elliptic-curve</code> 0.14.3,
<code>ecdsa</code> 0.17.0 and <code>crypto-bigint</code> 0.5.5 to 0.7.10) clear RUSTSEC-2026-0387, timing variability in
<code>crypto-bigint</code> 0.5.5. Kshana uses these crates only to verify public signatures (the Galileo OSNMA root key and RFC 3161
timestamp tokens) and signs only in tests with throwaway keys, so the practical exposure was nil; the upgrade removes the advisory
without an ignore entry. No verification result changes.</li>
<li>The minimum supported Rust is now 1.88, required by that fix.</li></ul>
<p class="note">The 0.40.0 CHANGELOG files these two entries under 0.39.0; they shipped in 0.40.0.</p>""")
cl40 = section(CHANGELOG, "[0.40.0] - 2026-10-29")
added = section(cl40, "Added", "###")
# the mission thread and its lunar segment, from the changelog
mt = [b for b in re.split(r"\n(?=- )", added) if "mission-thread" in b[:60] or "lunar segment" in b[:60]]
H.append("<h3>The mission thread, in the changelog's words</h3>" + md_blocks("\n".join(mt)))
H.append(fig("docs/figures/kinds/mission-thread.svg", "The chart the engine writes for <code>scenarios/mission-thread-artemis-class.toml</code>: an Artemis-class example over 11 phases. Every number MODELLED.", dark=True))
H.append("<h3>The lunar-segment evidence</h3>" + md_blocks(section(cl40, "Evidence", "###")))
H.append(fig("examples/emitter-region/output/emitter-adsb-custom-2026-03-01-figure.svg",
             "Emitter region from the synthetic example in <code>examples/emitter-region/</code>: regions at stated confidence and a centroid with its ellipse, never a point. MODELLED; calibrated on synthetic data only."))
H.append("<h3>Measured clocks, partner data and the benchmark</h3>")
other = [b for b in re.split(r"\n(?=- )", added) if re.match(r"- (\*\*Measured clock|\*\*A holdover driven|\*\*The 0\.40 benchmark|`kshana partner check|\*\*Second campaigns)", b)]
H.append(md_blocks("\n".join(other)))
H.append("<h3>Known limits of 0.40</h3>" + md_blocks(section(cl40, "Known limits", "###")))
H.append("<h2>0.39.0 · released 2026-10-22</h2>")
H.append(md_blocks(section(README, "What's new in 0.39")))
H.append("""<p>The 0.39 dependency updates were in the cross-validation tooling only: <code>memmap2</code> 0.9.10 to 0.9.11
(RUSTSEC-2026-0186) in the ANISE cross-check locks, and <code>pillow</code> 13.0.0 and <code>certifi</code> 2026.10.19 in the
oracle environment pins. 0.39 also completed the README: the scenario kinds filed under families with a page per kind, every
command, every surface and every capability domain, guarded by a completeness test.</p>""")
H.append("<h2>0.38.0 · released 2026-10-19</h2>")
H.append(md_blocks(section(README, "What's new in 0.38")))
for slug, cap, src in [("jammertest", "Jammertest scoring: a receiver log scored per scheduled test (synthetic log written by the figure script).", None),
                       ("receiver-flags", "The receiver's own flags beside Kshana's alarms (synthetic UBX log).", None),
                       ("timing", "Timing and holdover test scored against an ITU-T mask.", None),
                       ("standards", "Standards packs: tests against bars stated before the run.", None),
                       ("lunanet-afs", "LunaNet AFS codes and frame checked against the printed check columns.", None),
                       ("optical", "Optical link margin against time: one seeded realisation of a site's weather.", None),
                       ("lunar-od", "Lunar infrastructure: onboard and ground orbit determination compared.", None),
                       ]:
    H.append(fig("docs/figures/0.38/%s-light.svg" % slug, cap + " Drawn by <code>tools/gen_figures_038.py</code> from engine output.", cls="fig half"))
H.append(endsec())

# ===================================================================== PART II
H.append(part_page("II", "Every kind, family by family",
                   "The %d scenario kinds the engine v%s dispatches, filed under %d families. Each entry gives what the kind "
                   "answers, its inputs, the example command, headline numbers from one run, the ledger rows that name it and "
                   "the limitations its own report states." % (N_KINDS, VERSION, len(FAMILIES)),
                   [("6", "The families at a glance")] + [(str(7 + i), esc(n)) for i, (n, _) in enumerate(FAMILIES)]))

H.append(chapter("ch6", "06", "The %d families at a glance" % len(FAMILIES),
                 "How to read a kind entry, and where every kind sits."))
H.append("""<div class="cols2"><div><h3>How to read an entry</h3>
<p>Each entry is generated from the kind's page under <code>docs/kinds/by-kind/</code>, which the repository writes from the
engine itself (<code>kshana kinds --json</code>), the schema, the verification matrix and one run of the example scenario.</p>
<ul><li><b>The label</b> beside the name is the ledger's: the counts of VALIDATED and MODELLED rows that name the kind, or the label
the kind's result carries. A kind is MODELLED unless the matrix says otherwise.</li>
<li><b>What it answers</b> is the engine's own description of the kind.</li>
<li><b>One run</b> is the summary line of a real run of the example scenario, and <b>headline numbers</b> are scalar results of
that run, with the units the engine states.</li>
<li><b>Ledger rows</b> are the verification-matrix rows that name the kind (the first few; the count of the rest is given).</li>
<li><b>Limits</b> are what the run's own report says is not modelled or not validated.</li>
<li><b>The chart</b> is the one the engine writes for that run, unchanged (dark theme).</li></ul></div><div>
<h3>Run any of them</h3><pre class="code">kshana example                       # list the bundled scenarios
kshana example lunar-vlbi &gt; s.toml   # print one to start from
kshana s.toml                        # run it: result, chart, report
kshana kinds --json                  # every kind and its fields</pre>
<p>The repository bundles %d scenario files at the top of <code>scenarios/</code>. The example command in each entry runs from a
checkout.</p></div></div>""" % N_SCEN)
idx = ['<table class="tbl small"><thead><tr><th>Family</th><th>Kinds</th></tr></thead><tbody>']
for name, slug in FAMILIES:
    idx.append("<tr><td><b>%s</b></td><td>%s</td></tr>" % (esc(name), ", ".join("<code>%s</code>" % k for k, _, _ in FAMILY_KINDS[name])))
idx.append("</tbody></table>")
H.append("".join(idx))
H.append(endsec())

for fi, (name, slug) in enumerate(FAMILIES):
    num = "%02d" % (7 + fi)
    ks = FAMILY_KINDS[name]
    ans, runone = FAM_ANSWER[name]
    H.append(chapter("ch%d" % (7 + fi), num, esc(name), "%s. %d kind%s." % (inline(ans[0].upper() + ans[1:]), len(ks), "" if len(ks) == 1 else "s")))
    tops = ", ".join("<code>docs/%s.md</code>" % t for t in FAMILY_TOPICS[name])
    H.append('<p class="fam-meta">Run one: <code>%s</code>%s</p>' % (esc(runone), (" · Topic pages: " + tops) if tops else ""))
    H.append(fig("docs/figures/kinds/sheet-%s.svg" % slug, "Contact sheet: the charts the engine writes for the kinds of this family.", cls="fig sheet", dark=True))
    t = ['<table class="tbl small"><thead><tr><th>Kind</th><th>What it does</th><th>Ledger</th></tr></thead><tbody>']
    for k, d, l in ks:
        t.append("<tr><td><code>%s</code></td><td>%s</td><td>%s</td></tr>" % (k, inline(trunc(d, 150)), label_chips(esc(l))))
    t.append("</tbody></table>")
    H.append("".join(t))
    for k, _, _ in ks:
        d = KIND_DATA[k]
        g = "".join("<tr><td><code>%s</code></td><td>%s</td><td>%s</td></tr>" % (esc(a), esc(b), esc(c)) for a, b, c in d["gives"][:6])
        bl = [(a, t) for a, t in d["bullets"] if not t.startswith("Deterministic, scenario-hashed")]
        ev = "".join('<li>%s %s</li>' % (label_chips(a), inline(trunc(re.sub(r" Tests: .*$", "", t), 170))) for a, t in bl[:2])
        if d["more"] or len(bl) > 2:
            ev += "<li class='more'>and %d more ledger row(s) naming this kind</li>" % (d["more"] + max(0, len(bl) - 2))
        lm = [x for x in d["lims"] if not x.startswith("Reproducibility & software assurance")]
        lim = "".join("<li>%s</li>" % inline(trunc(re.sub(r"\s*\([^()]*\)\s*$", "", x), 170)) for x in lm[:2])
        summ = ""
        if d["summ"]:
            summ = '<p class="run1"><b>One run</b> of <code>%s</code>: <span class="mono">%s</span></p>' % (esc(d["summ"][0]), esc(neutral(d["summ"][1])))
        opt = d["opt"]
        H.append("""<article class="kind" id="k-%(k)s">
<header><h3><code>%(k)s</code></h3><span class="kl">%(lab)s</span></header>
<div class="kgrid"><div class="ktext">
<p class="desc">%(desc)s</p>
%(summ)s
<p class="fields"><b>Inputs.</b> Required: %(req)s. Optional: %(opt)s.</p>
<pre class="code">%(run)s</pre>
<div class="ev"><b>Ledger rows</b> (%(evh)s)<ul>%(ev)s</ul></div>
<div class="lim"><b>Limits</b><ul>%(lim)s</ul></div>
</div><div class="kside">
<img class="kchart" src="%(chart)s" alt="Chart the engine writes for %(k)s">
<table class="gives"><thead><tr><th>Headline result</th><th>Value</th><th>Unit</th></tr></thead><tbody>%(g)s</tbody></table>
</div></div></article>""" % dict(k=k, lab=label_chips(esc(KIND_LEDGER[k]["ledger"])), desc=inline(trunc(d["desc"], 520)), summ=summ,
                                   req=inline(d["req"]), opt=inline(trunc(opt, 220)), run=esc(d["run"]), evh=label_chips(esc(d["ev_head"])),
                                   ev=ev, lim=lim, chart=svgfile(KIND_CHART.get(k, "docs/figures/kinds/%s.svg" % k)), g=g))
    H.append(endsec())

# ===================================================================== PART III
nfam = 7 + len(FAMILIES)
H.append(part_page("III", "Using Kshana", "Every command, every surface, the sectors it serves and the languages it speaks.",
                   [(str(nfam), "Every command"), (str(nfam + 1), "Every surface"), (str(nfam + 2), "Missions: ten sectors"),
                    (str(nfam + 3), "Seventeen languages")]))
H.append(chapter("ch%d" % nfam, "%02d" % nfam, "Every command",
                 "<code>kshana &lt;scenario.toml&gt;</code> runs a scenario; every other command is a subcommand. <code>kshana --help</code> "
                 "prints the usage of all of them, and each subcommand's own <code>--help</code> lists its flags."))
H.append(md_table(["| Command | What it does | Example | Docs |", "|---|---|---|---|"] + CMD_ROWS, cls="tbl small cmds"))
H.append("<h3>Usage, from the README</h3>")
usage = README[README.index("### Usage\n"):README.index("### Scenario format")]
usage = usage[len("### Usage\n"):usage.index("**Interoperability role.")]
H.append(md_blocks(usage.replace("cargo run -- ", "kshana ")))
H.append("<p class='note'>The README writes these as <code>cargo run -- …</code> from a checkout; with an installed binary the same "
         "arguments follow <code>kshana</code>.</p>")
H.append(endsec())

H.append(chapter("ch%d" % (nfam + 1), "%02d" % (nfam + 1), "Every surface",
                 "One engine, the same results everywhere: the command line, Rust, Python, JavaScript and WebAssembly, Kshana Studio, "
                 "the MCP server, the JetBrains plugin, Signal K and OpenCPN, ROS 2, GNSS-SDR and the deploy kits."))
H.append(md_table(["| Surface | What it is | Get it | Docs |", "|---|---|---|---|"] + SURF_ROWS, cls="tbl small"))
H.append("<h3>Kshana Studio</h3>" + md_blocks(section(README, "Kshana Studio").split("### The site")[0]))
H.append('<div class="shots">%s%s</div>' % (
    fig("docs/assets/readme/studio/studio-task-light.jpg", "Studio, Simple view: the answer first, then key figures and at most five settings (bundled scenario <code>integrity-raim</code>).", mime="image/jpeg", cls="fig half"),
    fig("docs/assets/readme/studio/studio-advanced-light.jpg", "Studio, Advanced view: the scenario library, five numbered steps, every panel.", mime="image/jpeg", cls="fig half")))
H.append("""<h3>The MCP server for AI assistants</h3>
<p><code>kshana-mcp</code> is a Model Context Protocol server: an assistant runs the actual engine and reads back its JSON, instead of
guessing the maths. Each of its %d tools is a thin wrapper over a public function of the <code>kshana</code> library, so no simulation
logic lives in the server. It runs on the user's own computer and sends nothing to Ashforde OÜ; it is installed with
<code>cargo install kshana-mcp</code>, <code>npx -y kshana-mcp@0.40.0</code>, <code>uvx kshana-mcp==0.40.0</code>, a Docker image,
Homebrew, Scoop, winget, prebuilt binaries, desktop-extension bundles, one-click IDE links, the official MCP registry
(<code>io.github.ashfordeOU/kshana-mcp</code>) or as a self-hosted streamable-HTTP service. The plugin adds slash commands and skills
over the tools.</p>""" % N_TOOLS)
tools = section(README, "AI assistant")
tools = tools[tools.find("**Scenarios and reports**"):tools.find("</details>")]
groups = re.findall(r"\*\*(.+?)\*\*\n\n((?:\|.*\n?)+)", tools)
tl = []
for gname, tbl in groups:
    names = re.findall(r"^\| `([a-z_0-9]+)`", tbl, re.M)
    tl.append("<tr><td><b>%s</b></td><td>%d</td><td>%s</td></tr>" % (esc(gname), len(names), ", ".join("<code>%s</code>" % n for n in names)))
assert sum(len(re.findall(r"^\| `([a-z_0-9]+)`", t, re.M)) for _, t in groups) == N_TOOLS
H.append('<table class="tbl small"><thead><tr><th>Group</th><th>Tools</th><th>Names</th></tr></thead><tbody>%s</tbody></table>' % "".join(tl))
H.append("""<h3>Rust, Python and JavaScript</h3><div class="cols2"><div><pre class="code">import json, kshana

toml = open("clock-holdover.toml").read()
result = json.loads(kshana.run(toml))
print(kshana.version(),
      result["quantum"]["fom"]["holdover_s"],
      result["classical"]["fom"]["holdover_s"])
# 0.40.0 6600.0 2610.0</pre><p>Beyond <code>run</code>, the Python module exposes <code>run_full</code> (JSON, SVG and the
one-line summary at once), <code>run_typed</code>, <code>validate_toml</code>, <code>list_kinds</code> / <code>scenario_kinds</code>,
<code>error_kind</code> and, since 0.40, <code>run_human(toml, lang)</code>, with type stubs.</p></div><div>
<pre class="code">import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { initSync, run, summary, version } from "kshana";

const wasm = createRequire(import.meta.url)
  .resolve("kshana/kshana_bg.wasm");
initSync({ module: readFileSync(wasm) });
const toml = readFileSync("clock-holdover.toml", "utf8");
const r = JSON.parse(run(toml));
console.log(version(), r.quantum.fom.holdover_s,
            r.classical.fom.holdover_s);
// 0.40.0 6600 2610</pre><p>The npm package is the whole engine compiled to WebAssembly; the Rust library is
<code>cargo add kshana</code> with <code>kshana::api::run_toml</code> and the typed modules.</p></div></div>""")
H.append("""<h3>On a boat, in a lab, on a server</h3><div class="cols2"><div>
<p><b>Signal K and OpenCPN.</b> The trust score on a boat's Signal K server (<code>npm install signalk-kshana-trust</code>) and in
OpenCPN, with a reference build for a small computer next to the receiver (two systemd units and a container option). Advisory, not
type-approved navigation equipment; the operator remains responsible.</p>
<p><b>ROS 2.</b> A package that scores a receiver's topics through the same receiver-trust timeline.</p>
<p><b>GNSS-SDR and <code>gnss_lib_py</code>.</b> Adapters that feed their output to receiver trust.</p></div><div>
<p><b>Grafana and telemetry.</b> The trust stream as Prometheus metrics, CEF and LEEF syslog lines, OTLP and Home Assistant MQTT
messages, with a sample Grafana dashboard. Publish only, to a destination you give; no credentials held.</p>
<p><b>JetBrains.</b> Run a scenario from the IDE (plugin ID <code>dev.kshana.ide</code>).</p>
<p><b>Deploy kits.</b> <code>deploy/reference-build/</code> (systemd, Docker) and <code>deploy/grafana/</code>.
<code>docs/SURFACES.md</code> says, for every command and kind, which surface carries it and which do not, with the reason.</p></div></div>""")
H.append(endsec())

H.append(chapter("ch%d" % (nfam + 2), "%02d" % (nfam + 2), "Missions: ten sectors",
                 "Ten sectors ask ten different questions of the same engine. Each has bundled scenarios to start from."))
miss = section(README, "Missions")
tb = re.search(r"\| Sector \| The question it asks \|.*?\n((?:\|.*\n)+)", miss).group(0)
H.append(md_table(tb.strip().split("\n")))
H.append("<h3>Capabilities: what each area answers</h3>")
capt = re.search(r"\| Capability \| What it answers \|\n\|---\|---\|\n((?:\|.*\n)+)", README).group(0)
H.append(md_table(capt.strip().split("\n"), cls="tbl small"))
H.append(endsec())

H.append(chapter("ch%d" % (nfam + 3), "%02d" % (nfam + 3), "Seventeen languages",
                 "New in 0.40: the engine's human-facing text, the site and the Studio in seventeen languages beside English, as machine "
                 "translations that no native speaker has reviewed. English is the reference."))
H.append('<div class="langs">%s</div>' % "".join('<span class="chip">%s</span>' % esc(x) for x in LANG_LIST))
i18n = rd("docs/i18n.md")
H.append(md_blocks(section(i18n, "What is localised and what never is")))
H.append(md_blocks(section(i18n, "Choosing the language")))
H.append(md_blocks(section(i18n, "How a run works in another language (the run-twice rule)")))
H.append("<p><b>Known limits.</b> Non-English runs take about twice as long; some Studio sentences are assembled from fragments and may lack "
         "articles in languages with grammatical gender; the translations are machine translations that no native speaker has reviewed. "
         "Verdict words, evidence tiers and the advisory scope are kept in every language.</p>")
H.append(endsec())

# ===================================================================== PART IV
n4 = nfam + 4
H.append(part_page("IV", "Evidence", "How a label is earned, what the ledger holds, the strongest checks and every recorded miss.",
                   [(str(n4), "The evidence ledger and the honesty model"), (str(n4 + 1), "Validation highlights"),
                    (str(n4 + 2), "The recorded misses"), (str(n4 + 3), "Capability domains"), (str(n4 + 4), "Research built on the engine")]))
H.append(chapter("ch%d" % n4, "%02d" % n4, "The evidence ledger and the honesty model",
                 "%d of %d capabilities are VALIDATED against independent external oracles; %d are honestly labelled MODELLED; %d are "
                 "owned by a hardware partner." % (SUM["validated"], SUM["total"], SUM["modelled"], SUM["partner_owned"])))
H.append('<div class="stats">%s</div>' % (stat(SUM["total"], "rows") + stat(SUM["validated"], "VALIDATED", "v") + stat(SUM["modelled"], "MODELLED", "m") + stat(SUM["partner_owned"], "PARTNER", "p")))
ev = section(README, "Evidence")
lab_tbl = re.search(r"\| Label \| Rows \| Meaning \|\n(?:\|.*\n)+", ev).group(0)
H.append(md_table(lab_tbl.strip().split("\n")))
H.append("""<div class="cols2"><div><h3>The rules</h3>
<ul><li><b>A row is VALIDATED only with an independent external oracle</b>: real data, an independent implementation or published
reference vectors. The matrix invariant tests enforce it, so continuous integration cannot call a capability VALIDATED without one.</li>
<li><b>VALIDATED describes the method of checking, not a pass.</b> An honest miss against a real external oracle stays VALIDATED;
a comparison that no matrix row names as its oracle stays MODELLED, however good the agreement.</li>
<li><b>MODELLED is said out loud.</b> Checked against analytic truth or simulation self-consistency; why each MODELLED row has no
external oracle is written in <code>docs/MODELLED-RATIONALE.md</code>.</li>
<li><b>PARTNER</b> rows are owned by a hardware partner; Kshana holds no oracle for them.</li>
<li><b>The weakest row wins.</b> A run's tier is the weakest of the rows it exercised, so a run that leans on one MODELLED row is
labelled MODELLED however many VALIDATED rows sit beside it.</li></ul></div><div>
<h3>Pre-registered bars, misses shown</h3>
<ul><li>Comparisons against external data are pre-registered: the bar, the tolerance and the data are fixed in a dated file under
<code>xval/</code> before any run, and amendments are disclosed in that file.</li>
<li>A bar that is not met is reported as NOT MET, kept, and pinned by a test, so a later change cannot silently turn it into a pass.
No bar has been loosened to turn a miss into a pass.</li>
<li>Exploratory analyses are labelled EXPLORATORY and do not replace the run of record.</li>
<li>Each row names its requirement, the capability, the module, the tests and the oracle; the ledger is generated from
<code>src/verification.rs</code> and pinned by <code>tests/verification_artifacts_doc_sync.rs</code>.</li></ul></div></div>""")
oracle_data = sorted(by_oracle.items(), key=lambda kv: -sum(kv[1].values()))
H.append('<figure class="fig"><div class="chartbox">%s</div><figcaption>Ledger rows by oracle kind and status, counted from the ledger at build time.<span class="src">Source: <code>web/data/verification-matrix.json</code> (fields <code>oracle_kind</code>, <code>status</code>)</span></figcaption></figure>'
         % bar_svg([(k, c) for k, c in oracle_data], "Ledger rows by oracle kind"))
H.append(fig("docs/assets/figures/oracle-kind-stacked.svg", "Every VALIDATED row is backed by an external oracle, by construction."))
H.append(fig("docs/assets/figures/sgp4-regime-bars.svg", "SGP4 against the official AIAA 2006-6753 reference in every regime (worst 4.12 mm)."))
H.append("<h3>Validation, reproducibility and honesty</h3>" + md_blocks(section(README, "Validation, reproducibility & honesty", "###")))
H.append(endsec())

H.append(chapter("ch%d" % (n4 + 1), "%02d" % (n4 + 1), "Validation highlights",
                 "A curated highlight table from the README; every row is enforced by a named test in CI. The full matrix is Appendix A."))
glance = section(README, "Validation at a glance", "###")
gt = re.search(r"\| Status \| Capability \| Agreement \| Reference / oracle \|\n(?:\|.*\n)+", glance).group(0)
H.append(md_table(gt.strip().split("\n"), cls="tbl small"))
H.append("<h3>The quantum and classical packs</h3>")
res = section(README, "Results", "###")
H.append(md_table(re.search(r"\| Pack \|.*?\n(?:\|.*\n)+", res).group(0).strip().split("\n")))
H.append("<p>The advantage is outage- and vibration-dependent, with an explicit break-even where classical wins; optical-clock figures "
         "are ground-demonstrator targets (no strontium optical clock has flown). All four packs are MODELLED studies.</p>")
H.append(fig("docs/assets/figures/scenario-fom.svg", "Figures of merit of the bundled scenarios."))
H.append(endsec())

# misses
H.append(chapter("ch%d" % (n4 + 2), "%02d" % (n4 + 2), "The recorded misses",
                 "Every bar a pre-registered comparison missed, as the repository states it. Each stays pinned by a test; none was "
                 "loosened, and no row was promoted because of a later exploratory result."))
MISSES = [
    ("0.40", "Cislunar mean-state propagation against the public Artemis I trajectory, 72 h arc (<code>mission-thread</code>, <code>tli-and-cruise</code>)", "45.3 km and 0.25 m/s against 5 km and 0.05 m/s", "NOT MET", "The miss sits in five small impulsive events in the reference; between them the force model reproduces the hourly states to a median of 0.4 m. Row stays MODELLED."),
    ("0.40", "Lunar propagator against the CAPSTONE NRHO over one 6.5-day revolution (A2)", "end position 136.8 km against 25 km; periapsis radius 28.4 km against 10 km", "NOT MET", "Pinned by <code>tests/mission_thread_lunar_oracles.rs</code>. A1, against the LRO reconstructed trajectory over 24 h, is MET (33 m against 500 m; RMS radial 2.4 m against 60 m)."),
    ("0.40", "Fitted measured clock: recovering the registered random-walk FM, F1(a)", "missed on the C3 records and on two of three C4 records", "NOT MET", "Unidentifiable by construction on C3; absorbed by flicker FM and drift on C4. F1(b), F3, F4 met."),
    ("0.40", "Fitted measured clock: noise identification against allantools, F2", "met on eleven of twelve records and D3; missed on D2 as registered", "NOT MET", "Met like-for-like. The <code>fitted</code> block stays MODELLED."),
    ("0.40", "Measured clock-log ingest against allantools, first run (B1 to B4)", "B1 5 of 11 cases, B2, B3 7 of 24 cases, B4", "NOT MET", "After the fixes B1, B3, B4 met; B2 met except one MTIE pair, pinned and not an engine defect (a convention difference in allantools)."),
    ("0.40", "Emitter region against the Jammertest 2025 transmitter sites, run once", "too few scheduled sessions in the open data", "NOT ASSESSABLE", "Both ADS-B and AIS. Row <code>VM-bc70cb57</code> stays MODELLED."),
    ("0.40", "<code>gnss-ssv</code> second campaign: S7b, a roll-off-and-floor transmit shape fitted on Classic IIR panels", "3.83 to 3.95 dB against 3 dB on the IIR-M panels", "NOT MET", "Five single-reading ablations bring 2 of the 78 missed cells within tolerance (EXPLORATORY, no bar). Every original miss stays."),
    ("0.38–0.40", "kshana-bench A2: <code>agc_k_sigma</code> AUC on the test split", "0.766 against at least 0.80", "NOT MET", "0.40's A3 (EXPLORATORY bootstrap, 2.5 % quantile 0.8502) does not close it."),
    ("0.38–0.40", "kshana-bench D1: optimism gap", "largest gap 0.0301", "NOT MET", "D3 (EXPLORATORY) met in 0.40; D1 stays."),
    ("0.38–0.40", "kshana-bench task (b) R1", "carried from the earlier campaign, not re-run", "NOT MET", "R3 (AUC 1.0) and R4 (P_fa 0.0, P_d 1.0) met in 0.40."),
    ("0.38", "<code>gnss-ssv</code> against the interoperable GNSS Space Service Volume booklet: S1 to S5 and S7", "upper and lower service volume geometry (43 of 52 and 48 of 52 cells), the 15/20/25 dB-Hz upper volume, six GEO missions, the HEO mission, the sidelobe pattern", "NOT MET", "Only S0 (13 of 13 values) and S6 (the lunar mission) met. The formation run's absolute filter is overconfident."),
    ("0.38", "<code>lunar-infrastructure</code> C3: detection of an injected 3e-10 m/s² model error", "not detected (onboard solution known to about 350 m)", "NOT MET", "0.40's C3a and C3b, new bars with a detection limit of about 3e-9 m/s², are met as their own bars."),
    ("0.38", "<code>lunar-infrastructure</code>: integer ambiguity fixing between surface stations and an orbiter", "1 to 3 % success at 25 dBi; at the registered 12 dBi the problem cannot be formed", "NOT MET", "Met in the same study: orbit consistency (mean q 5.81 against 6) and a 111-fold datum eigenvalue gain."),
    ("0.38", "<code>lunar-infrastructure</code> G3, trace half: adding an observable family", "the trace rose in 12 of 28 additions", "NOT MET", "Each time because the family raised the rank."),
    ("0.38", "Jammertest scoring on the JammerTest 2024 data: minimum-data rule", "4 and 7 scorable windows against 10 required", "NOT ASSESSABLE", "No bar is assessable and nothing is claimed."),
    ("0.38", "LunaNet AFS: one printed tertiary-code column", "the document prints the same value for two PRNs", "NOT MET", "The code and synchronisation checks are VALIDATED against the printed check columns; LDPC and CRC words of subframes 2 to 4 are not checked."),
    ("earlier", "Force-model fit against the LRO lunar reconstructed orbit", "6.67 m reduced-dynamic, above the 5 m target", "NOT MET", "An honest miss in the README's highlight table; MODELLED."),
    ("earlier", "Lunar VLBI delays against ANISE 0.10 converged light times through DE440 (75 delays, three DSN baselines)", "no delay within the 1 ps tolerance; largest gap 24 µs", "NOT MET", "Cause: the analytic Moon centre. The kernel path agrees with NAIF SPICE to 0.06 ps. Row stays MODELLED."),
]
mt = ['<table class="tbl small misses"><thead><tr><th>Since</th><th>Comparison</th><th>Result against the bar</th><th>Verdict</th><th>What else the repository says</th></tr></thead><tbody>']
for a, b, c, d, e in MISSES:
    mt.append("<tr><td>%s</td><td>%s</td><td>%s</td><td>%s</td><td>%s</td></tr>" % (a, b, esc(c), label_chips(d), e))
mt.append("</tbody></table>")
H.append("".join(mt))
H.append("<p class='note'>Sources: README.md (What's new in 0.40, 0.39, 0.38; 0.38 in depth; Validation at a glance), CHANGELOG.md [0.40.0] "
         "and [0.39.0], <code>docs/BENCH.md</code>, <code>docs/SSV.md</code>, <code>docs/LUNAR-INFRASTRUCTURE.md</code>, "
         "<code>docs/kinds/by-kind/lunar-vlbi.md</code>. The result files are under <code>xval/</code> and <code>docs/emitter-region/</code>.</p>")
nm = [r for r in ROWS if re.search(r"NOT MET|NOT ASSESSABLE|honest miss", r["oracle"] + r["capability"] + r.get("tests", ""))]
H.append("<h3>Ledger rows whose oracle or tests record a miss (%d)</h3>" % len(nm))
H.append('<table class="tbl small"><thead><tr><th>ID</th><th>Requirement</th><th>Status</th></tr></thead><tbody>%s</tbody></table>'
         % "".join("<tr><td><code>%s</code></td><td>%s</td><td>%s</td></tr>" % (r["id"], inline(r["requirement"]), label_chips(r["status"])) for r in nm))
H.append("<p class='note'>Rows found by searching each row's oracle, capability and tests text for NOT MET, NOT ASSESSABLE or \"honest miss\". "
         "A VALIDATED row can record a miss: VALIDATED names the method of checking, not a pass.</p>")
H.append(endsec())

H.append(chapter("ch%d" % (n4 + 3), "%02d" % (n4 + 3), "Capability domains",
                 "Every capability of the public map, by domain, with its ledger label: the same file the Studio's map of everything reads."))
H.append(md_table(["| Domain | Capabilities | Labels |", "|---|---|---|"] + DOM_ROWS, cls="tbl small"))
H.append(fig("docs/assets/figures/domain-coverage-map.svg", "Domain coverage map."))
H.append("<h3>The domains in depth</h3><p>The README's domain-by-domain detail, shortened to the first sentences of each entry; the full "
         "text is in the README's Capabilities section and <code>docs/CAPABILITY.md</code>.</p>")
dd = re.search(r"\| Domain \| Capability \|\n\|-+\|-+\|\n((?:\|.*\n)+)", README).group(1)
rows = []
for l in dd.strip().split("\n"):
    c = [x.strip() for x in l.strip().strip("|").split("|", 1)]
    rows.append("<tr><td>%s</td><td>%s</td></tr>" % (inline(c[0]), label_chips(inline(trunc(c[1], 420)))))
H.append('<table class="tbl small"><tbody>%s</tbody></table>' % "".join(rows))
H.append(endsec())

H.append(chapter("ch%d" % (n4 + 4), "%02d" % (n4 + 4), "Research built on the engine",
                 "Studies built on the open engine are published on arXiv. Each names the engine command behind it, so its numbers can be "
                 "regenerated from a committed scenario or study example."))
rs = section(README, "Research")
pr = re.findall(r"^\| \[\*(.+?)\*\]\(https://arxiv.org/abs/([0-9.]+)\).*? \| (.+?) \| ", rs, re.M)
H.append('<table class="tbl"><thead><tr><th>Paper</th><th>What it shows</th></tr></thead><tbody>%s</tbody></table>'
         % "".join("<tr><td><b>%s</b><br><small>arXiv:%s</small></td><td>%s</td></tr>" % (esc(t), a, inline(w)) for t, a, w in pr))
H.append("<p>The three study examples write byte-deterministic artifacts (fixed seeds) that record their engine version, seeds and a "
         "configuration hash. The three lunar and cislunar papers are built on the lunar suite, whose evidence tier per piece is in the "
         "verification matrix; their geometry and dilution-of-precision claims are also cross-checked against separate oracles in "
         "<code>tests/validate_p*.rs</code>.</p>")
H.append(endsec())

# ===================================================================== PART V
n5 = n4 + 5
H.append(part_page("V", "Editions and reference", "How to use, buy, cite and reach Kshana; the full ledger; every kind; and the sources of every number.",
                   [(str(n5), "Editions: open core, Kshana Pro, custom study"), (str(n5 + 1), "Install, cite, licence and contact"),
                    ("A", "The verification ledger"), ("B", "The %d scenario kinds" % N_KINDS), ("C", "Sources and numbers")]))
H.append(chapter("ch%d" % n5, "%02d" % n5, "Editions: open core, Kshana Pro, custom study",
                 "The whole engine is free. Kshana Pro and custom studies are for programmes that need more, and every paid route starts "
                 "with a message: there are no prices here."))
H.append("""<table class="tbl editions"><thead><tr><th></th><th>Open core</th><th>Kshana Pro</th><th>Custom study</th></tr></thead><tbody>
<tr><td>What it is</td><td>the whole open engine, all %(nk)d scenario kinds, no feature gate</td><td>a proprietary overlay on the same engine, under contract: a strict superset of open Kshana</td><td>a MODELLED PNT-resilience study of your system, done for you</td></tr>
<tr><td>Who it is for</td><td>engineers of vehicles, networks, grids and spacecraft that take position or time from GNSS; researchers, students and reviewers; developers wiring the engine into code, CI or an AI assistant</td><td>programmes that must hand a mission dossier or an audit-grade evidence pack to a review board; architects choosing a design from a Pareto front or a trade study; timing and hardware teams who need a clock digital twin</td><td>programme and procurement teams who need an independent, modelled view of a PNT architecture's resilience; hardware builders who want models of their own devices; export-sensitive programmes</td></tr>
<tr><td>Licence</td><td>AGPL-3.0-only, or a commercial licence for closed integration</td><td>proprietary</td><td>contract</td></tr>
<tr><td>How to start</td><td>install, or open kshana.dev</td><td>request a Pro evaluation: contact@ashforde.org</td><td>request a study: contact@ashforde.org</td></tr>
</tbody></table>""" % dict(nk=N_KINDS))
pro = rd("docs/PRO.md")
H.append("<h3>What Kshana Pro is, in the words of docs/PRO.md</h3>" + md_blocks(section(pro, "What Pro is: the same engine, amplified")))
H.append("<h3>What Pro answers</h3>" + md_blocks(section(pro, "What Pro answers").split("The numbers below")[0]))
H.append("""<div class="cols2"><div><h3>The Pro capabilities, briefly</h3><ul>
<li><b>Design optimiser.</b> The Pareto front of a design space over runs of any open kind; exhaustive when the space fits the run
budget, a seeded NSGA-II search otherwise ("best found in N runs", never "optimal"); every front design written as a plain open
scenario file the free engine re-runs.</li>
<li><b>Uncertainty and sensitivity.</b> Seeded sampling of stated input distributions over thousands of open-engine runs: percentiles,
the probability of meeting a limit with its interval, and Sobol' or regression rankings, flagged when not reliable. More runs narrow
the sampling error, not the model error: a MODELLED kind stays MODELLED.</li>
<li><b>Mission dossier and requirements traceability.</b> Requirements (CSV, or documented subsets of ReqIF and SysML v2 text)
checked with their units against runs; a verification cross-reference matrix, a SysML v2 model, a change-impact report and an offline
evidence pack; requirements to be verified by test, inspection or demonstration stay open items.</li>
<li><b>Campaign watch.</b> Named figures in a pack of scenarios; a pipeline that fails when one gets worse; Monte Carlo figures judged
with Welch and F tests at a level fixed before the first run; a hash-chained history.</li></ul></div><div><ul>
<li><b>Spectrum coexistence.</b> Candidate signal plans tested against the open-service GNSS signals the engine models, in both
directions, over a worldwide grid; every physical number from open-engine runs of <code>leo-signal</code> and
<code>constellation-design</code>.</li>
<li><b>On-premises job service.</b> A queue that survives a restart, a stored result per job and a hash-chained delivery ledger.</li>
<li><b>Trade study.</b> Every architecture option run as an open-engine scenario, ranked on a figure of merit, with an evidence pack of
reproducible scenario hashes.</li>
<li><b>Clock digital twin.</b> Calibrated to a device's published Allan-deviation budget, checked with the open estimator. A twin of
your own device is built under contract with your data.</li>
<li><b>Detector audit.</b> An optimism-gap audit of an interference or spoofing detector, offered as a MODELLED custom study (web/editions.html). Not a certification.</li>
<li>Also: a scenario regression check, a study dossier and a small on-premises HTTP service.</li></ul></div></div>""")
H.append("<h3>What needs a contract, and what is not offered</h3>" + md_blocks(section(pro, "What needs a contract, and what is not offered")))
H.append("""<h3>A custom study</h3><p>When the question is yours but the time, the modelling or the clearance is not: the system is
modelled with the same engine, every figure labelled VALIDATED or MODELLED, and the result handed back so it can be reproduced. A
custom study is MODELLED and is not a certification. It can include sensor and resilience models calibrated to your hardware; export-controlled
resilience work is done only under the appropriate clearance and a non-disclosure agreement.</p>
<h3>Support and professional services</h3><p>Kshana is free and open source under the AGPL-3.0 and professionally developed and
maintained by Ashforde® OÜ (Estonia): commercial support and integration, custom sensor models, Kshana Pro, and training and
consulting on quantum and classical PNT performance analysis. This is the open-core model: the engine is, and stays, openly licensed;
the sustaining business is expertise, support and the proprietary extensions, not licence fees.</p>""")
H.append(endsec())

H.append(chapter("ch%d" % (n5 + 1), "%02d" % (n5 + 1), "Install, cite, licence and contact",
                 "One line gets the command-line tool: <code>cargo install kshana</code>. Every channel ships v0.40.0 from the same tagged commit."))
H.append("""<table class="tbl"><thead><tr><th>Channel</th><th>Install</th></tr></thead><tbody>
<tr><td>Try in the browser</td><td>open https://kshana.dev (Kshana Studio; nothing to install, nothing uploaded)</td></tr>
<tr><td>Command line (Rust)</td><td><code>cargo install kshana</code> (pin: <code>--version 0.40.0</code>)</td></tr>
<tr><td>Rust library</td><td><code>cargo add kshana</code> (API reference on docs.rs)</td></tr>
<tr><td>Python</td><td><code>pip install kshana</code> (pin: <code>kshana==0.40.0</code>)</td></tr>
<tr><td>JavaScript and WebAssembly</td><td><code>npm install kshana</code> (pin: <code>kshana@0.40.0</code>)</td></tr>
<tr><td>MCP server</td><td><code>cargo install kshana-mcp</code> · <code>npx -y kshana-mcp@0.40.0</code> · <code>uvx kshana-mcp==0.40.0</code> · <code>docker run -i --rm ghcr.io/ashfordeou/kshana-mcp:0.40.0</code> · Homebrew, Scoop, winget, prebuilt binaries checked against <code>SHA256SUMS</code>, the MCP registry</td></tr>
<tr><td>JetBrains IDE plugin</td><td>Settings → Plugins → Marketplace → "Kshana" (plugin ID <code>dev.kshana.ide</code>)</td></tr>
<tr><td>Signal K and OpenCPN</td><td><code>npm install signalk-kshana-trust</code>; the OpenCPN plugin tarball from the release</td></tr>
<tr><td>Grafana and reference build</td><td>the dashboard JSON from the release; <code>docker pull ghcr.io/ashfordeou/kshana-reference-build</code></td></tr>
<tr><td>From source</td><td>Rust 1.88 or newer (the minimum since 0.40.0)</td></tr>
</tbody></table>
<div class="cols2"><div><h3>Cite</h3><p>Cite the version you used, with the scenario and seed. <code>CITATION.cff</code> holds the
machine-readable metadata. Every release is archived on Zenodo; the concept DOI always resolves to the latest version.</p>
<blockquote><i>Kshana — a PNT-resilience simulator with quantum-sensor performance models</i>, version 0.40.0 (2026-10-29).
Ashforde OÜ. https://doi.org/10.5281/zenodo.20528627</blockquote>
<p>The papers of chapter %d are cited by their arXiv identifiers and DOIs (10.48550/arXiv.&lt;id&gt;), as listed in the README.</p></div><div>
<h3>Licence</h3><p>Kshana is dual-licensed: the GNU AGPL-3.0-only, or a commercial licence from Ashforde® OÜ for closed integration that
the AGPL does not suit (<code>LICENSING.md</code>). Contributions are licensed inbound under the AGPL and also grant Ashforde® OÜ the
right to include them in the commercial edition. "Kshana" and its marks are trademarks of Ashforde® OÜ: the licence covers the code,
not the name, so forks are renamed.</p>
<h3>Contact</h3><p><b>contact@ashforde.org</b> · ashforde.org · kshana.dev · github.com/ashfordeOU/kshana</p>
<p>Security reports: see <code>SECURITY.md</code> in the repository.</p></div></div>
<h3>Advisory statement</h3><div class="adv-box"><p>%s</p><p>For vessel outputs: advisory software, not type-approved navigation equipment
(IEC 61108, IEC 61162); the operator remains responsible for the navigation of the vessel. Nothing Kshana or Kshana Pro produces is a
certification.</p></div>""" % (n4 + 4, ADVISORY))
H.append(endsec())

# appendix A: the ledger
SEC.append(("appA", "A", "Appendix A: the verification ledger", None))
H.append('<section class="chapter appendix" id="appA"><div class="chap-head"><div class="chap-num">APPENDIX A</div><h1>The verification ledger</h1>'
         '<p class="lede">All %d rows at v%s, in the ledger\'s order: the stable id, the requirement, the oracle kind and the status. The full '
         'capability, module, tests and oracle text of each row is in <code>docs/VERIFICATION-MATRIX.md</code>.</p></div>' % (SUM["total"], VERSION))
lt = ['<table class="tbl ledger"><thead><tr><th>ID</th><th>Requirement</th><th>Oracle kind</th><th>Status</th></tr></thead><tbody>']
for r in ROWS:
    lt.append("<tr><td><code>%s</code></td><td>%s</td><td>%s</td><td>%s</td></tr>" % (r["id"], inline(trunc(r["requirement"], 150)), esc(r["oracle_kind"]), label_chips(r["status"])))
lt.append("</tbody></table>")
H.append("".join(lt) + endsec())

# appendix B: kinds
SEC.append(("appB", "B", "Appendix B: the %d scenario kinds" % N_KINDS, None))
H.append('<section class="chapter appendix" id="appB"><div class="chap-head"><div class="chap-num">APPENDIX B</div><h1>The %d scenario kinds</h1>'
         '<p class="lede">Every kind, its family, its ledger label and the example command, as <code>docs/KINDS.md</code> lists them.</p></div>' % N_KINDS)
kt = ['<table class="tbl small"><thead><tr><th>Kind</th><th>Family</th><th>Ledger</th><th>Example</th></tr></thead><tbody>']
for k in sorted(KIND_LEDGER):
    d = KIND_LEDGER[k]
    kt.append("<tr><td><code>%s</code></td><td>%s</td><td>%s</td><td><code>%s</code></td></tr>" % (k, esc(d["family"]), label_chips(esc(d["ledger"])), esc(d["example"])))
kt.append("</tbody></table>")
H.append("".join(kt) + endsec())

# appendix C: sources
SEC.append(("appC", "C", "Appendix C: sources and numbers", None))
H.append("""<section class="chapter appendix" id="appC"><div class="chap-head"><div class="chap-num">APPENDIX C</div><h1>Sources and numbers</h1>
<p class="lede">Every number in this dossier comes from the public repository github.com/ashfordeOU/kshana at tag v%(v)s (commit
1395dc68). Counts are computed by <code>docs/capability/src-v0.40.0/build.py</code> when the document is built.</p></div>
<table class="tbl small"><thead><tr><th>Number or content</th><th>Value</th><th>File</th></tr></thead><tbody>
<tr><td>Ledger rows, by status</td><td>%(t)d = %(va)d VALIDATED + %(mo)d MODELLED + %(pa)d PARTNER</td><td><code>web/data/verification-matrix.json</code> (<code>summary</code>, <code>rows</code>); ids from <code>docs/VERIFICATION-MATRIX.md</code></td></tr>
<tr><td>Scenario kinds and families</td><td>%(nk)d kinds, %(nf)d families</td><td><code>docs/KINDS.md</code>, <code>docs/kinds/*.md</code></td></tr>
<tr><td>Kind entries (description, inputs, run, results, ledger rows, limits)</td><td>%(nk)d pages</td><td><code>docs/kinds/by-kind/*.md</code></td></tr>
<tr><td>Kind charts and contact sheets</td><td>%(nk)d + %(nf)d SVG</td><td><code>docs/figures/kinds/</code></td></tr>
<tr><td>Commands</td><td>%(nc)d rows</td><td><code>docs/COMMANDS.md</code></td></tr>
<tr><td>Surfaces</td><td>%(ns)d rows</td><td><code>docs/ALL-SURFACES.md</code></td></tr>
<tr><td>MCP tools</td><td>%(nt)d</td><td><code>README.md</code> (AI assistant)</td></tr>
<tr><td>Languages beside English</td><td>%(nl)d</td><td><code>docs/i18n.md</code></td></tr>
<tr><td>Bundled scenario files at the top of <code>scenarios/</code></td><td>%(nsc)d</td><td><code>scenarios/*.toml</code></td></tr>
<tr><td>Capability domains</td><td>%(nd)d</td><td><code>docs/CAPABILITY-DOMAINS.md</code></td></tr>
<tr><td>What is new, misses, highlights, studio, editions text</td><td>quoted or summarised</td><td><code>README.md</code>, <code>CHANGELOG.md</code> ([0.40.0], [0.39.0], [0.38.0])</td></tr>
<tr><td>Security fix and Rust minimum</td><td>RUSTSEC-2026-0387; Rust 1.88</td><td><code>CHANGELOG.md</code> (filed under [0.39.0]; shipped in 0.40.0)</td></tr>
<tr><td>Kshana Pro</td><td>description only</td><td><code>docs/PRO.md</code>, <code>web/editions.html</code></td></tr>
<tr><td>Misses</td><td>table in chapter %(cm)d</td><td><code>README.md</code>, <code>CHANGELOG.md</code>, <code>docs/BENCH.md</code>, <code>docs/SSV.md</code>, <code>docs/LUNAR-INFRASTRUCTURE.md</code></td></tr>
</tbody></table>
<h3>Figures</h3><table class="tbl small"><thead><tr><th>Figure</th><th>File</th></tr></thead><tbody>
<tr><td>Logo</td><td><code>docs/assets/readme/kshana-logo-dark.svg</code></td></tr>
<tr><td>Ledger breakdown, oracle kinds, SGP4 regimes, scenario figures of merit, domain coverage</td><td><code>docs/assets/figures/*.svg</code></td></tr>
<tr><td>Architecture, pipeline</td><td><code>docs/assets/readme/*-light.svg</code></td></tr>
<tr><td>Diagrams (system overview, engine flow, module map, surfaces, trust flow, lab round trip)</td><td><code>docs/assets/diagrams/*.svg</code></td></tr>
<tr><td>0.38 figures</td><td><code>docs/figures/0.38/*-light.svg</code> (<code>MANIFEST.json</code>)</td></tr>
<tr><td>Kind charts and contact sheets</td><td><code>docs/figures/kinds/*.svg</code></td></tr>
<tr><td>Emitter region example</td><td><code>examples/emitter-region/output/emitter-adsb-custom-2026-03-01-figure.svg</code></td></tr>
<tr><td>Studio screenshots</td><td><code>docs/assets/readme/studio/studio-task-light.jpg</code>, <code>studio-advanced-light.jpg</code></td></tr>
<tr><td>Ledger by oracle kind (drawn here)</td><td>counted from <code>web/data/verification-matrix.json</code></td></tr>
</tbody></table>
<h3>Wording</h3><p>Programme names in the repository's prose are replaced by neutral wording ("Artemis-class", "lunar-relay-class",
"emitter region"); code identifiers and file names are kept. Long descriptions are shortened with "…"; no number is changed.</p>
""" % dict(v=VERSION, t=SUM["total"], va=SUM["validated"], mo=SUM["modelled"], pa=SUM["partner_owned"], nk=N_KINDS, nf=len(FAMILIES),
           nc=len(CMD_ROWS), ns=len(SURF_ROWS), nt=N_TOOLS, nl=len(LANG_LIST), nsc=N_SCEN, nd=len(DOM_ROWS), cm=n4 + 2))
H.append(endsec())

# closing
H.append("""<section class="closing"><div class="close-in"><img class="logo" src="%s" alt="Kshana"><p class="tag">Every PNT figure, with its evidence.</p>
<div class="close-cols"><div><small>STUDIO &amp; DOCS</small>kshana.dev</div><div><small>SOURCE</small>github.com/ashfordeOU/kshana</div><div><small>CONTACT</small>contact@ashforde.org</div></div>
<div class="brand big">ASHFORDE<sup>®</sup></div>
<p class="close-adv">%s Kshana transmits nothing. Nothing in this document is a certification, a type approval or a claim of endorsement.
Kshana v%s · AGPL-3.0-only or commercial licence · concept DOI 10.5281/zenodo.20528627.</p></div></section>""" % (svgfile("docs/assets/readme/kshana-logo-dark.svg"), ADVISORY, VERSION))

# contents --------------------------------------------------------------------
parts = {"ch1": "PART I · KSHANA AT A GLANCE", "ch6": "PART II · EVERY KIND, FAMILY BY FAMILY", "ch%d" % nfam: "PART III · USING KSHANA",
         "ch%d" % n4: "PART IV · EVIDENCE", "ch%d" % n5: "PART V · EDITIONS AND REFERENCE"}
toc = ['<section class="toc"><h1>Contents</h1><p class="lede">Part II has one chapter per family and one entry per kind.</p><ol>']
for cid, num, title, _ in SEC:
    if cid in parts:
        toc.append('<li class="tp">%s</li>' % parts[cid])
    toc.append('<li><span class="tn">%s</span><span class="tt">%s</span><span class="pg">%s</span></li>' % (num.lstrip("0") if num.isdigit() else num, title, TOC.get(cid, "")))
toc.append("</ol></section>")
H[TOC_SLOT] = "".join(toc)

# fonts and CSS ---------------------------------------------------------------
def font(fam, path, weight="100 900"):
    return "@font-face{font-family:'%s';font-weight:%s;font-style:normal;src:url(%s) format('woff2');}" % (fam, weight, b64(path, "font/woff2"))


FONTS = (font("Geist", "web/fonts/files/geist-gyByhwUxId8gMEwcGFWNOITd.woff2")
         + font("Geist Mono", "web/fonts/files/geistmono-or3nQ6H-1_WfwkMZI_qYFrcdmhHkjko.woff2")
         + font("Unbounded", "web/fonts/files/unbounded-Yq6W-LOTXCb04q32xlpwu8ZfvRIkSQ.woff2")
         + "@font-face{font-family:'Noto Sans Devanagari';src:url(%s) format('woff2');}" % b64("web/fonts/files/notosansdevanagari-text-87bf340d821848e2.woff2", "font/woff2"))
CSS = open(os.path.join(HERE, "dossier.css"), encoding="utf-8").read().replace("/*ADVISORY*/", ADVISORY_SHORT.replace('"', "'"))

doc = """<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Kshana capability dossier v%s</title>
<meta name="author" content="Ashforde OÜ">
<meta name="description" content="Public capability dossier of the open Kshana PNT-resilience engine, v%s.">
<style>%s
%s</style></head><body>
%s
</body></html>""" % (VERSION, VERSION, FONTS, CSS, "\n".join(H))
open(OUT, "w", encoding="utf-8").write(doc)
json.dump([[c, n, t] for c, n, t, _ in SEC], open(os.path.join(HERE, ".sections.json"), "w"))
print("wrote", OUT, "%.1f MB" % (len(doc) / 1e6), "sections", len(SEC))
