#!/usr/bin/env python3
"""Check the Kshana site against the generated feature register.

Exits non-zero, listing every problem, when:
  UNPLACED            a feature has no entry in placements.toml (or an entry with no page)
  EDITION             a register row's edition is not backed by its public source, or a built page
                      presents a feature under an edition that its public source contradicts
                      (for example a Free-core feature listed as a Pro deliverable)
  NOT_SHOWN           a feature is placed on a page whose built HTML does not mention it
  UNREGISTERED        a built page names a scenario kind, MCP tool, scenario id or code identifier
                      that the register does not have
  BAD_PLACEMENT       a placement names a page, section anchor, Studio control or docs anchor that
                      does not exist, or a placement entry names no feature (stale)
  COUNT_DRIFT         a page states a count (scenario kinds, bundled scenarios, MCP tools,
                      capabilities) that disagrees with the register

A feature whose only mention on its page is inside attributes or data (a tooltip, a search index,
a link target) counts as NOT_SHOWN: the reader never sees it.

Warnings (reported, never fail): WEAK (only an ambiguous plain word in prose mentions it), OTHER_SECTION (shown on the right page but outside the placed section), ALSO (a
secondary placement is not shown), PRO_LINK (a capability that Pro or a study extends shows no
Pro mention in its section), NO_STUDIO (a kind or bundled scenario has no Studio scenario that runs it), ABBREVIATION (a
description keeps an unexpanded abbreviation).

Matching is by whole identifier, never by substring: `clock` does not match `clock-holdover`,
`spoof` does not match `spoof-detect`, and an identifier that is also an ordinary English word
(`clock`, `orbit`, `integrity`, ...) only counts when it is set as code (<code>, <kbd>, <pre>) or
written as `kind = "..."`. Scripts and styles are not page text; JSON data islands count as WEAK.

Standard library only. Usage:
  python3 check_features.py [--site DIR] [--studio DIR] [--no-regen] [--json OUT] [--gaps GAPS.md]
"""
from __future__ import annotations

import argparse
import glob
import html as htmlmod
import json
import os
import re
import subprocess
import sys
import tomllib
from html.parser import HTMLParser
from urllib.parse import parse_qs, urlparse

HERE = os.path.dirname(os.path.abspath(__file__))
SITE_NEXT = os.path.dirname(HERE)
DEFAULT_SITE = os.path.join(SITE_NEXT, "explorations/2026-09-27-combo")
def _default_studio():
    """The Studio the site is built with (the site's src/data/sources.json: studio, relative to the site folder)."""
    cfg = os.path.join(DEFAULT_SITE, "src", "data", "sources.json")
    rel = os.environ.get("KSHANA_STUDIO") or (json.load(open(cfg, encoding="utf-8")).get("studio") if os.path.exists(cfg) else None) or "../2026-09-30-studio-next"
    return os.path.normpath(os.path.join(DEFAULT_SITE, os.path.expanduser(rel)))


DEFAULT_STUDIO = _default_studio()

PAGE_FILES = {
    "Home": "index.html", "Missions": "missions.html", "Capabilities": "capabilities.html",
    "Evidence": "evidence.html", "Developers": "developers.html", "Editions": "editions.html",
}
PAGES = list(PAGE_FILES) + ["Docs", "Studio"]
ERROR_CLASSES = ["UNPLACED", "EDITION", "NOT_SHOWN", "UNREGISTERED", "BAD_PLACEMENT", "COUNT_DRIFT"]
WARN_CLASSES = ["WEAK", "OTHER_SECTION", "ALSO", "PRO_LINK", "NO_STUDIO", "ABBREVIATION"]

VOID = {"area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"}
CODE_TAGS = {"code", "kbd", "pre", "samp", "tt"}
BLOCK_TAGS = {"h1", "h2", "h3", "h4", "h5", "h6", "li", "p", "td", "th", "dd", "dt", "figcaption", "button", "a", "summary", "blockquote"}
CLAIM_TAGS = {"h2", "h3", "h4", "h5", "li", "dt", "strong", "b"}
IDCH = r"A-Za-z0-9_\-"


# ------------------------------------------------------------------------------------------ DOM

class Node:
    __slots__ = ("tag", "attrs", "parent", "children", "id", "classes", "headings")

    def __init__(self, tag, attrs, parent):
        self.tag = tag
        self.attrs = dict(attrs)
        self.parent = parent
        self.children = []
        self.id = self.attrs.get("id") or ""
        self.classes = tuple((self.attrs.get("class") or "").split())
        self.headings = ()

    def ancestors(self):
        n = self
        while n is not None:
            yield n
            n = n.parent


class Text:
    __slots__ = ("text", "parent", "ctx", "headings", "low", "norm")

    def __init__(self, text, parent, ctx, headings):
        self.text = text
        self.norm = re.sub(r"\s+", " ", text)
        self.low = self.norm.lower()
        self.parent = parent
        self.ctx = ctx          # text | code | attr | data
        self.headings = headings


