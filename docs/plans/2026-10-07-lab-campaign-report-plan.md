# Pro lab campaign report — plan (PLAN ONLY, nothing built)

**Status:** plan APPROVED by the coordinator (2026-10-07). L1–L6 built on synthetic fixtures (PR 17): guard, loader, aggregation, HTML, PDF, command. L7 and L8 wait for the 0.34.0 engine release.
**Input contract:** the engine's lab-replay results, as proposed in public `kshana` branch
`feat/034-campaign`, `docs/design/LAB-CAMPAIGN.md` @ `f69efe9` (0.34.0 "Lab replay"):
`campaign.json`, `cells/<key>.json` (`kshana.campaign-cell/1`), `scorecard.csv/json`,
`report.html`, `DIGEST`.
**Traces to:** gap analysis items B5.3 (Pro overnight orchestration + lab campaign report) and
B8.3 (repeatability), requirements R4, R5, R7, R8.

## 0. Binding constraints

1. **Export control (N6, PR 14).** The generator may live in Pro. **No partner recordings,
   campaign results, scores or anti-jam/anti-spoof performance numbers enter either repository
   until counsel answers N6.** Every committed fixture and golden is a **synthetic** campaign: a
   Kshana scene with C/N0 profiles, per LAB-CAMPAIGN.md §5. Guards are in §6.
2. **Name-free.** Nothing in the repo names a partner, person or receiver vendor. The client name
   is a render-time argument (`--client`) and never a default, a fixture or a test string.
   Tests use "Example Laboratory".
3. **No synthesis claims.** The report describes measured loop response on recordings the client
   supplied. The only interference model shown is the analytic C/N0-vs-J/S reference curve
   (spectral separation, Q per jammer type), labelled **MODELLED** everywhere it appears.
4. **Honesty labels.** Every figure is tagged **MEASURED** (from the client's recording through
   our loops) or **MODELLED** (the reference curve). Nothing is called "validated". Predictions
   for untested conditions are out of scope here; they come from labfit (B7.1), labelled
   `PREDICTION`.
5. **The engine stays the source of the numbers.** The Pro report re-renders and never recomputes
   a metric. Every number it prints is read from `cells/*.json`, and the report says so. The
   engine's own `report.html` stays the plain technical face. The Pro report is the branded,
   partner-ready face, and the two must never disagree (test in §5).

## 1. What the reader gets (partner-ready, branded)

Format: one self-contained HTML file (Ashforde document style via `report_brand::finish_html`,
inline fonts and SVG, no network) plus the same content as PDF via `brand_pdf`. A4 print layout.

| § | Section | Content (all from cells unless stated) |
|---|---|---|
| Cover | Title, campaign | "Lab campaign report". Campaign name, client (render-time), document id, **data-class banner** (SYNTHETIC / CLIENT-CONFIDENTIAL), engine + Pro versions, DIGEST (short + full), completeness (cells done / total) |
| 1 | Executive summary | Generated from fixed sentence templates, no free prose about the client. (a) Matrix size. (b) Per front-end chain, the loop design with the best median time to loss and the best re-acquisition time, with values. (c) Pass/fail counts against the client's bars, if set. (d) Three caveats: MEASURED vs MODELLED, the lock definition, the scope of the conditions |
| 2 | Test matrix | One row per recording: id, DUT text (from `[receiver]`, client-supplied), duration, expected signals/satellites, and its events (kind, type, onset/offset, stated J/S range). Front-end chains with their parameters. Loop designs with key parameters (orders, bandwidths, T_coh, spacing, discriminators). Cell count and status |
| 3 | Scorecard | Design × chain table of whole-run availability, median time to loss, median re-acquisition time, false locks per locked hour, PLL/DLL jitter. A heatmap of the same (one sequential hue, values printed). A ranking for each metric, with ties shown |
| 4 | Per-event results | For each event: per satellite × design small multiples (lost?/time to loss/J/S at loss/re-acq time); event availability; a table of best designs. Spoofing/outage events show every metric except the J/S curve (no J/S) |
| 5 | C/N0 vs stated J/S | Per event, for each chain: measured degradation per J/S bin (median, `n`) per design as points/lines; the **MODELLED** reference as a dashed neutral line labelled MODELLED; Q and the formula printed under it. One y-axis. Bins with small `n` are flagged |
| 6 | Stability | PLL/DLL jitter, baseline vs in-event, per design. False-lock episodes (count, per locked hour) and their detection method (alias detector or Doppler truth) |
| 7 | Pass/fail | Only if `[scoring.bars]` was set: the client's bars, per cell and per event, with counts. No bars means no section and no implied pass |
| 8 | Method and definitions | The metric definitions table from LAB-CAMPAIGN.md §3, verbatim and versioned (`kshana.campaign-cell/1`). The lock state machine and its dwells per design. Settle and baseline windows. J/S binning |
| 9 | Provenance and reproducibility | For each cell: key, recording SHA-256, condition / front-end / design / run / scoring hashes, engine version. DIGEST recomputed and shown as **verified** or **MISMATCH**. The exact command to reproduce (`kshana iq campaign …`) |
| 10 | Limits | Measured on the supplied recordings only; no claims about receiver internals; the MODELLED curve is an analytic reference, not a jammer simulation; Kshana does not generate or transmit interference in this work; no extrapolation (see labfit) |
| A | Appendix | Full scorecard table (all rows of `scorecard.csv`), design files verbatim, condition files resolved (receiver text included) |

