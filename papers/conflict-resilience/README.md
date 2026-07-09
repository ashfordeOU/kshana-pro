# P7 / L55 — Layered-PNT conflict resilience

Generated artifacts for the Kshana Pro `pro-conflict-resilience-study` pack
(`src/packs/conflict_resilience_study_pack.rs`). The study drives the open engine's
layered-PNT conflict-resilience core (`kshana::conflict_resilience`, `::conflict_threat_params`)
— the intensity-swept seeded Monte-Carlo, the closed-form inverse-variance fuse and the
Gaussian-copula correlated sweep — to answer the two questions a resilience architect needs
before trusting *"more layers = safer"*: **how much does layering actually buy, and does that
benefit survive correlation?**

> **NON-CERTIFICATION.** This is a **MODELLED engineering study**, not a certified
> navigation-availability product and not ESA acceptance or TRL maturation.

## Files (regenerated deterministically; do not hand-edit)

| File | What it is |
|------|------------|
| `conflict_resilience_table.csv` | Byte-stable committed table: one row per threat intensity with the layered MC total-loss, the closed-form independent product, the median / mean fused error, the 4/3/2-layer total-loss curves and the per-layer usable fraction, with a provenance comment header. |
| `fig1_conflict_resilience.svg` | Three-panel Figure 1: (a) median fused error vs threat intensity; (b) total-loss probability vs intensity for the 4/3/2-layer configurations; (c) a per-vector survival panel (per-layer usable fraction vs intensity). |
| `report.html` | Branded HTML report (Ashforde cover, MODELLED banner, NON-CERTIFICATION disclaimer, Validated oracle-residual table, correlation-defeats-layering table, sourced per-layer provenance table). |
| `report.pdf` | Branded, self-contained PDF deliverable with the non-certification disclaimer on the artifact face (pure-Rust, no external tooling). |

## Provenance (honesty firewall)

- **Validated (vs an independent oracle)** — the Monte-Carlo total-loss probability
  converges to the **closed-form independent product** `Π_i layer_loss_prob_i` (residual
  reported per intensity); the **inverse-variance fuse** is a closed-form identity
  (`fuse([3,4]) = 12/5`); at **`ρ = 0` the Gaussian copula reduces to the independent model**
  (agreeing within Monte-Carlo error for the same seed).
- **Modelled** — the per-layer vulnerability / availability / accuracy magnitudes are
  **sourced-but-Modelled** inputs (cited via `kshana::conflict_threat_params` — JammerTest
  2024, TEXBAT, EASA SIB, RTCA DO-229, LunaNet/IOAG); the **~7× headline ratio** and the
  ratio-vs-correlation curve shape are properties of that Modelled parameterisation.

**Correlation defeats layering:** as the shared-vector denial correlation rises, the ~7×
benefit collapses toward 1. Layer *diversity*, not count, is what buys resilience.

## Regenerate

```
cargo test --lib -- --ignored packs::conflict_resilience_study_pack::tests::emit_paper_artifacts
```

The default (empty-config) baseline is the sourced four-layer conflict architecture over
`[0, 1]` threat intensity at 4000 Monte-Carlo trials per grid point.
