# Lunar and cislunar PNT evidence (`kshana-pro lunar-evidence`)

Research backlog E24 (items B2401, B2404, B2405, and a Pro view of B2403). Source:
`src/lunar_evidence/`, `src/cli/lunar_evidence_cmd.rs`. Tests: `tests/lunar_evidence.rs`
(synthetic inputs only).

Independent verification evidence for lunar navigation service providers (LNSPs), lunar receiver
makers, ground-station operators and landing teams. It sits beside the D-family lunar layer
(`kshana-pro lunar`, docs/LUNAR-PRO.md; D7 there is the monthly SLA check) and reuses the open
engine through the pinned `kshana` dependency rather than copying it: the AFS reference generator
(`kshana::lunar_afs`, LSIS V1.0), the Allan-family estimators (`kshana::allan`), the named lunar
clock classes (`kshana::clock_specs`), the lunar frame chain (`kshana::lunar_frame`) and the
Helmert datum Jacobian (`kshana::lunar_datum`).

Every result is a signed directory (`summary.txt`, `<command>.json`, the inputs under `inputs/`,
the Ed25519 signature) labelled **MODELLED**: checked on synthetic inputs with known faults, never
on measured lunar data (no lunar navigation service, lunar clock record or landing campaign has
been scored). Nothing here certifies, qualifies or approves anything, and no output shows that anything
meets a standard.

| Command | Backlog | Feature | Tier |
|---|---|---|---|
| `ura <request.toml>` | B2401 URA/SISA overbounding | `pro-lunar-service-verification` | Pro |
| `time-consistency <request.toml>` | B2401 cross-satellite time consistency | `pro-lunar-service-verification` | Pro |
| `afs-rx <request.toml>` | B2403 (Pro view): AFS receiver verification | `pro-lunar-service-verification` | Pro |
| `clock <request.toml>` | B2405 lunar clock characterisation | `pro-lunar-clock-evidence` | Pro |
| `station-delay <request.toml>` | B2405 station delay calibration (GUM) | `pro-lunar-clock-evidence` | Pro |
| `radiometric <request.toml>` | B2405 radiometric forensics and track quality | `pro-lunar-clock-evidence` | Pro |
| `fdir <request.toml>` | B2404 pre-registered FDIR verification | `pro-lunar-surface-evidence` | Pro |
| `config <request.toml>` | B2404 configuration-state attestation | `pro-lunar-surface-evidence` | Pro |
| `datum <request.toml>` | B2404 map-to-frame datum tie | `pro-lunar-surface-evidence` | Pro |
| `tipover <request.toml>` | B2404 tip-over probability | `pro-lunar-surface-evidence` | Pro |
| `check <dir> --trust <key>` | | none | free |
| `synth <dir>` | | none | free |

Producing commands take `--out <dir> --signer <name> --key-file <file> [--utc <iso>]` and a
licence (`--license <token>`, the stored licence, or `--dev` in debug builds). A request file has
one section per command; only the section of the command run is read. `synth <dir>` writes a
request and inputs for every command, each with a known fault named in its first line.

## Service verification (B2401)

