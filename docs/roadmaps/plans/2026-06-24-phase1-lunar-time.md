# Phase 1 — Lunar Coordinate Time: Implementation Plan

> Built TDD, bite-sized tasks, frequent commits. Capability code → PUBLIC `kshana`. Plan → PRIVATE.

**Goal:** add `src/lunar_time.rs` — a lunar coordinate time scale (LTC/TCL) and the relativistic
Earth↔Moon rate — wired as a runnable `lunar-time-offset` scenario with an honest verification row.

**Architecture:** first-principles relativistic clock-rate from named physical terms (self-potential
difference + Moon–Earth kinetic term), reported with an uncertainty band, cross-checked against the
published lunar-clock-rate figure (~56–59 µs/day). Conversions chain to TT/TAI/UTC via `timescales`.
A minimal inverse-variance ensemble forms a lunar paper-clock.

**Tech stack:** Rust, no new deps. Reuses `ephem` (Montenbruck–Gill Moon position), `forces`/`body`
constants (`MU_MOON`, `RE` ), `timetransfer_adv::C_M_PER_S`, `timescales` (JD f64 conversions).

**Honesty:** `Modelled`, oracle `InternalConsistency` (closed-form relativistic identity) + literature
cross-check. The headline µs/day figure is **reference-dependent**; always report the band + chosen
reference (Earth geoid W₀ vs lunar selenoid). No claim of sub-ns absolute accuracy.

---

## Physical model (reference)

Rate of a clock vs a coordinate time: `dτ/dt = 1 − (U + v²/2)/c²` (U = +GM/r, positive).
Secular rate of a lunar-surface clock relative to an Earth-geoid clock (TT):

```
d(LTC − TT)/dt ≈ (1/c²) [ (W0_earth − U_moon_self) − <v_rel²>/2 ]
```

- `W0_earth` = IAU/IERS conventional geopotential at the geoid = `L_G · c²`,
  with `L_G = 6.969290134e-10` (IAU defining constant) → `W0 ≈ 6.26369e7 m²/s²`.
- `U_moon_self` = `MU_MOON / RE_MOON ≈ 4.902800066e12 / 1.7374e6 ≈ 2.82214e6 m²/s²`
  (centrifugal + J2 corrections are < 1e-3 µs/day → folded into the band, not modelled).
- `<v_rel²>` = time-averaged squared Moon–Earth relative velocity (≈ (1.02 km/s)²); reduces the gain
  by ~0.5 µs/day.
- Dominant term alone: `(6.26369e7 − 2.82214e6)/8.987551787e16 × 86400 ≈ 57.50 µs/day`.
- Reported band: **[56.0, 59.0] µs/day** (covers geoid/selenoid reference + averaging choices).

`c² = 8.987551787368176e16 m²/s²`.

---

## File structure

- Create: `src/lunar_time.rs` (rate model + conversions + ensemble + scenario + svg)
- Modify: `src/lib.rs` (`pub mod lunar_time;`, alphabetical: after `lunar_od`, before `maneuver`)
- Modify: `src/api.rs` (`ScenarioKind::LunarTime` variant + classify + dispatch arm + metadata)
- Modify: `src/verification.rs` (one new `VerificationItem`)
- Modify: `README.md` (one capability-matrix row) + `docs/VALIDATION.md` (prose)
- Tests live in `src/lunar_time.rs` `#[cfg(test)] mod tests` (module-local, matching crate style)

---

## Tasks

### Task 1 — Module scaffold + constants + lib.rs registration
- [ ] Create `src/lunar_time.rs` with SPDX header `// SPDX-License-Identifier: AGPL-3.0-only`, a
      module doc comment (what it is, honesty caveat), and constants:
  ```rust
  /// IAU defining constant L_G = (rate of TT vs TCG); W0_earth = L_G * c^2.
  pub const L_G: f64 = 6.969_290_134e-10;
  pub const C2_M2_S2: f64 = 299_792_458.0 * 299_792_458.0; // c^2
  pub const W0_EARTH_M2_S2: f64 = L_G * C2_M2_S2;           // ~6.26369e7
  pub const RE_MOON_M: f64 = 1_737_400.0;                   // IAU mean lunar radius
  pub const RATE_BAND_LOW_US_DAY: f64 = 56.0;
  pub const RATE_BAND_HIGH_US_DAY: f64 = 59.0;
  ```
