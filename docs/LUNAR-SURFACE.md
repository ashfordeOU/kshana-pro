# Lunar surface operations evidence (second slice)

Research 2026-11-20 gaps, epic E24, backlog B2404 (remaining items). The first B2404 slice
(FDIR plan results, configuration attestation, datum tie, tip-over) is in `lunar-evidence`
(docs/LUNAR-EVIDENCE.md). Module `src/lunar_surface/`, command `kshana-pro lunar-surface`
(`src/cli/lunar_surface_cmd.rs`), tests `tests/lunar_surface.rs` (SYNTHETIC inputs only).

**Evidence label: MODELLED.** Exercised on synthetic inputs with known truth; not validated
against flight or surface measurements. Nothing here locates an emitter, fuses sensors for
navigation, or transmits.

## Commands and features

| Command | Feature | Tier | What it does |
|---|---|---|---|
| `lunar-surface plume <request.toml>` | `pro-lunar-plume-visibility` | Pro | Parametric ejecta-sheet extinction along declared lines of sight |
| `lunar-surface traverse <request.toml>` | `pro-lunar-traverse-safety` | Pro | LTV/EVA navigation safety evidence from a traverse log |
| `lunar-surface rf-forensics <request.toml>` | `pro-lunar-rf-forensics` | Pro | Self-interference spurs in a SigMF recording, matched to declared on-board sources |
| `lunar-surface synth <dir>` | none | free | SYNTHETIC inputs for all three |

Each analysis prints JSON (or `--json <file>`), or with `--out <dir> --signer <name>
--key-file <file> [--utc <iso>]` writes `report.json`, `report.html`, `inputs/request.toml`
and `signature.json`. Verify with `kshana-pro dossier verify <dir> --trust <key>` (free).

## Dust-plume visibility (MODELLED parametric)

A screening model for when a line of sight (beacon to rover, camera to landmark, relay to
lander) is likely to be lost to the ejecta sheet of a descending engine, and how much link
margin to hold. Every output lists its assumptions:

1. Erosion rate `Ṁ(H) = Ṁ₀·exp(−H/H_s)` below an onset height, zero above it and after cut-off.
2. Monodisperse spheres (radius `a`, density `ρ`) ejected radially at one speed `v` and one
   angle `θ`; gravity and ballistic curvature neglected within `r_max`.
3. Sheet `0 ≤ z ≤ h₀ + r·tan θ` between `r_min` and `r_max` over a flat surface; number density
   from mass conservation, `n = Ṁ(t − r/v) / (m_p·v·2πr·h(r))`.
4. Optical `Q_ext = 2` (large-particle limit; flagged when `2πa/λ < 10` at 550 nm).
5. RF in the Rayleigh limit with the stated complex permittivity:
   `Q_abs = 4x·Im[(ε−1)/(ε+2)]`, `Q_sca = (8/3)x⁴|(ε−1)/(ε+2)|²` (Bohren and Huffman 1983,
   §5.2); flagged when `x > 0.3`.
6. Single scattering; attenuation `4.343·τ` dB with `τ = ∫ n·πa²·Q ds`.

The physical parameters (`[ejecta]`, `[rf]`) have **no defaults**: they are the mission's to
state. Outputs per line of sight: the attenuation time series, the engine height at which the
optical threshold is first crossed, time obscured, and time the RF threshold is exceeded.

## Traverse safety evidence (LTV / EVA)

Log CSV `t_s,east_m,north_m,sigma_m[,true_east_m,true_north_m]` in a flat local frame;
`sigma_m` is the reported 1-sigma per horizontal axis. With `pl_k`, `alert_limit_m` and
`decorrelation_s` stated:

* **Sigma honesty** (where truth is logged): Stanford regions of the horizontal error against
  `PL = pl_k·σ` and the alert limit, an exact bound on the MI rate with an effective sample
  size, and the two-sided Gaussian overbound check of the per-axis normalised errors (with
  the inflation needed when it fails).
* **Hazards**: for each keep-out circle, the margin `distance − radius − PL` per epoch, its
  minimum, the epochs not shown clear, and with truth the incursions and whether the PL bound
  covered them.
* **Walk-back**: per epoch, `(distance to base + PL)/speed` against
  `consumables_end_s − reserve_s − t`; the first breach and the smallest margin.

Evidence for the traverse safety case, not a safety approval; straight-line walk-back at a
constant speed (no terrain, slope or fatigue model).

## RF self-interference forensics (analysis only)

From a SigMF recording (`cf32_le`, `ci16_le` or `ci8`) of the receiver band taken on the
vehicle: the Welch PSD of the whole recording (open engine), the noise floor from the median bin
corrected to the mean of a Welch bin with `K` equivalent segments (`median ≈ mean·(1 −
1/(9K))³`), spurs above the excess threshold at the stated false-alarm probability for the
largest noise bin, harmonic matches `n·f₀` to each declared source within its tolerance, the
Pearson correlation of each spur's block power with each source's on/off state from a declared
activity log, and the in-band interference-to-noise ratio with the white-equivalent C/N0 loss
`10·log₁₀(1 + I/N)` (a screening value: a correlator rejects part of a narrowband spur). Sources
are declared by the user; nothing is located. The DC bin is ignored; recorder spurs must be
ruled out with a terminated-input recording.
