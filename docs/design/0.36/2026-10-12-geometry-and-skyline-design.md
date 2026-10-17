# Geometry for 0.36: azimuth skyline masks, a Studio Geometry view, geometry-aware maritime trust, and the UERE × DOP ledger row

- **Date:** 2026-10-12
- **Status:** **DESIGN.** Nothing here is implemented. The baseline is the frozen 0.35.0 release commit (`release/0.35.0-rc` at `35c1558d`). Every statement about existing code below was checked against that commit; the file and line references are to it.
- **Release:** 0.36
- **Author:** Ashforde OÜ
- **Supersedes nothing.** Builds on the DOP kernel (`src/orbit.rs`, ledger row "GNSS geometry / dilution of precision (DOP)", VALIDATED against gnss_lib_py 1.0.4), the receiver-trust score (`src/receiver_trust/`, `docs/RECEIVER-TRUST.md`, `docs/MARITIME-TRUST.md`) and the Studio security rules (`web/studio/lib/engine.mjs`, `web/studio/lib/svgsafe.mjs`, `web/site.test.mjs`).

**Abbreviations.** DOP: dilution of precision (G geometric, P position, H horizontal, V vertical, T time). UERE: user-equivalent range error (1-sigma, m). ENU: local east, north, up. LS: least squares. LOS: line of sight. NLOS: non-line-of-sight reception. RAIM / ARAIM: (advanced) receiver autonomous integrity monitoring. HPL / VPL: horizontal / vertical protection level. LEO / MEO: low / medium Earth orbit. DEM: digital elevation model. TOML: the scenario file format. MC: Monte Carlo. CEF / LEEF: syslog event formats. OTLP: OpenTelemetry protocol. WASM: WebAssembly. MCP: Model Context Protocol.

## 1. Motivation, and what is already true

A common public explanation of GNSS geometry runs: the same satellites can give different
accuracy depending on where they sit in the sky; DOP amplifies measurement error; an urban
canyon clusters the visible sky into a strip along the street; and position error is roughly
measurement error × DOP. Kshana already carries most of the pieces, but a reader cannot see
them together, and the sky in every Earth scenario is a flat elevation cut.

What the 0.35.0 code does today (verified):

| Piece | Where | What it is | Ledger status |
|---|---|---|---|
| DOP kernel | `src/orbit.rs:648` `orbit::dop(user, sats) -> Option<Dop>` | `Q = (HᵀH)⁻¹`, one common clock column, ENU split for H/V. Returns the five scalars only; the covariance matrix is not exposed. | VALIDATED (`tests/dop_reference.rs`, 8 geometries, gnss_lib_py 1.0.4, 1e-6 relative) |
| Per-system-clock DOP | `src/leo_fusion/joint_pvt.rs:154` `dop(user, sats, clocks)` | One clock (estimated bias or known offset) per system. | inside MODELLED rows; the 0.30 Earth-design comparison (`tests/coverage_dop_full_oracle.rs`) agreed with gnss_lib_py and a numpy inverse but the row stays MODELLED for the Moon/Mars cells |
| `orbit` kind | `src/orbit.rs:1055` `OrbitClockScenario` | `mask_deg`, `sigma_uere_m` (default `DEFAULT_UERE_M`), `constellation` plus `constellations`; `summarize_dop` reports `pdop × sigma_uere_m`. **The user is an orbit (`OrbitCfg`), elevation is geocentric (`orbit::elevation_deg`, `:522`), and there is no Earth-fixed azimuth.** | MODELLED composition on the VALIDATED kernel |
| `integrity` kind | `src/raim.rs:1829` `IntegrityScenario` | RAIM / ARAIM HPL and VPL from the same `orbit::visible_positions`; Stanford-diagram classification at `src/raim.rs:1309`. Same orbiting-user geometry as `orbit`. | kernel VALIDATED (SciPy), ARAIM PL VALIDATED (WG-C example), availability MODELLED |
| `gnss-sim`, `jamming` | `src/gnss_sim.rs:412`, `src/jamming.rs:278` | A geodetic receiver; visibility through `frames::is_visible(station, sat, mask_deg)` (`src/frames.rs:208`), which uses `frames::look_angles` and so already has the azimuth. | MODELLED |
| `leo-pvt` / `leo-pass` | `src/leo_fusion/system.rs:390` `in_view`, `src/leo_fusion/polar.rs` | Per-system `mask_deg`; the polar mode sweeps latitude for MEO alone, LEO alone and both. | polar sweep geometry checked against Orekit 12.2 (`tests/leo_polar_coverage_orekit_oracle.rs`) |
| `automotive-urban-canyon` | `scenarios/automotive-urban-canyon.toml` | `gnss-ins` with **hand-authored GNSS windows**: no geometry at all. | MODELLED |
| LEO urban-canyon vertical | `scenarios/leo-vertical-autonomous-vehicle.toml` | A campaign whose canyon phase is `mask_deg = 35.0`; its own header says "a building mask as a single elevation cut-off". | MODELLED |
| Lunar service volume | `src/lunar_service.rs:204` `visible_sats` | Its own scalar mask; DOP through `orbit::dop`. | VALIDATED (ANISE visibility, gnss_lib_py-validated kernel) |
| Receiver trust | `src/receiver_trust/` | GGA HDOP is parsed into `MarineObs::hdop` (`mod.rs:139`) and **never read by any monitor or the score**. GSV elevation and azimuth are parsed past and **discarded** (`ingest.rs:1177-1195` keeps satellite, band and SNR only). `SatCn0` has no elevation or azimuth. | MODELLED, advisory |
| Studio | `web/studio/` | No skyplot anywhere; DOP appears only as plotted series of `run` output (`lib/kinds.mjs:212`, `:438-440`, `:460`). No WASM function exposes DOP, azimuth or elevation directly. | n/a |

