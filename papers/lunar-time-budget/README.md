# P3 / L48 — Coordinated Lunar Time (LTC) end-to-end time-error budget

Generated artifacts for the Kshana Pro `pro-lunar-time-budget-study` pack
(`src/packs/lunar_time_budget_study_pack.rs`). The study assembles the **seven** LTC
error terms as time-error curves `x_i(τ)` across a whole averaging-time grid, root-sums
them into `x_Σ(τ) = √(Σ x_i²)`, and locates the **clock-vs-frame crossover** τ at which
the growing clock term overtakes the constant real-time frame-realisation term — the
honest answer to the *"you picked the averaging time that flatters your clock"* single-τ
artifact.

## Files (regenerated deterministically; do not hand-edit)

| File | What it is |
|------|------------|
| `lunar_time_budget.csv` | Byte-stable committed budget: the curves `x_i(τ)` in wide form (one row per τ, one column per term + the RSS), with a provenance comment header. |
| `fig1_ltc_budget.svg` | Two-panel Figure 1: (a) the log-log `x_i(τ)` curves + RSS with the 1 ns floor and the crossover marker; (b) the sorted per-term contribution bar chart at a representative τ vs the 1 ns floor. |
| `report.html` | Branded HTML report (Ashforde cover, MODELLED banner, sorted provenance table). |
| `report.pdf` | Branded, self-contained PDF deliverable (pure-Rust, no external tooling). |

## Provenance (honesty firewall)

- **Validated** — the `clock` row reproduces the published one-day clock spec
  (`kshana::clock_specs`, calibrated to <0.1 % of the cited `x(1 day)`); the
  clock-vs-frame **crossover identity** is a closed form checked against an analytic
  oracle (`τ*·floor = δr/c` for a flicker-floor clock, `τ* = 2(δr/c)²/h₀` for a white-FM
  clock) to machine precision.
- **Modelled** — the RF/optical-link, frame-realisation, relativistic-residual,
  ephemeris and measurement floor **magnitudes** are documented budget allocations
  (caller-overridable defaults); the RSS total and the crossover τ *value* inherit that
  Modelled frame allocation.

An engineering study, **not a certification** and not certified for operational
timekeeping.

## Regenerate

```
cargo test --lib -- --ignored packs::lunar_time_budget_study_pack::tests::emit_paper_artifacts
```

The default (empty-config) baseline is the passive-H-maser clock over the 1 s … 1e7 s
grid at 8 points/decade.
