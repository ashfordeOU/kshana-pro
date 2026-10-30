#!/usr/bin/env python3
"""Helpers for make.sh: find each chapter's first page, check them, and write the PDF metadata."""
import json, os, re, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))


def pages(pdf):
    n = int(re.search(r"Pages:\s+(\d+)", subprocess.check_output(["pdfinfo", pdf], text=True)).group(1))
    txt = subprocess.check_output(["pdftotext", "-raw", pdf, "-"], text=True)
    return n, txt.split("\f")


def find(pdf):
    secs = json.load(open(os.path.join(HERE, ".sections.json")))
    n, pg = pages(pdf)
    toc = {}
    for cid, num, title in secs:
        mark = ("APPENDIX %s" % num) if not num.isdigit() else ("CHAPTER %s" % num)
        mark = mark.replace(" ", "")
        hit = [i + 1 for i, t in enumerate(pg) if mark in re.sub(r"\s+", "", t)]
        hit = [h for h in hit if h > 3]
        toc[cid] = hit[0] if hit else ""
    return toc


cmd = sys.argv[1]
if cmd == "toc":
    json.dump(find(sys.argv[2]), open(sys.argv[3], "w"))
elif cmd == "check":
    a, b = find(sys.argv[2]), json.load(open(sys.argv[3]))
    if a != b:
        sys.exit("contents page numbers moved between passes: %s" % {k: (b[k], a[k]) for k in a if a[k] != b[k]})
elif cmd == "meta":
    from pypdf import PdfReader, PdfWriter
    r = PdfReader(sys.argv[2])
    w = PdfWriter(clone_from=r)
    w.add_metadata({"/Title": "Kshana capability dossier v0.40.0", "/Author": "Ashforde OÜ",
                    "/Subject": "Public capability dossier of the open Kshana PNT-resilience engine, v0.40.0",
                    "/Keywords": "Kshana, PNT, GNSS, timing, holdover, lunar, cislunar, receiver trust, verification",
                    "/Creator": "Ashforde OÜ", "/Producer": "Ashforde OÜ"})
    w.compress_identical_objects()
    with open(sys.argv[3], "wb") as f:
        w.write(f)