So the four items below are mostly plumbing and presentation on validated geometry, plus one
honest new classification (item 3) and one new ledger row (item 4).

## 2. Item 1: azimuth-dependent skyline masks on Earth

### 2.1 Scope

One model, `skyline`, that answers "what is the minimum elevation a satellite must clear at
azimuth `az`?" for an Earth-fixed site, used by every Earth kind that has a geodetic site.
Three ways to describe it, which compose by taking the pointwise maximum:

1. **Table:** per-azimuth elevations, linearly interpolated in azimuth with wrap at 360°.
2. **Street canyon:** street azimuth `street_az_deg`, width `width_m`, building heights
   `height_left_m` and `height_right_m`, receiver height `antenna_m`, and optionally a
   finite canyon length `length_m` (open ends). For a LOS at azimuth `az` the distance to the
   facade is `d(az) = (width_m / 2) / |sin(az - street_az)|` and the cut is
   `atan((h - antenna_m) / d(az))`, so the open strip lies along the street. This is a closed
   form, not a ray tracer.
3. **Obstruction profile:** a list of `(az_from, az_to, elevation_deg)` sectors, or a terrain
   profile given as `(az, distance_m, height_m)` points that the engine reduces to a table
   (Earth curvature included: drop `distance² / (2 · R_eff)`, `R_eff = 4/3 R` stated as a
   chosen constant). No DEM is read or downloaded by the engine.

`mask_deg` stays and keeps its meaning: a flat skyline. If both are present the effective cut
is `max(mask_deg, skyline(az))`. A scenario with no `skyline` runs byte-identically to 0.35
(checked by `scripts/check-reproducible.sh` and the existing golden tests).

**Presets** (illustrative dimensions, chosen here and stated in each preset's provenance
string, not surveys of any real place):

| Preset | Model | Stated parameters |
|---|---|---|
| `city-street` | canyon | street 0°, width 20 m, heights 30 m and 30 m, antenna 1.5 m |
| `harbour-approach` | sectors | open to seaward (180° sector), quay buildings 8° over 90° sector, breakwater light 3° |
| `fjord` | terrain profile | steep walls rising to 600 m at 1.5 km either side of a north-south channel; open along the channel axis |
| `container-terminal` | sectors and table | crane booms 25–35° over two narrow sectors, container stacks 12° over the quay side, open to the water |

Earth only. The lunar service volume keeps its scalar mask (out of scope, §8).

### 2.2 Files and modules

- **New** `src/skyline.rs`: `Skyline` (a normalised 1°-bin table, 360 entries, built from any of
  the three descriptions), `SkylineCfg` (serde), `Skyline::cut_deg(az_deg) -> f64`,
  `Skyline::visible(az_deg, el_deg, mask_deg) -> bool`, `Skyline::open_fraction() -> f64`
  (solid-angle fraction of the upper hemisphere above the cut), `presets::{by_name, list}`,
  and the canyon and terrain reductions. A 1° table with linear interpolation is exact for
  tables given on whole degrees; finer tables are kept as given (no resampling) so an oracle
  comparison is not blurred.
- `src/frames.rs`: `is_visible_skyline(station, target, mask_deg, &Skyline)` next to
  `is_visible` (`:208`); `is_visible` becomes the flat-skyline special case.
- `src/gnss_sim.rs` (`:480`, `:741`) and `src/jamming.rs` (`:480`): call the skyline variant.
- `src/leo_fusion/system.rs` `in_view` (`:390`): takes an optional `&Skyline` and the site's
  ENU basis (azimuth needs east and north, which `in_view` does not take today; `geom::enu_at`
  provides them).
- `src/orbit.rs` and `src/raim.rs`: a **new ground-site user mode**, because both kinds model
  an orbiting user with a geocentric elevation and no Earth-fixed azimuth (§1). Add
  `user_site = { lat_deg, lon_deg, height_m }` as an alternative to `user` (exactly one of
  them; a scenario naming both is refused). The site is rotated into the inertial frame by
  the Earth-rotation angle the propagators already use, elevation and azimuth come from the
  geodetic normal (`frames::look_angles`), and `visible_positions*` (`:886`, `:900`) gain the
  skyline. The orbiting-user path is unchanged. This is the largest piece of item 1.
- `src/receiver_trust/` reads a skyline only through item 3.
- `scenarios/`: new `orbit-urban-canyon-skyline.toml`, `integrity-harbour-approach.toml`,
  `leo-pvt-fjord-skyline.toml`; `scenarios/leo-vertical-autonomous-vehicle.toml` gains a
  variant phase with the `city-street` preset beside the existing 35° cut (the 35° phase stays,
  so its published numbers do not move); `automotive-urban-canyon.toml` is **not** changed (it
  has no geometry; its windows stay hand-authored and its header says so).
