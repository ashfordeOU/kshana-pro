#!/usr/bin/env python3
# Roadmap 0.42 X8: the "In Arksa" copy check (owner decision, 15 November 2026).
#
#   scripts/protect/arksa-copy-check.py [--scope kpro|open|site] [--dir <checkout>] [--terms <file>]
#
# Marketing and documentation may say that something is available "in Arksa" only for these
# four outcome names, with names and outcomes only (no method, equation, parameter or
# diagram before filing):
#
#   Mission navigation digital twin
#   Lunar navigation and timing assurance
#   Independent interference referee reports
#   Spaceborne GNSS receiver interference assurance
#
# The check fails (exit 1) on
#   * an "in Arksa" claim (in / inside / within / only in / only within Arksa) whose sentence
#     names none of the four outcomes, and
#   * any term of protect/private-terms.txt (every section) within WINDOW characters of any
#     "in Arksa" claim, allowed or not: method words never sit next to an Arksa claim.
#
# Not claims: the repository name (Arksa followed by - or _), the label itself in quotes
# ("In Arksa"), and the neutral refusal text of the licence gate ("it is provided only
# within Arksa"). Findings print as "<path>:<line>: <reason>", with term IDs (the first 12
# hex characters of the SHA-256 of the term), never the term, so the output is safe for an
# open repository's CI log. scripts/protect/term-id.sh looks an ID up locally.
#
# Scopes: kpro (this repository's marketing and docs: docs/, site-next/, commercial/offers/,
# brand/, report-templates/ and the top-level *.md), open (the open repository: every
# tracked .md, .mdx, .html, .txt, .rst, .adoc, .yml file and the site/ and docs/ trees),
# site (a website source tree: every tracked text file of a web type).
import argparse
import hashlib
import os
import re
import subprocess
import sys

ALLOWED = [
    "Mission navigation digital twin",
    "Lunar navigation and timing assurance",
    "Independent interference referee reports",
    "Spaceborne GNSS receiver interference assurance",
]
WINDOW = 300

CLAIM = re.compile(r"\b(?:only\s+)?(?:in|inside|within)\s+Arksa\b(?![-_\w])", re.IGNORECASE)
NEUTRAL = re.compile(r"provided\s+only\s+within\s+Arksa", re.IGNORECASE)
QUOTED = re.compile(r"[\"“'`]\s*(?:only\s+)?(?:in|inside|within)\s+Arksa\s*[\"”'`]", re.IGNORECASE)

TEXT_EXT = {
    ".md", ".mdx", ".markdown", ".html", ".htm", ".txt", ".rst", ".adoc", ".yml", ".yaml",
    ".json", ".toml", ".astro", ".tsx", ".jsx", ".ts", ".js", ".vue", ".svelte", ".njk",
    ".liquid", ".csv", ".xml", ".svg",
}
SCOPES = {
    "kpro": {
        "dirs": ["docs", "site-next", "commercial/offers", "brand", "report-templates"],
        "top_md": True,
        "ext": {".md", ".mdx", ".html", ".htm", ".txt", ".astro", ".tsx", ".jsx", ".json", ".yml", ".yaml"},
    },
    "open": {"dirs": None, "top_md": True, "ext": {".md", ".mdx", ".html", ".htm", ".txt", ".rst", ".adoc", ".yml", ".yaml"}},
    "site": {"dirs": None, "top_md": True, "ext": TEXT_EXT},
}
# Files of this repository that describe the rule itself (they quote the label and the list).
SELF = {"scripts/protect/arksa-copy-check.py"}


def term_id(t):
    return hashlib.sha256(t.encode("utf-8")).hexdigest()[:12]


def load_terms(path):
    out = []
    with open(path, encoding="utf-8") as f:
        for line in f:
            s = line.rstrip("\n")
            if not s.strip() or s.lstrip().startswith("#") or (s.startswith("[") and s.endswith("]")):
                continue
            out.append(s)
    return out


def tracked(root):
    try:
        r = subprocess.run(["git", "-C", root, "ls-files", "-z"], capture_output=True, check=True)
        return [p for p in r.stdout.decode("utf-8", "replace").split("\0") if p]
    except (subprocess.CalledProcessError, FileNotFoundError):
        files = []
        for d, dirs, names in os.walk(root):
            dirs[:] = [x for x in dirs if not x.startswith(".") and x not in ("node_modules", "target")]
            for n in names:
                files.append(os.path.relpath(os.path.join(d, n), root))
        return files