Charts follow the dataviz rules: fixed categorical order per **design** (colour follows the design,
never its rank), one axis per chart, thin marks, a legend plus direct labels for 4 designs or
fewer, small multiples above that, and a table view for every chart (§A). Colours come from the
Ashforde palette (`report_brand` constants) after a validator pass for colour-vision deficiency.

## 2. Architecture

```
kshana-pro lab-report <campaign-out-dir> --data-class synthetic|client-confidential
                      [--client "<name>"] [--doc-id KP-…] [--prepared-utc …]
                      [--html out.html] [--pdf out.pdf] [--allow-incomplete]
```

- **`src/lab_campaign/model.rs`.** Serde mirror types for `campaign.json` and
  `kshana.campaign-cell/1`, with unknown-field rejection and an exact schema-string check. A list
  of accepted schema versions is kept, so a schema bump fails loudly instead of misreading.
- **`src/lab_campaign/load.rs`.**
  - Reads the out dir and verifies the DIGEST, recomputed from `cells/` as LAB-CAMPAIGN.md §3
    defines it.
  - Checks that every cell's `engine_version` is the same, and that `campaign.json` lists every
    cell key.
  - Missing cells mean incomplete: rendering is refused unless `--allow-incomplete` is given, and
    then an INCOMPLETE mark goes on every page.
- **`src/lab_campaign/aggregate.rs`.** Pure selection and ordering of numbers already in the
  cells, plus medians across satellites for the summary tables. Each derived number states its
  operation, e.g. "median over 4 satellites".
- **`src/report_lab_campaign.rs`.** The HTML face, using `report_brand::finish_html`,
  `report::StudyCover` / `StudyProvenance`, and inline SVG charts.
- **`src/report_lab_campaign_pdf.rs`.** The PDF face via `brand_pdf::{cover_lockup_ops, dress,
  save}`.
- **CLI wiring** in `src/main.rs`.

**Coupling to the engine.** Pro reads the results as JSON against mirror types, not through
engine Rust types, so it can be built and tested before 0.34.0 ships. When 0.34.0 is published
and Pro pins `=0.34.0`, a test asserts that the mirror types parse the engine's own synthetic
campaign output. If the engine exposes canonical hashing (`kshana::iq::campaign`), Pro calls it
for the cell-key check instead of re-implementing it.

## 3. Build items

| Id | Item | Size | Test basis | Depends on |
|---|---|---|---|---|
| L1 | Mirror types + loader + DIGEST/key/completeness checks | S | hand-made synthetic fixture shaped exactly like LAB-CAMPAIGN.md §3; refusals for digest mismatch, mixed engine versions, unknown schema, missing cell | W7 schema frozen |
| L2 | Aggregation (selection, medians, rankings with ties) | S | unit tests on the fixture; every printed number traced to a cell field | L1 |
| L3 | HTML report: sections 1–10 + appendix, charts as inline SVG | M | byte-stable golden HTML for the synthetic fixture; HTML validity check; numbers-match test against cells (§5) | L2 |
| L4 | PDF face | S | page count and content id stable; text extraction contains the same headline numbers as the HTML | L3 |
| L5 | CLI + data-class gate + client-name handling | S | refuses without `--data-class`; SYNTHETIC banner on synthetic; `--client` never defaulted | L3 |
| L6 | Data-hygiene guards (§6) | S | guard test fails on a planted non-synthetic cell file outside the fixture dir | — |
| L7 | Switch fixtures to the engine's synthetic campaign once 0.34.0 is published; pin `=0.34.0` | S | the engine's synthetic campaign parses, and its DIGEST verifies in Pro | 0.34.0 release |
| L8 | Overnight orchestration wrapper (B5.3): run, watch, then render on completion; reuse `batch/uq` checkpoint idioms and the `pack_c` hash chain for a campaign history | M | resume test as engine; history chain verifies | L5, 0.34.0 |

