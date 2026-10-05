#!/usr/bin/env python3
"""Build a branded Ashforde Mutual NDA .docx from the markdown source.

Brand (from ashforde.org): gold #B6985A, ink #11161E / #212121, paper #F6F4F0,
serif display Cormorant Garamond (Georgia fallback), body EB Garamond (Georgia
fallback), wordmark "A S H F O R D E ®", positioning "Sovereign Engineering ·
High Assurance". Adds a CONFIDENTIAL diagonal watermark on every page.
"""
import re
from docx import Document
from docx.shared import Pt, RGBColor, Inches, Twips
from docx.enum.text import WD_ALIGN_PARAGRAPH, WD_TAB_ALIGNMENT
from docx.enum.table import WD_TABLE_ALIGNMENT
from docx.oxml.ns import qn
from docx.oxml import parse_xml, OxmlElement

SRC = "/home/user/Ashforde-Mutual-NDA.md"
OUT = "/home/user/Ashforde-Mutual-NDA.docx"

GOLD = RGBColor(0xB6, 0x98, 0x5A)
GOLD_DK = RGBColor(0x7D, 0x66, 0x36)
INK = RGBColor(0x21, 0x21, 0x21)
INK_DEEP = RGBColor(0x11, 0x16, 0x1E)
GREY = RGBColor(0x5F, 0x5A, 0x50)
SERIF = "Georgia"          # Cormorant/EB Garamond fallback, present everywhere
MONO = "Consolas"

doc = Document()

# ---- base styles -----------------------------------------------------------
normal = doc.styles["Normal"]
normal.font.name = SERIF
normal.font.size = Pt(10.5)
normal.font.color.rgb = INK
pf = normal.paragraph_format
pf.space_after = Pt(6)
pf.line_spacing = 1.12

h1 = doc.styles["Heading 1"]
h1.font.name = SERIF
h1.font.size = Pt(13)
h1.font.bold = True
h1.font.color.rgb = INK_DEEP
h1.paragraph_format.space_before = Pt(12)
h1.paragraph_format.space_after = Pt(4)
h1.paragraph_format.keep_with_next = True

# ---- page geometry (US Letter, 1" margins) ---------------------------------
sec = doc.sections[0]
sec.page_width = Inches(8.5)
sec.page_height = Inches(11)
sec.top_margin = Inches(1.05)
sec.bottom_margin = Inches(0.9)
sec.left_margin = Inches(1.0)
sec.right_margin = Inches(1.0)
sec.header_distance = Inches(0.5)
sec.footer_distance = Inches(0.4)


def set_spacing(run, twentieths):
    """Letter-spacing in twentieths of a point."""
    rpr = run._element.get_or_add_rPr()
    el = OxmlElement("w:spacing")
    el.set(qn("w:val"), str(twentieths))
    rpr.append(el)


def _get_pbdr(paragraph):
    """Return the paragraph's w:pBdr, inserting it in schema-correct order."""
    ppr = paragraph._p.get_or_add_pPr()
    existing = ppr.find(qn("w:pBdr"))
    if existing is not None:
        return existing
    pbdr = OxmlElement("w:pBdr")
    successors = ["w:shd", "w:tabs", "w:suppressAutoHyphens", "w:spacing",
                  "w:ind", "w:jc", "w:rPr", "w:sectPr", "w:pPrChange"]
    anchor = None
    for tag in successors:
        anchor = ppr.find(qn(tag))
        if anchor is not None:
            break
    if anchor is not None:
        anchor.addprevious(pbdr)
    else:
        ppr.append(pbdr)
    return pbdr


def bottom_border(paragraph, color="B6985A", size=12):
    pbdr = _get_pbdr(paragraph)
    bottom = OxmlElement("w:bottom")
    bottom.set(qn("w:val"), "single")
    bottom.set(qn("w:sz"), str(size))
    bottom.set(qn("w:space"), "4")
    bottom.set(qn("w:color"), color)
    pbdr.append(bottom)


def top_border(paragraph, color="B6985A", size=8):
    pbdr = _get_pbdr(paragraph)
    top = OxmlElement("w:top")
    top.set(qn("w:val"), "single")
    top.set(qn("w:sz"), str(size))
    top.set(qn("w:space"), "4")
    top.set(qn("w:color"), color)
    pbdr.append(top)


