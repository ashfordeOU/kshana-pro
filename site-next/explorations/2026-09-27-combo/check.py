#!/usr/bin/env python3
"""Self-check for the multi-page kshana.dev redesign. Run after build.py. Exit 1 on any failure.

  1  tag balance          every page (site, docs, the Studio shell) closes every element
  2  hosts                scripts only from cdnjs / jsdelivr; stylesheets only Google Fonts;
                          no image, font or url() from any other host
  3  forbidden names      no Aperio, no AI vendor names, in any published text file
  4  no prices            no currency amounts or pricing terms on any page
  5  no stand-ins         no "Illustrative" label or .illus class on the site pages; no <canvas>
                          except the decorative planets (imagery, never data)
  6  provenance           every chart (figure.viz) carries "Engine vX · <file>.toml · seed" and an
                          "Open in <Studio>" link (/studio/?scenario=...) to a recorded scenario and a real view (tab)
  7  hand-typed numbers   every number typed in src/pages is found in the Kshana sources
  8  links                every relative href/src on every page resolves to a published file,
                          and every #anchor to an element id on the target page
  9  docs provenance      every rendered doc is git-tracked in the docs checkout (src/data/sources.json; never HANDOVER.md)
 10  Pro claims           every sentence about Pro/custom on the site pages uses an allow-listed
                          public claim, or is a reviewed framing sentence
 11  install drift        every rendered install command equals channels.json; the version shown
                          is Cargo.toml's; every README carries the same base commands
 12  publish list         PUBLISH.txt matches the files on disk and stays under Pages' limits (1 GB, 100 MB a file; --preview adds the preview host's)
 13  coverage             every `cover:` link in COVERAGE.md resolves to a published page and anchor
 14  studio name          the app is named only by the one constant in src/data/brand.json: no
                          visible "playground" on any site or docs page (URLs and code excepted),
                          and the nav carries the name; the Studio's <title> is the name and its
                          visible text and aria-labels never say "playground"
 15  glossary in Docs     no glossary on a marketing page; the Docs glossary page exists
 16  studio links         every visual that shows a run links the Studio view that reproduces it:
                          a provenance line naming a scenario the Studio carries has a link to that
                          scenario in the Studio; every scenario the page scripts draw (their run
                          data) is in js/studio-links.mjs and those scripts use it; and every Studio
                          link on every page names a scenario the published Studio carries and a
                          view (tab) that run has, by the Studio's own link list
 17 engine version       every engine version any published file shows (site, docs, the Studio,
                          data, scripts and the Studio's recorded packs) is Cargo.toml's: "v0.x.y",
                          "engine 0.x.y", "Kshana 0.x.y" and "engine"/"engine_version"/"version"
                          JSON fields. History is excepted: the changelog page and its search
                          entries, and the stated measurements in VERSION_HISTORY below. On the
                          docs pages and the search indexes (release history is part of the
                          repository's docs: "since v0.3.0") only the engine/Kshana and JSON
                          forms count; a bare "v0.x.y" counts on every other file
 18 changelog Pro versions the changelog page (and its search entries) never names a Pro version:
                          no "Pro 0.x", "Pro v0", "Pro 1.x", "Kshana Pro 2", "kshana-pro" or
                          "kshana_pro". The changelog is the open engine's; Pro is versioned elsewhere
 19 no roadmap           the roadmap is not on the site: no docs/roadmap.html, no link to it and no
                          search entry for it (ROADMAP.md stays in the repository)
 20 clean URLs           every link to a page of the site, on every page and in both search indexes,
                          is its clean address from the site root: "/" for Home (never /index.html),
                          /evidence#research, /docs/changelog, /studio/?scenario=... No href ends
                          in ".html" or "index.html". Engine run reports under assets/ are exempt

 21 old addresses        every internal link written with ".html" on any page (the Studio's too) reaches a
                          published file; no page links the Studio's old address /playground/; that address
                          (playground/index.html) forwards to /studio/ keeping the query and fragment, noindex
 22 sitemap              sitemap.xml lists every indexable page (site, docs, /studio/) once, by its clean
                          address with a lastmod, and nothing else
 23 meta                 every indexable page: <html lang>, a unique <title> of at most 60 characters, a unique
                          description of 50 to 155, canonical = its clean address, Open Graph and a large social card
 24 structured data      every JSON-LD block parses; Home: SoftwareApplication (version, AGPL-3.0-only, free),
                          SoftwareSourceCode (codeRepository), Organization (Ashforde OÜ), WebSite + SearchAction
                          (?q=, which site.js answers); Evidence: a ScholarlyArticle per paper; docs: BreadcrumbList;
                          the Studio: WebApplication
 25 crawl files          robots.txt allows everything and names the major crawlers, with the Sitemap line;
                          llms.txt (H1, summary, sections) links every page, doc, paper, the DOI, the repository
                          and the install commands; llms-full.txt carries every page; the IndexNow key file
 26 crawlable text       the script-drawn sections carry <noscript> text from the same data (the Missions threat
                          matrix as a table); the Studio page lists every scenario it runs

Navigation (a browser check, run against a served build, not part of the rules above):
  python3 check.py --nav [base-url]      runs ../../features/check_nav.mjs; base-url defaults to
                                         http://127.0.0.1:8847, a preview that serves clean URLs:
                                         python3 ../../features/serve.py . 8847

Mutation self-test: python3 check.py --mutate  (each named check must go red once, then green; --only "5 stand-ins" runs just the cases whose name contains the text).
"""
import glob, html, json, os, re, subprocess, sys
from html.parser import HTMLParser
from urllib.parse import urlsplit, unquote

HERE = os.path.dirname(os.path.abspath(__file__))
SOURCES = json.load(open(os.path.join(HERE, "src", "data", "sources.json"), encoding="utf-8"))
REPO = os.path.expanduser(os.environ.get("KSHANA_REPO") or SOURCES["repo"])
DOCS_REPO = os.path.expanduser(os.environ.get("KSHANA_DOCS_REPO") or SOURCES.get("docs_repo") or SOURCES["repo"])
# Classes the built pages carry that have no stylesheet rule of their own, each reviewed on 17 Oct 2026: a wrapper
# or modifier beside a class that is styled (block, window, hx-win, glass, pb-col, pw-seg, ...), a hook a page script
# selects, or an SVG state class. A class that is meant to look like something must have a rule, not an entry here.
CLASS_HOOKS = set("""asst-inst cc-main cd-win cg crawl-text cw-win cx-legend cx-rows d1 dt-win dx-channels ed-custom ed-free
eo-win ev-ct ev-lg ev-mx ev-nx ev-rp ev-sd explorer free ic km kmod l2 lg-list mh-copy mw no pb-custom pb-free pl-ic plan
pub-note pw-scrub pw-seg2 pw-src sc-leo sc-searoad sd-wrap sec-studio sp-bar ss unchanged wy-panels docs-page
class_ function_ invoke__ language_""".split())
# highlight.js token and language classes (the docs code blocks)
HOOK_PREFIXES = ("hljs", "language-")

SITE_PAGES = ["index.html", "missions.html", "capabilities.html", "evidence.html", "developers.html", "editions.html"]
PUB = [l.strip() for l in open(os.path.join(HERE, "PUBLISH.txt")) if l.strip()]
STUDIO = json.load(open(os.path.join(HERE, "src", "data", "brand.json"), encoding="utf-8"))["studio"]
PUBSET = set(PUB)
# The all-caps tier, verdict and status tokens of the i18n token list (TOKENS in kshana tools/i18n-lang.mjs, 26 words).
# Inside the evidence page's kpage JSON blob (the engine's ledger text, quoted verbatim) these are the only caps words
# rule 5 lets through.
KPAGE_TOKENS = ["NOT CORROBORATED", "NOT MET", "VALIDATED", "MODELLED", "PARTNER", "EXPLORATORY", "MET", "PREDICTION", "CORROBORATED",
                "REFUSED", "DEGRADED", "DEFICIENT", "OBSERVED", "LOST", "MISMATCH", "VERIFIED", "CONSISTENT", "INCONSISTENT",
                "ILLUSTRATIVE", "PUBLIC", "REPRESENTATIVE", "WORKSHOP", "MIXED", "MEASURED", "NEEDS INPUT", "GREEN"]
_KPAGE_TOKEN_RE = re.compile(r"(?<![A-Za-z_])(?:" + "|".join(re.escape(x) for x in sorted(KPAGE_TOKENS, key=len, reverse=True)) + r")(?![A-Za-z_])")  # case-sensitive


def kpage_text(blob):
    """The text of a kpage JSON blob that rule 5 scans: every string in it, with the exact caps tokens removed."""
    try:
        strings = []
        def walk(v):
            if isinstance(v, str): strings.append(v)
            elif isinstance(v, dict):
                for k, x in v.items(): strings.append(k); walk(x)
            elif isinstance(v, list):
                for x in v: walk(x)
        walk(json.loads(blob))
        text = "\n".join(strings)
    except ValueError:
        text = blob  # not JSON: scan it as it stands
    return _KPAGE_TOKEN_RE.sub(" ", text)

TEXT_EXT = (".html", ".js", ".mjs", ".css", ".json", ".txt", ".svg", ".md")
# The Studio's own list of what it can open (build.py writes it from the Studio's tools_links.mjs):
# scenario -> {"tabs": the views that run has, "best": the view that reproduces it}.
STUDIO_VIEWS = json.load(open(os.path.join(HERE, ".build", "studio-views.json"), encoding="utf-8"))


