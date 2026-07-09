# P5 / L51 — Heterogeneous optical + RF PNT: precision, integrity, availability

Generated artifacts for the Kshana Pro `pro-optical-rf-hybrid-study` pack
(`src/packs/optical_rf_hybrid_study_pack.rs`). The study pairs a tight but
weather-limited **optical (1550 nm)** link with a loose but always-on **RF** link and
quantifies the three axes that decide whether the pairing is worth it, driving five open
engine modules through their public contracts:

- `kshana::optical_linkbudget` (L26) — the 1550 nm link budget: the diffraction footprint
  `(λ/D)·L ≈ 0.7 km` at lunar range, the detected-photon rate, and the photon-limited
  ranging/timing CRLB `σ_τ/√N`.
- `kshana::linkbudget` — the RF column: the CCSDS/DSN link equation `C/N₀`, from which a
  coherent early–late DLL thermal code-tracking ranging jitter is formed.
- `kshana::cross_raim` (L22) — cross-modality RAIM: the χ² solution-separation detector
  and the position + timing protection levels from disparate RF/optical covariances.
- `kshana::handoff` (L23) — the optical↔RF state/covariance handoff with the bit-for-bit
  **no-jump** mean invariant and the NEES consistency gate.
- `kshana::optical_availability` (L24) — per-site clear-sky climatology and the
  `N`-station diversity combination (independent union + spatially-correlated union).

## Files (regenerated deterministically; do not hand-edit)

| File | What it is |
|------|------------|
| `optical_rf_precision.csv` | Byte-stable **Table 1 (precision)**: RF vs optical vs hybrid ranging/timing 1σ, with a provenance comment header. |
| `optical_availability.csv` | Byte-stable **Table 2 (availability)**: the `N`-station diversity curve (single-site through five-station), independent and spatially-correlated. |
| `fig1_optical_rf_hybrid.svg` | Two-panel Figure 1: (a) ranging precision 1σ on a log scale (RF vs optical = photon-limited CRLB vs hybrid); (b) availability vs number of ground stations. |
| `report.html` | Branded HTML report (Ashforde cover, MODELLED banner, both tables, oracle residuals). |
| `report.pdf` | Branded, self-contained PDF deliverable (pure-Rust, no external tooling). |

## Headline numbers (default, empty-config baseline)

- Optical footprint ≈ **700 m** at the Earth–Moon range (`(λ/D)·L`, Validated).
- Ranging 1σ: RF ≈ **0.83 m**, optical ≈ **0.24 µm** (photon-limited CRLB with `N ≈ 1.0×10⁹`
  photons), hybrid ≈ optical (the inverse-variance fusion inherits the tighter modality).
- Availability: single-site ≈ **53 %** → three-station ≈ **90 %** → four-station ≈ **95 %**
  → five-station spatially-correlated ≈ **96 %** (the independent union optimistically reads
  ≈ 98 %).

## Provenance (honesty firewall)

- **Validated** — the photon-limited ranging/timing CRLB equals the closed form `σ_τ/√N`;
  the diffraction footprint equals `(λ/D)·L`; the inverse-variance fused precision equals
  `1/√(1/σ_rf² + 1/σ_opt²)`; and the `N`-station independent-union availability equals
  `1 − Π(1−a_i)`. Each is checked against an independent hand value to machine precision
  (residuals reported in the HTML/JSON).
- **Modelled** — the RF and optical link-budget *magnitudes* (`C/N₀`, the detected-photon
  count `N`), the RF DLL ranging model, the per-site clear-sky availabilities and the
  spatial correlation `ρ`, and the cross-RAIM / handoff covariance *magnitudes*. The
  precision, protection-level and availability *values* inherit them.

An engineering study, **not a certification** and not ESA acceptance or TRL maturation.

## Regenerate

```
cargo test --lib -- --ignored packs::optical_rf_hybrid_study_pack::tests::emit_paper_artifacts
```

The default (empty-config) baseline is a 1550 nm, 0.85 m-aperture Earth–Moon optical
terminal paired with an X-band RF link, and the bundled five-site optical ground network.
