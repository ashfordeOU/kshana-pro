"""Render IQ-LAYER-VALIDATION-SUMMARY.md to a printable, self-contained HTML page.

    pip install markdown==3.9 && python render_iq_validation_summary.py
"""
import pathlib
import markdown

here = pathlib.Path(__file__).resolve().parent
src = (here / "IQ-LAYER-VALIDATION-SUMMARY.md").read_text(encoding="utf-8")
body = markdown.markdown(src, extensions=["tables"])
for mark, cls in (("[BUILT]", "built"), ("[TESTED]", "tested"), ("[PLANNED]", "planned")):
    body = body.replace(mark, f'<span class="mark {cls}">{mark[1:-1]}</span>')

CSS = """
:root { --ink:#0b0b0b; --ink2:#52514e; --rule:#d9d8d3; --bg:#ffffff; --tint:#f4f3ef;
        --built:#1c5cab; --tested:#0f7a3a; --planned:#8a5a00; }
@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) {
        --ink:#f4f3ef; --ink2:#c3c2b7; --rule:#3a3a37; --bg:#1a1a19; --tint:#242422;
        --built:#86b6ef; --tested:#5fd08f; --planned:#e8b44a; } }
* { box-sizing: border-box; }
body { margin:0; background:var(--bg); color:var(--ink);
       font:9.6pt/1.38 "Inter","Helvetica Neue",Arial,sans-serif; }
main { max-width: 190mm; margin: 0 auto; padding: 10mm 12mm; }
h1 { font-size: 15pt; margin: 0 0 2mm; letter-spacing: -0.01em; }
h2 { font-size: 11pt; margin: 4mm 0 1.5mm; padding-bottom: 0.8mm; border-bottom: 1px solid var(--rule); }
p, li { margin: 0.8mm 0; } ul { padding-left: 5mm; margin: 1mm 0; }
em { color: var(--ink2); }
code { font: 8.6pt "JetBrains Mono",Menlo,Consolas,monospace; background: var(--tint);
       padding: 0 0.6mm; border-radius: 2px; }
table { border-collapse: collapse; width: 100%; margin: 1.5mm 0; font-size: 8.8pt; }
th, td { text-align: left; vertical-align: top; padding: 1mm 1.6mm; border-bottom: 1px solid var(--rule); }
th { color: var(--ink2); font-weight: 600; }
.mark { display:inline-block; font: 600 7.2pt/1.5 "Inter",Arial,sans-serif; letter-spacing: .04em;
        padding: 0 1.2mm; border-radius: 2px; border: 1px solid currentColor; white-space: nowrap; }
.built { color: var(--built); } .tested { color: var(--tested); } .planned { color: var(--planned); }
@media (max-width: 640px) { main { padding: 16px; } table { display:block; overflow-x:auto; } }
@page { size: A4; margin: 7mm 8mm; }
@media print { :root { --ink:#000; --ink2:#444; --bg:#fff; --tint:#f1f1f1; }
               main { padding: 0; max-width: none; } body { font-size: 7.9pt; line-height: 1.3; }
               h1 { font-size: 12.5pt; margin-bottom: 1mm; } h2 { font-size: 9.6pt; margin: 2.4mm 0 1mm; }
               p, li { margin: 0.4mm 0; } th, td { padding: 0.6mm 1.2mm; } code { font-size: 7.4pt; }
               table { font-size: 7.4pt; } h2 { break-after: avoid; } tr, li { break-inside: avoid; } }
"""

html = f"""<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Kshana IQ Validation</title>
<style>{CSS}</style></head>
<body><main>
{body}
</main></body></html>
"""
(here / "IQ-LAYER-VALIDATION-SUMMARY.html").write_text(html, encoding="utf-8")
print("wrote IQ-LAYER-VALIDATION-SUMMARY.html")
