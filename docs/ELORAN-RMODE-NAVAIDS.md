# eLoran, R-Mode and terrestrial navaids (E37)

Research 2026-11-20 gaps, epic E37: B3701 (R-Mode receiver tests and multi-source trust),
B3702 (eLoran claim verification, interoperability and safety-of-life evidence; first slice),
B3703 (DME/VOR network twin, multi-sensor RNP scenarios, e-conspicuity backup and
flight-inspection truth assurance).

Everything here is **MODELLED** unless a user runs it on measured data, and every output is a
pre-assessment, an evidence index or a planning result. No output states that equipment or a
service meets a standard. Nothing here transmits, locates an emitter or computes a position for
anyone to navigate by.

| command | feature | tier | item |
|---|---|---|---|
| `eloran rmode-perf` | `pro-rmode-perf` | Pro | B3701 |
| `eloran wwrns-trust` | `pro-wwrns-trust` | Pro | B3701 |
| `eloran claim`, `eloran interop` | `pro-eloran-claims` | Pro | B3702 |
| `eloran sol-evidence` | `pro-eloran-sol-evidence` | Enterprise | B3702 |
| `eloran ldc-fde` | `pro-eloran` + on-prem grant, `unfiled-methods` builds only | on-prem contract | B3702 |
| `navaids coverage`, `rnp`, `inspect` | `pro-navaids` | Pro | B3703 |
| `navaids ssv` | none | free | B3703 |

## B3701 R-Mode receiver performance tests (`eloran rmode-perf`)

NCSR 13 (June 2026) agreed a draft MSC resolution on R-Mode receiver performance standards. The
draft is not adopted and its text was not read for this work, so **no limit is built in**. The
command takes a limits profile:

```toml
profile = "my profile"
source = "where the numbers come from (document, paragraph)"
source_status = "placeholder"   # placeholder | unverified | checked
accuracy_95_m = 10.0            # every limit is optional; a missing one gives not_assessed
availability_min = 0.998
continuity_min = 0.9997
continuity_interval_s = 900
alert_limit_m = 25.0
time_to_alarm_s = 10.0
max_update_interval_s = 2.0
acquisition_max_s = 60.0
min_beacons = 3
ambiguity_success_min = 0.999
confidence = 0.95               # one-sided, every bound
independence_s = 0              # thin records to one per interval before the statistics
```

Records: a receiver log as CSV with the header
`t_s,valid,lat_deg,lon_deg,truth_lat_deg,truth_lon_deg,alarm,hpl_m,beacons_tracked,ambiguities_ok`
(missing cells allowed; JSON array also accepted), or `--simulate <plan.toml>`, which repeats the
MF R-Mode bench (`eloran rmode`) with new noise per epoch:

```toml
epochs = 600
interval_s = 10
night_from = 300                       # optional: skywave from this epoch
night_skywave = { excess_delay_us = 60, rel_amp = 0.5, phase_rad = 1.0 }
alarm_residual_m = 30                  # stand-in for the receiver's own integrity monitor
[bench]                                # the `eloran rmode` configuration
rx_lat_deg = 55.0
rx_lon_deg = 13.8
# [[bench.beacons]] ...
```

| id | parameter | method | outcome |
|---|---|---|---|
| RMP-01 | horizontal accuracy (95 %) | distribution-free order-statistic bounds on the 95th percentile | pass if upper bound ≤ limit, fail if lower bound > limit |
| RMP-02 | availability | one-sided Wilson bounds | pass / fail / inconclusive / insufficient data (zero-failure sample size) |
| RMP-03 | continuity over T | loss events per valid time, Poisson upper bound, `exp(−λT)` | pass / fail; `samples_needed` is the loss-free time that shows the limit |
| RMP-04 | misleading information | valid, unalarmed epochs with error over the alert limit (and the stated HPL) | fail on any; upper bound on the per-epoch probability |
| RMP-05 | time to alarm | first epoch over the alert limit to the first alarm or invalid flag | not exercised without an episode (use fault-injection data) |
| RMP-06 | update interval | largest record spacing | |
| RMP-07 | acquisition | first record to the first valid fix within the accuracy limit | |
| RMP-08 | tone-cycle ambiguity | fraction resolved (simulated records only) | |

## B3701 WWRNS multi-source trust (`eloran wwrns-trust`)

IMO A.1046(27) as amended by MSC.599(111) lets terrestrial systems join the WWRNS. The command
applies the F3 vote rule (`kshana-pro trust`) to position fixes: a source votes through its
group's best fix, two fixes agree within `K·√(σ_a² + σ_b²)` (`K = √(−2 ln p_fa)`), a source
qualifies with at least one agreeing vote and no more disagreeing than agreeing, and the
qualified source with the smallest sigma is chosen. The output is that source's own fix; sources
are never blended. GNSS is flagged suspect when it is disqualified while two other groups agree.
The required 95 % accuracy is the user's (with its source); none is built in.

```toml
p_fa = 1e-5
required_accuracy_95_m = 100.0
requirement_source = "the figure and paragraph the user relies on"
[[source]]
id = "gnss"
group = "gnss"
[[source]]
id = "rmode"
group = "rmode"
[[source]]
id = "eloran"
group = "eloran"
```

Fixes CSV: `t_s,source,status,lat_deg,lon_deg,sigma_m` (`status` `ok` or anything else; sigma
per axis). The time-domain selector (`kshana-pro trust`) also takes an `rmode` source in its own
group.

## B3702 timing claims, interoperability, safety-of-life evidence