def add_field(paragraph, instr):
    """Insert a Word field (e.g. PAGE, NUMPAGES)."""
    r = paragraph.add_run()
    fld_begin = OxmlElement("w:fldChar"); fld_begin.set(qn("w:fldCharType"), "begin")
    instr_el = OxmlElement("w:instrText"); instr_el.set(qn("xml:space"), "preserve"); instr_el.text = instr
    fld_sep = OxmlElement("w:fldChar"); fld_sep.set(qn("w:fldCharType"), "separate")
    fld_end = OxmlElement("w:fldChar"); fld_end.set(qn("w:fldCharType"), "end")
    r._r.append(fld_begin); r._r.append(instr_el); r._r.append(fld_sep); r._r.append(fld_end)
    return r


# ---- header: wordmark + positioning, gold rule -----------------------------
header = sec.header
header.is_linked_to_previous = False
hp = header.paragraphs[0]
hp.text = ""
# tab stop at right margin so positioning sits flush right
tabs = hp.paragraph_format.tab_stops
tabs.add_tab_stop(Inches(6.5), WD_TAB_ALIGNMENT.RIGHT)
wm = hp.add_run("A S H F O R D E")
wm.font.name = SERIF; wm.font.size = Pt(13); wm.font.bold = True; wm.font.color.rgb = GOLD
set_spacing(wm, 20)
reg = hp.add_run("®")
reg.font.name = SERIF; reg.font.size = Pt(7); reg.font.bold = True; reg.font.color.rgb = GOLD
reg.font.superscript = True
tabrun = hp.add_run("\t")
pos = hp.add_run("Sovereign Engineering · High Assurance")
pos.font.name = SERIF; pos.font.size = Pt(8.5); pos.font.italic = True; pos.font.color.rgb = GREY
bottom_border(hp, "B6985A", 10)

# ---- watermark: diagonal CONFIDENTIAL on every page ------------------------
WATERMARK_XML = (
    '<w:p xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" '
    'xmlns:v="urn:schemas-microsoft-com:vml" '
    'xmlns:o="urn:schemas-microsoft-com:office:office">'
    '<w:pPr><w:jc w:val="center"/></w:pPr>'
    '<w:r><w:rPr><w:noProof/></w:rPr><w:pict>'
    '<v:shapetype id="_x0000_t136" coordsize="21600,21600" o:spt="136" adj="10800" '
    'path="m@7,l@8,m@5,21600l@6,21600e">'
    '<v:formulas><v:f eqn="sum #0 0 10800"/><v:f eqn="prod #0 2 1"/>'
    '<v:f eqn="sum 21600 0 @1"/></v:formulas>'
    '<v:path textpathok="t" o:connecttype="custom"/><v:textpath on="t" fitshape="t"/>'
    '</v:shapetype>'
    '<v:shape id="AshfordeWatermark" o:spid="_x0000_s2049" type="#_x0000_t136" '
    'style="position:absolute;left:0;text-align:left;margin-left:0;margin-top:0;'
    'width:520pt;height:130pt;rotation:315;z-index:-251656192;'
    'mso-position-horizontal:center;mso-position-horizontal-relative:margin;'
    'mso-position-vertical:center;mso-position-vertical-relative:margin" '
    'o:allowincell="f" fillcolor="#E3D9C4" stroked="f">'
    '<v:fill opacity="45875f"/>'
    '<v:textpath style="font-family:&quot;Georgia&quot;;font-size:1pt;font-weight:bold" '
    'string="CONFIDENTIAL"/>'
    '</v:shape></w:pict></w:r></w:p>'
)
import os
if os.environ.get("WM", "1") == "1":
    header._element.append(parse_xml(WATERMARK_XML))

# ---- footer: entity + contact, confidential + page x of y ------------------
footer = sec.footer
footer.is_linked_to_previous = False
fp = footer.paragraphs[0]
fp.text = ""
ftabs = fp.paragraph_format.tab_stops
ftabs.add_tab_stop(Inches(6.5), WD_TAB_ALIGNMENT.RIGHT)
top_border(fp, "B6985A", 6)
fl = fp.add_run("Ashforde OÜ   ·   contact@ashforde.org   ·   ashforde.org")
fl.font.name = SERIF; fl.font.size = Pt(8); fl.font.color.rgb = GREY
fp.add_run("\t")
fc = fp.add_run("Confidential   ·   Page ")
fc.font.name = SERIF; fc.font.size = Pt(8); fc.font.color.rgb = GREY
r1 = add_field(fp, "PAGE")
r1.font.name = SERIF; r1.font.size = Pt(8); r1.font.color.rgb = GREY
fof = fp.add_run(" of ")
fof.font.name = SERIF; fof.font.size = Pt(8); fof.font.color.rgb = GREY
r2 = add_field(fp, "NUMPAGES")
r2.font.name = SERIF; r2.font.size = Pt(8); r2.font.color.rgb = GREY

