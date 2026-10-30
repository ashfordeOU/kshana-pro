# Capability dossier

The public capability dossier of the open Kshana engine: what it is, how a run works, every kind, every surface,
the evidence ledger with its misses, and the editions (open core, Kshana Pro, custom study). Every count is read
from the public repository at a release tag when the document is built.

| Version | PDF | Source |
| --- | --- | --- |
| 0.40.0 | [Kshana-capability-dossier-v0.40.0.pdf](Kshana-capability-dossier-v0.40.0.pdf) (131 pages) | [src-v0.40.0/](src-v0.40.0/) |
| 0.38.0 | [Kshana-capability-dossier-v0.38.0.pdf](Kshana-capability-dossier-v0.38.0.pdf) (119 pages) | |
| 0.37.0 | [Kshana-capability-dossier-v0.37.0.pdf](Kshana-capability-dossier-v0.37.0.pdf) (85 pages) | |

## Living copy

The current dossier is also kept as a private web page (light and dark, A4 print CSS):
<https://claude.ai/artifact/R7CJGWYekjo93HNbtxkAmn>. It is republished at the same address whenever the dossier
changes, so a shared link always shows the latest version. It is private until shared from the page's Share menu.

## Updating it

At each release, or when a capability, a ledger row or an edition changes:

1. Copy `src-<previous>/` to `src-<new version>/` and update the version strings in `make.sh` and `build.py`.
2. Add or edit the chapters in `build.py` for what changed (new kinds, families, commands, surfaces, ledger rows,
   misses, editions). Keep the advisory statement on the cover, in every interior footer, at the end of every
   chapter and on the closing page.
3. Build against a read-only checkout of public kshana at the release tag:
   `./src-<new version>/make.sh <path to kshana at the tag>`. This writes the HTML in the source folder and the PDF
   here, with the contents page numbers checked.
4. Check the text before committing: no personal names, programme names written neutrally (file and kind names
   stay verbatim as code), no attribution lines, author Ashforde OÜ.
5. Republish the HTML to the living copy above, add a row to the table, and commit as Ashforde OÜ.