# Rule 17: engine versions that are dated history, not the version the page runs. Each is
# (published file, the exact text, why it may name another version). Keep this list short.
VERSION_HISTORY = [
    ("docs/line-coverage.html", "engine 0.29.0, the", "the release commit the published line coverage was measured on (docs/COVERAGE.md, CI run 36809850252)"),
    ("docs/line-coverage.html", "engine 0.27.2 + unreleased", "the commit the previous line-coverage figure was measured on (docs/COVERAGE.md history)"),
    ("editions.html", "on open engine 0.29.0", "the engine the Kshana Pro 0.2.0 showcases were run on (pro-power.json); engine output is unchanged in 0.29.1"),
    ("docs/overview.html", "drawn at engine 0.22.0", "the engine the README's figure-of-merit chart was drawn at; its figures are re-checked each build"),
    ("docs/search-docs.json", "drawn at engine 0.22.0", "the same sentence in the docs search index"),
    ("data/search.json", "drawn at engine 0.22.0", "the same sentence in the site search index"),
    ("docs/capability-map.html", "engine 0.35.0, 75 kinds", "the dated 2026-10-10 revision line of docs/CAPABILITY.md, kept as the history of the kind count"),
    ("docs/tutorials.html", "kshana 0.35.0 printed", "docs/tutorials/README.md says the table is one dated run (2026-10-10), not pinned by tests, and to rerun for the current value"),
    ("docs/mcp-install.html", "0.0.1", "the Continue client's own configuration-file version in its example snippet, not the engine's"),
]
# A third-party tool's own version, named in an oracle row of the verification ledger (the GNSS-SDR release the
# IQ tracking engine is compared with), is not an engine version. Only these exact phrasings are set aside, in
# rule 11 and rule 17; any other "v0.x.y" on a page must still be the engine's.
THIRD_PARTY_VERSION = re.compile(r"GNSS-SDR (?:v|version )?0\.0\.19\b|\btag v0\.0\.19\b|\bfresh v0\.0\.19\b")
def without_third_party(text): return THIRD_PARTY_VERSION.sub("[third-party tool version]", text)
VERSION_CLAIMS = [
    re.compile(r"(?<![\w@.-])[vV](0\.\d+\.\d+)(?![\w-]|\.\d)"),
    re.compile(r"\b(?:[Ee]ngine|Kshana|kshana)(?:\s+version)?(?:\s*[:·]\s*|\s+)v?(0\.\d+\.\d+)(?![\w-]|\.\d)"),
    re.compile(r"""["']?\b(?:engine_version|engine|version)["']?\s*:\s*["'](0\.\d+\.\d+)["']"""),
]


# Rule 18: a Pro version number ("Pro 0.2.0", "Pro v0", "Pro 1.0", "Kshana Pro 2", "kshana-pro").
PRO_VERSION = re.compile(r"\bPro\s+v?\d+(?:\.\d+)*|\bkshana[-_]pro\b", re.I)


def version_claims(rel, text):
    "Every (version, context) a published file shows, minus the history this rule excepts."
    if rel == "docs/changelog.html": return []
    if rel == "llms-full.txt":
        # the full text is the pages, one after another: each part is held to its own page's rule
        out = []
        for m in re.finditer(r"^URL: https://kshana\.dev/(\S*)\n(.*?)(?=^={20,}\nURL: |\Z)", text, re.M | re.S):
            page = m.group(1)
            page = (page + "index.html") if page == "" or page.endswith("/") else page + ".html"
            out += version_claims(page, m.group(2))
        return out
    if rel.endswith("search.json") or rel.endswith("search-docs.json"):
        try: text = json.dumps([e for e in json.loads(text) if not is_changelog_h(e.get("h", ""))], ensure_ascii=False)
        except ValueError: pass
    text = without_third_party(text)
    plain = html.unescape(re.sub(r"<[^>]+>", " ", text)) if rel.endswith(".html") else text
    out = []
    history_prose = rel.startswith("docs/") or rel.endswith("search.json")
    for body in {text, plain}:
        for rx in VERSION_CLAIMS[1:] if history_prose else VERSION_CLAIMS:
            for m in rx.finditer(body):
                ctx = body[max(0, m.start() - 30): m.end() + 30]
                if any(rel == f and hist in ctx for f, hist, _ in VERSION_HISTORY): continue
                out.append((m.group(1), " ".join(ctx.split())))
    return out


def is_changelog_h(h):
    "A search entry's address that points into the changelog page (clean or file form)."
    return bool(re.match(r"^/?docs/changelog(?:\.html)?(?:[#?]|$)", str(h)))


def resolve(base_rel, u):
    """A link as written in the published file base_rel -> (published file it reaches, fragment).
    Clean addresses resolve the way the host serves them: /evidence -> evidence.html, / and /docs/ ->
    their index.html, /studio/?q -> studio/index.html. None when it is not a site link."""
    sp = urlsplit(u)
    if sp.scheme or sp.netloc or not sp.path:
        return (None, sp.fragment) if not sp.path and not sp.scheme and not sp.netloc else (None, None)
    path = unquote(sp.path)
    full = path.lstrip("/") if path.startswith("/") else os.path.normpath(os.path.join(os.path.dirname(base_rel), path)).replace(os.sep, "/")
    if path.endswith("/") or full in ("", "."):
        full = (full.rstrip("/") + "/index.html").lstrip("/") if full not in ("", ".") else "index.html"
    elif full not in PUBSET and full + ".html" in PUBSET:
        full += ".html"
    return full, sp.fragment


def link_base(rel, text):
    """The file a page's relative links resolve against: the page itself, or, when it carries a
    <base href>, a file in the folder that base names (the Studio's Advanced view, studio/advanced/
    index.html, has <base href="../"> so it shares the Studio's files)."""
    m = re.search(r'<base\s+href="([^"]*)"', text)
    if not m or re.match(r"^(?:[a-z][a-z0-9+.-]*:|/)", m.group(1), re.I):
        return rel
    return os.path.normpath(os.path.join(os.path.dirname(rel), m.group(1), "_")).replace(os.sep, "/")


def rd(rel):
    with open(os.path.join(HERE, rel), encoding="utf-8", errors="replace") as f:
        return f.read()