- [ ] Add `pub mod lunar_time;` to `src/lib.rs` (after `pub mod lunar_od;`).
- [ ] Add a trivial test asserting `W0_EARTH_M2_S2` ≈ 6.26369e7 (rel 1e-4). Run `cargo test -p kshana lunar_time` → PASS. Commit.

### Task 2 — Self-potential secular rate (TDD)
- [ ] **RED**: test `self_potential_rate_is_in_band`: `let r = self_potential_rate_us_per_day(); assert!((r - 57.5).abs() < 0.5);` Run → fails (fn missing).
- [ ] **GREEN**: implement
  ```rust
  pub fn self_potential_rate_us_per_day() -> f64 {
      let u_moon = crate::forces::MU_MOON / RE_MOON_M;
      ((W0_EARTH_M2_S2 - u_moon) / C2_M2_S2) * 86_400.0 * 1e6
  }
  ```
  (Confirm `crate::forces::MU_MOON` is `pub`; if not, use `crate::body::Body::moon().mu`.)
- [ ] Run → PASS. Commit.

### Task 3 — Moon geocentric velocity + kinetic term (TDD)
- [ ] **RED**: test `moon_speed_is_about_1km_s`: finite-difference `crate::ephem::moon_position`
      at a sample `t_tt_jc`, assert speed ∈ [0.8, 1.3] km/s.
- [ ] **GREEN**: `pub fn moon_geocentric_velocity_m_s(t_tt_jc: f64) -> [f64;3]` via central difference
      (`dt` = 60 s expressed in Julian centuries: `dt_jc = 60.0 / 86400.0 / 36525.0`).
- [ ] **RED**: test `kinetic_term_small_negative`: `kinetic_rate_us_per_day(t)` ∈ [−1.0, 0.0].
- [ ] **GREEN**: `kinetic_rate_us_per_day(t) = -(v·v)/(2*C2) * 86400 * 1e6` using the velocity above.
- [ ] Run → PASS. Commit.

### Task 4 — Total rate + breakdown + band (TDD)
- [ ] **RED**: test `total_rate_in_published_band`: `let b = lunar_rate_breakdown(t);
      assert!(b.total_us_per_day >= 56.0 && b.total_us_per_day <= 59.0);
      assert!((b.self_potential + b.kinetic - b.total_us_per_day).abs() < 1e-9);`
- [ ] **GREEN**:
  ```rust
  pub struct LunarRateBreakdown {
      pub self_potential: f64, pub kinetic: f64, pub total_us_per_day: f64,
      pub band_low: f64, pub band_high: f64,
  }
  pub fn lunar_rate_breakdown(t_tt_jc: f64) -> LunarRateBreakdown { /* sum terms, attach band */ }
  ```
- [ ] Run → PASS. Commit.

### Task 5 — TT↔LTC conversions (TDD)
- [ ] **RED**: test `ltc_offset_over_a_day_is_about_57us` and `tt_ltc_roundtrip_under_1ns`:
      pick `epoch_jd_tt`, compute `ltc = tt_to_ltc(epoch + 1 day, epoch)`, assert offset ≈ 57e-6 s
      within band; assert `(ltc_to_tt(tt_to_ltc(x,e),e) - x)·86400 < 1e-9`.
- [ ] **GREEN**: LTC = TT + rate·Δt(seconds since `ltc_epoch_jd_tt`), rate from `lunar_rate_breakdown`
      total converted to s/s; provide `tt_to_ltc(jd_tt, ltc_epoch_jd_tt)` and inverse.
- [ ] Run → PASS. Commit.

### Task 6 — Minimal lunar ensemble timescale (TDD)
- [ ] **RED**: test `ensemble_is_inverse_variance_weighted`: two clocks offsets [a,b] with variances
      [va,vb] → ensemble = (a/va + b/vb)/(1/va+1/vb); assert equals hand value; ensemble variance =
      1/(1/va+1/vb).