- Docs: `docs/SCENARIOS.md` (generated and checked by `tests/scenarios_reference_doc_sync.rs`
  and `tests/scenario_count_doc_sync.rs`, so the counts move in the same commit),
  `docs/SCHEMA.md`, `docs/CONCEPTS.md` (a short "geometry" section), `docs/GLOSSARY.md`.

### 2.3 Data model and TOML

```toml
# any Earth kind with a site (gnss-sim, jamming, leo-pvt, leo-pass, and orbit/integrity with user_site)
mask_deg = 5.0                       # unchanged; a flat skyline

[skyline]
preset = "city-street"               # optional; fields below override or add to it
street_az_deg = 35.0
width_m = 18.0
height_left_m = 24.0
height_right_m = 40.0
antenna_m = 1.5
# table = [[0.0, 12.0], [90.0, 40.0], [180.0, 12.0], [270.0, 40.0]]   # [az_deg, el_deg]
# sectors = [{ az_from = 80.0, az_to = 100.0, el_deg = 30.0 }]
# terrain = [{ az = 0.0, distance_m = 1500.0, height_m = 600.0 }]
```

Rules: azimuth in degrees from true north, clockwise; elevations in `[0, 90)`; `table`
azimuths strictly increasing in `[0, 360)`; unknown fields refused (`deny_unknown_fields`, as
`MaritimeConfig` does); at most 3600 table rows and 360 sectors (an input cap, not a
performance figure). Results gain `skyline: { source, open_fraction, provenance }` and, per
sample, `masked_by_skyline` (satellites above `mask_deg` but below the skyline). The result
schema change is additive.

### 2.4 Surfaces (per `docs/SURFACES.md`)

| Surface | Entry point |
|---|---|
| CLI | the `[skyline]` table in any supported scenario (`kshana run`); `kshana skyline --preset <name> --json` prints a preset's table and provenance |
| Rust | `kshana::skyline::{Skyline, SkylineCfg, presets}` |
| Python | through `run` / `run_all` on TOML text; `skyline_presets()` |
| WASM | through `run`; `skyline_presets`, `skyline_profile` (item 2) |
| MCP | through `run_scenario`, `validate_scenario`; `get_example_scenario` serves the new scenarios |
| Plugin | `/kshana-run` covers it; the scenario skill text names `[skyline]` |
| JetBrains | "Run Scenario" covers it (the action runs the CLI on the open TOML); no new action |
| Notebook | a section in a new `notebooks/geometry.ipynb` (shared with item 4) |
| Studio | the Geometry view (item 2) and the existing scenario pages (the new example scenarios) |

### 2.5 Validation: candidate public sources, and the proposal

| Candidate | What it offers | Fit |
|---|---|---|
| **Orekit 12.2** `ElevationMask` with `TopocentricFrame` (Apache-2.0; already the oracle in `tests/leo_polar_coverage_orekit_oracle.rs` and `xval/orekit-passes/`) | An independent azimuth-dependent elevation-mask implementation on the WGS-84 ellipsoid | **Proposed.** Same pattern as the 0.30 polar sweep: both sides read the same committed Earth-fixed satellite states, so only the masked geometry is compared. The interpolation convention between table points must be confirmed to be linear in azimuth with wrap before the tolerances are pre-registered; if it is not, the comparison uses tables whose points fall on every satellite's azimuth bin (exact either way) and says so. |
| gnss_lib_py 1.0.4 | The DOP oracle already | Kept for the DOP of the masked set. This design does not rely on it for the mask itself; whether it offers an azimuth mask is to be checked, not assumed. (Note: `xval/oracles/README.md` lists its licence as MIT and `tests/fixtures/dop/NOTICE` says Apache-2.0; settle that before a new fixture is committed.) |
| Open 3D city models (for example Japan's PLATEAU, the Netherlands' 3D BAG, published CityGML city models) | Real building geometry from which a skyline can be computed | Not a validation: the skyline would be derived by our own reduction. Possible later source of an illustrative real-street preset, licence to be checked first. |
| Open urban GNSS datasets (for example UrbanNav, Hong Kong) | Real urban receiver logs with ground truth | Not a 0.36 oracle: real visibility includes NLOS reception and diffraction, so a mismatch would not isolate a model error, and no per-azimuth skyline is published with the logs as far as this review found. Recorded as a later MEASURED comparison candidate. |
| Copernicus DEM (GLO-30) | Public terrain heights for a fjord profile | Input data, not an oracle; the engine does not download it, and the Studio may not fetch it. |

Honest ledger outcome for item 1:

| Row | Oracle | Kind | Status |
|---|---|---|---|
| Azimuth-dependent skyline mask: visible set, azimuth and elevation, and DOP under the mask | Orekit 12.2 `ElevationMask` + `TopocentricFrame` on committed states; gnss_lib_py 1.0.4 DOP on the masked set; pre-registered tolerances (visible set exact away from a stated boundary band of 1e-6°; DOP 1e-9 relative) | ExternalDataset (basis Library) | **VALIDATED only once the pre-registered comparison passes; MODELLED until then** |
| Street-canyon and terrain reductions to a table | the closed form, by hand (internal) | InternalConsistency | MODELLED |
| Skyline presets (city street, harbour approach, fjord, container terminal) | none: illustrative dimensions | NoneKind | MODELLED |