def run_checks(override=None):
    """override: {relpath: text} to test mutations without touching disk."""
    get = lambda rel: (override or {}).get(rel) if override and rel in override else rd(rel)
    fails = []
    pages_html = [p for p in PUB if p.endswith(".html") and not p.startswith("studio/recorded")]
    texts = {p: get(p) for p in pages_html}
    site = {p: texts[p] for p in SITE_PAGES}

    # 1 tag balance ---------------------------------------------------------------------------
    VOID = {"area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track", "wbr", "path", "circle", "ellipse", "line", "rect", "stop", "use", "polyline", "polygon"}

    class P(HTMLParser):
        def __init__(s): super().__init__(convert_charrefs=True); s.stack = []; s.err = []
        def handle_starttag(s, t, a):
            if t not in VOID: s.stack.append(t)
        def handle_endtag(s, t):
            if t in VOID: return
            if not s.stack or s.stack[-1] != t:
                s.err.append(f"</{t}> (open: {s.stack[-3:]})")
                for i in range(len(s.stack) - 1, -1, -1):
                    if s.stack[i] == t: del s.stack[i:]; break
            else: s.stack.pop()
    bad = {}
    for p, t in texts.items():
        pr = P(); pr.feed(t); pr.close()
        if pr.err or pr.stack: bad[p] = pr.err[:3] + [f"unclosed {x}" for x in pr.stack[:3]]
    if bad: fails.append(("1 tag balance", bad))

    # 2 hosts ----------------------------------------------------------------------------------
    bad = []
    for p, t in texts.items():
        for m in re.finditer(r'<script[^>]*\bsrc="([^"]+)"', t):
            u = m.group(1)
            if re.match(r"https?:", u) and not re.match(r"https://(cdnjs\.cloudflare\.com|cdn\.jsdelivr\.net/npm/)", u): bad.append((p, u))
        for m in re.finditer(r'<link[^>]*\bhref="(https?://[^"]+)"', t):
            if re.search(r'\brel="canonical"', m.group(0)) and m.group(1).startswith("https://kshana.dev/"): continue  # an address, not a fetch
            if not re.match(r"https://fonts\.(googleapis|gstatic)\.com", m.group(1)): bad.append((p, m.group(1)))
        for m in re.finditer(r'"imports":\{[^}]*\}', t):
            for u in re.findall(r'"(https?://[^"]+)"', m.group(0)):
                if not u.startswith("https://cdn.jsdelivr.net/npm/"): bad.append((p, u))
        for m in re.finditer(r'<img[^>]*\ssrc="(https?://[^"]+)"|url\((https?://[^)]+)\)', t): bad.append((p, m.group(0)[:80]))
    for p in PUB:
        if p.endswith((".mjs", ".js", ".css")) and not p.startswith("studio/pkg"):
            t = get(p)
            for u in re.findall(r"""import\s*(?:[^'"]*from\s*)?['"](https?://[^'"]+)['"]""", t):
                if not u.startswith("https://cdn.jsdelivr.net/npm/"): bad.append((p, u))
            for u in re.findall(r"url\((https?://[^)]+)\)", t): bad.append((p, u))
    if bad: fails.append(("2 hosts", bad[:10]))

    # 3 forbidden names ------------------------------------------------------------------------
    bad = []
    for p in PUB:
        if not p.endswith(TEXT_EXT) or p.startswith("studio/recorded"): continue
        t = get(p).lower()
        if p == "robots.txt":  # crawler user-agent names are technical identifiers, not copy
            t = re.sub(r"(?m)^user-agent:.*$", "", t)
        for w in ("aperio", "claude", "anthropic"):
            if re.search(r"\b" + w + r"\b", t): bad.append((p, w))
    if bad: fails.append(("3 forbidden names", bad[:10]))

    # 4 no prices ------------------------------------------------------------------------------
    PRICE = re.compile(r"(€\s?\d|(?<![\w'\">$])\$\s?\d[\d,.]*\s*(k|m|/|per|a )|\bEUR\s?\d|\bUSD\s?\d|\bper seat\b|\bpricing\b|\bprice list\b|\bfrom\s+€|\bper user\b|\bper licence\b)", re.I)
    bad = []
    for p, t in texts.items():
        body = re.sub(r"<script.*?</script>|<style.*?</style>|<pre.*?</pre>|<code.*?</code>", " ", t, flags=re.S)
        for m in PRICE.finditer(html.unescape(re.sub(r"<[^>]+>", " ", body))):
            bad.append((p, m.group(0)))
    if bad: fails.append(("4 price wording", bad[:10]))

    # 5 no stand-ins ---------------------------------------------------------------------------
    bad = []
    for p, t in site.items():
        # capability text and ledger rows quoted verbatim from the repository (its own wording) are not scanned.
        # The evidence page's JSON copy of the matrix rows (id kpage) is scanned too, minus only the caps tokens of
        # the i18n token list (KPAGE_TOKENS): anything else it says (a lowercase or capitalised "illustrative", a
        # .illus class, an aria-label) still fails.
        blobs = re.findall(r'<script type="application/json" id="kpage">(.*?)</script>', t, flags=re.S)
        scan = re.sub(r'<div class="cx-body"><p>.*?</p>|<dl class="lg-body">.*?</dl>|<summary><span class="st st-\w">\w+</span><span class="lg-req">.*?</span>|<script type="application/json" id="kpage">.*?</script>', " ", t, flags=re.S)
        scan += " " + " ".join(kpage_text(b) for b in blobs)
        if re.search(r"illustrative", re.sub(r"<[^>]+>", " ", scan), re.I) or re.search(r'class="[^"]*\billus\b', scan) or re.search(r'aria-label="[^"]*illustrative', scan, re.I):
            bad.append((p, "Illustrative"))
        for m in re.finditer(r"<canvas\b[^>]*>", t):
            # a planet canvas is allowed only for a body with a NASA map under assets/planets/<body>-256.jpg
            bodies = sorted({os.path.basename(f).split("-")[0] for f in glob.glob(os.path.join(HERE, "assets", "planets", "*-256.jpg"))})
            if not re.search(r'id="globe"|data-planet="(' + "|".join(bodies) + ')"', m.group(0)): bad.append((p, m.group(0)[:80]))
    for p in ("studio/index.html", "studio/app.js"):
        # The scenario list quotes each bundled file's first header line verbatim, and several files state
        # honestly that their inputs are illustrative (an ILLUSTRATIVE launcher, camera, station pair or
        # satellite set). That is the repository's own wording about its inputs, not a stand-in for data,
        # so those list items are not scanned; everything else in the Studio still is.
        t = re.sub(r'<li><a href="/studio/[^"]*">[^<]*</a>: [^<]*</li>', " ", get(p), flags=re.S)
        if re.search(r"\billustrative\b", t, re.I): bad.append((p, "Illustrative"))
    if bad: fails.append(("5 stand-ins", bad))

    # 6 provenance -----------------------------------------------------------------------------
    # What the published Studio carries: its recorded-run index, and each optional group's.
    rec = set()
    for g in [""] + (json.loads(get("studio/groups.json"))["groups"] if "studio/groups.json" in PUBSET else []):
        if f"studio/{g}recorded/index.json" in PUBSET:
            rec |= {f[:-5] for f, e in json.loads(get(f"studio/{g}recorded/index.json"))["runs"].items() if not e.get("error")}
    bad = []
    PROV = re.compile(r'<span class="prov-src">(?:<b>[^<]*</b> )?Engine v(?P<v>[\d.]+) · (?P<files>(?:<code>[\w-]+\.toml</code>(?: \+ )?)+) · (?:seed \d+|deterministic, no seed)</span><a class="prov-open" href="(?P<h>[^"]+)">Open in ' + re.escape(STUDIO))
    ver = re.search(r'^version\s*=\s*"([^"]+)"', open(os.path.join(REPO, "Cargo.toml")).read(), re.M).group(1)
    nfig = 0
    for p, t in site.items():
        for m in re.finditer(r'<figure class="viz([^"]*)" data-viz="([\w-]+)"[^>]*>(.*?)</figure>', t, re.S):
            nfig += 1
            cls, cid, inner = m.group(1), m.group(2), m.group(3)
            if "viz-bare" in cls:
                # bare waterfall panels share one provenance line on the page
                if not re.search(r'class="prov"><span class="prov-src">Engine v[\d.]+ · <code>jamming-demo\.toml', t): bad.append((p, cid, "bare figure without a page provenance line"))
                continue
            pm = PROV.search(inner)
            if not pm: bad.append((p, cid, "no provenance line")); continue
            if pm.group("v") != ver: bad.append((p, cid, f"engine v{pm.group('v')} != {ver}"))
            q = dict(x.split("=", 1) for x in urlsplit(html.unescape(pm.group("h"))).query.split("&") if "=" in x)
            if not pm.group("h").startswith("/studio/?"): bad.append((p, cid, "link is not the Studio's clean address (/studio/?...)"))
            if q.get("scenario") not in rec: bad.append((p, cid, f"scenario {q.get('scenario')} not recorded"))
            if q.get("tab") not in STUDIO_VIEWS.get(q.get("scenario"), {}).get("tabs", []): bad.append((p, cid, f"tab {q.get('tab')} is not a view of that run in the Studio"))
            first = re.search(r"<code>([\w-]+)\.toml</code>", pm.group("files")).group(1)
            if first != q.get("scenario"): bad.append((p, cid, "link opens a different scenario than the label"))
    if nfig < 20: bad.append(("site", f"only {nfig} charts found; the checker is not seeing them"))
    if bad: fails.append(("6 provenance", bad[:12]))

    # 7 hand-typed numbers ---------------------------------------------------------------------
    corpus = ""
    for g in ["README*.md", "CITATION.cff", "Cargo.toml", "web/index.html", "web/capabilities.json", "scenarios/*.toml", "mcp/kshana-mcp/README.md", "docs/*.md", "src/orbit.rs", "src/jamming.rs", "src/telecom_timing.rs", "src/lunar_time.rs", "src/ephem.rs"]:
        for f in glob.glob(os.path.join(REPO, g)): corpus += open(f, encoding="utf-8", errors="ignore").read()
    ORD = {"0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10"}
    untraced = []
    for f in sorted(glob.glob(os.path.join(HERE, "src", "pages", "*.html"))):
        t = open(f, encoding="utf-8").read()
        t = re.sub(r"\{\{[^}]*\}\}|<[^>]+>|&[a-z#0-9]+;", " ", t)
        for n in set(re.findall(r"(?<![\w.#-])\d[\d,]*(?:\.\d+)?(?![\w])", t)):
            if n in ORD: continue
            if n not in corpus and n.replace(",", "") not in corpus: untraced.append((os.path.basename(f), n))
    for f in sorted(glob.glob(os.path.join(HERE, "src", "data", "results.json"))):
        def nums(v):
            if isinstance(v, dict): return [x for vv in v.values() for x in nums(vv)]
            if isinstance(v, list): return [x for vv in v for x in nums(vv)]
            if isinstance(v, (int, float)) and not isinstance(v, bool): return [f"{v:g}"]
            if isinstance(v, str): return re.findall(r"(?<![\w.#-])\d[\d,]*(?:\.\d+)?(?![\w])", v)
            return []
        for n in set(nums(json.load(open(f, encoding="utf-8")))):
            if n in ORD: continue
            if n not in corpus and n.replace(",", "") not in corpus and (n + "0") not in corpus: untraced.append((os.path.basename(f), n))
    if untraced: fails.append(("7 hand-typed numbers not in the Kshana sources", untraced))

    # 8 links ----------------------------------------------------------------------------------
    ids = {}
    def ids_of(rel):
        if rel not in ids:
            t = texts.get(rel) or (get(rel) if rel in PUBSET and rel.endswith(".html") else "")
            ids[rel] = set(re.findall(r'\bid="([^"]+)"', t))
        return ids[rel]
    bad = []
    for p, t in texts.items():
        if p.startswith("studio/"): continue
        base = os.path.dirname(p)
        for m in re.finditer(r'\s(?:href|src)="([^"]+)"', t):
            u = html.unescape(m.group(1))
            if re.match(r"^(https?:|mailto:|data:|javascript:)", u) or u.startswith("{{"): continue
            sp = urlsplit(u)
            if not sp.path:
                if sp.fragment and sp.fragment not in ids_of(p): bad.append((p, u))
                continue
            target, _ = resolve(p, u)
            if target not in PUBSET: bad.append((p, u)); continue
            if sp.fragment and target.endswith(".html") and sp.fragment not in ids_of(target): bad.append((p, u))
    if bad: fails.append(("8 links", bad[:15] + ([f"... {len(bad) - 15} more"] if len(bad) > 15 else [])))

    # 9 docs provenance ------------------------------------------------------------------------
    tracked = set(subprocess.run(["git", "-C", DOCS_REPO, "ls-files"], capture_output=True, text=True).stdout.split("\n"))
    man = json.loads(open(os.path.join(HERE, ".build", "docs-manifest.json"), encoding="utf-8").read())
    bad = [pg["source"] for pg in man["pages"] if not pg["source"].startswith("@") and pg["source"] not in tracked]
    bad += [a for a in man["assets"] if a not in tracked]
    if any("HANDOVER" in p.upper() for p in PUB) or any("HANDOVER.md" in get(p) for p in PUB if p.startswith("docs/") and p.endswith(".html")): bad.append("HANDOVER")
    if bad: fails.append(("9 docs provenance", bad))

    # 10 Pro claims ----------------------------------------------------------------------------
    ALLOW = [
        "clock digital twin", "calibrated to your device", "calibrated to a device", "allan budget", "trade stud", "ranked on a figure of merit", "ranks architecture",
        "evidence pack", "audit-grade", "reproducib", "proprietary overlay", "never forks", "depends on the", "under contract", "commercial licence",
        "closed or proprietary", "closed integration", "proprietary products", "agpl", "export-controlled", "export-sensitive", "clearance and nda",
        "calibrated to your hardware", "integration, training and consulting", "modelled pnt-resilience study", "not a certification", "optimism-gap audit",
        "named team", "scenario, seed and engine version", "request an evaluation", "request a pro evaluation", "talk to us", "request a study",
        "the image is the open engine", "what it builds on", "you walk away with", "kshana pro", "packages the evidence", "every figure",
        "systems-engineering", "systems engineering", "programme tooling", "described from the public readme", "custom study", "calibrated clock twin",
    ]
    FRAMING = {  # reviewed sentences that frame Pro without adding a claim
        "when the answer has to stand up in a review, pro turns the open engine into evidence.",
        "the open engine answers the question. pro makes the answer defensible.",
        "free to build on. pro when it has to stand up in review.",
        "why pro, when it matters more",
        "compare the editions",
        "what pro includes",
        "see kshana pro",
        "pro adds", "pro and custom add",
        "pro extends it.", "pro makes the answer defensible.",
        "evidence you can hand to a reviewer.",
        "the rest of the workflow.", "ask us for a walkthrough.",
        "why pro, even more", "open core, kshana pro, custom study", "free for everyone.", "pro when the answer has to stand up in a review.",
        "pro answers the programme's question.",  # Editions #pro heading (build.py pro_power); the lede under it is quoted from docs/PRO.md
        "custom study · done for you", "custom study: the answer, not the tool", "kshana pro · under contract", "request a pro evaluation", "what pro includes",
    }
    bad = []
    for p, t in site.items():
        blocks = re.findall(r'<(?:section|div|article|a|p)[^>]*class="[^"]*\b(?:pro-band|pro-line|ed-pro|why-col pro|tier pro|dv)\b[^"]*"[^>]*>(.*?)</(?:section|div|article|a|p)>', t, re.S)
        blocks += re.findall(r'<section class="pro-band"[^>]*>(.*?)</section>', t, re.S)
        for b in blocks:
            b = re.sub(r'<figure.*?</figure>|<div class="(?:evi|repro)">.*?</div>\s*</div>', " ", b, flags=re.S)
            txt = html.unescape(re.sub(r"<[^>]+>", " ", b))
            for s in re.split(r"(?<=[.!?])\s+|\n", re.sub(r"[ \t]+", " ", txt)):
                s = s.strip().lower()
                if len(s) < 12 or not re.search(r"\bpro\b|custom|overlay|study|licence|twin|evidence", s): continue
                if s in FRAMING or any(a in s for a in ALLOW): continue
                bad.append((p, s[:140]))
    if bad: fails.append(("10 Pro claims needing review", sorted(set(bad))[:12]))

    # 11 install drift -------------------------------------------------------------------------
    ch = json.loads(get("channels.json"))
    cmds = {c["command"] for c in ch["channels"] if c["command"]} | {c["pinned"] for c in ch["channels"] if c["pinned"]}
    bad = []
    readmes = "".join(open(os.path.join(REPO, f), encoding="utf-8").read() for f in ["README.md", "README.crates.md", "README.pypi.md", "README.npm.md", "mcp/kshana-mcp/README.md"])
    if ch["version"] != ver: bad.append(f"channels.json version {ch['version']} != Cargo.toml {ver}")
    for p, t in site.items():
        for m in re.finditer(r'<div class="ip-cmd" data-latest="([^"]+)" data-pinned="([^"]+)">.*?data-copy="([^"]+)"', t, re.S):
            lat, pin, shown = map(html.unescape, m.groups())
            if lat not in cmds or pin not in cmds or shown != lat: bad.append((p, shown))
        for m in re.finditer(r'data-copy="((?:cargo|pip|npm|docker) [^"]+)"', t):
            c = html.unescape(m.group(1))
            # a study example (Evidence #research) is not an install channel: it must be the README's own command
            if c.startswith("cargo run --release --example "):
                if c not in readmes: bad.append((p, c + " (not in the README)"))
                continue
            if c.split(" ")[1] in ("install", "add", "run") and c not in cmds and not c.startswith(("cargo install kshana-mcp", "pip install maturin")): bad.append((p, c))
        for v in re.findall(r"\bv(0\.\d+\.\d+)\b", without_third_party(re.sub(r"<[^>]+>", " ", t))):
            if v != ver: bad.append((p, f"shows v{v}"))
    for c in ch["channels"]:
        if c["command"] and c["command"] not in readmes and c["command"].replace("docker run --rm -i", "docker run -i") not in readmes:
            bad.append(f"README does not carry {c['command']}")
    if bad: fails.append(("11 install drift", bad[:10]))

    # 12 publish list --------------------------------------------------------------------------
    # History: this rule used to cap the list at 255 files, 64 MB and 15 MB a file, which are the
    # claude.ai preview host's limits, not kshana.dev's. kshana.dev deploys on GitHub Pages, so the
    # gate is now Pages' real limits: published size under 1 GB, each file under 100 MB. The preview
    # host's limits are not softened: `check.py --preview` (and `tools_bundle.mjs --preview-root`)
    # fail over them, and a plain run prints them as a non-gating note (see RE-PORT.md, HANDOVER.md).
    bad = [p for p in PUB if not os.path.exists(os.path.join(HERE, p))]
    sizes = {p: os.path.getsize(os.path.join(HERE, p)) for p in PUB if os.path.exists(os.path.join(HERE, p))}
    total = sum(sizes.values())
    if total > 1e9: bad.append(f"{total / 1e6:.1f} MB, over the 1 GB Pages limit")
    bad += [f"{p} over 100 MB" for p, n in sizes.items() if n > 100e6]
    pv = []
    if len(PUB) > 255: pv.append(f"{len(PUB)} files, over the preview host's 255-entry limit")
    if total > 64e6: pv.append(f"{total / 1e6:.1f} MB, over the preview host's 64 MB")
    pv += [f"{p} over the preview host's 15 MB" for p, n in sizes.items() if n > 15e6]
    if "--preview" in sys.argv: bad += pv
    elif pv: print("note (not gating; kshana.dev deploys on Pages): " + "; ".join(pv) + ". Run with --preview to gate on these.")
    if bad: fails.append(("12 publish list", bad))
    # 12b translations (design i18n v2, section 10): the 17 language trees are generated at deploy time by kshana's
    # tools/i18n.mjs (apply) from the English pages with every inlined image shared (assets/img), so a language tree is
    # about the pages' HTML without their inlined images (4 KB or more). The whole deploy tree, English and 17 languages,
    # stays under 500 MB: the Pages limit is 1 GB and the cap is the room we keep.
    img_re = re.compile(r"""(?:\s(?:src|href|xlink:href|poster)=(["'])data:image/[a-z+]+(?:;[a-z0-9-]+=[a-z0-9-]+)*(?:;base64)?,.*?\1|url\((["']?)data:image/[a-z+]+(?:;[a-z0-9-]+=[a-z0-9-]+)*(?:;base64)?,.*?\2\))""", re.S)
    per_lang = 0
    for p in PUB:
        if p.endswith(".html") and not p.startswith(("studio/", "playground/", "assets/")) and os.path.exists(os.path.join(HERE, p)):
            h = open(os.path.join(HERE, p), encoding="utf-8").read()
            per_lang += len(h.encode("utf-8")) - sum(len(m.group(0).encode("utf-8")) for m in img_re.finditer(h) if len(m.group(0)) >= 4096)
    projected = total + 17 * per_lang
    if projected > 500e6: fails.append(("12b translated size", [f"projected {projected / 1e6:.0f} MB with 17 language trees ({per_lang / 1e6:.1f} MB each), over the 500 MB cap"]))
    else: print(f"note: projected deploy size with 17 language trees: {projected / 1e6:.0f} MB ({per_lang / 1e6:.1f} MB each), cap 500 MB")
    # 13 coverage ------------------------------------------------------------------------------
    bad = []
    for link in re.findall(r"`cover:([^`]+)`", open(os.path.join(HERE, "COVERAGE.md"), encoding="utf-8").read()):
        pth, _, frag = link.partition("#")
        if pth not in PUBSET: bad.append(link); continue
        if frag and frag not in ids_of(pth): bad.append(link)
    if bad: fails.append(("13 coverage links", bad))
    # 14 studio name ---------------------------------------------------------------------------
    bad = []
    # Site pages and the generated docs pages; rendered repository Markdown is quoted as committed.
    for p in SITE_PAGES + ["docs/index.html", "docs/scenario-reference.html", "docs/capability-reference.html"]:
        t = texts.get(p, "")
        body = re.sub(r"<script.*?</script>|<style.*?</style>|<pre.*?</pre>|<code.*?</code>", " ", t, flags=re.S)
        vis = html.unescape(re.sub(r"<[^>]+>", " ", body))
        for m in re.finditer(r"playground", vis, re.I):
            bad.append((p, vis[max(0, m.start() - 40): m.end() + 20].strip()))
    # The Studio itself: its title and visible text carry the name, never "playground".
    pg = texts.get("studio/index.html", "")
    pgt = re.search(r"<title>([^<]*)</title>", pg)
    if not pgt or html.unescape(pgt.group(1)) != STUDIO: bad.append(("studio/index.html", f"title is not {STUDIO} (app.js reads it as the app's name)"))
    pgvis = html.unescape(re.sub(r"<[^>]+>", " ", re.sub(r"<script.*?</script>|<style.*?</style>|<code.*?</code>|<svg.*?</svg>", " ", pg, flags=re.S)))
    for m in re.finditer(r"playground", pgvis, re.I):
        bad.append(("studio/index.html", pgvis[max(0, m.start() - 40): m.end() + 20].strip()))
    for m in re.finditer(r'aria-label="([^"]*playground[^"]*)"', pg, re.I):
        bad.append(("studio/index.html", m.group(1)))
    for p in SITE_PAGES:
        nav = re.search(r'<header class="nav".*?</header>', texts[p], re.S)
        if not nav or STUDIO not in nav.group(0): bad.append((p, f"nav does not name {STUDIO}"))
    if bad: fails.append(("14 studio name", bad[:10]))
    # 15 glossary in Docs ------------------------------------------------------------------------
    bad = [p for p in SITE_PAGES if re.search(r'id="glossary"|class="gloss"', texts[p])]
    if "docs/glossary.html" not in PUBSET: bad.append("docs/glossary.html missing")
    if bad: fails.append(("15 glossary in Docs", bad))
    # 16 studio links ----------------------------------------------------------------------------
    bad = []
    STUDIO_HREF = re.compile(r'href="((?:\.\./)*/?studio/(?:index\.html)?\?[^"]*)"')
    def q_of(h):
        return dict(x.split("=", 1) for x in urlsplit(html.unescape(h)).query.split("&") if "=" in x)
    nlinks = 0
    # (a) every Studio link on every page opens a scenario the published Studio carries, on a view that run has
    for p, t in texts.items():
        if p.startswith("studio/"): continue
        for m in STUDIO_HREF.finditer(t):
            q = q_of(m.group(1))
            if "scenario" not in q: continue
            nlinks += 1
            sc, tab = q["scenario"], q.get("tab")
            if sc not in rec or sc not in STUDIO_VIEWS: bad.append((p, f"{sc}: not a scenario the Studio carries")); continue
            if tab and tab not in STUDIO_VIEWS[sc]["tabs"]: bad.append((p, f"{sc}: no {tab} view (it has {', '.join(STUDIO_VIEWS[sc]['tabs'])})"))
    if nlinks < 100: bad.append(("site", f"only {nlinks} Studio links found; the checker is not seeing them"))
    # (b) every provenance line on a site page that names a scenario the Studio carries links to it there
    nprov = 0
    for p, t in site.items():
        for m in re.finditer(r'<(p|figcaption)\b[^>]*class="[^"]*\bprov\b[^"]*"[^>]*>(.*?)</\1>', t, re.S):
            named = re.findall(r"<code>([\w-]+)\.toml</code>", m.group(2))
            if not named or named[0] not in STUDIO_VIEWS: continue
            nprov += 1
            if named[0] not in {q_of(h).get("scenario") for h in STUDIO_HREF.findall(m.group(2))}:
                bad.append((p, f"the visual of {named[0]}.toml has no link to that run in the Studio"))
    if nprov < 40: bad.append(("site", f"only {nprov} run visuals found; the checker is not seeing them"))
    # (c) the provenance lines the page scripts write: every scenario in their run data has a Studio
    #     view in js/studio-links.mjs, that module is the Studio's own list, and the scripts use it
    links_mjs = get("js/studio-links.mjs")
    lm = re.search(r"^const LINKS = (\{.*\});$", links_mjs, re.M)
    vm_ = re.search(r"^const VIEWS = (\{.*\});$", links_mjs, re.M)
    LINKS = json.loads(lm.group(1)) if lm else {}
    if not lm or not vm_ or LINKS != {n: v["best"] for n, v in STUDIO_VIEWS.items()} or json.loads(vm_.group(1)) != {n: v["tabs"] for n, v in STUDIO_VIEWS.items()}:
        bad.append(("js/studio-links.mjs", "is not the Studio's own link list: run build.py"))
    for n, h in LINKS.items():
        q = q_of(h)
        if q.get("scenario") != n or n not in rec or q.get("tab") not in STUDIO_VIEWS.get(n, {}).get("tabs", []): bad.append(("js/studio-links.mjs", f"{n}: {h} is not a view the Studio has"))
    drawn = {}
    mr = get("js/missions-runs.mjs")
    for n in json.loads(mr[mr.index("export default ") + len("export default "):].rstrip().rstrip(";"))["cards"]: drawn.setdefault(n, "js/missions-runs.mjs")
    for f in ("assets/capabilities/capabilities.json", "assets/waterfall/home-waterfall.json"):
        for n in re.findall(r'"file":\s*"([\w-]+)\.toml"', get(f)): drawn.setdefault(n, f)
    for n, where in sorted(drawn.items()):
        if n not in LINKS: bad.append((where, f"draws {n}.toml, which has no Studio view in js/studio-links.mjs"))
    if len(drawn) < 30: bad.append(("site", f"only {len(drawn)} script-drawn runs found; the checker is not seeing them"))
    for mod in ("js/capabilities.mjs", "js/missions.mjs"):
        if "studioHref(" not in get(mod) or 'from "./studio-links.mjs"' not in get(mod): bad.append((mod, "writes provenance lines without the Studio link list"))
    if bad: fails.append(("16 studio links", bad[:15] + ([f"... {len(bad) - 15} more"] if len(bad) > 15 else [])))
    # 17 engine version ---------------------------------------------------------------------------
    import gzip
    bad, nver = [], 0
    # The remaining allowances, by published file and by its exact wording (not a phrase-wide exemption): CAPABILITY.md's
    # dated revision lines for 0.38.0 and 0.39.0. They are a true historical record of what the engine was on that date, so each keeps the
    # version it states; every other 0.38.0 statement in the docs has been restated at the current version.
    HISTORIC = [
        (r"^(docs/capability-map\.html|llms-full\.txt)$", r"2026-10-19: engine 0\.38\.0, 87 kinds \(new in 0\.38\.0"),
        (r"^(docs/capability-map\.html|llms-full\.txt)$", r"2026-10-22: engine 0\.39\.0, 87 kinds, no new kind"),
    ]
    for p in PUB:
        if p.endswith(TEXT_EXT): t = get(p)
        elif p.endswith(".gz") and not (override and p in override): t = gzip.open(os.path.join(HERE, p)).read().decode("utf-8", "replace")
        elif p.endswith(".gz"): t = override[p]
        else: continue
        for v, ctx in version_claims(p, t):
            nver += 1
            if v != ver and not any(re.search(fp, p) and re.search(cp, ctx) for fp, cp in HISTORIC): bad.append((p, f"shows {v}, the engine is {ver}: …{ctx}…"))
    if nver < 100: bad.append(("site", f"only {nver} engine versions found; the checker is not seeing them"))
    if bad: fails.append(("17 engine version", bad[:15] + ([f"... {len(bad) - 15} more"] if len(bad) > 15 else [])))
    # 18 changelog Pro versions -------------------------------------------------------------------
    bad = []
    cl = get("docs/changelog.html") if "docs/changelog.html" in PUBSET else ""
    if not cl: bad.append("docs/changelog.html missing")
    scan = [("docs/changelog.html", html.unescape(re.sub(r"<[^>]+>", " ", cl)) + " " + cl)]
    for idx in ("docs/search-docs.json", "data/search.json"):
        try: scan.append((idx, json.dumps([e for e in json.loads(get(idx)) if is_changelog_h(e.get("h", ""))], ensure_ascii=False)))
        except (ValueError, KeyError): bad.append(f"{idx}: not readable")
    for rel, body in scan:
        for m in PRO_VERSION.finditer(body):
            bad.append((rel, " ".join(body[max(0, m.start() - 40): m.end() + 20].split())))
    if bad: fails.append(("18 changelog Pro versions", bad[:10]))
    # 19 no roadmap -------------------------------------------------------------------------------
    bad = [p for p in PUB if re.search(r"(^|/)roadmap\.html$", p)]
    for p, t in texts.items():
        if any(re.search(r"(?:^|/)roadmap(?:\.html)?$", h) for h in re.findall(r'href="([^"#?]*)', t)): bad.append((p, "links the roadmap page"))
    for idx in ("docs/search-docs.json", "data/search.json"):
        if re.search(r'"h":\s*"[^"]*docs/roadmap(?:\.html)?(?=["#?])', get(idx)): bad.append((idx, "has an entry for the roadmap page"))
    if bad: fails.append(("19 no roadmap", bad[:10]))
    # 20 clean URLs -------------------------------------------------------------------------------
    # Every link to a page of the site is its clean address: no ".html", no "index.html" (Home is "/").
    bad = []
    DIRTY = re.compile(r"(?:^|/)index\.html$|\.html$")
    for p, t in texts.items():
        if p.startswith("assets/") or (p.startswith("studio/") and p != "studio/index.html"): continue
        for m in re.finditer(r'<(?:a|area|link|iframe)\b[^>]*?\s(?:href|src)="([^"]*)"', t):
            u = html.unescape(m.group(1))
            if re.match(r"^(?:[a-z][a-z0-9+.-]*:|//|#|\{\{)", u, re.I): continue
            if DIRTY.search(urlsplit(u).path): bad.append((p, u))
    for idx in ("data/search.json", "docs/search-docs.json"):
        for e in json.loads(get(idx)):
            h = str(e.get("h", ""))
            if not re.match(r"^(?:[a-z][a-z0-9+.-]*:|//)", h, re.I) and DIRTY.search(urlsplit(h).path): bad.append((idx, h))
    if bad: fails.append(("20 clean URLs", sorted(set(bad))[:12] + ([f"... {len(set(bad)) - 12} more"] if len(set(bad)) > 12 else [])))
    # 21 old addresses and .html links ----------------------------------------------------------
    # Every internal link written with ".html" on any page (the Studio's included) reaches a published
    # file; nothing links the Studio's old address; that address forwards to /studio/ with the query
    # and the fragment kept, and is never indexed.
    bad = []
    for p, t in texts.items():
        for m in re.finditer(r'\s(?:href|src)="([^"]+)"', t):
            u = html.unescape(m.group(1))
            if re.match(r"^(?:[a-z][a-z0-9+.-]*:|//|#|\{\{)", u, re.I): continue
            if urlsplit(u).path.endswith(".html") and resolve(link_base(p, t), u)[0] not in PUBSET: bad.append((p, f"{u}: no such file"))
            if p != "playground/index.html" and re.search(r"(?:^|/)playground/", urlsplit(u).path): bad.append((p, f"{u}: the Studio's old address (it is /studio/)"))
    stub = texts.get("playground/index.html", "")
    if not stub: bad.append(("playground/index.html", "missing: /playground/ and /playground/index.html?... must keep working"))
    else:
        if 'location.replace("/studio/" + location.search + location.hash)' not in stub: bad.append(("playground/index.html", "does not forward to /studio/ with the query and the fragment"))
        if not re.search(r'<meta name="robots" content="noindex">', stub): bad.append(("playground/index.html", "is not noindex"))
        if '<link rel="canonical" href="https://kshana.dev/studio/">' not in stub: bad.append(("playground/index.html", "canonical is not https://kshana.dev/studio/"))
    if bad: fails.append(("21 old addresses and .html links", bad[:12]))

    # pages a search engine should index: every page but the Studio's assets, run reports and noindex pages
    def noindex(t): return bool(re.search(r'<meta name="robots" content="[^"]*noindex', t))
    indexable = sorted(p for p, t in texts.items() if not p.startswith("assets/") and (not p.startswith("studio/") or p == "studio/index.html") and not noindex(t))

    def canon(rel):
        if rel == "index.html": return "https://kshana.dev/"
        if rel.endswith("/index.html"): return "https://kshana.dev/" + rel[:-len("index.html")]
        return "https://kshana.dev/" + rel[:-len(".html")]

    # 22 sitemap ----------------------------------------------------------------------------------
    bad = []
    sm = get("sitemap.xml") if "sitemap.xml" in PUBSET else ""
    if not sm: bad.append("sitemap.xml missing")
    locs = re.findall(r"<url><loc>([^<]+)</loc><lastmod>(\d{4}-\d{2}-\d{2})</lastmod></url>", sm)
    if sm and len(locs) != sm.count("<url>"): bad.append("sitemap.xml: a <url> without a clean <loc> and a YYYY-MM-DD <lastmod>")
    listed = [l for l, _ in locs]
    bad += [f"listed twice: {l}" for l in sorted({l for l in listed if listed.count(l) > 1})]
    bad += [f"not listed: {p} ({canon(p)})" for p in indexable if canon(p) not in listed]
    known = {canon(p): p for p in indexable}
    bad += [f"lists {l}, which is not an indexable published page" for l in listed if l not in known]
    if len(listed) < 60: bad.append(f"only {len(listed)} addresses listed; the checker is not seeing them")
    if bad: fails.append(("22 sitemap", bad[:12]))

    # 23 meta -------------------------------------------------------------------------------------
    bad, titles, descs = [], {}, {}
    def tag(t, rx):
        m = re.search(rx, t)
        return html.unescape(m.group(1)) if m else None
    for p in indexable:
        t = texts[p]
        head = t.split("</head>", 1)[0]
        if not re.search(r'<html lang="[a-z]{2}(?:-[A-Z]{2})?"', t): bad.append((p, "no <html lang>"))
        ti, de = tag(head, r"<title>([^<]*)</title>"), tag(head, r'<meta name="description" content="([^"]*)"')
        if not ti: bad.append((p, "no <title>")); continue
        if len(ti) > 60: bad.append((p, f"title is {len(ti)} characters (at most 60): {ti}"))
        if not de or not 50 <= len(de) <= 155: bad.append((p, f"description is {len(de or '')} characters (50 to 155)"))
        if ti in titles: bad.append((p, f"title repeats {titles[ti]}: {ti}"))
        if de and de in descs: bad.append((p, f"description repeats {descs[de]}"))
        titles[ti], descs[de] = p, p
        if head.count('rel="canonical"') != 1 or tag(head, r'<link rel="canonical" href="([^"]+)">') != canon(p): bad.append((p, f"canonical is not {canon(p)}"))
        for k in ("og:title", "og:description", "og:url", "og:image", "og:type"):
            if not tag(head, r'<meta property="' + k + r'" content="([^"]+)">'): bad.append((p, f"no {k}"))
        if tag(head, r'<meta property="og:url" content="([^"]+)">') != canon(p): bad.append((p, "og:url is not the canonical address"))
        if not (tag(head, r'<meta property="og:image" content="([^"]+)">') or "").startswith("https://"): bad.append((p, "og:image is not an absolute https address"))
        if tag(head, r'<meta name="twitter:card" content="([^"]+)">') != "summary_large_image" or not tag(head, r'<meta name="twitter:image" content="([^"]+)">'): bad.append((p, "no large social card"))
    if len(indexable) < 60: bad.append(("site", f"only {len(indexable)} indexable pages; the checker is not seeing them"))
    if bad: fails.append(("23 meta", bad[:12]))

    # 24 structured data ----------------------------------------------------------------------------
    bad = []
    def lds(t):
        out = []
        for m in re.finditer(r'<script type="application/ld\+json">(.*?)</script>', t, re.S):
            try: out.append(json.loads(m.group(1)))
            except ValueError as e: out.append(e)
        return out
    def nodes(objs):
        return [n for o in objs if isinstance(o, dict) for n in (o.get("@graph") or [o])]
    for p in indexable:
        for o in lds(texts[p]):
            if isinstance(o, Exception): bad.append((p, f"JSON-LD does not parse: {o}"))
            elif o.get("@context") != "https://schema.org": bad.append((p, "JSON-LD without the schema.org context"))
    home = nodes(lds(texts["index.html"]))
    by = {}
    for n in home: by.setdefault(n.get("@type"), n)
    app, code, org, web = by.get("SoftwareApplication") or {}, by.get("SoftwareSourceCode") or {}, by.get("Organization") or {}, by.get("WebSite") or {}
    if app.get("softwareVersion") != ver: bad.append(("index.html", f"SoftwareApplication version {app.get('softwareVersion')} is not {ver}"))
    if "AGPL-3.0-only" not in str(app.get("license")) or "AGPL-3.0-only" not in str(code.get("license")): bad.append(("index.html", "licence is not AGPL-3.0-only"))
    if not app.get("operatingSystem") or not app.get("applicationCategory") or str((app.get("offers") or {}).get("price")) != "0": bad.append(("index.html", "SoftwareApplication lacks operatingSystem, applicationCategory or a free offer"))
    if not str(code.get("codeRepository", "")).startswith("https://github.com/"): bad.append(("index.html", "SoftwareSourceCode has no codeRepository"))
    if org.get("name") != "Ashforde OÜ": bad.append(("index.html", f"Organization is {org.get('name')!r}, the site names Ashforde OÜ"))
    tpl = str(((web.get("potentialAction") or {}).get("target") or {}).get("urlTemplate", ""))
    if "{search_term_string}" not in tpl or "?q=" not in tpl or 'URLSearchParams(location.search).get("q")' not in get("site.js"): bad.append(("index.html", "WebSite SearchAction does not match a ?q= search the site answers"))
    papers = json.load(open(os.path.join(HERE, "src", "data", "publications.json"), encoding="utf-8"))["papers"]
    arts = [n for n in nodes(lds(texts["evidence.html"])) if n.get("@type") == "ScholarlyArticle"]
    if sorted(a.get("url", "").rsplit("/", 1)[-1] for a in arts) != sorted(p["arxiv"] for p in papers): bad.append(("evidence.html", f"{len(arts)} ScholarlyArticle blocks for {len(papers)} papers"))
    for p in indexable:
        if not p.startswith("docs/"): continue
        bc = [n for n in nodes(lds(texts[p])) if n.get("@type") == "BreadcrumbList"]
        if not bc or bc[0]["itemListElement"][-1].get("item") != canon(p): bad.append((p, "no BreadcrumbList ending at the page"))
    if not any(n.get("@type") == "WebApplication" for n in nodes(lds(texts.get("studio/index.html", "")))): bad.append(("studio/index.html", "no WebApplication"))
    if bad: fails.append(("24 structured data", bad[:12]))

    # 25 crawl files ---------------------------------------------------------------------------------
    bad = []
    CRAWLERS = ["Googlebot", "Bingbot", "GPTBot", "OAI-SearchBot", "ChatGPT-User", "ClaudeBot", "Claude-User", "Claude-SearchBot", "PerplexityBot", "Google-Extended", "Applebot", "Applebot-Extended", "CCBot"]
    rb = get("robots.txt") if "robots.txt" in PUBSET else ""
    if not rb: bad.append("robots.txt missing")
    for c in CRAWLERS + ["*"]:
        if not re.search(r"(?m)^User-agent: " + re.escape(c) + r"\nAllow: /\n", rb): bad.append(f"robots.txt: {c} is not allowed everything")
    if re.search(r"(?mi)^Disallow:\s*\S", rb): bad.append("robots.txt disallows a path")
    if "Sitemap: https://kshana.dev/sitemap.xml" not in rb: bad.append("robots.txt has no Sitemap line")
    ll = get("llms.txt") if "llms.txt" in PUBSET else ""
    if not ll: bad.append("llms.txt missing")
    if not re.match(r"# \S", ll) or not re.search(r"(?m)^> \S", ll) or len(re.findall(r"(?m)^## ", ll)) < 4: bad.append("llms.txt: needs an H1, a > summary and sections")
    for p in indexable:
        if (p in SITE_PAGES or p.startswith("docs/") or p == "studio/index.html") and f"({canon(p)})" not in ll: bad.append(f"llms.txt does not link {canon(p)}")
    for pp in papers:
        if f"https://arxiv.org/abs/{pp['arxiv']}" not in ll: bad.append(f"llms.txt does not link arXiv {pp['arxiv']}")
    cff = open(os.path.join(REPO, "CITATION.cff"), encoding="utf-8").read()
    doi = re.search(r"^doi:\s*(\S+)", cff, re.M).group(1)
    if doi not in ll or "https://github.com/ashfordeOU/kshana" not in ll: bad.append("llms.txt lacks the DOI or the repository")
    for c in ch["channels"]:
        if c["command"] and f"`{c['command']}`" not in ll: bad.append(f"llms.txt lacks the install command {c['command']}")
    full = get("llms-full.txt") if "llms-full.txt" in PUBSET else ""
    if len(full) < 100_000: bad.append(f"llms-full.txt is {len(full)} bytes; it should carry every page")
    for p in indexable:
        if (p in SITE_PAGES or p.startswith("docs/")) and f"URL: {canon(p)}\n" not in full: bad.append(f"llms-full.txt has no section for {canon(p)}")
    key = json.load(open(os.path.join(HERE, "src", "data", "indexnow.json"), encoding="utf-8"))["key"]
    if f"{key}.txt" not in PUBSET or get(f"{key}.txt").strip() != key: bad.append("the IndexNow key file is missing or does not hold the key")
    if bad: fails.append(("25 crawl files", bad[:12]))

    # 27 every class has a rule -------------------------------------------------------------------
    # A class on a page of the site or the docs that no stylesheet mentions is how the "New in" strip, the
    # Capabilities cards and the install channels shipped as bare, overlapping text: the markup was written
    # and the CSS never was. Every class the built pages use must be named in a published stylesheet or an
    # inline <style>, or be a reviewed hook below (a wrapper or modifier that sits beside a styled class, a
    # script selector, an SVG state, or a syntax-highlighter token).
    CSS_ALL = "".join(get(p) for p in PUB if p.endswith(".css"))
    sel = set(re.findall(r"\.([A-Za-z_][\w-]*)", re.sub(r"/\*.*?\*/", "", CSS_ALL, flags=re.S)))
    used = {}
    for p, t in texts.items():
        if p.startswith("studio/"): continue
        sel |= set(re.findall(r"\.([A-Za-z_][\w-]*)", "".join(re.findall(r"<style[^>]*>(.*?)</style>", t, re.S))))
        body = re.sub(r"<(script|style)[^>]*>.*?</\1>", "", t, flags=re.S)
        for m in re.finditer(r'class="([^"]*)"', body):
            for c in m.group(1).split():
                if "{{" not in c: used.setdefault(c, p)
    bad = sorted((c, p) for c, p in used.items() if c not in sel and c not in CLASS_HOOKS and not c.startswith(HOOK_PREFIXES))
    if bad: fails.append(("27 every class has a rule", [f"{c} ({p})" for c, p in bad[:20]] + ([f"... {len(bad) - 20} more"] if len(bad) > 20 else [])))
    # 28 no hand-typed scenario count -----------------------------------------------------------------
    # "160 bundled scenarios" was typed by hand once and went stale when the library grew (0.37 shipped 160, a
    # later tree has more). The count in a page is {{FILES}}, {{ALLFILES}} or {{KINDS}}, written by build.py from the
    # engine checkout; a literal number in front of "bundled scenario(s)" or "scenario files" in a source page is a bug.
    bad = []
    for root, _, names in os.walk(os.path.join(HERE, "src")):
        if os.sep + "data" in root or os.sep + "runs" in root: continue
        for n in names:
            if not n.endswith((".html", ".mjs", ".js", ".py")) or n.endswith(("-runs.mjs", "-data.mjs")): continue
            p_ = os.path.join(root, n)
            for m in re.finditer(r"\b(\d{2,3})\s+(?:bundled\s+scenarios?|(?:bundled\s+)?scenario\s+files|built-in\s+scenarios?)", open(p_, encoding="utf-8", errors="replace").read()):
                bad.append(f"{os.path.relpath(p_, HERE)}: {m.group(0)!r}")
    if bad: fails.append(("28 no hand-typed scenario count", bad[:12]))
    # 26 crawlable text ------------------------------------------------------------------------------
    # The sections a page script draws carry the same key text in <noscript>, from the same data; the
    # Studio carries a description and a link to every scenario it runs.
    bad = []
    WANT = {"index.html": 2, "capabilities.html": 2, "developers.html": 2, "evidence.html": 3, "missions.html": 1}
    for p, n in WANT.items():
        got = texts[p].count('<noscript><div class="crawl-text">')
        if got < n: bad.append((p, f"{got} crawlable text blocks for {n} script-drawn sections"))
    mm = re.search(r'<div class="mm-body" data-mm>.*?<noscript>(.*?)</noscript>', texts["missions.html"], re.S)
    if not mm or mm.group(1).count("<tr>") < 9: bad.append(("missions.html", "the threat matrix has no table for crawlers"))
    st = re.search(r"<noscript><section class=\"crawl-text\">(.*?)</section></noscript>", texts.get("studio/index.html", ""), re.S)
    if not st or len(re.findall(r'href="/studio/\?scenario=', st.group(1))) < len(STUDIO_VIEWS) * 0.9: bad.append(("studio/index.html", "no crawlable list of the scenarios it runs"))
    if bad: fails.append(("26 crawlable text", bad[:12]))
    return fails, {"pages": len(texts), "charts": nfig, "files": len(PUB), "mb": round(total / 1e6, 1), "studio_links": nlinks, "run_visuals": nprov + len(drawn)}