Total L1–L6 ≈ 2 weeks. L7 and L8 follow the engine release.

## 4. Rendering rules (non-negotiable)

- Every number carries its unit and its MEASURED/MODELLED tag. Every chart caption names the cells,
  the operation (for example "median over N satellites") and the bin rule.
- `null` metrics (held lock, no re-acquisition within the grace window) render as words ("held
  lock", "not within 30 s"), never as 0 or blank.
- J/S curves: bins with `n` below a stated minimum (default 50 epochs) are greyed and flagged.
  The MODELLED curve is never drawn where the measured curve has no data.
- No adjectives about the client's receiver, and no comparison with any named product.
- Deterministic output: no clock reads (`--prepared-utc` is passed in), fixed ordering, shortest
  round-trip float formatting. The same inputs and arguments give a byte-identical HTML file.

## 5. Tests (synthetic only)

1. A golden HTML/PDF for a synthetic campaign: 2 recordings, 2 chains, 3 designs; 1 interference
   event with a J/S profile and 1 outage event; bars set on one recording.
2. **Numbers-match.** Every numeric token in the report's data tables is found in the cells, or is
   a declared aggregate of them. This guards rule 5 of §0.
3. **Engine agreement.** Headline numbers equal the engine's `scorecard.csv` for the same cells.
4. Refusals: DIGEST mismatch, an unknown schema version, mixed engine versions, incomplete without
   the flag, and a missing `--data-class`.
5. Edge cases: no events (whole-run only), a spoofing-only event (no J/S section), all designs
   holding lock (time to loss is "held lock" throughout), and a single design.
6. Name-free: the rendered synthetic report contains no string from a deny-list of partner/vendor
   terms kept outside the repo, plus a check that `--client` is the only source of a client name.

## 6. Data hygiene (until counsel answers N6, and after)

- `.gitignore`: `lab-campaigns/`, `**/campaign-out/`, `*.sigmf-data`, `**/cells/*.json`, with
  `!tests/fixtures/lab_campaign_synthetic/**` re-included.
- A guard test, `tests/lab_campaign_no_client_data.rs`, fails if any `kshana.campaign-cell/1` or
  `kshana.test-conditions/1` file exists outside `tests/fixtures/lab_campaign_synthetic/`, or if a
  fixture's campaign name does not start with `synthetic-`.
- **Proposal to W7:** add an optional `data_class` field to the campaign file, carried into
  `campaign.json`. Pro would then refuse to render a mismatch between it and `--data-class`.
- Client runs happen outside the repository (a client-confidential directory on an
  access-controlled machine). Only the generator code lives in git.

## 7. Open questions

1. **To W7.** Will `campaign.json` carry each design's resolved parameters and each recording's
   resolved conditions (event J/S profiles, receiver text)? The report needs them, and should not
   reopen design or condition files itself.
2. **To W7.** Will the canonical-JSON hash and DIGEST functions be public in `kshana::iq::campaign`,
   so Pro verifies with the engine's own code?
3. **To W7.** Can `data_class` (§6) go into the campaign schema?
4. **Answered (coordinator): yes.** The Pro lab campaign report is the deliverable for the
   after-0.34.0 demo; the engine's `report.html` stays the technical face.
5. **Answered (coordinator): yes.** CLIENT-CONFIDENTIAL renders carry an export-control notice
   slot on the cover; its wording stays a placeholder until counsel answers N6.

Questions 1–3 were sent to the engine-side worker (coordinator's preference: yes to all three).
Answered from the engine's code (`feat/034-campaign` @ `51ba4787` (first read at `da95b9c`)): `campaign.json` is
`kshana.campaign-plan/1` and carries resolved designs and conditions (Q1); the hash functions are
public and the DIGEST is the SHA-256 of sorted `"<key> <sha256(cell)>\n"` lines (Q2); `data_class`
is required in the plan, every cell and the scorecard (Q3). PR 17 mirrors these exactly; no provisional markers remain.