### 2.6 Test plan

Tests first, values derived by hand: a flat skyline equals `mask_deg` bit for bit on every
existing scenario; a canyon with street azimuth 0° gives cut `atan((h - a) / (w/2))` at 90° and
270° and the scalar mask at 0° and 180°; the terrain drop at 1.5 km matches the hand value;
table interpolation at mid-points and across the 360° wrap; `open_fraction` of a flat 0° mask is
1 and of a uniform 30° cut is `1 - sin 30° = 0.5`; refusal cases (both `user` and `user_site`,
unsorted table, elevation ≥ 90°, unknown field). Property tests: adding obstructions never adds
a satellite; DOP under a skyline is never better than under the flat mask with the same
`mask_deg` (or the geometry has no fix). Then the Orekit fixture
(`tests/skyline_orekit_oracle.rs`, `tests/fixtures/skyline_orekit_oracle/`) and a ground-site
`orbit` run checked against the gnss-sim visibility on the same states.

### 2.7 Risks

The ground-site mode in `orbit`/`integrity` touches the propagation frames; the elevation
convention changes from geocentric to geodetic for that mode (up to about 0.2° at mid-latitudes,
`src/orbit.rs:515-521`), which must be stated in the result. The canyon form ignores
reflections, so a canyon run reads optimistic for NLOS and pessimistic for diffraction; the
result says geometry only. A preset can be mistaken for a survey of a real port: names stay
generic and every result carries the provenance line.

**Effort:** 9 worker-days (skyline core and presets 3, ground-site mode 3, wiring and scenarios
1.5, Orekit fixture and row 1.5).

## 3. Item 2: the Studio Geometry view

### 3.1 Scope

A new Studio page `geometry/` built in the kshana-pro `site-next` Studio source and ported into
`web/studio` with `web/tools/port_site.py` (a hand edit to `web/studio` fails the
`web/PORT-MANIFEST.json` check in `web/site.test.mjs`). It shows:

- a **skyplot** (azimuth around, elevation inward) with satellites coloured by system, the
  skyline drawn as a filled polygon, satellites below the skyline hollow;
- **inputs:** site (latitude, longitude, height), time offset, toggles for GPS, Galileo, BeiDou
  and a LEO layer (one of the `leo_fusion` presets), a scalar mask, UERE, and the skyline: the
  four presets, a canyon form, or freehand drawing on the skyplot (drag points, snapped to 1°
  azimuth and 0.5° elevation);
- **live DOP:** GDOP, PDOP, HDOP, VDOP, TDOP, satellites in view and masked, and the open-sky
  fraction;
- the **horizontal error ellipse** from the ENU covariance `σ²_UERE · Q_EN` (semi-axes from the
  2 × 2 eigen decomposition, orientation from north, drawn at 1-sigma and 95 %, with the 95 %
  scale `√5.991 = 2.448` stated);
- a **Monte Carlo cloud:** `n` LS solutions with independent Gaussian range errors of
  `σ_UERE`, drawn as east-north points over the ellipse, with the empirical RMS beside
  `σ_UERE × HDOP` and the fraction inside the 95 % ellipse. This is the item-4 computation, run
  by the engine, not a JavaScript re-implementation.

Honesty on the sky: the constellations are the engine's presets (`src/constellation.rs`),
whose relative phases come from different reference epochs, so **the picture is a
representative geometry, not the sky of that date at that place**, and the page says so above
the plot. A user may paste TLE text (local, never fetched) for a dated sky; that text is not put
in the URL (§3.4).

### 3.2 Engine functions (new WASM exports, for `ENGINE_FNS`)

| Function | Input → output | Notes |
|---|---|---|
| `sky_geometry(request_json)` | site, time, systems (preset names or TLE text), mask, skyline, UERE → per-satellite az/el/system/visible/masked, DOP five, ENU covariance (4 × 4, one clock per system), ellipse axes and angle | wraps a new `orbit::dop_cov` (covariance exposed; `orbit::dop` becomes a thin reduction of it, so the VALIDATED scalars are unchanged and still pinned by `tests/dop_reference.rs`) and `leo_fusion::joint_pvt` for one clock per system |
| `sky_monte_carlo(request_json, seed, n)` | same plus seed and `n` → east-north-up error samples, empirical covariance, RMS, 95 % containment | `n` capped at 5000 in the browser (an input cap); seeded `ChaCha8Rng`, so a shared URL reproduces the cloud exactly |
| `skyline_presets()` | → names, tables, provenance | |
| `skyline_profile(skyline_json)` | canyon / sectors / terrain → the normalised 360-entry table | lets the page draw exactly what the engine will use |
| `skyplot_svg(request_json)` | → an SVG skyplot | for export; rendered only through `svgsafe.parseSvgMarkup` + `sanitizeSvg` |

All five go into the frozen `ENGINE_FNS` list in `web/studio/lib/engine.mjs` (after a
`// 0.36` comment, with an `ENGINE_FNS_036` slice like `ENGINE_FNS_035`), into
`web/studio/lib/engine.test.mjs`, and into `src/wasm.rs` through `kshana::surface` so Python and
MCP call the same code. A page loaded against an older engine shows "needs engine 0.36" from
`engineAtLeast` instead of failing.

