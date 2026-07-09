# P6 / L52-L54 — Cislunar constellation observability over a tracking arc

Generated artifacts for the Kshana Pro `pro-cislunar-observability-study` pack
(`src/packs/cislunar_observability_study_pack.rs`). The study drives the open engine's
observability-Gramian core (`kshana::observability_gramian`, `::intersat_range`, `::dro`,
`::cislunar_srif`) over a **differential-corrected four-DRO constellation** and tabulates,
**per tracking-arc length**, the observable rank, the Gramian eigen-spectrum extremes
`λ_min` / `λ_max`, and the condition number — the honest answer to *"how long a tracking arc
do I need before the spacecraft four-state becomes observable?"*

## Files (regenerated deterministically; do not hand-edit)

| File | What it is |
|------|------------|
| `cislunar_observability_table.csv` | Byte-stable committed table (L52): one row per tracking-arc prefix with the observable rank, `λ_min`, `λ_max` and condition number, with a provenance comment header. |
| `fig1_cislunar_observability.svg` | Four-panel Figure 1 (L54): (a) the four DROs in the Earth–Moon rotating frame; (b) observable rank vs tracking-arc; (c) the final Gramian eigenvalue spectrum; (d) condition-number vs tracking-arc. |
| `report.html` | Branded HTML report (Ashforde cover, MODELLED banner, Validated oracle-residual table, per-arc provenance table). |
| `report.pdf` | Branded, self-contained PDF deliverable (pure-Rust, no external tooling). |

## Provenance (honesty firewall)

- **Validated** — the observable **rank**, Gramian **eigen-spectrum** and **condition
  number** come from the finite-difference-validated open core and satisfy the spectral
  invariants `trace(W) = Σλ`, `‖W‖_F² = Σλ²`, and (full rank) `det(W) = Πλ` via an
  **independent Gaussian-elimination determinant** — three residuals reported to machine
  precision. The rank transition is **cross-validated against an independent Square-Root
  Information Filter**: its posterior covariance turns finite exactly at the arc where the
  observable rank reaches four, and `cond(P) = cond(OᵀO)`. The four initial conditions are
  **differential-corrected planar DROs** whose periodicity residual closes below `1e-8` and
  which are retrograde.
- **Modelled** — the **constellation design** (which DRO perilune amplitudes and which orbit
  phases the four DROs occupy) and the specific rank progression it produces.

An engineering study, **not a certification** and not a certified navigation-performance
product.

## Regenerate

```
cargo test --lib -- --ignored packs::cislunar_observability_study_pack::tests::emit_paper_artifacts
```

The default (empty-config) baseline differential-corrects four DROs (perilunes
18/26/34/42 × 10³ km) and tracks a chief spacecraft over a 24-epoch range-only arc; the
rank grows from 0 (zero-weight first epoch) through 2 to the full four-state, and the
condition number falls monotonically as the arc extends.