# ---- title block (page 1) --------------------------------------------------
tp = doc.add_paragraph()
tp.alignment = WD_ALIGN_PARAGRAPH.LEFT
tp.paragraph_format.space_before = Pt(10)
tr = tp.add_run("Mutual Non-Disclosure Agreement")
tr.font.name = SERIF; tr.font.size = Pt(24); tr.font.bold = True; tr.font.color.rgb = INK_DEEP

sub = doc.add_paragraph()
sr = sub.add_run("Between Ashforde OÜ and the Counterparty")
sr.font.name = SERIF; sr.font.size = Pt(11); sr.font.italic = True; sr.font.color.rgb = GOLD_DK
bottom_border(sub, "B6985A", 10)

meta = doc.add_paragraph()
meta.paragraph_format.space_before = Pt(4)
m1 = meta.add_run("CONFIDENTIAL")
m1.font.name = SERIF; m1.font.size = Pt(9); m1.font.bold = True; m1.font.color.rgb = GOLD_DK
set_spacing(m1, 30)
m2 = meta.add_run("      Dated: [Effective Date]      Prepared by Ashforde OÜ")
m2.font.name = SERIF; m2.font.size = Pt(9); m2.font.color.rgb = GREY

doc.add_paragraph()  # spacer


# ---- markdown body ---------------------------------------------------------
def add_runs(paragraph, text):
    parts = re.split(r"(\*\*.+?\*\*)", text)
    for part in parts:
        if part.startswith("**") and part.endswith("**"):
            r = paragraph.add_run(part[2:-2]); r.bold = True
        else:
            sub = re.split(r"(\*.+?\*)", part)
            for s in sub:
                if len(s) >= 2 and s.startswith("*") and s.endswith("*"):
                    r = paragraph.add_run(s[1:-1]); r.italic = True
                elif s:
                    paragraph.add_run(s)


with open(SRC, encoding="utf-8") as fh:
    lines = fh.read().split("\n")

# skip the markdown's own title (#...) and the byline line; we render our own block
start = 0
for idx, ln in enumerate(lines):
    if ln.strip().startswith("## "):
        start = idx
        break
lines = lines[start:]

i, n = 0, len(lines)
while i < n:
    stripped = lines[i].strip()
    if not stripped:
        i += 1
        continue

    if stripped.startswith("## "):
        doc.add_heading(stripped[3:], level=1)
        i += 1
        continue

    if stripped == "---":
        hr = doc.add_paragraph()
        bottom_border(hr, "B6985A", 6)
        i += 1
        continue

    if stripped.startswith("> "):
        p = doc.add_paragraph(style="Intense Quote")
        add_runs(p, stripped[2:])
        i += 1
        continue

    if stripped.startswith("|"):
        rows = []
        while i < n and lines[i].strip().startswith("|"):
            rows.append(lines[i].strip()); i += 1
        parsed = []
        for r in rows:
            cells = [c.strip() for c in r.strip().strip("|").split("|")]
            if all(set(c) <= set("-: ") and c for c in cells):
                continue
            parsed.append(cells)
        if parsed:
            ncol = max(len(r) for r in parsed)
            table = doc.add_table(rows=0, cols=ncol)
            table.style = "Table Grid"
            table.alignment = WD_TABLE_ALIGNMENT.LEFT
            for ridx, cells in enumerate(parsed):
                cells = cells + [""] * (ncol - len(cells))
                row_cells = table.add_row().cells
                for cidx, ctext in enumerate(cells):
                    para = row_cells[cidx].paragraphs[0]
                    add_runs(para, ctext)
                    for rr in para.runs:
                        rr.font.size = Pt(9.5)
                        if ridx == 0:
                            rr.bold = True; rr.font.color.rgb = INK_DEEP
        continue

    m = re.match(r"^(\d+)\.\s+(.*)$", stripped)
    if m:
        p = doc.add_paragraph(style="List Number")
        add_runs(p, m.group(2))
        i += 1
        continue

    if stripped.startswith("- "):
        p = doc.add_paragraph(style="List Bullet")
        add_runs(p, stripped[2:])
        i += 1
        continue

    p = doc.add_paragraph()
    add_runs(p, stripped)
    i += 1

# ---- fix stock-template zoom (missing required percent attr) ----------------
try:
    zoom = doc.settings.element.find(qn("w:zoom"))
    if zoom is not None and zoom.get(qn("w:percent")) is None:
        zoom.set(qn("w:percent"), "100")
except Exception:
    pass

doc.save(OUT)
print("Wrote", OUT)