### 3.3 Files

kshana-pro `site-next` Studio source (ported): `geometry/index.html`, `geometry/geometry.js`,
`geometry/geometry.css`, `lib/skyplot.mjs` and `lib/skyplot.test.mjs` (pure drawing from engine
JSON), `lib/skyline-edit.mjs` and test (drawing and snapping), `lib/geometry-url.mjs` and test.
This repo: `src/wasm.rs`, `src/surface.rs`, `web/studio/lib/engine.mjs`, `web/site.test.mjs`
(`VIEWS` gains `studio/geometry/index.html`), `.github/workflows/ci.yml` (the new `*.test.mjs`
files must be listed, `:362-366`), `web/PORT-MANIFEST.json` (regenerated by the port),
`docs/SURFACES.md` (the Studio page list and a row).

### 3.4 State in the URL

Every state is in the query string, read and written through `lib/urlstate.mjs`
(`parseState`, `buildSearch`), with `history.replaceState` on change:

`?site=59.91,10.75,5&t=3600&sys=G,E,C,L:xona-pulsar&mask=5&uere=1.5&sky=p:fjord&mc=2000&seed=36`

A drawn skyline is `sky=t:<base64url>` (one byte per 1° bin, elevation in 0.5° steps, 360 bytes
→ 480 characters). Every parameter is parsed against bounds and a length cap; anything out of
range is dropped with a visible notice, never interpreted. Pasted TLE text is not encoded;
the URL then carries `sys=tle` and a shared link opens on the presets with a notice that the
sender used their own elements.

### 3.5 Security rules (unchanged, applied)

No remote data: the page fetches nothing beyond its own ported files and the engine; the
`web/site.test.mjs` remote-address scan covers the new files. Engine SVG passes through
`svgsafe`. Worker messages go through `classifyMessage` and `dispatch` (only listed names, at
most 16 arguments). Pasted text is capped at the existing 4 MiB input limit.

### 3.6 Advisory statement

The page carries, as the trust page does, the full statement:

> Advisory software, not type-approved navigation equipment (IEC 61108, IEC 61162): the operator remains responsible for the navigation of the vessel.

and states beside the numbers that DOP and the cloud are **geometry under a stated error
model** (independent, equal, zero-mean Gaussian range errors), not a prediction of a receiver's
real error, which also has biases, multipath and NLOS errors.

### 3.7 Surfaces

Studio (this page); WASM (the five functions); Python `sky_geometry`, `sky_monte_carlo`,
`skyline_presets`; MCP one new tool `sky_geometry` (returns the snapshot and the advisory; no
Monte Carlo over MCP, because the samples are bulk numbers an agent should summarise, not
carry: N/A with that reason in `docs/SURFACES.md`); CLI `kshana sky --site ... --json`
(`--monte-carlo N --seed S`); Plugin: `/kshana-run` skill text points to `sky_geometry`;
JetBrains: **N/A** (an interactive skyplot is a browser view; the CLI command covers the
numbers); Notebook: `notebooks/geometry.ipynb`.

### 3.8 Tests, ledger, risks

Tests: `lib/*.test.mjs` for drawing, URL round trip (state → URL → state is the identity),
bounds refusal and snapping; `engine.test.mjs` for the allow-list; Rust tests that
`sky_geometry` equals `orbit::dop` on the same visible set and that `dop_cov` reduces to the five
scalars to 1e-12; the Studio selfcheck re-recorded. Ledger: one candidate row, **"Horizontal
error ellipse from the geometry covariance"** (ENU rotation of Q and 2 × 2 eigen axes), checked
against numpy `linalg.eigh` as the Fisher-information row is: **VALIDATED once that comparison
passes** (ExternalDataset, basis Library), MODELLED until then. The view itself adds no row.
Risks: readers taking a representative sky for a real one (mitigated by the notice); URL
parameters as an attack surface (bounds and caps, tests); engine version skew (feature check).

**Effort:** 8 worker-days (engine functions and `dop_cov` 2.5, Studio page and libraries 4,
port, CI wiring and selfcheck 1.5).

## 4. Item 3: geometry-aware maritime trust

### 4.1 Scope

Today a masked sky reaches the score only indirectly: fewer satellites raise `LossOfLock`
(weight 25), and a noisier fix can push the kinematic and position-jump checks. That reads as
attack-like inconsistency when it is natural degradation. 0.36 adds a separate **geometry**
assessment with its own cause class, explicit thresholds, and provenance for every number.

### 4.2 Data model

- `SatCn0` gains `el_deg: Option<f64>` and `az_deg: Option<f64>` (filled from NMEA GSV,
  u-blox NAV-SAT and the Android logger's `Status` rows, which the reader skips today, where
  the log carries them; serde-default, so old JSON reads).