def report(fails, stats):
    for name, items in fails:
        print(f"FAIL {name}")
        items = items.items() if isinstance(items, dict) else items
        for it in list(items)[:15]:
            print("   ", it)
    print(f"{'FAILED' if fails else 'OK'}: {stats['pages']} pages, {stats['charts']} charts, {stats['files']} files, {stats['mb']} MB, {stats['studio_links']} Studio links, {stats['run_visuals']} run visuals")


if __name__ == "__main__":
    if "--nav" in sys.argv:
        # Navigation: every in-page, cross-page and page link on every page, phone and desktop, in a
        # real browser against a served build (../../features/check_nav.mjs documents the rule).
        rest = sys.argv[sys.argv.index("--nav") + 1:]
        base = rest[0] if rest and not rest[0].startswith("-") else "http://127.0.0.1:8847"
        nav = os.path.normpath(os.path.join(HERE, "..", "..", "features", "check_nav.mjs"))
        sys.exit(subprocess.run(["node", nav, base] + [a for a in rest if a != base]).returncode)
    if "--mutate" in sys.argv:
        idx = rd("index.html")
        cases = {
            "27 every class has a rule": {"index.html": idx.replace("</main>", '<p class="zz-unstyled-class">x</p></main>', 1)},
            "5 stand-ins": {"index.html": idx.replace("</main>", '<p class="illus">Illustrative</p></main>', 1)},
            # inside the kpage blob only the i18n caps tokens may stand: a capitalised "Illustrative" there must go red
            "5 stand-ins (a non-token word inside the kpage blob)": {"evidence.html": rd("evidence.html").replace('{"matrix":{', '{"note":"Illustrative figures","matrix":{', 1)},
            "5 stand-ins (a lowercase word inside the kpage blob)": {"evidence.html": rd("evidence.html").replace('{"matrix":{', '{"note":"an illustrative relay set","matrix":{', 1)},
            "6 provenance": {"index.html": re.sub(r'<figcaption class="prov">.*?</figcaption>', "", idx, count=1, flags=re.S)},
            "11 install drift": {"index.html": idx.replace('data-copy="pip install kshana"', 'data-copy="pip install kshana-engine"', 1)},
            "3 forbidden names": {"index.html": idx.replace("</main>", "<p>Aperio</p></main>", 1)},
            "8 links": {"index.html": idx.replace("</main>", '<a href="nowhere.html">x</a></main>', 1)},
            "14 studio name": {"index.html": idx.replace("</main>", "<p>Open the playground</p></main>", 1)},
            "15 glossary in Docs": {"index.html": idx.replace("</main>", '<section id="glossary"></section></main>', 1)},
            "14 studio name (in the Studio)": {"studio/index.html": rd("studio/index.html").replace("<title>", "<title>Kshana Playground · ", 1)},
            # a run visual that lost its Studio link (the Home L-band waterfall)
            "16 studio links (a run visual without its link)": {"index.html": re.sub(r'(<code>l-band-waterfall-jamming\.toml</code></span>)<a class="prov-open" href="/studio/[^"]*">.*?</a>', r"\1", idx, count=1, flags=re.S)},
            # a link to a view that run does not have, and to a scenario the Studio does not carry
            "16 studio links (a view the run does not have)": {"index.html": idx.replace("scenario=l-band-waterfall-jamming&amp;tab=spectrum", "scenario=l-band-waterfall-jamming&amp;tab=holdover", 1)},
            "16 studio links (a scenario the Studio does not carry)": {"missions.html": rd("missions.html").replace("/studio/?scenario=jamming-demo&amp;", "/studio/?scenario=jamming-demo-2&amp;", 1)},
            # a run a page script draws that the link list no longer carries
            "16 studio links (a script-drawn run without a view)": {"js/studio-links.mjs": re.sub(r'"leo-timing-utc":"[^"]*",', "", rd("js/studio-links.mjs"), count=1)},
            # the Studio itself no longer carries a scenario the pages link to
"17 engine version (a provenance line)": {"missions.html": re.sub(r"Engine v0\.\d+\.\d+", "Engine v0.28.0", rd("missions.html"), count=1)},
            "17 engine version (a script's data)": {"js/missions.mjs": re.sub(r'(const FOOTPRINT = \{\n  engine: ")[^"]+"', r'\g<1>0.28.0"', rd("js/missions.mjs"), count=1)},
            "17 engine version (a run report in a docs page)": {"docs/overview.html": rd("docs/overview.html").replace("</main>", "<p>Generated by Kshana 0.1.0.</p></main>", 1)},
            "17 engine version (a JSON field in the Studio)": {"studio/channels.json": re.sub(r'"version":\s*"[^"]+"', '"version": "0.28.0"', rd("studio/channels.json"), count=1)},
            "17 engine version (history text outside the changelog)": {"editions.html": rd("editions.html").replace("</main>", "<p>drawn at engine 0.22.0</p></main>", 1)},
            "18 changelog Pro versions": {"docs/changelog.html": rd("docs/changelog.html").replace("</main>", "<p>Kshana Pro 0.3.0 ships the twin.</p></main>", 1)},
            "18 changelog Pro versions (a package name)": {"docs/changelog.html": rd("docs/changelog.html").replace("</main>", "<p>Built with kshana-pro.</p></main>", 1)},
            "18 changelog Pro versions (in the search index)": {"docs/search-docs.json": rd("docs/search-docs.json").replace('"h":"/docs/changelog"', '"h":"/docs/changelog","note":"Pro v1.2"', 1)},
            "19 no roadmap": {"docs/index.html": rd("docs/index.html").replace("</main>", '<a href="/docs/roadmap">Roadmap</a></main>', 1)},
            "20 clean URLs (a .html link)": {"index.html": idx.replace('href="/evidence#research"', 'href="evidence.html#research"', 1)},
            "20 clean URLs (Home as /index.html)": {"docs/changelog.html": rd("docs/changelog.html").replace('href="/"', 'href="/index.html"', 1)},
            "20 clean URLs (a docs index.html link)": {"evidence.html": rd("evidence.html").replace('href="/docs/"', 'href="/docs/index.html"', 1)},
            "20 clean URLs (in the search index)": {"data/search.json": rd("data/search.json").replace('"h":"/evidence#research"', '"h":"evidence.html#research"', 1)},
            "16 studio links (the Studio lost a scenario)": {"studio/recorded/index.json": rd("studio/recorded/index.json").replace('"clock-holdover.toml"', '"clock-holdover-gone.toml"', 1)},
            "21 old addresses and .html links (a dead .html link)": {"index.html": idx.replace("</main>", '<a href="/no-such-page.html">x</a></main>', 1)},
            "21 old addresses and .html links (a link to the old address)": {"index.html": idx.replace('href="/studio/"', 'href="/playground/"', 1)},
            "21 old addresses and .html links (the old address drops the query)": {"playground/index.html": rd("playground/index.html").replace("location.search + location.hash", "location.hash", 1)},
            "22 sitemap (a page left out)": {"sitemap.xml": re.sub(r"  <url><loc>https://kshana\.dev/evidence</loc>.*?\n", "", rd("sitemap.xml"), count=1)},
            "22 sitemap (an address that is not a page)": {"sitemap.xml": rd("sitemap.xml").replace("</urlset>", "  <url><loc>https://kshana.dev/nope</loc><lastmod>2026-10-01</lastmod></url>\n</urlset>")},
            "23 meta (a duplicate title)": {"missions.html": re.sub(r"<title>[^<]*</title>", re.search(r"<title>[^<]*</title>", idx).group(0), rd("missions.html"), count=1)},
            "23 meta (a description over 155)": {"evidence.html": re.sub(r'(<meta name="description" content=")', r"\1" + "x" * 160, rd("evidence.html"), count=1)},
            "23 meta (no canonical)": {"developers.html": re.sub(r'<link rel="canonical" href="[^"]*">\n', "", rd("developers.html"), count=1)},
            "24 structured data (JSON-LD that does not parse)": {"index.html": idx.replace('<script type="application/ld+json">{', '<script type="application/ld+json">{,', 1)},
            "24 structured data (a paper without its block)": {"evidence.html": rd("evidence.html").replace('"@type": "ScholarlyArticle"', '"@type": "Article"', 1)},
            "24 structured data (another version)": {"index.html": re.sub(r'"softwareVersion": "[^"]+"', '"softwareVersion": "0.0.1"', idx, count=1)},
            "25 crawl files (robots.txt shuts a crawler out)": {"robots.txt": rd("robots.txt").replace("User-agent: GPTBot\nAllow: /", "User-agent: GPTBot\nDisallow: /", 1)},
            "25 crawl files (llms.txt loses a paper)": {"llms.txt": re.sub(r"(?m)^- \[[^\n]*arxiv\.org/abs/2606\.22054[^\n]*\n", "", rd("llms.txt"), count=1)},
            "25 crawl files (llms-full.txt loses a docs page)": {"llms-full.txt": rd("llms-full.txt").replace("URL: https://kshana.dev/docs/glossary\n", "URL: https://kshana.dev/docs/glossary-old\n", 1)},
            "26 crawlable text (the Missions matrix)": {"missions.html": re.sub(r'(<div class="mm-body" data-mm>.*?)<noscript>.*?</noscript>', r"\1", rd("missions.html"), count=1, flags=re.S)},
            "17 engine version (a docs part of llms-full.txt)": {"llms-full.txt": rd("llms-full.txt").replace("URL: https://kshana.dev/docs/glossary\n", "URL: https://kshana.dev/docs/glossary\nRuns on Kshana 0.1.0.\n", 1)},
        }
        ok = True
        only = sys.argv[sys.argv.index("--only") + 1] if "--only" in sys.argv else ""  # --mutate --only "5 stand-ins": just the cases whose name contains it
        for name, ov in cases.items():
            if only and only not in name: continue
            f, _ = run_checks(ov)
            red = any(n == name.split(" (")[0] for n, _ in f)
            if idx in ov.values() or any(v == rd(k) for k, v in ov.items()):
                red = False; name += " [the mutation changed nothing]"
            print(f"mutation {name}: {'RED (good)' if red else 'STAYED GREEN (bad)'}")
            ok &= red
        # the other half: a caps token of the i18n list inside the kpage blob must stay green
        f, _ = run_checks({"evidence.html": rd("evidence.html").replace('{"matrix":{', '{"note":"The relay set is ILLUSTRATIVE; NOT MET, VERIFIED","matrix":{', 1)})
        tok_ok = not any(n == "5 stand-ins" for n, _ in f)
        print(f"kpage blob with caps tokens only (ILLUSTRATIVE, NOT MET, VERIFIED): {'GREEN (good)' if tok_ok else 'WENT RED (bad)'}")
        ok &= tok_ok
        f, st = run_checks()
        print(f"unmutated: {'green' if not f else 'red'}")
        sys.exit(0 if ok and not f else 1)
    fails, stats = run_checks()
    report(fails, stats)
    sys.exit(1 if fails else 0)