- [ ] **GREEN**:
  ```rust
  pub struct EnsembleTime { pub mean_offset_s: f64, pub variance_s2: f64 }
  pub fn lunar_ensemble(offsets_s: &[f64], variances_s2: &[f64]) -> EnsembleTime { /* inv-var */ }
  ```
  (Guard empty / non-positive variance → return a clear error or skip; document.)
- [ ] Run → PASS. Commit.

### Task 7 — Scenario + SVG + api.rs wiring (TDD)
- [ ] Define in `lunar_time.rs`:
  ```rust
  #[derive(Clone, Copy, Debug, serde::Deserialize)]
  pub struct LunarTimeScenario {
      #[serde(default = "d_epoch_y")] pub epoch_year: i32, /* + month/day defaults */
      #[serde(default = "d_horizon_days")] pub horizon_days: f64,
  }
  #[derive(Clone, Debug, serde::Serialize)]
  pub struct LunarTimeReport {
      pub secular_rate_us_per_day: f64, pub band_low: f64, pub band_high: f64,
      pub self_potential_us_per_day: f64, pub kinetic_us_per_day: f64,
      pub horizon_days: f64, pub offset_at_horizon_us: f64,
  }
  impl LunarTimeScenario { pub fn run(&self) -> LunarTimeReport { /* compute */ } }
  pub fn lunar_time_svg(r: &LunarTimeReport) -> String { /* accumulated-offset line */ }
  ```
- [ ] In `src/api.rs`: add `LunarTime` to the `ScenarioKind` enum + its string classify
      (`"lunar-time-offset"`), add the dispatch arm mirroring `LunarIntegrity`
      (`toml::from_str` → `run()` → `RunOutput { json: json_of(&report), svg, summary }`), and any
      metadata table entry the other kinds have.
- [ ] **RED→GREEN**: test `run_toml_lunar_time_offset`: feed a minimal TOML
      `kind = "lunar-time-offset"\nhorizon_days = 1.0`, assert summary contains `lunar-time-offset`
      and the JSON `secular_rate_us_per_day` ∈ band.
- [ ] Run → PASS. Commit.

### Task 8 — Verification row + no-overclaims (TDD)
- [ ] Add to `verification_matrix()` in `src/verification.rs`:
  ```rust
  VerificationItem {
      requirement: "Lunar coordinate time",
      capability: "Relativistic Earth–Moon clock-rate (LTC/TCL) + TT/TAI/UTC chaining + ensemble",
      module: "lunar_time",
      tests: "lunar_time::tests (closed-form rate; published 56–59 µs/day cross-check; round-trip)",
      oracle: "Closed-form relativistic rate (IAU L_G / IERS Conv.) vs published lunar-clock-rate band",
      oracle_kind: OracleKind::InternalConsistency,
      status: VerificationStatus::Modelled,
  },
  ```
- [ ] Run `cargo test -p kshana` incl. `tests/no_overclaims.rs` + verification invariants → PASS.
- [ ] Commit.

### Task 9 — Docs (PUBLIC, honest)
- [ ] Add one README capability-matrix row for lunar coordinate time (status MODELLED, the band).
- [ ] Add a short `docs/VALIDATION.md` paragraph (model, oracle, reference-dependence caveat).
- [ ] Commit.

### Task 10 — Gate + banned scan + local commit (NO push)
- [ ] `cargo fmt --all` ; `cargo fmt --all --check`.
- [ ] `cargo clippy --all-targets -- -D warnings`.
- [ ] `cargo test -p kshana` (full) → green.
- [ ] Banned-text scan on staged diff: no AI-tool/vendor fingerprints (the four forbidden substrings) or personal email.
- [ ] Stage explicit paths only; commit authored `Chakshu Baweja <contact@ashforde.org>`, no footer.
      **Do not push** until the founder says.

---

## Self-review
- Spec coverage: rate ✓ (T2–4), LTC conversions ✓ (T5), ensemble ✓ (T6), scenario ✓ (T7), honesty
  row ✓ (T8). All Phase-1 spec bullets covered.
- Placeholders: none — every code step shows the code or exact signature.
- Type consistency: `LunarRateBreakdown`, `LunarTimeReport`, `EnsembleTime` names used consistently.
- Pitfall guard: reference-dependence band enforced by the in-band assertions; velocity by
  finite-difference since `ephem` is position-only.