- `MarineObs::hdop` (already parsed) is used; GSA is parsed for PDOP, HDOP and VDOP.
- A new per-epoch `GeometryObs { hdop, pdop, vdop, n_used, open_fraction, source }` where
  `source` is `reported` (the receiver's GGA/GSA), `derived` (the engine's DOP from the logged
  azimuths and elevations through `orbit::dop` on unit LOS vectors) or `engine` (the RINEX
  engine fix's own geometry).
- A new `Monitor::Geometry` and a `class` on every `Deduction`: `geometry`, `environment`
  (C/N0, AGC, jamming indicator, loss of lock) or `inconsistency` (kinematic, heading, speed
  log, sea level, time, clock, RAIM, position jump, OSNMA, solve failure). The band edges and the
  existing weights do not change.

`[geometry]` table, every default stated with its reason in the result:

| Field | Default | Reason |
|---|---|---|
| `hdop_max` | 4.0 | at the stated `sigma_uere_m` of 1.5 m, HDOP 4 is a 2drms of 12 m, equal to the kinematic check's own 12 m allowance (`MaritimeConfig::kin_pos_tol_m`); above it, geometry noise alone can approach that allowance. Chosen here, not a standard's figure. |
| `pdop_max` | 6.0 | the PDOP availability threshold the bundled `leo-pvt` polar scenario already uses |
| `min_used` | 5 | one more than a fix needs, so a single loss does not end the fix |
| `sigma_uere_m` | 1.5 | stated input for the 2drms link above; the operator should set their receiver's figure |
| `skyline` | none | optional, item 1; gives `open_fraction` and an expected visible count |
| `weight` | 15 | geometry alone moves the score from nominal (≥ 90) into degraded, never to untrusted (< 55) |
| `reported_vs_derived_tol` | 1.0 HDOP | a reported HDOP that disagrees with the derived one by more is not geometry: it is raised as an `inconsistency` |

### 4.3 Behaviour

At each epoch the geometry ratio is `max(hdop / hdop_max, pdop / pdop_max, min_used / n_used)`
and enters the existing `points()` mapping with weight 15 and class `geometry`. A reason reads,
for example, "degraded by geometry: HDOP 5.2 above 4.0 (derived from 7 logged satellites);
not an inconsistency". `LossOfLock` is reclassified `geometry` when the lost satellites were
below the configured skyline or below 15° elevation at their last report; otherwise it stays
`environment`. Nothing widens an inconsistency check automatically: a spoofer could fake a
poor geometry (false GSV, a reported high HDOP) to buy tolerance, so the design labels and
does not forgive. The derived-versus-reported comparison is the guard.

The trust summary reports epochs per class beside the band counts. A degradation the score
classes as geometry is a statement about the sky, not a finding that no attack occurred; the
monitors still cannot see a spoofer consistent with every other sensor (`docs/AGENTS.md`).

### 4.4 Flow into telemetry, evidence packs and the Studio trust view

- **Live JSON lines:** schema 1.3, adding `geometry` and each deduction's `class` (1.2 readers
  ignore unknown keys; the `advisory` stays on every epoch).
- **Telemetry** (`src/telemetry/`): Prometheus `kshana_trust_geometry_hdop`,
  `kshana_trust_geometry_pdop`, `kshana_trust_geometry_used_satellites`,
  `kshana_trust_geometry_degraded` (0/1), and a `class` label on `kshana_trust_reason_active`;
  CEF/LEEF extension keys `geoHdop`, `geoClass`; OTLP attributes `kshana.geometry.*`. The
  references in `scripts/gen_telemetry_formats_ref.py` are regenerated and the format oracle
  rerun.
- **Evidence packs** (`src/evidence/`): the manifest records the `[geometry]` table actually
  used and its provenance; `summary.html` shows a geometry lane separate from the inconsistency
  lane; verification of 0.35 packs is unchanged (the new fields are optional), with a test.
- **Studio trust view** (`trust/` in site-next, ported): a geometry lane in the timeline in a
  colour distinct from inconsistency and environment, a legend that says "geometry: the sky,
  not the signal", and the per-class counts. The existing advisory note stays.

The statement carried on every one of these surfaces, verbatim (`receiver_trust::ADVISORY`,
enforced by `tests/advisory_statement_sync.rs`, which gains the new functions):

> Advisory software, not type-approved navigation equipment (IEC 61108, IEC 61162): the operator remains responsible for the navigation of the vessel.

### 4.5 Files, surfaces, tests, ledger

Files: `src/receiver_trust/{mod.rs, ingest.rs, monitors.rs, maritime.rs, score.rs,
scenario.rs, live.rs, synth.rs}`, `src/telemetry/{prometheus.rs, syslog.rs, otlp.rs}`,
`src/evidence/{assemble.rs, html.rs}`, `src/nmea_synth/` (a natural `geometry` event: a fjord
or terminal passage that removes low satellites and raises HDOP, so training NMEA can teach the
difference), `docs/RECEIVER-TRUST.md`, `docs/MARITIME-TRUST.md`, `docs/TRUST-TELEMETRY.md`,
`docs/EVIDENCE-PACKS.md`, `docs/NMEA-TRAINING.md`.

Surfaces: every existing receiver-trust entry point carries it with no new function
(CLI `receiver-trust`, Rust, Python, WASM, MCP `assess_*`, plugin skill
`vessel-receiver-trust`, JetBrains "Assess Receiver Trust", Studio trust page); telemetry and
evidence as above.

