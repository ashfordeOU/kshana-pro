# Kshana × Lunar/Cislunar PNT Papers — Capability-Gap Buildout

This directory tracks the program to make **every analysis and claim in the seven
lunar/cislunar PNT preprints (P1–P7) producible in Kshana** — turning imported/asserted
"Modelled" numbers into Kshana-computed "Validated" results.

## Published on arXiv (2026-07-09)

Four program outputs are now live on arXiv (peer-visible); each cites the open `kshana`
engine and every headline figure regenerates from a committed scenario + seed:

| arXiv | Title | Program map |
|-------|-------|-------------|
| [2607.06212](https://arxiv.org/abs/2607.06212) | The Cost of Lunar South-Polar Geometry, and Surface Beacons as the Efficient Fix | P2 · surface-beacon DOP → `src/lunar_beacon.rs` |
| [2607.02566](https://arxiv.org/abs/2607.02566) | Earth-baseline VLBI restores the observability of a lunar surface station in joint orbit-and-clock determination | lunar joint OD+clock → `lunar_vlbi.rs` · `lunar_combination.rs` (cites kshana v0.23.0) |
| [2606.24210](https://arxiv.org/abs/2606.24210) | A Conditional Timing Protection Level | `src/tpl.rs` (JammerTest 2024) |
| [2606.22054](https://arxiv.org/abs/2606.22054) | Anticipating the Optimism Gap | `impairment_study.rs` |

Papers stay **arXiv-first** — LaTeX sources live in [`papers/`](../../papers/); nothing
else goes public until the founder announces.

## What's here

- **`pnt-kshana-gap-report.html`** — the visual implementation tracker (open in a browser).
  Phase roadmap + filterable master gap ledger (click a status pill to cycle
  to-build → in-progress → done; saved in your browser) + per-paper analysis/gap detail.
- **`ledger.json`** — machine-readable audit output: `{ perPaper, synthesis:{ summary, themes, ledger, build_phases } }`.
- **`gen-report.js`** — regenerates the HTML from `ledger.json` + `status.json`.
- **`status.json`** — committed implementation status (source of truth for the rendered progress).

## How it was produced

A per-paper audit (one agent per paper reading the full LaTeX + grepping both repos),
each followed by an adversarial gap-verifier that re-checks every claimed gap against the
real 193-module public engine so nothing already-implemented is counted as a gap, then a
synthesis pass that dedups cross-paper and orders the work into shippable phases.

## The plan

55 deduped gaps (23 P0, of which 18 are the hostile-peer-review "upgrade" prescriptions),
**42 in public `kshana`** (core scientific analysis, scenarios, oracles, fixtures) and
**13 in `kshana-pro`** (paper-grade packs, branded reports, committed CSVs, figure renderers),
ordered into **10 phases**: five public-engine core phases (one per paper cluster) each
followed by its kshana-pro deliverable phase, so every phase is a clean single-repo commit
and no pack precedes the core analysis it consumes.

## Regenerate after a phase lands

```bash
node gen-report.js ledger.json status.json pnt-kshana-gap-report.html "$(date +'%Y-%m-%d %H:%M %Z')"
```

Architecture rule: core scientific/analytical capability lives in **public `kshana`**;
`kshana-pro` packages it into studies / packs / branded reports / serve endpoints.
"Validated" requires an independent external oracle (closed form, reference library, or field data).