`eloran claim <request.toml> --out <report.json>`: readings of the receiver 1PPS against a local
UTC(k) (TIC CSV), the eLoran crate's autocorrelation-aware statistics, UTC(k) − UTC from BIPM
Circular T (linear between five-day values, the circular's `u` plus a stated interpolation
uncertainty; never extrapolated) or entered, and GUM Type B entries (`normal` or `rectangular`).
Decisions: mean offset (`|offset| + U ≤ limit` supported, `|offset| − U > limit` not supported,
otherwise inconclusive) and 95 % of instants (95th percentile of `|x_i|` with `k·u_B`).

```toml
[claim]
statement = "timing receiver within 20 ns of UTC"
limit_ns = 20.0
coverage_k = 2.0
[readings]
file = "pps.csv"
unit = "ns"
sign = "rx_minus_ref"
reference = "UTC(NPL)"
[reference_offset]
circular_t = "circular-t.txt"
lab = "NPL"
mjd = 61346.5
u_interp_ns = 0.5
[[budget]]
name = "receiver delay calibration"
u_ns = 2.0
```

`eloran interop <request.toml>`: each station's `receiver − UTC` from
`(UTC(k) − UTC) + steering − ref_minus_rx`; every pair compared with `E_n = |d| / U` (consistent
when ≤ 1), the laboratory term left out of `U` for two stations steered to the same UTC(k), an
optional tolerance check, the weighted mean and its Birge ratio. Laboratory offsets from
`circular_t` + `mjd` or `[lab_offsets] NPL = [value_ns, u_ns]`.

`eloran sol-evidence <request.toml> --out <dir> --signer <name> --key-file <file>`: the UK
National PNT Office asks for "a clear pathway to certification for safety of life services" and
publishes no checklist, so the categories are **Kshana's own**: requirements, timing
traceability, position performance, integrity/FDE, interoperability, failure modes,
configuration, reproducibility. Each lists the supplied artefacts by SHA-256 with the headline
of any Kshana report, or states that no evidence was supplied. The directory is signed; verify
with `kshana-pro dossier verify <dir> --trust <key>`.

`eloran ldc-fde` (patent-pending; compiled only with `unfiled-methods`, runs only under a
machine-bound licence with the on-premises grant): screens LDC corrections (unhealthy warning,
age, plausibility against the temporal ASF bound), detects with a χ² test on the weighted least
squares residuals, separates a corrupted differential correction (keep the station on the ASF
map alone) from a station signal fault (exclude the station) by leave-one-out prediction of the
residual with and without the correction, and gives Brown's slope HPL; alarms when unresolved,
without redundancy, or when HPL exceeds HAL. Thresholds follow from the user's `p_fa`, `p_md`.

Not built in this slice (B3702 remainder): the Canada and Korea eLoran coverage twins (no
station data with sources in the GRI database for a future Canadian network, and the Korean
build-out's sites need a cited source), and a link from `ldc-fde` to the `eloran rx` receiver
output (it reads its own epoch file today).

## B3703 DME/VOR network twin, RNP scenarios, inspection truth (`navaids`)

Published values, each checked in `tests/navaids.rs`: radio horizon `4.12 (√h₁ + √h₂)` km (the
FAA AIM `1.23 √h` NM rule); VOR standard service volumes T, L, H of FAA AIM 1-1-8 Table 1-1-1
(classic; `navaids ssv`; check against the edition in force; a facility's DOC overrides them);
ICAO Doc 9613 DME/DME criteria (3 to 160 NM, below 40° elevation, pair angle 30° to 150°) and
error model (`σ_air = max(0.085 NM, 0.125 % D)`, `σ_sis = 0.05 NM`); DO-260B NACp EPU bounds.

Facilities (shared by all three commands):

```toml
[[station]]
id = "ABC"
kind = "vor_dme"     # dme | vor | vor_dme
lat_deg = 50.0
lon_deg = 10.0
elev_ft = 300
ssv = "H"            # or doc_nm / doc_ft
out = false          # NOTAM
```

* `navaids coverage`: `name`, `altitude_ft`, `area = [lat_min, lat_max, lon_min, lon_max]`,
  `step_deg`, optional `max_2drms_nm`. Per grid point: DMEs and VORs usable, DME/DME (best pair,
  2drms), VOR/DME; area fractions; **critical DMEs** (the area that loses DME/DME when each is
  out). `--csv` writes the grid.
* `navaids rnp`: `[route]` (`points`, `altitude_ft`, `ground_speed_kt`, `sample_nm`, `nav_spec`,
  `rnp_nm`, `fte_95_nm`, `gnss_nse_95_nm`, `vor_sigma_deg`, `required_nacp`) and
  `[[scenario]]` (`gnss`, `gnss_out_nm = [from, to]`, `dme_dme`, `vor_dme`, `out = [...]`,
  `irs_drift_nm_per_h`, `backup = { name, epu_95_m }`). Per sample: sensor used, NSE, TSE against
  RNP, the monitoring alert (`NSE > RNP`), NACp; per scenario the fractions, first alert and
  longest inertial coast. The inertial term is a planning rule (`drift × time`), not an inertial
  model; the backup source covers a declared non-aviation position (for example a mobile network
  for drones).
* `navaids inspect`: `track` CSV `t_s,lat_deg,lon_deg,alt_ft,truth_sigma_m`, `measurements` CSV
  `t_s,station,kind,value` (`dme` slant NM, `vor` radial in degrees **true**), `dme_sigma_m`,
  `vor_sigma_deg`, `k_sigma`. Per epoch: consistent, facility deviation (one facility out),
  truth suspect (one common offset of the truth explains several; its size is reported, never a
  corrected track) or unresolved.

Not built: terrain masking, multipath and measured facility performance in the twin; the match
against a State's published coverage diagrams (needs the AIP data for a named network).