Tests: hand-derived ratios at the thresholds; the 0.35 golden trust results unchanged when the
log carries no geometry (old logs score identically); a synthetic fjord passage yields only
`geometry` deductions and stays in the degraded band; a synthetic position push during poor
geometry still raises `inconsistency` and reaches untrusted; a log whose reported HDOP
contradicts its GSV raises `inconsistency`; schema 1.2 reader on 1.3 output; old evidence packs
verify.

Ledger: **"Geometry-aware trust classification"**: MODELLED (InternalConsistency). The
thresholds are chosen, not fitted, and there is no external labelled dataset of natural
geometry degradation against attacks to validate the classification. The derived DOP reuses
the VALIDATED kernel, which this row does not borrow.

Risks: an operator may read "geometry" as "safe"; the reason text and docs say it is not.
Receivers report HDOP differently (some cap it, some report 99.9 when no fix); `reported`
values are treated as the receiver's claim and compared, not trusted.

**Effort:** 7 worker-days (ingest and data model 1.5, monitor and score 2, telemetry and
evidence 1.5, training event 1, Studio trust lane 1).

## 5. Item 4: the ledger row "position error ≈ UERE × DOP"

### 5.1 The claim, scoped

Under the stated model (independent, zero-mean Gaussian range errors of equal variance
`σ²`), the LS position and clock estimate has covariance `σ² Q` with `Q = (HᵀH)⁻¹`, so the
per-axis RMS error is `σ × DOP`. With weights (`σ_i = σ₀ / sin el_i`, say), `Q_w = (HᵀWH)⁻¹`.
The row checks that the engine's LS solver and seeded noise generator realise this
covariance; it does not claim a real receiver's error follows it.

### 5.2 Method

`tests/uere_dop_monte_carlo.rs`: for each of the **8 geometries of `tests/dop_reference.rs`**
(the ones whose DOP is already matched to gnss_lib_py) plus two masked-skyline geometries from
item 1, place satellites at their az/el at GNSS range, draw `N = 20 000` epochs of range errors
`ε ~ N(0, σ²)` with `σ = 1.5 m` from `ChaCha8Rng::seed_from_u64(36)` (seed derivation per
geometry stated in the test), solve each epoch with the engine's iterative
`pvt::solve_spp` from the true position, and compare the ENU errors with `σ² Q`. The
linearisation error at metre-level noise and 2 × 10⁷ m range is of order `σ²/R ≈ 1e-7 m`,
negligible against the tolerances.

Tolerances, fixed in a pre-registration commit before the first run:

| Quantity | Tolerance | Basis |
|---|---|---|
| each ENU variance and clock variance vs `σ² Q_ii` | relative 5 % | 5 standard errors of a sample variance, `√(2/N) = 1.0 %` |
| RMS vs `σ × HDOP`, `σ × VDOP`, `σ × PDOP` | relative 2.5 % | half the variance bound |
| east-north correlation vs Q | absolute 0.035 | `5/√N` |
| mean error per axis | `5 σ_i / √N` | unbiasedness |
| fraction inside the 95 % horizontal ellipse | 0.95 ± 0.0077 | 5 binomial standard errors |

Negative controls (the test must be able to fail): with elevation-dependent errors and the
unweighted Q the variance check fails on at least the low-elevation geometries; with PDOP
doubled in a scratch copy every geometry fails. This answers the 0.30 finding that a one-sided
floor could not detect a 100 % PDOP error.

### 5.3 How it composes, and its honest status

The row names the VALIDATED DOP row as the source of `Q` and does not inherit its status. The
MC agreement is the engine's estimator and noise checked against the closed-form covariance
of the same model, which is an internal identity.

| Row | Oracle | Kind | Status |
|---|---|---|---|
| Position error ≈ UERE × DOP (LS Monte Carlo vs analytic covariance) | the closed-form LS covariance `σ²(HᵀH)⁻¹` on the gnss_lib_py-validated geometries; seed 36, N 20 000, tolerances above | InternalConsistency | **MODELLED** |

It stays MODELLED because no external dataset checks it: agreement with the analytic
covariance shows the estimator and noise are implemented correctly, not that any receiver's
error equals UERE × DOP. Surfaces: the test and the ledger row (generated into
`docs/VERIFICATION-MATRIX.md`, `docs/MODELLED-RATIONALE.md` and
`web/data/verification-matrix.json` by `cargo run --bin gen_validation_artifacts`, never by
hand); the computation itself is reachable as `sky_monte_carlo` (item 2). The row must name a
test that exists (`tests/verification_rows_name_a_test_that_exists.rs`).

**Effort:** 2 worker-days.

## 6. Ordering and branch plan

All worker branches start from the 0.36 integration branch the coordinator names (cut from
`release/0.35.0-rc` at `35c1558d` or from `main` after the 0.35.0 tag) and merge back into it.