def in_scope(path, scope):
    cfg = SCOPES[scope]
    p = path.replace(os.sep, "/")
    if p in SELF:
        return False
    ext = os.path.splitext(p)[1].lower()
    if ext not in cfg["ext"]:
        return False
    if cfg["dirs"] is None:
        return True
    if cfg["top_md"] and "/" not in p and ext == ".md":
        return True
    return any(p == d or p.startswith(d + "/") for d in cfg["dirs"])


def sentence_bounds(text, start, end):
    # A claim's sentence: back to the previous sentence end or blank line, forward to the
    # next one (headings and table cells end at a line break).
    lo = start
    while lo > 0 and not (text[lo - 1] in ".!?" and (lo >= len(text) or text[lo].isspace())) \
            and text[lo - 1] != "\n" and text[lo - 1] != "|":
        lo -= 1
    hi = end
    while hi < len(text) and text[hi] not in ".!?\n|":
        hi += 1
    # Let a claim sentence continue onto the next line until a blank line or sentence end.
    while hi < len(text) and text[hi] == "\n" and hi + 1 < len(text) and text[hi + 1] not in "\n#|-*":
        hi += 1
        while hi < len(text) and text[hi] not in ".!?\n|":
            hi += 1
    return lo, hi


def check_text(text, terms):
    """Findings for one text: (offset, reason)."""
    found = []
    neutral = [m.span() for m in NEUTRAL.finditer(text)]
    quoted = [m.span() for m in QUOTED.finditer(text)]
    low_allowed = [a.lower() for a in ALLOWED]
    for m in CLAIM.finditer(text):
        s, e = m.span()
        if any(a <= s and e <= b for a, b in neutral + quoted):
            continue
        lo, hi = sentence_bounds(text, s, e)
        # The outcome may also sit in the heading or line just before (a list under "In Arksa:").
        context = text[lo:hi].lower()
        prev_line_start = text.rfind("\n", 0, max(lo - 1, 0))
        prev = text[prev_line_start + 1:lo].lower() if prev_line_start >= -1 else ""
        nxt_end = text.find("\n\n", hi)
        following = text[hi:nxt_end if nxt_end != -1 else min(len(text), hi + 400)].lower()
        if not any(a in context or a in prev or (text[e:e + 3].strip().startswith(":") and a in following) for a in low_allowed):
            found.append((s, "an \"in Arksa\" claim that names none of the four allowed outcomes"))
        w_lo, w_hi = max(0, s - WINDOW), min(len(text), e + WINDOW)
        window = text[w_lo:w_hi]
        for t in terms:
            if t and t in window:
                found.append((s, "private term %s within %d characters of an \"in Arksa\" claim" % (term_id(t), WINDOW)))
    return found


def line_of(text, offset):
    return text.count("\n", 0, offset) + 1


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    ap = argparse.ArgumentParser(description="X8 \"In Arksa\" copy check")
    ap.add_argument("--scope", choices=sorted(SCOPES), default="kpro")
    ap.add_argument("--dir", default=os.path.normpath(os.path.join(here, "..", "..")))
    ap.add_argument("--terms", default=os.environ.get("KSHANA_PRO_PRIVATE_TERMS",
                                                      os.path.join(here, "..", "..", "protect", "private-terms.txt")))
    a = ap.parse_args()
    if not os.path.isfile(a.terms):
        print("copy-check: no term list at %s" % a.terms, file=sys.stderr)
        return 2
    terms = load_terms(a.terms)
    n_files = 0
    findings = []
    for rel in tracked(a.dir):
        if not in_scope(rel, a.scope):
            continue
        p = os.path.join(a.dir, rel)
        try:
            with open(p, encoding="utf-8") as f:
                text = f.read()
        except (UnicodeDecodeError, OSError):
            continue
        n_files += 1
        if not CLAIM.search(text):
            continue
        for off, why in check_text(text, terms):
            findings.append("%s:%d: %s" % (rel, line_of(text, off), why))
    for f in sorted(set(findings)):
        print(f)
    if findings:
        print("copy-check: FAIL (%d finding(s) in %d file(s) of scope %s)" % (len(set(findings)), n_files, a.scope),
              file=sys.stderr)
        return 1
    print("copy-check: OK (%d file(s) of scope %s, %d private term(s))" % (n_files, a.scope, len(terms)),
          file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
