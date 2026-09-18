# P4 / L49+L50 — Real-time lunar frame / EOP, a data-forward decomposition

Generated artifacts for the Kshana Pro `pro-realtime-frame-eop-study` pack
(`src/packs/realtime_frame_eop_study_pack.rs`). The study recasts P4's real-time lunar
frame / Earth-orientation check as a **data-forward decomposition** — the direct answer
to a hostile reviewer's *"this is circular"* objection.

It computes the frame error **forward from real data**: it takes the data-derived UT1
prediction error `ΔUT1(h)` at horizon `h` (the L18 curve read off the real IERS
`finals2000A` series bundled by the open `realtime-frame-eop` scenario) and pushes it
through the L19 lever arm `Δr = D_EM·ω⊕·ΔUT1` to a position / timing error **at the
Moon**. It then presents the resulting figures against the *independently* modelled
OD-covariance budget as an **independent prediction, not a consistency identity**.

The two roads are structurally disjoint — **Road A** (data → dUT1(h) → lever arm) uses
**no** OD covariance, and **Road B** (the modelled OD-covariance budget) uses **no** EOP
data — so their agreement is a prediction, not a tautology. The pack's `independence_*`
tests prove it: perturbing the OD covariance moves Road B but leaves Road A's positions
bit-identical, and perturbing the EOP horizons leaves Road B unchanged.

## Files (regenerated deterministically; do not hand-edit)

| File | What it is |
|------|------------|
| `realtime_frame_eop.csv` | Byte-stable committed decomposition: Road A (data-forward horizon rows) then Road B (modelled OD budget), with the anti-circularity narrative header. |
| `fig1_two_roads.svg` | Branded "two roads meet" figure: Moon-frame position from real EOP data through the lever arm vs the modelled OD budget, at the post-processed and real-time regimes. |
| `fig2_frame_eop.svg` | The open engine's two-panel P4 chart (validated rendering), reused verbatim. |
| `report.html` | Branded HTML report (Ashforde cover, MODELLED banner, Road A / Road B tables, the prediction). |
| `report.pdf` | Branded, self-contained PDF deliverable (pure-Rust, no external tooling). |

## Provenance (honesty firewall)

- **Validated** — the data-forward Moon positions are the L18 real-data UT1
  prediction-error curve (real IERS `finals2000A`) mapped through the L19 closed-form
  lever arm (`1 ms ↔ 28.03 m ↔ 93.5 ns`; `ω⊕` cross-checked against the CIO
  Earth-rotation angle).
- **Modelled** — the OD-covariance budget magnitudes (0.27 m post-processed, ~14.4 m
  real-time at 1 h latency) and the frame-realisation floor are representative
  allocations; the multi-day predictor is persistence, not IERS's operational Bulletin A
  algorithm.

**Not a certified real-time frame product.**

## Regenerate

```
cargo test --lib -- --ignored packs::realtime_frame_eop_study_pack::tests::emit_paper_artifacts
```

The default (empty-config) baseline runs the representative lunar-relay OD budget over
the bundled real `finals2000A` fixture for the 1/2/3-day horizons.