| Branch | Worker | Content | Depends on | Parallel with |
|---|---|---|---|---|
| `feat/0.36-skyline-core` | A | `src/skyline.rs`, presets, `frames::is_visible_skyline`, `orbit::dop_cov` (with `orbit::dop` as its reduction) | none | test/0.36-uere-dop-row, feat/0.36-geometry-trust (ingest part) |
| `test/0.36-uere-dop-row` | B | item 4: pre-registration commit, MC test, ledger row | none (uses `pvt::solve_spp` and the existing fixtures) | everything |
| `feat/0.36-skyline-wiring` | A | ground-site mode in `orbit`/`integrity`, gnss-sim, jamming, leo-pvt/leo-pass wiring, new scenarios, Orekit fixture and row | skyline-core | geometry-wasm, geometry-trust |
| `feat/0.36-geometry-wasm` | C | the five engine functions in `surface`, `wasm`, Python, MCP `sky_geometry`, CLI `kshana sky`, `ENGINE_FNS`, numpy ellipse row | skyline-core | skyline-wiring, geometry-trust |
| `feat/0.36-geometry-trust` | D | item 3: ingest of az/el and GSA first (no dependency), then monitor, score, telemetry, evidence, training event | skyline-core for the optional skyline only | skyline-wiring, geometry-wasm |
| kshana-pro `ws/studio-geometry` | E | the Geometry page and the trust geometry lane in `site-next`, then the port into `web/studio` | geometry-wasm (functions), geometry-trust (schema 1.3) | none at the end; the page can be built against the function contract from day one |
| `docs/0.36-geometry-surfaces` | final pass | `docs/SURFACES.md`, `docs/AGENTS.md`, CONCEPTS, CHANGELOG `[Unreleased]`, regenerated counts and figures | all of the above | none |

Critical path: skyline-core (3 days) → geometry-wasm (2.5) → Studio port (4 + 1.5), about 11
worker-days elapsed with the other branches alongside. Total effort about 28 worker-days
including the 2-day docs pass.

## 7. 0.36 definition of done (geometry)

Format of the kshana-pro 0.35 definition of done. Evidence (commit or run id) is recorded per
row before the release PR opens.

| # | Area | Done when | Owner | Status |
|---|------|-----------|-------|--------|
| 1 | Skyline model | `src/skyline.rs` with table, canyon, sectors and terrain; four maritime/street presets with provenance; `mask_deg` unchanged and every 0.35 golden byte-identical | A | not started |
| 2 | Skyline wiring | ground-site mode in `orbit` and `integrity`; skyline in gnss-sim, jamming, leo-pvt, leo-pass; three new scenarios; SCENARIOS.md and counts regenerated | A | not started |
| 3 | Skyline validation | Orekit 12.2 `ElevationMask` comparison pre-registered and run; row VALIDATED only if it passes, otherwise MODELLED with the finding written into the row | A, critic | not started |
| 4 | Engine functions | `sky_geometry`, `sky_monte_carlo`, `skyline_presets`, `skyline_profile`, `skyplot_svg` on WASM, Python, CLI; MCP `sky_geometry`; `ENGINE_FNS` and its test updated | C | not started |
| 5 | Studio Geometry view | page ported via `port_site.py`; in `VIEWS`; URL round trip, bounds and svgsafe tests green; representative-sky notice and advisory statement shown; selfcheck re-recorded | E | not started |
| 6 | Geometry-aware trust | `Monitor::Geometry`, deduction classes, `[geometry]` thresholds with reasons; schema 1.3; telemetry, evidence pack and Studio trust lane; 0.35 trust goldens unchanged | D | not started |
| 7 | UERE × DOP row | pre-registered MC test with negative controls; row MODELLED (InternalConsistency); artifacts regenerated | B | not started |
| 8 | Surfaces | every new capability on CLI, Rust, Python, WASM, MCP, plugin, JetBrains, Notebook, Studio or an N/A reason in SURFACES.md | final pass | not started |
| 9 | Docs and figures | CONCEPTS geometry section, RECEIVER-TRUST, MARITIME-TRUST, TRUST-TELEMETRY, EVIDENCE-PACKS, SCHEMA, CHANGELOG; ledger counts, kinds and MCP tool count regenerated everywhere | final pass | not started |
| 10 | Rules | no attribution, no personal names, the advisory statement on every score and trust surface, no unverifiable performance claims, VALIDATED only with an ExternalDataset oracle | all, critic | continuous |
| 11 | Gates | `scripts/gate.sh` receipt; full CI green on the integration head; `scripts/check-no-attribution.sh`, version sync and doc coverage green | coordinator | after 1–10 |

## 8. Explicitly out of scope

- Ray tracing, reflections, NLOS reception, diffraction and multipath from the skyline; a
  skyline removes LOS satellites and nothing more.
- Reading or downloading 3D city models or DEMs in the engine or the Studio; a terrain profile
  is text the user supplies.
- A real-date sky in the Studio from fetched ephemerides; only user-pasted TLE text.
- Skylines on the Moon or Mars (the lunar service volume keeps its scalar mask) and in the IQ
  scene layer (`src/iq/scene/`, `src/interop/scene.rs`).
- Changing `automotive-urban-canyon`, `ins-trn-coast` or `spoof-detect`, whose GNSS is
  hand-authored windows with no geometry.
- Using a skyline to detect spoofing (for example "a satellite tracked below the skyline"), and
  any anti-spoofing or anti-jamming performance figure.
- A validation against a real urban GNSS dataset (recorded as a later MEASURED candidate).
- Any claim that UERE × DOP predicts a real receiver's error, any type approval, and any claim
  of conformity with IEC 61108 or IEC 61162.
- Changing the band edges or existing weights of the trust score.