**`ura`** (`[ura]`: `data`, `decorrelation_s` = 900, `thresholds` = [1, 2, 3, 4], `source`).
Input `time_s,node,sise_m,ura_m`: the signal-in-space error (broadcast product minus the
verifier's precise reference, projected on the range) and the broadcast URA. Per node and pooled,
the empirical two-sided tail `P(|SISE/URA| > k)` is compared with the Gaussian tail
`2Q(k) = erfc(k/√2)` at each threshold (the paired-Gaussian overbounding test, DeCleene, ION GPS
2000), with a Wilson 95 % interval on the number of decorrelation blocks. Verdicts: OVERBOUNDED,
NOT OVERBOUNDED, INCONCLUSIVE (with the independent-sample count a zero-exceedance demonstration
would need). The inflation factor is the smallest URA scale that makes every threshold the sample
resolves overbounded on the point estimate. The LNSP URA field definition and any overbounding
requirement tied to it are **to confirm** against the LunaNet Interoperability Specification and
LSIS (research note 13 reports the bit allocation is still being fixed).

**`time-consistency`** (`[time_consistency]`: `data`, `threshold_ns` = 15, `cusum_k` = 0.5,
`cusum_h` = 10, `class_window_s` = 3600). Input `time_s,node,clock_ns`: each node's broadcast
clock offset minus a monitor's estimate. At each epoch with three or more nodes the median of the
nodes not in an event is removed (a monitor common-mode error cancels; a faulty node does not bias
the others); deviations are normalised by the pooled MAD sigma and fed to a two-sided CUSUM per
node, with a plain threshold alongside. An event has an onset (last zero of the CUSUM), an alarm
time and a class (step or ramp from a line over the following window). One event per excursion.

## AFS receiver verification (Pro view of B2403)

**`afs-rx`** (`[afs_rx]`: `receiver`, `codes`, `frames`, `assignments`,
`max_frame_error_rate`). Against the open LSIS V1.0 generator (29 January 2025):

- `codes` (`prn,channel,hex`, right-aligned hex as LSIS Annex 3 prints): AFS-Q primary codes
  chip by chip against `afs_q_primary` (LSIS-222); AFS-I codes checked to be Gold codes of the
  LSIS-221 pair, and against the Annex 3 PRN assignment only when the verified LSIS cache is
  present (otherwise "PRN assignment not checked").
- `frames` (`time_s,node,toi,bits,hex`, data with its 24 CRC bits): CRC-24 (LSIS-FID0-469)
  recomputed; TOI continuity at one frame per 12 s, modulo 100 (LSIS-720); frame error rate with a
  Wilson interval against the verifier's own `max_frame_error_rate` (not a figure from LSIS).
- `assignments` (`node,i_prn,q_prn,secondary,tertiary_prn,tertiary_phase`): PRNs in 1 to 210, no
  shared AFS-I PRN, AFS-Q PRN or tertiary code and phase. LSIS V1.0 Table 11 is interim
  ({LSIS-TBD-2001}).

The bit-true RTL vectors, the cross-LNSP PRN registry and the output message profile of B2403 are
open-engine items and are not part of this change.

## Clocks and tracking (B2405)

**`clock`** (`[clock]`: `data`, `clock`, `spec` or `mask`). Input `time_s,phase_s`, uniformly
spaced. A quadratic fit removes the frequency offset and drift (both reported); the overlapping
ADEV with noise-type 95 % intervals comes from `kshana::allan::overlapping_adev_curve` (Riley,
NIST SP 1065). Each averaging time is WITHIN, EXCEEDS or INCONCLUSIVE against a named class of
`kshana::clock_specs` (`optical-master`, `passive-h-maser`, `rafs`, `mini-rafs`) or a caller's
`[[tau_s, adev], ...]` mask (log-log interpolation).

**`station-delay`** (`[station_delay]`: `station`, `sessions`, `[[station_delay.terms]]` with
`name`, `value_ns`, `u_ns`, `dist` = normal or rectangular (half-width), `source`;
`previous_ns`, `previous_expanded_ns`). Type A from the sessions (mean, standard deviation of the
mean), Type B from the terms, combined in a GUM budget (JCGM 100:2008); coverage factor the
Student t 95 % quantile at the Welch–Satterthwaite degrees of freedom (JCGM 100 G.4). Against a
previous calibration, the En number `|Δ| / √(U₁² + U₂²)` (ISO 13528): En ≤ 1 consistent, En > 1
moved.

**`radiometric`** (`[radiometric]`: `data`, `planned`, `outlier_k` = 5, `model_k` = 6,
`gap_s` = 60). Input `time_s,pass,kind,residual,sigma` (post-fit residuals) and optionally planned
windows `pass,start_s,end_s`. Per pass and observable: outliers about a running median of 11
samples; then a constant, a line and one step compared (BIC), with a ramp or step a finding when
its χ² reduction exceeds `model_k²`, and a bias beyond `model_k` standard errors; gaps; coverage
against the plan. Score `100 × coverage × (1 − outlier fraction) × min(1, 1/NRMS)`, halved by a
step, ramp or bias; A ≥ 90, B ≥ 75, C ≥ 50, else D. A planned pass with no data is a lost pass
(D). The score is this module's convention, stated so a reviewer can recompute it.

The backlog's acceptance test (CAPSTONE public data reproduced) is **not done**: the data are not
in this repository.

## Landing and surface (B2404)

None of these navigates, estimates a state or fuses sensors: they score results and
configurations a landing team supplies.

**`fdir`** (`[fdir]`: `plan`, `registered_sha256`, `results`). The plan (`id`, `min_detection`,
`max_false_alarm`, `[[case]]` with `id`, `fault`, `expect_detect`, `max_latency_s`, `min_runs`)
is hash-locked at registration; a plan that no longer hashes to `registered_sha256` makes the
whole result "PLAN CHANGED AFTER REGISTRATION". Results `case,run,detected,latency_s`. Fault cases:
timely detection (within `max_latency_s`) with a Wilson interval against `min_detection`; nominal
cases: false alarms against `max_false_alarm`. PASS, FAIL or INSUFFICIENT RUNS; unplanned cases
are listed and not scored.

**`config`** (`[config]`: `vehicle`, `expected`, `loaded`). The reviewed configuration (TOML
`[expected]` table) against the configuration as loaded (`key,value`): mismatches, missing keys,
unexpected keys.

**`datum`** (`[datum]`: `points`, `map_frame`, `ref_frame` (PA or ME), `map`, `max_rms_m` = 5,
`epoch_jd_tdb`). Control points `id,map_x,map_y,map_z,ref_x,ref_y,ref_z,sigma_m` (lunar body-fixed,
metres). A weighted translation, or a 7-parameter Helmert fit when the points span 100 km or
more. The centroid offset is compared with the mean-Earth/principal-axis displacement at the site,
from the open frame chain (DE421 ME→PA rotation, NAIF `moon_080317.tf`); a match says the map is
in the other frame than declared (about 0.6 to 0.9 km at the surface). TIED or NOT TIED.

**`tipover`** (`[tipover]`: `slopes`, `target_m`, `dispersion_m`, `slope_sigma_deg`,
`footprint_radius_m`, `cg_height_m`, `dynamic_margin_deg`, `design_slope_deg`, `samples`,
`seed`). A regular slope grid `x_m,y_m,slope_deg`. Monte Carlo over the Gaussian landing
dispersion and the DEM slope error; a touchdown tips when the slope exceeds `atan(r/h)` less the
dynamic margin. Reported at the stated dispersion, half and double; off-map samples are excluded
and counted. A static geometric model, not a touchdown dynamics simulation.

## Not built in this change

- **B2401**: the LNSP ephemeris and almanac exchange format (the D5 LUNEX prototype covers part)
  and SPS-style performance-standard authoring (D7 monitors an SLA already).
- **B2402** and **B2403** proper: open-engine items (time-scale module, TCB–TCL bound,
  terminology, ILuRF; RTL vectors, PRN registry, output message profile).
- **B2404**: the dust-plume degradation model (needs validation data), LTV/EVA safety evidence
  and RF self-interference forensics from IQ.
- **B2405**: reproducing CAPSTONE public data, and the independent evaluation of public demos.
- Anything that depends on Gateway (paused by NASA, 24 March 2026).
