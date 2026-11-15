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
# X7, patent before disclosure: the same run also reads the trade-secret register
# (protect/trade-secret-register.toml, --register) and fails on any disclosure term of an
# item that is secret, still to be filed, or filed with a filing date after today (--today,
# default the UTC date), in any in-scope file, claim or not. Findings print the item ID and
# the term ID only. A malformed register (unknown status, bad date, a term ID not in the
# term list, an item listed twice) is an error (exit 2), so a broken register never passes
# silently.
#
# Scopes: kpro (this repository's marketing and docs: docs/, site-next/, commercial/offers/,
# brand/, report-templates/ and the top-level *.md), open (the open repository: every
# tracked .md, .mdx, .html, .txt, .rst, .adoc, .yml file and the site/ and docs/ trees),
# site (a website source tree: every tracked text file of a web type).
import argparse
import datetime
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


REGISTER_FORMAT = "kshana-pro-trade-secret-register/1"
STATUSES = {"secret", "to-file", "filed", "public"}
DATE = re.compile(r"^\d{4}-\d{2}-\d{2}$")


def load_register(path, terms):
    """The register's blocked items as [(item id, status, [term])]; raises ValueError."""
    import tomllib
    with open(path, "rb") as f:
        reg = tomllib.load(f)
    if reg.get("format") != REGISTER_FORMAT:
        raise ValueError("register format must be %s" % REGISTER_FORMAT)
    by_id = {term_id(t): t for t in terms}
    items, seen = [], set()
    for it in reg.get("item", []):
        iid = str(it.get("id", ""))
        if not re.match(r"^[A-Z][A-Z0-9-]{0,15}$", iid):
            raise ValueError("item id %r must be 1-16 of A-Z, 0-9, - starting with a letter" % iid)
        if iid in seen:
            raise ValueError("item %s listed twice" % iid)
        seen.add(iid)
        st = it.get("status")
        if st not in STATUSES:
            raise ValueError("item %s: unknown status %r" % (iid, st))
        fd = it.get("filing_date", "")
        if fd and not DATE.match(str(fd)):
            raise ValueError("item %s: filing_date must be YYYY-MM-DD" % iid)
        if fd:
            datetime.date.fromisoformat(str(fd))
        if st == "filed" and not fd:
            raise ValueError("item %s: a filed item needs its filing_date" % iid)
        item_terms = []
        for tid in it.get("term_ids", []):
            if tid not in by_id:
                raise ValueError("item %s: term ID %s is not in the term list" % (iid, tid))
            item_terms.append(by_id[tid])
        for t in it.get("terms", []):
            if not isinstance(t, str) or not t.strip():
                raise ValueError("item %s: empty term" % iid)
            item_terms.append(t)
        items.append((iid, st, str(fd), item_terms))
    return items


def blocked_items(items, today):
    """Items whose terms may not be published on `today` (an ISO date string)."""
    out = []
    for iid, st, fd, ts in items:
        if st == "public":
            continue
        if st == "filed" and fd <= today:
            continue
        out.append((iid, st, fd, ts))
    return out


def load_baseline(path):
    allowed = set()
    if path and os.path.isfile(path):
        with open(path, encoding="utf-8") as f:
            for line in f:
                if line.strip() and not line.startswith("#") and "\t" in line:
                    p, t = line.rstrip("\n").split("\t", 1)
                    allowed.add((p, t.strip()))
    return allowed


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
    ap.add_argument("--register", default=os.environ.get("KSHANA_PRO_SECRET_REGISTER",
                                                         os.path.join(here, "..", "..", "protect", "trade-secret-register.toml")))
    ap.add_argument("--today", default=datetime.datetime.now(datetime.timezone.utc).date().isoformat())
    a = ap.parse_args()
    if not os.path.isfile(a.terms):
        print("copy-check: no term list at %s" % a.terms, file=sys.stderr)
        return 2
    terms = load_terms(a.terms)
    blocked = []
    if os.path.isfile(a.register):
        try:
            # Term IDs resolve against the given list and this repository's own list (the
            # register belongs with it), so a run with another --terms still reads it.
            own = os.path.join(here, "..", "..", "protect", "private-terms.txt")
            pool = terms + (load_terms(own) if os.path.isfile(own) else [])
            blocked = blocked_items(load_register(a.register, pool), a.today)
        except (ValueError, OSError) as e:
            print("copy-check: trade-secret register: %s" % e, file=sys.stderr)
            return 2
    elif a.register != ap.get_default("register"):
        print("copy-check: no trade-secret register at %s" % a.register, file=sys.stderr)
        return 2
    baseline = load_baseline(os.path.join(here, "..", "..", "protect", "open-leak-baseline.txt")) \
        if a.scope == "open" else set()
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
        for iid, st, fd, ts in blocked:
            for t in ts:
                i = text.find(t)
                if i >= 0 and (rel, term_id(t)) not in baseline:
                    why = "secret" if st == "secret" else ("before its filing date %s" % fd if fd else "not yet filed")
                    findings.append("%s:%d: trade-secret register item %s (%s): term %s"
                                    % (rel, line_of(text, i), iid, why, term_id(t)))
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
    print("copy-check: OK (%d file(s) of scope %s, %d private term(s), %d register item(s) not yet disclosable)"
          % (n_files, a.scope, len(terms), len(blocked)), file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