class DomBuilder(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.root = Node("#root", [], None)
        self.cur = self.root
        self.texts: list[Text] = []
        self.skip = 0
        self.json_script = False
        self.heading_stack: list[tuple[int, str]] = []
        self.ids: set[str] = set()

    def _headings(self):
        return tuple(h for _, h in self.heading_stack)

    def handle_starttag(self, tag, attrs):
        node = Node(tag, attrs, self.cur)
        if node.id:
            self.ids.add(node.id)
        if re.fullmatch(r"h[1-6]", tag) and node.id:
            lvl = int(tag[1])
            while self.heading_stack and self.heading_stack[-1][0] >= lvl:
                self.heading_stack.pop()
            self.heading_stack.append((lvl, node.id))
        node.headings = self._headings()
        self.cur.children.append(node)
        for k, v in attrs:
            if v and (k in ("title", "href", "aria-label", "alt", "content") or k.startswith("data-")):
                self.texts.append(Text(v, node, "attr", node.headings))
        if tag in ("script", "style"):
            self.skip += 1
            self.json_script = tag == "script" and "json" in (node.attrs.get("type") or "")
        if tag not in VOID:
            self.cur = node

    def handle_startendtag(self, tag, attrs):
        self.handle_starttag(tag, attrs)
        if tag not in VOID and self.cur.tag == tag:
            self.cur = self.cur.parent

    def handle_endtag(self, tag):
        if tag in ("script", "style") and self.skip:
            self.skip -= 1
            self.json_script = False
        n = self.cur
        while n is not None and n.tag != tag:
            n = n.parent
        if n is not None and n.parent is not None:
            self.cur = n.parent

    def handle_data(self, data):
        if not data.strip():
            return
        if self.skip:
            if self.json_script:
                self.texts.append(Text(data, self.cur, "data", self._headings()))
            return
        ctx = "code" if any(a.tag in CODE_TAGS for a in self.cur.ancestors()) else "text"
        t = Text(data, self.cur, ctx, self._headings())
        self.texts.append(t)
        self.cur.children.append(t)


class Page:
    def __init__(self, name, path, extra_data=None):
        self.name = name
        self.path = path
        self.exists = os.path.exists(path)
        self.texts: list[Text] = []
        self.ids: set[str] = set()
        self.root = None
        if self.exists:
            b = DomBuilder()
            b.feed(open(path, encoding="utf-8", errors="replace").read())
            b.close()
            self.texts, self.ids, self.root = b.texts, b.ids, b.root
        for p in extra_data or []:
            if os.path.exists(p):
                self.texts.append(Text(open(p, encoding="utf-8", errors="replace").read(), self.root, "studio-data", ()))

    _trunc = None
    requirement_names: list[str] = []

    def truncated(self):
        """Visible text fragments that end in an ellipsis (a truncated long name)."""
        if self._trunc is None:
            self._trunc = [t for t in self.texts if t.ctx in ("text", "code") and t.norm.rstrip().endswith("…")]
        return self._trunc

    def prefix_unique(self, pre: str) -> bool:
        return sum(1 for r in Page.requirement_names if r.startswith(pre)) == 1

    def in_section(self, t: Text, section: str) -> bool:
        if not section or t.ctx == "studio-data":
            return True
        if section in t.headings:
            return True
        n = t.parent
        while n is not None:
            if n.id == section:
                return True
            n = n.parent
        return False


# ------------------------------------------------------------------------------------ matching

def is_ambiguous(ident: str) -> bool:
    """An identifier that reads as an ordinary word in prose: no separator, no digit, lower-case."""
    return bool(re.fullmatch(r"[a-z]+", ident))


def probe_regex(kind: str, text: str) -> re.Pattern:
    if kind in ("ident", "ident_cs"):
        return re.compile(rf"(?<![{IDCH}]){re.escape(text)}(?![{IDCH}])")
    if kind == "exact":
        return re.compile(re.escape(re.sub(r"\s+", " ", text)))
    if kind == "regex":
        return re.compile(text)
    # phrase: case-insensitive, whole words at the ends, flexible whitespace
    body = r"\s+".join(re.escape(w) for w in text.split())
    pre = r"(?<![A-Za-z0-9])" if text[:1].isalnum() else ""
    post = r"(?![A-Za-z0-9])" if text[-1:].isalnum() else ""
    return re.compile(pre + body + post, re.I)


def hit_strength(kind: str, text: str, t: Text, m: re.Match) -> str:
    """strong | weak | hidden for one match. hidden = only in an attribute or a JSON data island,
    which the reader never sees as text; weak = an ambiguous word in prose."""
    if t.ctx in ("attr", "data"):
        return "hidden"
    if t.ctx == "studio-data":
        return "strong"
    if kind == "ident" and is_ambiguous(text):
        if t.ctx == "code":
            return "strong"
        before = t.text[max(0, m.start() - 12): m.start()]
        if re.search(r"kind\s*=\s*\"?$", before):
            return "strong"
        # a label or table cell that is exactly the identifier is not prose
        if t.text.strip() == text:
            return "strong"
        # link text that IS the id of the scenario the link opens (an atlas node labelled `passes`)
        n = t.parent
        while n is not None and n.tag != "a":
            n = n.parent
        if n is not None and t.text.strip() == text:
            q = parse_qs(urlparse(htmlmod.unescape(n.attrs.get("href") or "")).query)
            if text in [x.removesuffix(".toml") for x in q.get("scenario", [])]:
                return "strong"
        return "weak"
    return "strong"


_rx_cache: dict = {}


def find_hits(page: Page, probes) -> list[tuple[Text, str]]:
    hits = []
    for kind, text in probes:
        key = (kind, text)
        if key not in _rx_cache:
            _rx_cache[key] = probe_regex(kind, text)
        rx = _rx_cache[key]
        if kind in ("ident", "ident_cs"):
            needle, field = text, "text"
        elif kind == "exact":
            needle, field = re.sub(r"\s+", " ", text)[:24], "norm"
        elif kind == "phrase":
            needle, field = text.split()[0].lower(), "low"
        else:
            needle, field = None, "text"
        if kind == "exact":
            full = re.sub(r"\s+", " ", text)
            for t in page.truncated():
                pre = t.norm.strip().rstrip("…").rstrip()
                if len(pre) >= 25 and full.startswith(pre) and full != pre:
                    hits.append((t, "strong" if page.prefix_unique(pre) else "weak"))
                    break
        for t in page.texts:
            if needle is not None and needle not in getattr(t, field):
                continue
            s = t.norm if kind in ("exact", "phrase") else t.text
            for m in rx.finditer(s):
                hits.append((t, hit_strength(kind, text, t, m)))
                break
    return hits


def where(page: Page, t: Text) -> str:
    sec = ""
    n = t.parent
    while n is not None:
        if n.tag == "section" and n.id:
            sec = n.id
            break
        n = n.parent
    if not sec and t.headings:
        sec = t.headings[-1]
    return f"{page.name}#{sec}" if sec else page.name


# ----------------------------------------------------------------------------------- the check

class Report:
    def __init__(self):
        self.items: list[dict] = []

    def add(self, cls, fid, page, msg, **kw):
        d = dict(cls=cls, id=fid, page=page, msg=msg)
        d.update(kw)
        self.items.append(d)

    def of(self, cls):
        return [i for i in self.items if i["cls"] == cls]


def load_studio_catalog(studio: str):
    cat_p = os.path.join(studio, "lib/catalog.mjs")
    if not os.path.exists(cat_p):
        return set(), set()
    cat = open(cat_p, encoding="utf-8").read()
    files = set(re.findall(r'\[\s*"([a-z0-9.\-]+)\.toml"\s*,', cat))
    nib_block = cat[cat.find("NOT_IN_BROWSER"):]
    nib_block = nib_block[: nib_block.find("};") + 2]
    nib = set(re.findall(r'"([a-z0-9.\-]+)\.toml"\s*:', nib_block))
    # A file the browser build cannot run but the Studio shows as a run recorded with the native
    # engine (RECORDED_NATIVELY) is shown in the Studio, not command-line only.
    nat = re.search(r"RECORDED_NATIVELY\s*=\s*\[([^\]]*)\]", cat)
    nib -= set(re.findall(r'"([a-z0-9.\-]+)\.toml"', nat.group(1))) if nat else set()
    return files, nib


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--site", default=DEFAULT_SITE)
    ap.add_argument("--studio", default=DEFAULT_STUDIO)
    ap.add_argument("--features", default=os.path.join(HERE, "features.json"))
    ap.add_argument("--placements", default=os.path.join(HERE, "placements.toml"))
    ap.add_argument("--no-regen", action="store_true", help="do not re-run gen_features.py first")
    ap.add_argument("--json", help="write the machine-readable result here")
    ap.add_argument("--gaps", help="rewrite the generated block of this GAPS.md")
    ap.add_argument("--quiet", action="store_true")
    args = ap.parse_args()

    if not args.no_regen:
        p = subprocess.run([sys.executable, os.path.join(HERE, "gen_features.py"), "--placements", args.placements,
                            "--out", os.path.dirname(os.path.abspath(args.features))], capture_output=True, text=True)
        if p.returncode != 0:
            print(p.stdout + p.stderr)
            return 2

    reg = json.load(open(args.features, encoding="utf-8"))
    feats = reg["features"]
    # The docs pages may render a different engine checkout than the rest of the site
    # (src/data/sources.json: docs_repo). Their identifiers and counts are then checked against a
    # register generated from that checkout (features/.docs-register/, placements not graded there).
    dreg = reg
    src_cfg = os.path.join(args.site, "src", "data", "sources.json")
    if os.path.exists(src_cfg):
        sc = json.load(open(src_cfg, encoding="utf-8"))
        docs_repo = os.path.abspath(os.path.expanduser(os.environ.get("KSHANA_DOCS_REPO") or sc.get("docs_repo") or sc.get("repo") or ""))
        if docs_repo and os.path.realpath(docs_repo) != os.path.realpath(reg["engine"]["repo"]):
            dout = os.path.join(HERE, ".docs-register")
            os.makedirs(dout, exist_ok=True)
            if not args.no_regen or not os.path.exists(os.path.join(dout, "features.json")):
                p = subprocess.run([sys.executable, os.path.join(HERE, "gen_features.py"), "--repo", docs_repo, "--placements", args.placements, "--out", dout], capture_output=True, text=True)
                if p.returncode != 0:
                    print("docs register:\n" + p.stdout + p.stderr)
                    return 2
            dreg = json.load(open(os.path.join(dout, "features.json"), encoding="utf-8"))
            dreg["_known_tokens"] = os.path.join(dout, "known-tokens.json")
    Page.requirement_names = [re.sub(r"\s+", " ", f["name"]) for f in feats if f["category"] == "validation"]
    pl = tomllib.load(open(args.placements, "rb"))
    placements = pl.get("placements", {})
    ignore_tokens = set(pl.get("site_tokens", {}).get("ignore", []))
    rep = Report()

    # ---- pages
    pages: dict[str, Page] = {}
    for name, f in PAGE_FILES.items():
        pages[name] = Page(name, os.path.join(args.site, f))
    docs_pages: dict[str, Page] = {}
    for p in sorted(glob.glob(os.path.join(args.site, "docs", "*.html"))):
        docs_pages[os.path.basename(p)] = Page("Docs/" + os.path.basename(p), p)
    studio_page = Page("Studio", os.path.join(args.studio, "index.html"),
                       extra_data=[os.path.join(args.studio, "lib/catalog.mjs"), os.path.join(args.studio, "app.js")])
    studio_files, studio_nib = load_studio_catalog(args.studio)
    all_pages = list(pages.values()) + list(docs_pages.values()) + [studio_page]

    # ---- 1. UNPLACED + stale entries (read placements.toml directly, never trust a stale json)
    ids = {f["id"] for f in feats}
    for f in feats:
        p = placements.get(f["id"])
        if not p or not p.get("page"):
            rep.add("UNPLACED", f["id"], "", "no entry in placements.toml")
    for k in sorted(set(placements) - ids):
        rep.add("BAD_PLACEMENT", k, "", "placements.toml entry names no feature in the register (stale)")

    # ---- 2. EDITION (register vs public source)
    for f in feats:
        if f["category"] in ("pro", "study"):
            if not f["facts"].get("quote_found"):
                rep.add("EDITION", f["id"], "", f"edition '{f['edition']}' is not backed by the public source: quote not found in {f['source']}")
        else:
            if f["edition"] != "Free core":
                rep.add("EDITION", f["id"], "", f"engine feature marked '{f['edition']}'; the public promise is no feature gate on the core")
            if "UNSOURCED" in (f.get("edition_source") or ""):
                rep.add("EDITION", f["id"], "", "Free-core source quote ('no feature gate') not found in its public source (gen_features.py FREE_CORE_SOURCE)")

    # ---- 3. placement validity + NOT_SHOWN
    status: dict[str, dict] = {}
    for f in feats:
        p = placements.get(f["id"]) or {}
        page_name = p.get("page", "")
        section = p.get("section", "")
        probes = [tuple(x) for x in f["probes"]]
        st = {"page": page_name, "section": section, "state": "UNPLACED", "shown_on": [], "weak_on": []}
        status[f["id"]] = st
        if not page_name:
            continue
        # the placed page
        if page_name == "Docs":
            file, _, anchor = section.partition("#")
            page = docs_pages.get(file)
            sec = anchor
            if page is None:
                rep.add("BAD_PLACEMENT", f["id"], "Docs", f"docs page '{file}' is not built")
        elif page_name == "Studio":
            page, sec = studio_page, section
        elif page_name in pages:
            page, sec = pages[page_name], section
        else:
            rep.add("BAD_PLACEMENT", f["id"], page_name, f"unknown page '{page_name}'")
            page = None
            sec = ""
        if page is not None and not page.exists:
            rep.add("NOT_SHOWN", f["id"], page_name, f"page file not built: {os.path.relpath(page.path, SITE_NEXT)}")
            st["state"] = "NOT_SHOWN"
            page = None
        if page is not None and sec and sec not in page.ids:
            rep.add("BAD_PLACEMENT", f["id"], page_name, f"section anchor #{sec} does not exist on {page.name}")
        # hits everywhere
        strong_elsewhere, weak_elsewhere = set(), set()
        state = "NOT_SHOWN"
        for pg in all_pages:
            hits = find_hits(pg, probes)
            if not hits:
                continue
            is_placed = page is not None and pg is page
            strong_in = [t for t, s in hits if s == "strong" and (not is_placed or page.in_section(t, sec))]
            strong_any = [t for t, s in hits if s == "strong"]
            if is_placed:
                if strong_in:
                    state = "SHOWN"
                elif strong_any:
                    state = "OTHER_SECTION"
                    st["other_section"] = sorted({where(pg, t) for t in strong_any})[:4]
                elif any(s == "weak" for _, s in hits):
                    state = "WEAK"
                else:
                    state = "HIDDEN"
            else:
                if strong_any:
                    strong_elsewhere.add(where(pg, strong_any[0]) if pg.name.startswith("Docs/") else pg.name)
                else:
                    weak_elsewhere.add(pg.name)
        st["shown_on"] = sorted(strong_elsewhere)
        st["weak_on"] = sorted(weak_elsewhere)
        if page is None and st["state"] == "NOT_SHOWN":
            continue
        st["state"] = state
        if state == "HIDDEN":
            rep.add("NOT_SHOWN", f["id"], page_name, f"on {page_name}#{section} only inside attributes or data (a tooltip, search index or link), never as visible text",
                    section=section, category=f["category"], shown_on=st["shown_on"], hidden=True)
        elif state == "NOT_SHOWN":
            extra = f"; shown instead on {', '.join(st['shown_on'][:5])}" if st["shown_on"] else ""
            extra += f"; only weakly on {', '.join(st['weak_on'][:3])}" if st["weak_on"] and not st["shown_on"] else ""
            rep.add("NOT_SHOWN", f["id"], page_name, f"not mentioned on {page_name}{('#' + section) if section else ''}{extra}",
                    section=section, category=f["category"], shown_on=st["shown_on"])
        elif state == "WEAK":
            rep.add("WEAK", f["id"], page_name, f"only as an ordinary word in prose on {page_name}#{section} (an ambiguous identifier needs code styling or an exact label)",
                    section=section, category=f["category"])
        elif state == "OTHER_SECTION":
            rep.add("OTHER_SECTION", f["id"], page_name, f"shown on {page_name} but not in #{section} (found in {', '.join(st['other_section'])})",
                    section=section, category=f["category"])
        # secondary placements
        for a in p.get("also", []):
            ap_, _, asec = a.partition("#")
            apage = pages.get(ap_)
            if apage is None or not apage.exists:
                rep.add("ALSO", f["id"], ap_, f"secondary placement {a}: page not built")
                continue
            if asec and asec not in apage.ids:
                rep.add("ALSO", f["id"], ap_, f"secondary placement {a}: anchor missing")
                continue
            if not any(s == "strong" and apage.in_section(t, asec) for t, s in find_hits(apage, probes)):
                rep.add("ALSO", f["id"], ap_, f"secondary placement {a}: not shown there")
        # Studio control
        sv = p.get("studio", "")
        if sv and sv != "none":
            if sv.startswith("#"):
                if not studio_page.exists or sv[1:] not in studio_page.ids:
                    rep.add("BAD_PLACEMENT", f["id"], "Studio", f"Studio control {sv} does not exist in the Studio page")
            else:
                if sv not in studio_files:
                    rep.add("BAD_PLACEMENT", f["id"], "Studio", f"Studio scenario '{sv}' is not in the Studio library")
                elif sv in studio_nib:
                    rep.add("BAD_PLACEMENT", f["id"], "Studio", f"Studio scenario '{sv}' is marked command-line only in the Studio")
                elif not os.path.exists(os.path.join(args.studio, "scenarios", sv + ".toml")):
                    rep.add("BAD_PLACEMENT", f["id"], "Studio", f"Studio scenario file scenarios/{sv}.toml is missing")
        elif sv == "":
            rep.add("BAD_PLACEMENT", f["id"], "Studio", "no studio field (write 'none' when nothing in the Studio runs it)")
        if sv == "none" and (f["category"] == "kind" or (f["category"] == "scenario" and f["facts"].get("bundled"))):
            rep.add("NO_STUDIO", f["id"], "Studio", "nothing in the Studio runs this" + (" (not in the Studio library)" if f["name"] not in studio_files else ""),
                    category=f["category"])
        # docs anchor
        dv = p.get("docs", "")
        if dv and dv != "none":
            dfile, _, danchor = dv.partition("#")
            dp = docs_pages.get(os.path.basename(dfile))
            if dp is None:
                rep.add("BAD_PLACEMENT", f["id"], "Docs", f"docs anchor {dv}: page not built")
            elif danchor and danchor not in dp.ids:
                rep.add("BAD_PLACEMENT", f["id"], "Docs", f"docs anchor {dv}: anchor #{danchor} missing")
        elif dv == "":
            rep.add("BAD_PLACEMENT", f["id"], "Docs", "no docs field (write 'none' when no docs page covers it)")
        # Pro link symmetry (warning)
        pro_link = p.get("pro_link")
        if pro_link and page is not None and page.exists:
            if pro_link not in ids:
                rep.add("BAD_PLACEMENT", f["id"], page_name, f"pro_link {pro_link} is not a register row")
            else:
                rx = re.compile(r"(?<![A-Za-z])(Pro|custom study|Request a study)(?![A-Za-z])")
                if not any(t.ctx in ("text", "code") and page.in_section(t, sec) and rx.search(t.text) for t in page.texts):
                    rep.add("PRO_LINK", f["id"], page_name, f"{pro_link} extends this capability, but {page_name}#{section} never mentions Pro or a study",
                            section=section, category=f["category"])
        for a in f.get("unexpanded_abbreviations", []):
            rep.add("ABBREVIATION", f["id"], "", f"description keeps the abbreviation {a} unexpanded")

    # ---- 4. EDITION on the site: claims inside edition-marked containers
    claim_rows = [f for f in feats if f.get("claim")]
    edition_of_label = {"pro": "Pro", "custom": "Custom study", "commercial licence": "Free core", "free": "Free core", "open core": "Free core"}
    seen_claims = set()
    for pg in list(pages.values()):
        if not pg.exists:
            continue
        for t in pg.texts:
            if t.ctx != "text":
                continue
            cont, cont_ed = container_edition(t.parent)
            block = nearest_block(t.parent)
            if block is None:
                continue
            key0 = (pg.name, id(block))
            if key0 in seen_claims:
                continue
            seen_claims.add(key0)
            text = block_text(block)
            units = split_labelled(text)
            for label, unit in units:
                if label:
                    ed = edition_of_label[label.lower()]
                    ctx_label = f"labelled '{label}:'"
                elif cont_ed and (block.tag in CLAIM_TAGS or block.tag == "p"):
                    ed = cont_ed
                    ctx_label = f"inside the {cont_ed} container <{cont.tag} {('id=' + cont.id) if cont.id else 'class=' + ' '.join(cont.classes)}>"
                else:
                    continue
                matched = [r for r in claim_rows if re.search(r["claim"], unit, re.I)]
                if not matched and label and label.lower() == "pro":
                    rep.add("EDITION", "(site claim)", pg.name, f"'{unit[:120]}' is presented as Pro but matches no Pro row with a public source",
                            where=where(pg, t))
                for r in matched:
                    if r["edition"] != ed:
                        rep.add("EDITION", r["id"], pg.name,
                                f"'{unit[:110]}' is presented as {ed} ({ctx_label}); the public source makes it {r['edition']}: {r['edition_source'][:140]}",
                                where=where(pg, t), shown_as=ed, source_edition=r["edition"])

    # ---- 5. UNREGISTERED identifiers on the site
    def token_sets(rg, kt):
        fs_ = rg["features"]
        kinds_ = {f["name"] for f in fs_ if f["category"] == "kind"}
        scen_ = {f["name"] for f in fs_ if f["category"] == "scenario"}
        tools_ = {f["name"] for f in fs_ if f["category"] == "mcp-tool"}
        known_ = (set(json.load(open(kt))["tokens"]) if os.path.exists(kt) else set()) | ignore_tokens
        for f in fs_:
            known_.update(f.get("identifiers", []))
            known_.update(t for k, t in f["probes"] if k in ("ident", "ident_cs"))
        known_ |= kinds_ | scen_ | tools_
        cli_ = {f["name"] for f in fs_ if f["category"] == "cli"} | {"--json"} | ignore_tokens
        # The site reads the MCP server's tools from src/data/sources.json: mcp_repo while the branch
        # that adds tools is unmerged (the rule build.py and gen_features.py use); the docs pages
        # are rendered from their own checkout.
        mrepo_ = rg["engine"]["repo"]
        cfg_ = os.path.join(DEFAULT_SITE, "src", "data", "sources.json")
        if rg is reg and os.path.exists(cfg_):
            mrepo_ = os.path.expanduser(os.environ.get("KSHANA_MCP_REPO") or json.load(open(cfg_, encoding="utf-8")).get("mcp_repo") or mrepo_)
        srv_ = os.path.join(mrepo_, "mcp/kshana-mcp/src/server.rs")
        if not os.path.exists(srv_):
            srv_ = os.path.join(rg["engine"]["repo"], "mcp/kshana-mcp/src/server.rs")
        # The tool functions live in server.rs and, since 0.33.0 (the IQ tools), in iq.rs.
        mcp_src_ = "".join(open(f).read() + "\n" for f in (srv_, os.path.join(os.path.dirname(srv_), "iq.rs")) if os.path.exists(f))
        mcp_ = set(re.findall(r"\b([a-z][a-z0-9]*(?:_[a-z0-9]+)+)\b", mcp_src_))
        return kinds_, scen_, tools_, known_, cli_, mcp_
    site_sets = token_sets(reg, os.path.join(os.path.dirname(os.path.abspath(args.features)), "known-tokens.json"))
    docs_sets = token_sets(dreg, dreg.get("_known_tokens", os.path.join(os.path.dirname(os.path.abspath(args.features)), "known-tokens.json"))) if dreg is not reg else site_sets
    kinds, scen, tools, known, cli_flags, mcp_fields = site_sets
    toml_ok = {"Cargo", "pyproject", "scenario", "suite", "rust-toolchain", "rustfmt", "deny", "config", "my-scenario", "your-scenario"} | ignore_tokens
    seen_unreg = set()

    def unreg(kind, tok, pg, t, why):
        key = (kind, tok, pg.name)
        if key in seen_unreg:
            return
        seen_unreg.add(key)
        rep.add("UNREGISTERED", tok, pg.name, f"{why} `{tok}` is not in the register", where=where(pg, t), token_kind=kind)

    for pg in all_pages:
        if not pg.exists:
            continue
        kinds, scen, tools, known, cli_flags, mcp_fields = docs_sets if pg.name.startswith("Docs/") else site_sets
        for t in pg.texts:
            if t.ctx == "attr":
                owner = t.parent
                for k in ("href", "src", "data-scenario", "data-kind"):
                    v = owner.attrs.get(k)
                    if not v or v != t.text:
                        continue
                    if k in ("href", "src"):
                        qs = parse_qs(urlparse(htmlmod.unescape(v)).query)
                        for s in qs.get("scenario", []):
                            s2 = s.removesuffix(".toml")
                            if s2 not in scen:
                                unreg("scenario", s2, pg, t, "link opens scenario")
                        for s in qs.get("kind", []):
                            if s not in kinds:
                                unreg("kind", s, pg, t, "link names kind")
                    elif k == "data-scenario" and v.removesuffix(".toml") not in scen:
                        unreg("scenario", v.removesuffix(".toml"), pg, t, "data-scenario")
                    elif k == "data-kind" and v not in kinds:
                        unreg("kind", v, pg, t, "data-kind")
                continue
            if t.ctx == "studio-data":
                for m in re.finditer(r'\[\s*"([a-z0-9.\-]+)\.toml"\s*,', t.text):
                    if m.group(1) not in scen:
                        unreg("scenario", m.group(1), pg, t, "Studio library lists scenario")
                continue
            if t.ctx not in ("text", "code"):
                continue
            s = t.text
            for m in re.finditer(r"(?<![A-Za-z0-9_\-/<])([a-z0-9][a-z0-9\-]*(?:\.suite)?)\.toml\b", s):
                if m.group(1) not in scen and m.group(1) not in toml_ok:
                    unreg("scenario", m.group(1), pg, t, "scenario file")
            if t.ctx == "code":
                # only a TOP-LEVEL `kind = "..."` names a scenario kind; one under a [table]
                # header (e.g. [attack] kind = "meaconing") is a field of that table
                # header (e.g. [attack] kind = "meaconing") is a field of that table. TOML forbids a
                # duplicate top-level key, so a second `kind =` line in one snippet is a table field
                # even when the snippet was printed without its [table] headers.
                in_table = False
                seen_kind = False
                for line in s.splitlines():
                    if re.match(r"\s*\[", line):
                        in_table = True
                    m = re.match(r"\s*kind\s*=\s*\"([a-z0-9\-]+)\"", line)
                    if m and not in_table and not seen_kind:
                        seen_kind = True
                        if m.group(1) not in kinds:
                            unreg("kind", m.group(1), pg, t, "scenario kind")
            if t.ctx == "code":
                tok = s.strip()
                if re.fullmatch(r"[a-z][a-z0-9]*(?:-[a-z0-9]+)+", tok) or re.fullmatch(r"[a-z][a-z0-9]*(?:_[a-z0-9]+)+", tok):
                    in_mcp = pg.name == "Docs/mcp.html" or (pg.name == "Developers" and pg.in_section(t, "mcp"))
                    if in_mcp and "_" in tok and tok not in tools and tok not in mcp_fields:
                        unreg("MCP tool", tok, pg, t, "MCP tool or field")
                    elif tok not in known:
                        unreg("code identifier", tok, pg, t, "code identifier")
                # options of a command whose FIRST word is kshana (after an optional "$ " prompt)
                for seg in re.split(r"[\n|;&]+", s):
                    if not re.match(r"\s*(?:\$\s*)?kshana\s", seg):
                        continue
                    for m in re.finditer(r"(?<![\w-])(--[a-z][a-z0-9-]+)", seg):
                        if m.group(1) not in cli_flags:
                            unreg("cli option", m.group(1), pg, t, "command-line option")

    # ---- 6. COUNT_DRIFT
    def count_checks(rg):
        fs_ = rg["features"]
        k_ = sum(1 for f in fs_ if f["category"] == "kind")
        return [
            (r"\b(\d+) scenario kinds\b", k_, "scenario kinds"),
            (r"\b(\d+) bundled scenario", sum(1 for f in fs_ if f["category"] == "scenario" and f["facts"].get("bundled")), "bundled scenarios"),
            (r"\bAll (\d+) scenario files\b", sum(1 for f in fs_ if f["category"] == "scenario"), "scenario files"),
            (r"\b(\d+) (?:MCP )?tools\b", sum(1 for f in fs_ if f["category"] == "mcp-tool"), "MCP tools"),
            (r"\bof (\d+) capabilities\b", sum(1 for f in fs_ if f["category"] == "validation"), "capabilities in the verification matrix"),
            (r"\b(\d+) of \d+ capabilities\b", sum(1 for f in fs_ if f["category"] == "validation" and f["facts"].get("status") == "VALIDATED"), "validated capabilities"),
        ]
    site_checks, docs_checks = count_checks(reg), count_checks(dreg)
    seen_cd = set()
    for pg in list(pages.values()) + list(docs_pages.values()):
        if not pg.exists:
            continue
        checks = docs_checks if pg.name.startswith("Docs/") else site_checks
        for t in pg.texts:
            if t.ctx not in ("text", "code"):
                continue
            s = re.sub(r"\s+", " ", t.text)
            for rx, want, what in checks:
                for m in re.finditer(rx, s):
                    got = int(m.group(1))
                    if got != want and (pg.name, what, got) not in seen_cd:
                        seen_cd.add((pg.name, what, got))
                        rep.add("COUNT_DRIFT", what, pg.name, f"page says {got} {what}; the register has {want}", where=where(pg, t))

    # ---- 7. edition presentation matrix (information for GAPS)
    matrix = presentation_matrix(pages)

    errors = [i for i in rep.items if i["cls"] in ERROR_CLASSES]
    warns = [i for i in rep.items if i["cls"] in WARN_CLASSES]
    result = {
        "site": os.path.relpath(args.site, SITE_NEXT), "studio": os.path.relpath(args.studio, SITE_NEXT),
        "engine": reg["engine"], "counts": reg["counts"], "total": reg["total"],
        "summary": {c: len(rep.of(c)) for c in ERROR_CLASSES + WARN_CLASSES},
        "status": status, "items": rep.items, "presentation": matrix,
        "studio_library": sorted(studio_files), "studio_cli_only": sorted(studio_nib),
    }
    if args.json:
        json.dump(result, open(args.json, "w", encoding="utf-8"), indent=1, ensure_ascii=False)
    if args.gaps:
        write_gaps_block(args.gaps, result, feats)
    print_report(result, errors, warns, args.quiet)
    return 1 if errors else 0


# --------------------------------------------------------------------------- edition containers

def container_edition(node):
    """The nearest ancestor that marks an edition, and which edition it marks."""
    n = node
    while n is not None and n.tag != "#root":
        toks = set(n.classes)
        if n.id in ("pro", "pro-band") or "pro" in toks or any(c.startswith("pro-band") or c.endswith("-pro") for c in toks):
            return n, "Pro"
        if "free" in toks or any(c.endswith("-free") for c in toks):
            return n, "Free core"
        if toks & {"ed", "tier", "edition", "plan", "card"}:
            h = first_heading_text(n)
            if re.search(r"\bopen core\b|\bfree\b", h, re.I):
                return n, "Free core"
            if re.search(r"\bcustom\b|\bstud(y|ies)\b", h, re.I):
                return n, "Custom study"
            if re.search(r"\bpro\b", h, re.I):
                return n, "Pro"
        n = n.parent
    return None, None


def first_heading_text(n):
    stack = list(n.children)
    while stack:
        c = stack.pop(0)
        if isinstance(c, Node):
            if re.fullmatch(r"h[1-6]", c.tag):
                return block_text(c)
            stack = list(c.children) + stack
    return ""


def nearest_block(node):
    n = node
    while n is not None and n.tag != "#root":
        if n.tag in BLOCK_TAGS or n.tag in CLAIM_TAGS:
            # prefer the enclosing block over an inline strong/b
            if n.tag in ("strong", "b") and n.parent is not None and n.parent.tag in BLOCK_TAGS:
                return n.parent
            return n
        n = n.parent
    return None


_text_cache: dict[int, str] = {}


def block_text(n) -> str:
    k = id(n)
    if k in _text_cache:
        return _text_cache[k]
    out = []

    def walk(x):
        for c in x.children:
            if isinstance(c, Node):
                if c.tag in ("script", "style", "svg"):
                    continue
                walk(c)
            elif c.ctx in ("text", "code"):
                out.append(c.text)
    walk(n)
    txt = re.sub(r"\s+", " ", " ".join(out)).strip()
    _text_cache[k] = txt
    return txt


def split_labelled(text: str):
    parts = re.split(r"(?:(?<=^)|(?<=[.;:]\s))(Pro|Custom|Commercial licence|Free|Open core):\s", text)
    units = []
    if parts and parts[0].strip():
        units.append(("", parts[0].strip()))
    for i in range(1, len(parts) - 1, 2):
        units.append((parts[i], parts[i + 1].strip()))
    return units


def presentation_matrix(pages):
    rx = {
        "Free core": re.compile(r"\b(open core|free|AGPL-3\.0)\b", re.I),
        "Pro": re.compile(r"(?<![A-Za-z])(Kshana Pro|Pro)(?![A-Za-z])"),
        "Custom study": re.compile(r"\b(custom work|custom study|request a study|studies)\b", re.I),
        "contact route": re.compile(r"mailto:|contact@ashforde\.org", re.I),
    }
    out = {}
    for name, pg in pages.items():
        if not pg.exists:
            out[name] = {"built": False}
            continue
        row = {"built": True}
        for k, r in rx.items():
            row[k] = sum(1 for t in pg.texts if t.ctx in ("text", "code", "attr") and r.search(t.text))
        # hero = the first <section> (or #top)
        hero = [t for t in pg.texts if t.ctx == "text" and in_first_section(t)]
        row["Pro in hero"] = any(rx["Pro"].search(t.text) for t in hero)
        out[name] = row
    return out


def in_first_section(t):
    n = t.parent
    while n is not None:
        if n.tag in ("section", "header") and n.parent is not None:
            sibs = [c for c in n.parent.children if isinstance(c, Node) and c.tag in ("section", "header")]
            return bool(sibs) and sibs[0] is n and n.tag == "section" or (n.id in ("top",))
        n = n.parent
    return False


# ------------------------------------------------------------------------------------ reporting

def print_report(result, errors, warns, quiet):
    s = result["summary"]
    print(f"Kshana feature register check: {result['total']} features, site {result['site']}, Studio {result['studio']}")
    print("  errors:   " + "  ".join(f"{c}={s[c]}" for c in ERROR_CLASSES))
    print("  warnings: " + "  ".join(f"{c}={s[c]}" for c in WARN_CLASSES))
    if quiet:
        return
    for cls in ERROR_CLASSES + WARN_CLASSES:
        items = [i for i in result["items"] if i["cls"] == cls]
        if not items:
            continue
        print(f"\n== {cls} ({len(items)})")
        byp = {}
        for i in items:
            byp.setdefault(i["page"] or "-", []).append(i)
        for pg, lst in sorted(byp.items()):
            print(f"  [{pg}] {len(lst)}")
            for i in lst[: (400 if cls in ERROR_CLASSES else 25)]:
                print(f"    {i['id']}: {i['msg']}")
            if len(lst) > (400 if cls in ERROR_CLASSES else 25):
                print(f"    ... {len(lst) - 25} more")
    print("\nFAIL" if errors else "\nPASS")


GAPS_BEGIN = "<!-- BEGIN GENERATED: check_features.py --gaps -->"
GAPS_END = "<!-- END GENERATED -->"


def recommend(i, feat):
    cat = feat["category"] if feat else ""
    sec = i.get("section", "")
    name = feat["name"] if feat else i["id"]
    shown = i.get("shown_on") or []
    if i["cls"] == "NOT_SHOWN":
        if cat == "kind":
            return f"Add a card for `{name}` to Capabilities#{sec} with its one-line description, its tier and a Run link to the Studio{(' (shown today on ' + ', '.join(shown[:2]) + ')') if shown else ''}."
        if cat == "scenario":
            return f"Add `{name}` as a node on the {i['page']}#{sec} map with a Run link{'' if feat['facts'].get('bundled') else ' (command line only: say so)'}."
        if cat == "validation":
            return "Render this verification-matrix row in the ledger (the ledger must list every row the matrix has)."
        if cat in ("cli", "export"):
            return f"List `{name}` in the Developers#{sec} table (what it does, one copyable command)."
        if cat == "mcp-tool":
            return f"List the `{name}` tool on Developers#{sec}."
        if cat == "binding":
            return f"Name `{name}` on {i['page']}#{sec} with a one-line description."
        if cat in ("clock", "band", "body"):
            return f"Show `{name}` on {i['page']}#{sec}, with the Studio scenario that exercises it."
        if cat in ("pro", "study"):
            return f"State this offer on {i['page']}#{sec} in the words of its public source, with a contact button."
    return ""


def write_gaps_block(path, result, feats):
    fmap = {f["id"]: f for f in feats}
    L = [GAPS_BEGIN, ""]
    s = result["summary"]
    L.append(f"_Checked site `{result['site']}` and Studio `{result['studio']}` against {result['total']} register rows "
             f"(engine v{result['engine']['version']} at {result['engine']['git_head']})._")
    L.append("")
    L.append("| Class | Count |")
    L.append("|---|---:|")
    for c in ERROR_CLASSES + WARN_CLASSES:
        L.append(f"| {c}{' (error)' if c in ERROR_CLASSES else ' (warning)'} | {s[c]} |")
    L.append("")
    L.append("### Edition presentation per page")
    L.append("")
    L.append("Counts are text fragments mentioning each edition; 'Pro in hero' is whether the first section names Pro.")
    L.append("")
    L.append("| Page | Free core | Pro | Custom study | Contact route | Pro in hero |")
    L.append("|---|---:|---:|---:|---:|---|")
    for pg, row in result["presentation"].items():
        if not row.get("built"):
            L.append(f"| {pg} | not built | | | | |")
            continue
        L.append(f"| {pg} | {row['Free core']} | {row['Pro']} | {row['Custom study']} | {row['contact route']} | {'yes' if row['Pro in hero'] else 'no'} |")
    L.append("")
    for cls in ["EDITION", "COUNT_DRIFT", "UNREGISTERED", "BAD_PLACEMENT", "UNPLACED", "NOT_SHOWN", "PRO_LINK", "OTHER_SECTION", "WEAK", "ALSO"]:
        items = [i for i in result["items"] if i["cls"] == cls]
        if not items:
            continue
        L.append(f"### {cls} ({len(items)})")
        L.append("")
        byp = {}
        for i in items:
            byp.setdefault(i["page"] or "-", []).append(i)
        for pg, lst in sorted(byp.items()):
            L.append(f"**{pg}** ({len(lst)})")
            L.append("")
            if cls == "NOT_SHOWN":
                bycat = {}
                for i in lst:
                    bycat.setdefault(fmap.get(i["id"], {}).get("category", "?"), []).append(i)
                for cat, cl in sorted(bycat.items()):
                    L.append(f"- _{cat}_ ({len(cl)}):")
                    for i in cl:
                        rec = recommend(i, fmap.get(i["id"]))
                        L.append(f"  - `{i['id']}`: {i['msg']}. **Do:** {rec}" if rec else f"  - `{i['id']}`: {i['msg']}")
            else:
                for i in lst:
                    w = f" ({i['where']})" if i.get("where") else ""
                    L.append(f"- `{i['id']}`{w}: {i['msg']}")
            L.append("")
    L.append(GAPS_END)
    block = "\n".join(L)
    if os.path.exists(path):
        cur = open(path, encoding="utf-8").read()
        if GAPS_BEGIN in cur and GAPS_END in cur:
            cur = cur[: cur.find(GAPS_BEGIN)] + block + cur[cur.find(GAPS_END) + len(GAPS_END):]
        else:
            cur = cur.rstrip() + "\n\n" + block + "\n"
    else:
        cur = "# GAPS\n\n" + block + "\n"
    open(path, "w", encoding="utf-8").write(cur)


if __name__ == "__main__":
    sys.exit(main())
