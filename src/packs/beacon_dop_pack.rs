// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The `pro-beacon-dop` (L45) and `pro-surface-beacon-dop-study` (L46) packs —
//! compose the **open** P2 surface-beacon-DOP analyses ([`kshana::lunar_beacon`],
//! [`kshana::lunar_service`], [`kshana::lunar`]) into two paper-grade deliverables.
//!
//! ## L45 — `pro-beacon-dop`
//! Sweeps [`kshana::lunar_beacon::dop_with_beacons`] across (a) a south-polar user
//! latitude band (−70…−90°) and (b) surface-beacon geometry (count, ring radius,
//! terrain/mast height, azimuth spread), reporting per-config **median GDOP / HDOP /
//! VDOP** — separately, never a lone scalar GDOP — plus the **realized position
//! accuracy in metres** ([`BeaconErrorBudget`] σ_URE × the DOP, via
//! [`realized_accuracy`]). Each per-config median carries a **seeded, deterministic**
//! bootstrap confidence interval. This replaces P2's single −80° scalar-GDOP run.
//!
//! ## L46 — `pro-surface-beacon-dop-study`
//! Orchestrates the three P2 Figure-1 mechanisms: the constellation-size sweep
//! ([`kshana::lunar_service::sweep_over_n`], the GDOP-vs-N curve with the 4–6
//! planned-fleet band and the N≈12 GDOP-6 crossing), the polar spatial GDOP map
//! ([`kshana::lunar::lunar_gdop_map`]), and the beacon before/after
//! (`dop_with_beacons` vs orbit-only). It emits a committed, byte-stable
//! `lunar_dop_summary.csv`, renders P2 Figure 1 (spatial-map SVG + GDOP-vs-N SVG +
//! before/after SVG) and a branded HTML report.
//!
//! ## What "Validated" vs "Modelled" means here (load-bearing — honesty discipline)
//! * **Validated** — the per-config DOP *inherits the DOP kernel* ([`kshana::orbit::dop`],
//!   cross-checked against `gnss_lib_py`/NumPy); the median is a deterministic order
//!   statistic; and the realized-accuracy relation `σ = DOP · σ_URE` is a Validated
//!   closed form (Kaplan & Hegarty, UERE budget). These are *computational* checks; they
//!   do **not** assert flight validation, certification, or ESA acceptance.
//! * **Modelled** — the bootstrap confidence interval (an internal-consistency resampling
//!   of the finite epoch sample), and every scenario input (constellation size, beacon
//!   placement, error-budget component magnitudes) — a representative choice, never a
//!   fielded measurement.
//!
//! HDOP and VDOP are always reported **separately** from GDOP: a scalar GDOP hides
//! which axis the surface beacons actually fix (the horizontal one), so collapsing it
//! would defeat the whole point of the analysis.

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};
use kshana::lunar::{
    lunar_gdop_map, selenographic_to_mcmf, GdopMapCell, LunarRelay, Selenographic, R_MOON_M,
};
use kshana::lunar_beacon::{
    dop_with_beacons, realized_accuracy, BeaconErrorBudget, RealizedAccuracy,
};
use kshana::lunar_service::{sweep_over_n, LunarConstellation, NSweepRow};
use kshana::orbit::Dop;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde_json::{json, Value};

/// The honesty label every `pro-beacon-dop` result carries, in the open engine's `label`
/// form. Figures keep their own Validated / Modelled tags (see `provenance_legend`).
pub const BEACON_DOP_LABEL: &str = "MODELLED - a surface-beacon-augmented lunar dilution-of-precision sweep over a representative constellation, beacon geometry and error budget. Figures tagged Validated inherit the open engine's dilution-of-precision kernel, a computational check only; the sweep is not flight validation and not a certification.";

/// The honesty label every `pro-surface-beacon-dop-study` result carries, in the open
/// engine's `label` form. Figures keep their own Validated / Modelled tags.
pub const SURFACE_BEACON_DOP_STUDY_LABEL: &str = "MODELLED - a surface-beacon dilution-of-precision study over an illustrative constellation, relay set, beacon placement and error budget. Figures tagged Validated inherit the open engine's dilution-of-precision kernel, a computational check only; the study is not flight validation and not a certification.";

use crate::report_brand::{
    cover_html, modelled_banner_html, BRASS, BRASS_MUTED, FONT_BODY, FONT_HEAD, INK, IVORY,
    IVORY_LIGHT, NAVY,
};

type Vec3 = [f64; 3];

// ── Provenance tag ───────────────────────────────────────────────────────────

/// Provenance of a reported value. See the module docs for the precise
/// (non-overclaiming) meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tag {
    Validated,
    Modelled,
}

impl Tag {
    fn as_str(self) -> &'static str {
        match self {
            Tag::Validated => "Validated",
            Tag::Modelled => "Modelled",
        }
    }
    fn color(self) -> &'static str {
        match self {
            Tag::Validated => "#2dd4bf", // teal — cross-checked closed form / kernel
            Tag::Modelled => BRASS,      // brass — scenario input / resampling
        }
    }
}

// ── Shared geometry & statistics helpers (used by both packs) ────────────────

/// MCMF position of a surface site from selenographic latitude/longitude (deg) and
/// antenna height (m) above the mean lunar sphere — the [`selenographic_to_mcmf`]
/// convention (+Z at the north pole).
fn user_site(lat_deg: f64, lon_deg: f64, height_m: f64) -> Vec3 {
    selenographic_to_mcmf(Selenographic {
        lat_rad: lat_deg.to_radians(),
        lon_rad: lon_deg.to_radians(),
        alt_m: height_m,
    })
}

/// A surface site at ground distance `dist_m` and azimuth `az_deg` from (lat, lon),
/// at antenna height `height_m` (spherical direct geodesic). Used to place beacons a
/// few km from the user at diverse azimuths, within the airless horizon.
fn offset_site(lat_deg: f64, lon_deg: f64, dist_m: f64, az_deg: f64, height_m: f64) -> Vec3 {
    let lat = lat_deg.to_radians();
    let lon = lon_deg.to_radians();
    let az = az_deg.to_radians();
    let ang = dist_m / R_MOON_M;
    let lat2 = (lat.sin() * ang.cos() + lat.cos() * ang.sin() * az.cos()).asin();
    let lon2 = lon + (az.sin() * ang.sin() * lat.cos()).atan2(ang.cos() - lat.sin() * lat2.sin());
    let r = R_MOON_M + height_m;
    [
        r * lat2.cos() * lon2.cos(),
        r * lat2.cos() * lon2.sin(),
        r * lat2.sin(),
    ]
}

/// Place `count` surface beacons on a ring of radius `ring_radius_m` about the user
/// at (lat, lon), evenly spread across `az_spread_deg` of azimuth, each at
/// `height_m`. `az_spread = 360` gives a full even ring; a smaller spread gives a
/// fan (the azimuth-diversity knob the P2 study varies).
fn ring_beacons(
    lat_deg: f64,
    lon_deg: f64,
    count: usize,
    ring_radius_m: f64,
    az_spread_deg: f64,
    height_m: f64,
) -> Vec<Vec3> {
    (0..count)
        .map(|i| {
            let az = if count == 0 {
                0.0
            } else {
                az_spread_deg * (i as f64) / (count as f64)
            };
            offset_site(lat_deg, lon_deg, ring_radius_m, az, height_m)
        })
        .collect()
}

/// Median of a sample as an exact order statistic (`None` if empty).
fn median_of(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = v.len() / 2;
    Some(if v.len() % 2 == 0 {
        0.5 * (v[mid - 1] + v[mid])
    } else {
        v[mid]
    })
}

/// Seeded, deterministic percentile-bootstrap confidence interval for the **median**
/// of `samples` at level `1 − alpha`. Uses [`ChaCha8Rng`] seeded from a fixed `seed`
/// (no wall-clock), so a reported interval is byte-reproducible from the scenario. An
/// internal-consistency measure of the finite epoch sample — Modelled.
fn bootstrap_median_ci(samples: &[f64], b: usize, seed: u64, alpha: f64) -> Option<(f64, f64)> {
    let n = samples.len();
    if n == 0 || b == 0 {
        return None;
    }
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut meds: Vec<f64> = Vec::with_capacity(b);
    let mut buf = vec![0.0_f64; n];
    for _ in 0..b {
        for slot in buf.iter_mut() {
            *slot = samples[rng.gen_range(0..n)];
        }
        if let Some(m) = median_of(buf.clone()) {
            meds.push(m);
        }
    }
    if meds.is_empty() {
        return None;
    }
    meds.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let bn = meds.len();
    let lo_i = (((alpha / 2.0) * bn as f64) as usize).min(bn - 1);
    let hi_i = (((1.0 - alpha / 2.0) * bn as f64) as usize).min(bn - 1);
    Some((meds[lo_i], meds[hi_i]))
}

/// The per-config DOP result: medians over the epoch sample of each DOP component
/// (all Validated, kernel-inherited), a bootstrap CI on the key medians (Modelled),
/// and the realized 1σ accuracy in metres (Validated closed form).
#[derive(Clone, Debug)]
struct ConfigDop {
    n_epochs: usize,
    n_defined: usize,
    gdop_median: Option<f64>,
    hdop_median: Option<f64>,
    vdop_median: Option<f64>,
    pdop_median: Option<f64>,
    tdop_median: Option<f64>,
    gdop_ci: Option<(f64, f64)>,
    hdop_ci: Option<(f64, f64)>,
    vdop_ci: Option<(f64, f64)>,
    accuracy: Option<RealizedAccuracy>,
}

/// Evaluate one configuration: run [`dop_with_beacons`] at `user` against the
/// (per-epoch) satellite positions and the static `beacons` for every epoch,
/// collect the defined-DOP samples, and reduce them to component medians, bootstrap
/// CIs, and a realized accuracy (median DOP × `sigma_ure`).
#[allow(clippy::too_many_arguments)]
fn eval_config(
    user: Vec3,
    sats_per_epoch: &[Vec<Vec3>],
    beacons: &[Vec3],
    mask_rad: f64,
    sigma_ure: f64,
    b: usize,
    seed: u64,
    alpha: f64,
) -> ConfigDop {
    let (mut g, mut h, mut v, mut p, mut t) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for sats in sats_per_epoch {
        if let Some(d) = dop_with_beacons(user, sats, beacons, mask_rad) {
            g.push(d.gdop);
            h.push(d.hdop);
            v.push(d.vdop);
            p.push(d.pdop);
            t.push(d.tdop);
        }
    }
    let n_defined = g.len();
    let gdop_median = median_of(g.clone());
    let hdop_median = median_of(h.clone());
    let vdop_median = median_of(v.clone());
    let pdop_median = median_of(p.clone());
    let tdop_median = median_of(t.clone());

    // Realized accuracy from the component-median DOP (σ = DOP · σ_URE, per component).
    let accuracy = match (
        pdop_median,
        hdop_median,
        vdop_median,
        tdop_median,
        gdop_median,
    ) {
        (Some(pd), Some(hd), Some(vd), Some(td), Some(gd)) => Some(realized_accuracy(
            &Dop {
                gdop: gd,
                pdop: pd,
                hdop: hd,
                vdop: vd,
                tdop: td,
            },
            sigma_ure,
        )),
        _ => None,
    };

    ConfigDop {
        n_epochs: sats_per_epoch.len(),
        n_defined,
        gdop_median,
        hdop_median,
        vdop_median,
        pdop_median,
        tdop_median,
        gdop_ci: bootstrap_median_ci(&g, b, seed, alpha),
        hdop_ci: bootstrap_median_ci(&h, b, seed.wrapping_add(1), alpha),
        vdop_ci: bootstrap_median_ci(&v, b, seed.wrapping_add(2), alpha),
        accuracy,
    }
}

/// Satellite MCMF positions for an illustrative ELFO constellation of `n` satellites
/// at each of the requested epochs (seconds past epoch).
fn sats_per_epoch(n_fleet: usize, n_epochs: usize, step_s: f64) -> Vec<Vec<Vec3>> {
    let con = LunarConstellation::illustrative_lcns(n_fleet);
    (0..n_epochs)
        .map(|k| con.positions_mcmf(k as f64 * step_s))
        .collect()
}

// JSON helpers.
fn onum(x: Option<f64>) -> Value {
    match x {
        Some(v) => json!(v),
        None => Value::Null,
    }
}
fn tagged(v: Value, t: Tag) -> Value {
    json!({ "value": v, "tag": t.as_str() })
}
fn ci_json(ci: Option<(f64, f64)>) -> Value {
    match ci {
        Some((lo, hi)) => json!({ "lo": lo, "hi": hi, "tag": Tag::Modelled.as_str() }),
        None => Value::Null,
    }
}
fn accuracy_json(a: Option<RealizedAccuracy>) -> Value {
    match a {
        Some(a) => json!({
            "pos_3d_m": a.pos_3d_m,
            "horizontal_m": a.horizontal_m,
            "vertical_m": a.vertical_m,
            "time_m": a.time_m,
            "tag": Tag::Validated.as_str(),
            "note": "realized 1σ accuracy = median DOP × σ_URE (Validated closed form); σ_URE is a Modelled budget",
        }),
        None => Value::Null,
    }
}
fn config_json(c: &ConfigDop) -> Value {
    json!({
        "n_epochs": c.n_epochs,
        "n_defined": c.n_defined,
        "gdop_median": tagged(onum(c.gdop_median), Tag::Validated),
        "hdop_median": tagged(onum(c.hdop_median), Tag::Validated),
        "vdop_median": tagged(onum(c.vdop_median), Tag::Validated),
        "pdop_median": tagged(onum(c.pdop_median), Tag::Validated),
        "tdop_median": tagged(onum(c.tdop_median), Tag::Validated),
        "gdop_median_ci": ci_json(c.gdop_ci),
        "hdop_median_ci": ci_json(c.hdop_ci),
        "vdop_median_ci": ci_json(c.vdop_ci),
        "realized_accuracy_m": accuracy_json(c.accuracy),
    })
}

// ═════════════════════════════════════════════════════════════════════════════
// L45 — pro-beacon-dop
// ═════════════════════════════════════════════════════════════════════════════

// Geometry sub-sweep lists (held at scenario baseline for the OTHER factors).
const RING_RADII_M: [f64; 4] = [1_000.0, 2_000.0, 4_000.0, 8_000.0];
const MAST_HEIGHTS_M: [f64; 4] = [1.8, 3.0, 10.0, 100.0];
const AZ_SPREADS_DEG: [f64; 4] = [90.0, 180.0, 270.0, 360.0];

fn d_lat_min() -> f64 {
    -90.0
}
fn d_lat_max() -> f64 {
    -70.0
}
fn d_lat_step() -> f64 {
    5.0
}
fn d_headline_lat() -> f64 {
    -80.0
}
fn d_n_fleet() -> usize {
    6
}
fn d_headline_beacons() -> usize {
    3
}
fn d_max_beacons() -> usize {
    4
}
fn d_ring_radius() -> f64 {
    4_000.0
}
fn d_beacon_height() -> f64 {
    3.0
}
fn d_azimuth_spread() -> f64 {
    360.0
}
fn d_user_antenna_h() -> f64 {
    1.8
}
fn d_mask_deg() -> f64 {
    5.0
}
fn d_pdop_threshold() -> f64 {
    6.0
}
fn d_n_epochs() -> usize {
    24
}
fn d_epoch_step() -> f64 {
    1_800.0
}
fn d_clock_sync() -> f64 {
    1.0
}
fn d_multipath() -> f64 {
    2.0
}
fn d_survey() -> f64 {
    0.5
}
fn d_bootstrap() -> usize {
    1_000
}
fn d_ci_alpha() -> f64 {
    0.05
}
fn d_seed() -> u64 {
    0xB3AC_0D0B
}

/// A `pro-beacon-dop` scenario. Every field defaults to the P2 baseline, so an empty
/// scenario (`kind = "pro-beacon-dop"`) reproduces the paper's headline sweeps.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct BeaconDopScenario {
    #[serde(default = "d_lat_min")]
    pub lat_min_deg: f64,
    #[serde(default = "d_lat_max")]
    pub lat_max_deg: f64,
    #[serde(default = "d_lat_step")]
    pub lat_step_deg: f64,
    #[serde(default = "d_headline_lat")]
    pub headline_lat_deg: f64,
    #[serde(default = "d_n_fleet")]
    pub n_fleet: usize,
    #[serde(default = "d_headline_beacons")]
    pub headline_beacons: usize,
    #[serde(default = "d_max_beacons")]
    pub max_beacons: usize,
    #[serde(default = "d_ring_radius")]
    pub ring_radius_m: f64,
    #[serde(default = "d_beacon_height")]
    pub beacon_height_m: f64,
    #[serde(default = "d_azimuth_spread")]
    pub azimuth_spread_deg: f64,
    #[serde(default = "d_user_antenna_h")]
    pub user_antenna_h_m: f64,
    #[serde(default = "d_mask_deg")]
    pub mask_deg: f64,
    #[serde(default = "d_pdop_threshold")]
    pub pdop_threshold: f64,
    #[serde(default = "d_n_epochs")]
    pub n_epochs: usize,
    #[serde(default = "d_epoch_step")]
    pub epoch_step_s: f64,
    #[serde(default = "d_clock_sync")]
    pub clock_sync_m: f64,
    #[serde(default = "d_multipath")]
    pub multipath_m: f64,
    #[serde(default = "d_survey")]
    pub survey_m: f64,
    #[serde(default = "d_bootstrap")]
    pub bootstrap_resamples: usize,
    #[serde(default = "d_ci_alpha")]
    pub ci_alpha: f64,
    #[serde(default = "d_seed")]
    pub seed: u64,
}

impl Default for BeaconDopScenario {
    fn default() -> Self {
        Self {
            lat_min_deg: d_lat_min(),
            lat_max_deg: d_lat_max(),
            lat_step_deg: d_lat_step(),
            headline_lat_deg: d_headline_lat(),
            n_fleet: d_n_fleet(),
            headline_beacons: d_headline_beacons(),
            max_beacons: d_max_beacons(),
            ring_radius_m: d_ring_radius(),
            beacon_height_m: d_beacon_height(),
            azimuth_spread_deg: d_azimuth_spread(),
            user_antenna_h_m: d_user_antenna_h(),
            mask_deg: d_mask_deg(),
            pdop_threshold: d_pdop_threshold(),
            n_epochs: d_n_epochs(),
            epoch_step_s: d_epoch_step(),
            clock_sync_m: d_clock_sync(),
            multipath_m: d_multipath(),
            survey_m: d_survey(),
            bootstrap_resamples: d_bootstrap(),
            ci_alpha: d_ci_alpha(),
            seed: d_seed(),
        }
    }
}

/// A row of the latitude sweep: the orbit-only vs beacon-augmented DOP at one user
/// latitude.
struct LatRow {
    lat_deg: f64,
    orbit_only: ConfigDop,
    with_beacons: ConfigDop,
}

/// A row of a one-factor geometry sub-sweep.
struct GeomRow {
    factor_value: f64,
    config: ConfigDop,
}

/// Everything the L45 pack derives, computed once and shared by JSON / figure / HTML.
struct BeaconDopModel {
    sigma_ure_m: f64,
    lat_rows: Vec<LatRow>,
    count_rows: Vec<(usize, ConfigDop)>,
    ring_rows: Vec<GeomRow>,
    height_rows: Vec<GeomRow>,
    azimuth_rows: Vec<GeomRow>,
    headline_before: ConfigDop,
    headline_after: ConfigDop,
}

fn lat_grid(min: f64, max: f64, step: f64) -> Vec<f64> {
    let mut v = Vec::new();
    if step <= 0.0 {
        return vec![min];
    }
    let mut x = min;
    while x <= max + 1e-9 {
        v.push(x);
        x += step;
    }
    v
}

impl BeaconDopScenario {
    pub const KIND: &'static str = "pro-beacon-dop";

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: Self::KIND,
            description: "Kshana Pro P2/L45 pack: sweep the open beacon-augmented DOP (kshana::lunar_beacon::dop_with_beacons) across a south-polar user latitude band and surface-beacon geometry (count, ring radius, mast height, azimuth spread), reporting per-config median GDOP/HDOP/VDOP separately plus realized position accuracy in metres (σ_URE × DOP), each median with a seeded deterministic bootstrap CI. Replaces P2's single −80° scalar-GDOP run. Per-config DOP is Validated (inherits the DOP kernel); bootstrap CIs are Modelled. An engineering study, not a certification.",
            required_fields: &[],
            optional_fields: &[
                "lat_min_deg",
                "lat_max_deg",
                "lat_step_deg",
                "headline_lat_deg",
                "n_fleet",
                "headline_beacons",
                "max_beacons",
                "ring_radius_m",
                "beacon_height_m",
                "azimuth_spread_deg",
                "user_antenna_h_m",
                "mask_deg",
                "pdop_threshold",
                "n_epochs",
                "epoch_step_s",
                "clock_sync_m",
                "multipath_m",
                "survey_m",
                "bootstrap_resamples",
                "ci_alpha",
                "seed",
            ],
        }
    }

    fn mask_rad(&self) -> f64 {
        self.mask_deg.to_radians()
    }

    fn sigma_ure(&self) -> f64 {
        BeaconErrorBudget {
            clock_sync_m: self.clock_sync_m,
            multipath_m: self.multipath_m,
            survey_m: self.survey_m,
        }
        .sigma_ure_m()
    }

    /// Evaluate one beacon configuration at a given user latitude, with a distinct
    /// deterministic bootstrap seed.
    #[allow(clippy::too_many_arguments)] // deterministic eval helper: each scalar knob is a distinct config axis
    fn eval_at(
        &self,
        lat_deg: f64,
        sats: &[Vec<Vec3>],
        count: usize,
        ring_radius_m: f64,
        beacon_height_m: f64,
        az_spread_deg: f64,
        sigma_ure: f64,
        seed_salt: u64,
    ) -> ConfigDop {
        let user = user_site(lat_deg, 0.0, self.user_antenna_h_m);
        let beacons = ring_beacons(
            lat_deg,
            0.0,
            count,
            ring_radius_m,
            az_spread_deg,
            beacon_height_m,
        );
        eval_config(
            user,
            sats,
            &beacons,
            self.mask_rad(),
            sigma_ure,
            self.bootstrap_resamples,
            self.seed.wrapping_add(seed_salt.wrapping_mul(0x9E37_79B9)),
            self.ci_alpha,
        )
    }

    fn compute(&self) -> Result<BeaconDopModel, KshanaError> {
        if self.n_epochs == 0 {
            return Err(KshanaError::InvalidInput(
                "n_epochs must be positive".into(),
            ));
        }
        let sigma_ure = self.sigma_ure();
        let sats = sats_per_epoch(self.n_fleet, self.n_epochs, self.epoch_step_s);

        // (a) Latitude sweep: orbit-only vs headline-beacon geometry at each latitude.
        let lats = lat_grid(self.lat_min_deg, self.lat_max_deg, self.lat_step_deg);
        let mut lat_rows = Vec::new();
        for (i, &lat) in lats.iter().enumerate() {
            let orbit_only = self.eval_at(
                lat,
                &sats,
                0,
                self.ring_radius_m,
                self.beacon_height_m,
                self.azimuth_spread_deg,
                sigma_ure,
                1000 + i as u64,
            );
            let with_beacons = self.eval_at(
                lat,
                &sats,
                self.headline_beacons,
                self.ring_radius_m,
                self.beacon_height_m,
                self.azimuth_spread_deg,
                sigma_ure,
                2000 + i as u64,
            );
            lat_rows.push(LatRow {
                lat_deg: lat,
                orbit_only,
                with_beacons,
            });
        }

        // (b) Beacon-geometry sub-sweeps at the headline latitude.
        let hl = self.headline_lat_deg;

        // Count sweep 0..=max_beacons.
        let count_rows: Vec<(usize, ConfigDop)> = (0..=self.max_beacons)
            .map(|c| {
                (
                    c,
                    self.eval_at(
                        hl,
                        &sats,
                        c,
                        self.ring_radius_m,
                        self.beacon_height_m,
                        self.azimuth_spread_deg,
                        sigma_ure,
                        3000 + c as u64,
                    ),
                )
            })
            .collect();

        // Ring-radius sweep (headline count, baseline height + azimuth spread).
        let ring_rows: Vec<GeomRow> = RING_RADII_M
            .iter()
            .enumerate()
            .map(|(i, &r)| GeomRow {
                factor_value: r,
                config: self.eval_at(
                    hl,
                    &sats,
                    self.headline_beacons,
                    r,
                    self.beacon_height_m,
                    self.azimuth_spread_deg,
                    sigma_ure,
                    4000 + i as u64,
                ),
            })
            .collect();

        // Mast-height sweep.
        let height_rows: Vec<GeomRow> = MAST_HEIGHTS_M
            .iter()
            .enumerate()
            .map(|(i, &h)| GeomRow {
                factor_value: h,
                config: self.eval_at(
                    hl,
                    &sats,
                    self.headline_beacons,
                    self.ring_radius_m,
                    h,
                    self.azimuth_spread_deg,
                    sigma_ure,
                    5000 + i as u64,
                ),
            })
            .collect();

        // Azimuth-spread sweep.
        let azimuth_rows: Vec<GeomRow> = AZ_SPREADS_DEG
            .iter()
            .enumerate()
            .map(|(i, &a)| GeomRow {
                factor_value: a,
                config: self.eval_at(
                    hl,
                    &sats,
                    self.headline_beacons,
                    self.ring_radius_m,
                    self.beacon_height_m,
                    a,
                    sigma_ure,
                    6000 + i as u64,
                ),
            })
            .collect();

        // Headline before/after at the headline latitude.
        let headline_before = self.eval_at(
            hl,
            &sats,
            0,
            self.ring_radius_m,
            self.beacon_height_m,
            self.azimuth_spread_deg,
            sigma_ure,
            7000,
        );
        let headline_after = self.eval_at(
            hl,
            &sats,
            self.headline_beacons,
            self.ring_radius_m,
            self.beacon_height_m,
            self.azimuth_spread_deg,
            sigma_ure,
            7001,
        );

        Ok(BeaconDopModel {
            sigma_ure_m: sigma_ure,
            lat_rows,
            count_rows,
            ring_rows,
            height_rows,
            azimuth_rows,
            headline_before,
            headline_after,
        })
    }

    fn json(&self, m: &BeaconDopModel) -> Value {
        let lat_rows: Vec<Value> = m
            .lat_rows
            .iter()
            .map(|r| {
                json!({
                    "user_lat_deg": r.lat_deg,
                    "orbit_only": config_json(&r.orbit_only),
                    "with_beacons": config_json(&r.with_beacons),
                })
            })
            .collect();
        let count_rows: Vec<Value> = m
            .count_rows
            .iter()
            .map(|(c, cfg)| json!({ "beacon_count": c, "config": config_json(cfg) }))
            .collect();
        let geom = |rows: &[GeomRow], key: &str| -> Vec<Value> {
            rows.iter()
                .map(|g| json!({ key: g.factor_value, "config": config_json(&g.config) }))
                .collect()
        };
        json!({
            "sigma_ure_m": tagged(json!(m.sigma_ure_m), Tag::Modelled),
            "sigma_ure_note": "σ_URE = sqrt(clock_sync² + multipath² + survey²) (Validated RSS of Modelled component budgets)",
            "error_budget_m": {
                "clock_sync_m": tagged(json!(self.clock_sync_m), Tag::Modelled),
                "multipath_m": tagged(json!(self.multipath_m), Tag::Modelled),
                "survey_m": tagged(json!(self.survey_m), Tag::Modelled),
            },
            "latitude_sweep": lat_rows,
            "beacon_count_sweep": count_rows,
            "ring_radius_sweep_m": geom(&m.ring_rows, "ring_radius_m"),
            "mast_height_sweep_m": geom(&m.height_rows, "beacon_height_m"),
            "azimuth_spread_sweep_deg": geom(&m.azimuth_rows, "azimuth_spread_deg"),
            "headline_before_after": {
                "user_lat_deg": self.headline_lat_deg,
                "beacon_count": self.headline_beacons,
                "orbit_only": config_json(&m.headline_before),
                "with_beacons": config_json(&m.headline_after),
            },
        })
    }
}

impl Scenario for BeaconDopScenario {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let m = self.compute()?;
        let fig = fig_gdop_vs_lat_svg(self, &m);
        let report_html = beacon_dop_report_html(self, &m, &fig);
        let model_json = self.json(&m);

        let doc = json!({
            "kind": Self::KIND,
            "overlay": "kshana-pro",
            "overlay_version": crate::version(),
            "label": BEACON_DOP_LABEL,
            "paper": "P2 — Surface-beacon-augmented lunar DOP (L45)",
            "provenance_legend": {
                "Validated": "the DOP inherits the kshana::orbit::dop kernel (cross-checked vs gnss_lib_py/NumPy); the median is a deterministic order statistic; σ = DOP · σ_URE is a closed form. A computational check, NOT flight validation or certification.",
                "Modelled": "the bootstrap CI (internal-consistency resampling) and every scenario input (constellation, beacon placement, error-budget magnitudes).",
            },
            "model": model_json,
            "figure": { "fig_gdop_vs_latitude_svg": fig },
            "report_html": report_html,
        })
        .to_string();

        let before = m.headline_before.gdop_median.unwrap_or(f64::NAN);
        let after = m.headline_after.gdop_median.unwrap_or(f64::NAN);
        let summary = format!(
            "Kshana Pro P2 beacon-DOP (L45) | {:.0}° user: median GDOP {:.1} → {:.1} with {} beacons [Validated] | \
             HDOP {:.1}→{:.1}, VDOP {:.1}→{:.1} reported separately | σ_URE {:.2} m ⇒ horizontal {:.1}→{:.1} m [Validated] | \
             bootstrap CIs seeded/deterministic [Modelled]",
            self.headline_lat_deg,
            before,
            after,
            self.headline_beacons,
            m.headline_before.hdop_median.unwrap_or(f64::NAN),
            m.headline_after.hdop_median.unwrap_or(f64::NAN),
            m.headline_before.vdop_median.unwrap_or(f64::NAN),
            m.headline_after.vdop_median.unwrap_or(f64::NAN),
            m.sigma_ure_m,
            m.headline_before.accuracy.map(|a| a.horizontal_m).unwrap_or(f64::NAN),
            m.headline_after.accuracy.map(|a| a.horizontal_m).unwrap_or(f64::NAN),
        );

        Ok(RunOutput {
            json: doc,
            svg: fig,
            summary,
            csv: None,
        })
    }
}

impl ExternalPack for BeaconDopScenario {
    fn kind_name(&self) -> &'static str {
        Self::KIND
    }
    fn meta(&self) -> ScenarioMeta {
        Self::meta_static()
    }
    fn register_into(reg: &mut kshana::registry::PackRegistry) {
        reg.register(crate::registry::pro_factory::<Self>(Self::KIND));
    }
}

// ── L45 figure: GDOP vs latitude, orbit-only vs beacon-augmented ────────────

fn fig_gdop_vs_lat_svg(scn: &BeaconDopScenario, m: &BeaconDopModel) -> String {
    let (w, h) = (640.0_f64, 380.0_f64);
    let (ml, mr, mt, mb) = (60.0_f64, 170.0_f64, 62.0_f64, 46.0_f64);
    let lats: Vec<f64> = m.lat_rows.iter().map(|r| r.lat_deg).collect();
    let (lat_lo, lat_hi) = (
        scn.lat_min_deg.min(scn.lat_max_deg),
        scn.lat_min_deg.max(scn.lat_max_deg),
    );
    let px = |lat: f64| {
        let f = if (lat_hi - lat_lo).abs() < 1e-9 {
            0.5
        } else {
            (lat - lat_lo) / (lat_hi - lat_lo)
        };
        ml + (w - ml - mr) * f
    };
    // y range from data (GDOP), with a floor at 0 and headroom.
    let gmax = m
        .lat_rows
        .iter()
        .flat_map(|r| [r.orbit_only.gdop_median, r.with_beacons.gdop_median])
        .flatten()
        .fold(6.0_f64, f64::max)
        * 1.15;
    let py = |g: f64| mt + (h - mt - mb) * (1.0 - (g / gmax).clamp(0.0, 1.0));

    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" font-family=\"{FONT_BODY}\" font-size=\"11\">"
    );
    s += &format!("<rect width=\"100%\" height=\"100%\" fill=\"{NAVY}\"/>");
    s += &format!("<text x=\"20\" y=\"26\" fill=\"{BRASS}\" font-family=\"{FONT_HEAD}\" font-size=\"17\" font-weight=\"bold\">Beacon-augmented GDOP vs user latitude</text>");
    s += &format!("<text x=\"20\" y=\"44\" fill=\"{IVORY}\">Median GDOP over {} epochs · N={} fleet · {} surface beacons <tspan fill=\"#2dd4bf\">[Validated kernel]</tspan></text>",
        scn.n_epochs, scn.n_fleet, scn.headline_beacons);

    let (x0, x1) = (ml, w - mr);
    let (yb, yt) = (h - mb, mt);
    s += &format!("<line x1=\"{x0}\" y1=\"{yb}\" x2=\"{x1}\" y2=\"{yb}\" stroke=\"{BRASS_MUTED}\" stroke-width=\"1\"/>");
    s += &format!("<line x1=\"{x0}\" y1=\"{yt}\" x2=\"{x0}\" y2=\"{yb}\" stroke=\"{BRASS_MUTED}\" stroke-width=\"1\"/>");
    // GDOP = 6 usable-geometry line.
    let y6 = py(6.0);
    s += &format!("<line x1=\"{x0}\" y1=\"{y6:.1}\" x2=\"{x1}\" y2=\"{y6:.1}\" stroke=\"#7f6b3a\" stroke-dasharray=\"4 3\" stroke-width=\"1\"/>");
    s += &format!(
        "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{BRASS_MUTED}\" font-size=\"9\">GDOP 6</text>",
        x1 + 4.0,
        y6 + 3.0
    );
    // x labels.
    for &lat in &lats {
        let x = px(lat);
        s += &format!("<text x=\"{x:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" text-anchor=\"middle\">{lat:.0}°</text>", yb + 16.0);
    }
    s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{BRASS}\" text-anchor=\"middle\">selenographic latitude</text>", (x0 + x1) / 2.0, h - 8.0);
    // y labels.
    for k in 0..=4 {
        let g = gmax * k as f64 / 4.0;
        let y = py(g);
        s += &format!("<line x1=\"{x0}\" y1=\"{y:.1}\" x2=\"{x1}\" y2=\"{y:.1}\" stroke=\"#1c2637\" stroke-width=\"1\"/>");
        s += &format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" text-anchor=\"end\">{g:.0}</text>",
            x0 - 6.0,
            y + 4.0
        );
    }

    let poly = |sel: &dyn Fn(&LatRow) -> Option<f64>| -> String {
        m.lat_rows
            .iter()
            .filter_map(|r| sel(r).map(|g| format!("{:.1},{:.1}", px(r.lat_deg), py(g))))
            .collect::<Vec<_>>()
            .join(" ")
    };
    s += &format!(
        "<polyline points=\"{}\" fill=\"none\" stroke=\"#f0a868\" stroke-width=\"2.4\"/>",
        poly(&|r| r.orbit_only.gdop_median)
    );
    s += &format!(
        "<polyline points=\"{}\" fill=\"none\" stroke=\"#2dd4bf\" stroke-width=\"2.4\"/>",
        poly(&|r| r.with_beacons.gdop_median)
    );
    for r in &m.lat_rows {
        if let Some(g) = r.orbit_only.gdop_median {
            s += &format!(
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"3\" fill=\"#f0a868\"/>",
                px(r.lat_deg),
                py(g)
            );
        }
        if let Some(g) = r.with_beacons.gdop_median {
            s += &format!(
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"3\" fill=\"#2dd4bf\"/>",
                px(r.lat_deg),
                py(g)
            );
        }
    }

    // Legend.
    let lx = w - mr + 10.0;
    s += &format!(
        "<rect x=\"{lx:.1}\" y=\"{:.1}\" width=\"14\" height=\"3\" fill=\"#f0a868\"/>",
        mt + 10.0
    );
    s += &format!(
        "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" font-size=\"10\">orbit-only</text>",
        lx + 18.0,
        mt + 14.0
    );
    s += &format!(
        "<rect x=\"{lx:.1}\" y=\"{:.1}\" width=\"14\" height=\"3\" fill=\"#2dd4bf\"/>",
        mt + 28.0
    );
    s += &format!(
        "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" font-size=\"10\">+{} beacons</text>",
        lx + 18.0,
        mt + 32.0,
        scn.headline_beacons
    );
    s += &format!("<text x=\"{lx:.1}\" y=\"{:.1}\" fill=\"{BRASS_MUTED}\" font-size=\"9\">HDOP/VDOP in report</text>", mt + 56.0);

    s += "</svg>";
    s
}

// ── L45 branded HTML report ─────────────────────────────────────────────────

fn badge(t: Tag) -> String {
    format!(
        "<span style=\"background:{};color:{INK};font-size:11px;font-weight:700;\
         padding:1px 7px;border-radius:3px\">{}</span>",
        t.color(),
        t.as_str()
    )
}
fn td(s: &str) -> String {
    format!("<td style=\"padding:6px 12px;border-bottom:1px solid #24303f\">{s}</td>")
}
fn th(s: &str) -> String {
    format!("<th style=\"padding:6px 12px;text-align:left;color:{BRASS};border-bottom:1px solid {BRASS_MUTED}\">{s}</th>")
}
fn fmt_opt(x: Option<f64>, prec: usize) -> String {
    match x {
        Some(v) => format!("{v:.*}", prec),
        None => "—".to_string(),
    }
}
fn fmt_ci(ci: Option<(f64, f64)>, prec: usize) -> String {
    match ci {
        Some((lo, hi)) => format!("[{lo:.*}, {hi:.*}]", prec, prec),
        None => "—".to_string(),
    }
}

fn beacon_dop_report_html(scn: &BeaconDopScenario, m: &BeaconDopModel, fig: &str) -> String {
    // Latitude sweep table: before/after GDOP/HDOP/VDOP + horizontal accuracy.
    let lat_rows: String = m
        .lat_rows
        .iter()
        .map(|r| {
            let acc_o = r.orbit_only.accuracy.map(|a| a.horizontal_m);
            let acc_b = r.with_beacons.accuracy.map(|a| a.horizontal_m);
            format!(
                "<tr>{}{}{}{}{}{}{}{}</tr>",
                td(&format!("{:.0}°", r.lat_deg)),
                td(&fmt_opt(r.orbit_only.gdop_median, 2)),
                td(&fmt_opt(r.with_beacons.gdop_median, 2)),
                td(&fmt_opt(r.with_beacons.hdop_median, 2)),
                td(&fmt_opt(r.with_beacons.vdop_median, 2)),
                td(&fmt_opt(acc_o, 1)),
                td(&fmt_opt(acc_b, 1)),
                td(&fmt_ci(r.with_beacons.gdop_ci, 2)),
            )
        })
        .collect();

    let count_rows: String = m
        .count_rows
        .iter()
        .map(|(c, cfg)| {
            format!(
                "<tr>{}{}{}{}{}</tr>",
                td(&c.to_string()),
                td(&fmt_opt(cfg.gdop_median, 2)),
                td(&fmt_opt(cfg.hdop_median, 2)),
                td(&fmt_opt(cfg.vdop_median, 2)),
                td(&fmt_opt(cfg.accuracy.map(|a| a.horizontal_m), 1)),
            )
        })
        .collect();

    let geom_table = |rows: &[GeomRow], unit: &str, prec: usize| -> String {
        rows.iter()
            .map(|g| {
                format!(
                    "<tr>{}{}{}{}{}</tr>",
                    td(&format!("{:.*} {unit}", prec, g.factor_value)),
                    td(&fmt_opt(g.config.gdop_median, 2)),
                    td(&fmt_opt(g.config.hdop_median, 2)),
                    td(&fmt_opt(g.config.vdop_median, 2)),
                    td(&fmt_opt(g.config.accuracy.map(|a| a.horizontal_m), 1)),
                )
            })
            .collect()
    };
    let ring_rows = geom_table(&m.ring_rows, "m", 0);
    let height_rows = geom_table(&m.height_rows, "m", 1);
    let az_rows = geom_table(&m.azimuth_rows, "°", 0);

    let before = &m.headline_before;
    let after = &m.headline_after;

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <title>Kshana Pro — P2 Beacon-Augmented Lunar DOP (L45)</title></head>\
         <body style=\"margin:0;background:{NAVY};color:{IVORY};font-family:{FONT_BODY};\
         max-width:920px;margin:0 auto;padding:0 28px 60px\">\
         {cover}{banner}\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Headline — {hl:.0}° user, before vs after {nb} beacons</h2>\
         <p>Orbit-only median GDOP <strong>{gob}</strong> {bo}, cut to <strong>{gaf}</strong> {ba} by {nb} surface beacons. \
         HDOP {hob}→{haf}, VDOP {vob}→{vaf} (reported separately — the beacons fix the horizontal axis). \
         With σ_URE {sig:.2} m the realized horizontal 1σ falls {acco} m → {accb} m {av}.</p>\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Figure — GDOP vs latitude</h2>{fig}\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Latitude sweep (median over epochs)</h2>\
         <table style=\"border-collapse:collapse;width:100%\"><tr>{h_lat}{h_go}{h_gb}{h_hb}{h_vb}{h_ao}{h_ab}{h_ci}</tr>{lat_rows}</table>\
         <p style=\"font-size:12px;opacity:.8\">GDOP columns {bv}; realized accuracy {av}; bootstrap CI {mv}.</p>\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Beacon-count sweep (at {hl:.0}°)</h2>\
         <table style=\"border-collapse:collapse;width:100%\"><tr>{h_n}{h_g}{h_h}{h_v}{h_a}</tr>{count_rows}</table>\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Ring-radius sweep</h2>\
         <table style=\"border-collapse:collapse;width:100%\"><tr>{h_f}{h_g}{h_h}{h_v}{h_a}</tr>{ring_rows}</table>\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Mast-height sweep</h2>\
         <table style=\"border-collapse:collapse;width:100%\"><tr>{h_f}{h_g}{h_h}{h_v}{h_a}</tr>{height_rows}</table>\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Azimuth-spread sweep</h2>\
         <table style=\"border-collapse:collapse;width:100%\"><tr>{h_f}{h_g}{h_h}{h_v}{h_a}</tr>{az_rows}</table>\
         </body></html>",
        cover = cover_html(
            "Surface-Beacon-Augmented Lunar DOP",
            "P2 / L45 — a composed, kernel-inherited engineering study"
        ),
        banner = modelled_banner_html(
            "the DOP inherits the cross-checked open kernel and the σ = DOP · σ_URE accuracy is a closed form; \
             the constellation, beacon placement, error budget and bootstrap CIs are Modelled."
        ),
        hl = scn.headline_lat_deg,
        nb = scn.headline_beacons,
        gob = fmt_opt(before.gdop_median, 2),
        gaf = fmt_opt(after.gdop_median, 2),
        bo = badge(Tag::Validated),
        ba = badge(Tag::Validated),
        hob = fmt_opt(before.hdop_median, 2),
        haf = fmt_opt(after.hdop_median, 2),
        vob = fmt_opt(before.vdop_median, 2),
        vaf = fmt_opt(after.vdop_median, 2),
        sig = m.sigma_ure_m,
        acco = fmt_opt(before.accuracy.map(|a| a.horizontal_m), 1),
        accb = fmt_opt(after.accuracy.map(|a| a.horizontal_m), 1),
        av = badge(Tag::Validated),
        bv = badge(Tag::Validated),
        mv = badge(Tag::Modelled),
        fig = fig,
        h_lat = th("Lat"),
        h_go = th("GDOP orbit-only"),
        h_gb = th("GDOP +beacons"),
        h_hb = th("HDOP +beacons"),
        h_vb = th("VDOP +beacons"),
        h_ao = th("Horiz σ orbit (m)"),
        h_ab = th("Horiz σ +beacons (m)"),
        h_ci = th("GDOP CI"),
        lat_rows = lat_rows,
        h_n = th("Beacons"),
        h_f = th("Value"),
        h_g = th("GDOP"),
        h_h = th("HDOP"),
        h_v = th("VDOP"),
        h_a = th("Horiz σ (m)"),
        count_rows = count_rows,
        ring_rows = ring_rows,
        height_rows = height_rows,
        az_rows = az_rows,
    )
}

// ═════════════════════════════════════════════════════════════════════════════
// L46 — pro-surface-beacon-dop-study
// ═════════════════════════════════════════════════════════════════════════════

fn ds_n_min() -> usize {
    1
}
fn ds_n_max() -> usize {
    24
}
fn ds_band_lat_min() -> f64 {
    -90.0
}
fn ds_band_lat_max() -> f64 {
    -70.0
}
fn ds_map_n_relays() -> usize {
    9
}
fn ds_map_n_lat() -> usize {
    9
}
fn ds_map_n_lon() -> usize {
    12
}
fn ds_sweep_epochs() -> usize {
    12
}
fn ds_sweep_step() -> f64 {
    3_600.0
}
fn ds_seed() -> u64 {
    0x50D0_BEAC
}

/// A `pro-surface-beacon-dop-study` scenario — the P2 Figure-1 orchestrator. Empty
/// TOML reproduces the paper baseline.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SurfaceBeaconDopStudyScenario {
    #[serde(default = "ds_n_min")]
    pub n_min: usize,
    #[serde(default = "ds_n_max")]
    pub n_max: usize,
    #[serde(default = "ds_band_lat_min")]
    pub band_lat_min_deg: f64,
    #[serde(default = "ds_band_lat_max")]
    pub band_lat_max_deg: f64,
    #[serde(default = "ds_map_n_relays")]
    pub map_n_relays: usize,
    #[serde(default = "ds_map_n_lat")]
    pub map_n_lat: usize,
    #[serde(default = "ds_map_n_lon")]
    pub map_n_lon: usize,
    #[serde(default = "ds_sweep_epochs")]
    pub n_epochs: usize,
    #[serde(default = "ds_sweep_step")]
    pub epoch_step_s: f64,
    #[serde(default = "d_headline_lat")]
    pub headline_lat_deg: f64,
    #[serde(default = "d_n_fleet")]
    pub n_fleet: usize,
    #[serde(default = "d_headline_beacons")]
    pub headline_beacons: usize,
    #[serde(default = "d_ring_radius")]
    pub ring_radius_m: f64,
    #[serde(default = "d_beacon_height")]
    pub beacon_height_m: f64,
    #[serde(default = "d_azimuth_spread")]
    pub azimuth_spread_deg: f64,
    #[serde(default = "d_user_antenna_h")]
    pub user_antenna_h_m: f64,
    #[serde(default = "d_mask_deg")]
    pub mask_deg: f64,
    #[serde(default = "d_pdop_threshold")]
    pub pdop_threshold: f64,
    #[serde(default = "d_clock_sync")]
    pub clock_sync_m: f64,
    #[serde(default = "d_multipath")]
    pub multipath_m: f64,
    #[serde(default = "d_survey")]
    pub survey_m: f64,
    #[serde(default = "d_bootstrap")]
    pub bootstrap_resamples: usize,
    #[serde(default = "d_ci_alpha")]
    pub ci_alpha: f64,
    #[serde(default = "ds_seed")]
    pub seed: u64,
}

impl Default for SurfaceBeaconDopStudyScenario {
    fn default() -> Self {
        Self {
            n_min: ds_n_min(),
            n_max: ds_n_max(),
            band_lat_min_deg: ds_band_lat_min(),
            band_lat_max_deg: ds_band_lat_max(),
            map_n_relays: ds_map_n_relays(),
            map_n_lat: ds_map_n_lat(),
            map_n_lon: ds_map_n_lon(),
            n_epochs: ds_sweep_epochs(),
            epoch_step_s: ds_sweep_step(),
            headline_lat_deg: d_headline_lat(),
            n_fleet: d_n_fleet(),
            headline_beacons: d_headline_beacons(),
            ring_radius_m: d_ring_radius(),
            beacon_height_m: d_beacon_height(),
            azimuth_spread_deg: d_azimuth_spread(),
            user_antenna_h_m: d_user_antenna_h(),
            mask_deg: d_mask_deg(),
            pdop_threshold: d_pdop_threshold(),
            clock_sync_m: d_clock_sync(),
            multipath_m: d_multipath(),
            survey_m: d_survey(),
            bootstrap_resamples: d_bootstrap(),
            ci_alpha: d_ci_alpha(),
            seed: ds_seed(),
        }
    }
}

/// The illustrative circular relay set used for the polar spatial GDOP map: `n`
/// relays at `R_moon + 8000 km`, inclination 57.7°, evenly phased in RAAN and mean
/// anomaly. Distinct from the (elliptical, south-favouring) planned-fleet used for
/// the before/after — this is the denser reference set that resolves the spatial GDOP
/// gradient across the polar band.
fn map_relays(n: usize) -> Vec<LunarRelay> {
    let n = n.max(1);
    (0..n)
        .map(|k| LunarRelay {
            radius_m: R_MOON_M + 8_000_000.0,
            inc_deg: 57.7,
            raan_deg: 360.0 * k as f64 / n as f64,
            phase_deg: 360.0 * k as f64 / n as f64,
        })
        .collect()
}

/// A per-latitude aggregate of the spatial GDOP map (median over the longitudes).
struct MapBandRow {
    lat_deg: f64,
    gdop_median: Option<f64>,
    hdop_median: Option<f64>,
    vdop_median: Option<f64>,
}

struct StudyModel {
    sigma_ure_m: f64,
    n_sweep: Vec<NSweepRow>,
    crossing_n: Option<usize>,
    planned_lo: usize,
    planned_hi: usize,
    map_cells: Vec<GdopMapCell>,
    map_bands: Vec<MapBandRow>,
    before: ConfigDop,
    after: ConfigDop,
}

impl SurfaceBeaconDopStudyScenario {
    pub const KIND: &'static str = "pro-surface-beacon-dop-study";

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: Self::KIND,
            description: "Kshana Pro P2/L46 study: orchestrate the open surface-beacon-DOP analyses into P2 Figure 1 — the constellation-size sweep (kshana::lunar_service::sweep_over_n, GDOP-vs-N with the 4–6 planned-fleet band and the N≈12 GDOP-6 crossing), the polar spatial GDOP map (kshana::lunar::lunar_gdop_map), and the beacon before/after (dop_with_beacons vs orbit-only). Emits a committed byte-stable lunar_dop_summary.csv, three Figure-1 SVGs and a branded HTML report. Per-sample DOP inherits the Validated kernel; scenario inputs are Modelled. An engineering study, not a certification.",
            required_fields: &[],
            optional_fields: &[
                "n_min",
                "n_max",
                "band_lat_min_deg",
                "band_lat_max_deg",
                "map_n_relays",
                "map_n_lat",
                "map_n_lon",
                "n_epochs",
                "epoch_step_s",
                "headline_lat_deg",
                "n_fleet",
                "headline_beacons",
                "ring_radius_m",
                "beacon_height_m",
                "azimuth_spread_deg",
                "user_antenna_h_m",
                "mask_deg",
                "pdop_threshold",
                "clock_sync_m",
                "multipath_m",
                "survey_m",
                "bootstrap_resamples",
                "ci_alpha",
                "seed",
            ],
        }
    }

    fn mask_rad(&self) -> f64 {
        self.mask_deg.to_radians()
    }
    fn sigma_ure(&self) -> f64 {
        BeaconErrorBudget {
            clock_sync_m: self.clock_sync_m,
            multipath_m: self.multipath_m,
            survey_m: self.survey_m,
        }
        .sigma_ure_m()
    }

    /// The south-polar grid used by both the GDOP-vs-N sweep and the map band lats.
    fn band_grid(&self) -> Vec<Selenographic> {
        let lats = lat_grid(self.band_lat_min_deg, self.band_lat_max_deg, 5.0);
        let lons = [-135.0_f64, -45.0, 45.0, 135.0];
        let mut g = Vec::new();
        for &lat in &lats {
            for &lon in &lons {
                g.push(Selenographic {
                    lat_rad: lat.to_radians(),
                    lon_rad: lon.to_radians(),
                    alt_m: 0.0,
                });
            }
        }
        g
    }

    fn times(&self) -> Vec<f64> {
        (0..self.n_epochs)
            .map(|k| k as f64 * self.epoch_step_s)
            .collect()
    }

    fn compute(&self) -> Result<StudyModel, KshanaError> {
        if self.n_epochs == 0 {
            return Err(KshanaError::InvalidInput(
                "n_epochs must be positive".into(),
            ));
        }
        let sigma_ure = self.sigma_ure();
        let grid = self.band_grid();
        let times = self.times();

        // (1) GDOP-vs-N sweep over the south-polar band.
        let n_sweep = sweep_over_n(
            self.n_min,
            self.n_max,
            &grid,
            &times,
            self.mask_rad(),
            self.pdop_threshold,
        );
        // First N whose median GDOP drops below 6 — the ~12 crossing.
        let crossing_n = n_sweep
            .iter()
            .find(|r| r.gdop_median.map(|g| g < 6.0).unwrap_or(false))
            .map(|r| r.n_sats);

        // (2) Polar spatial GDOP map.
        let relays = map_relays(self.map_n_relays);
        let map_cells = lunar_gdop_map(
            &relays,
            &times,
            self.band_lat_min_deg,
            self.band_lat_max_deg,
            self.map_n_lat,
            self.map_n_lon,
            self.mask_deg,
        );
        // Per-latitude-band aggregate: the median across the longitudes.
        let mut map_bands: Vec<MapBandRow> = Vec::new();
        {
            // Preserve first-seen latitude order.
            let mut seen: Vec<f64> = Vec::new();
            for c in &map_cells {
                if !seen.iter().any(|&l| (l - c.lat_deg).abs() < 1e-9) {
                    seen.push(c.lat_deg);
                }
            }
            for &lat in &seen {
                let pick = |sel: &dyn Fn(&GdopMapCell) -> Option<f64>| -> Option<f64> {
                    let vals: Vec<f64> = map_cells
                        .iter()
                        .filter(|c| (c.lat_deg - lat).abs() < 1e-9)
                        .filter_map(sel)
                        .collect();
                    median_of(vals)
                };
                map_bands.push(MapBandRow {
                    lat_deg: lat,
                    gdop_median: pick(&|c| c.gdop_median),
                    hdop_median: pick(&|c| c.hdop_median),
                    vdop_median: pick(&|c| c.vdop_median),
                });
            }
        }

        // (3) Beacon before/after at the headline latitude.
        let sats = sats_per_epoch(self.n_fleet, self.n_epochs, self.epoch_step_s);
        let user = user_site(self.headline_lat_deg, 0.0, self.user_antenna_h_m);
        let beacons = ring_beacons(
            self.headline_lat_deg,
            0.0,
            self.headline_beacons,
            self.ring_radius_m,
            self.azimuth_spread_deg,
            self.beacon_height_m,
        );
        let before = eval_config(
            user,
            &sats,
            &[],
            self.mask_rad(),
            sigma_ure,
            self.bootstrap_resamples,
            self.seed,
            self.ci_alpha,
        );
        let after = eval_config(
            user,
            &sats,
            &beacons,
            self.mask_rad(),
            sigma_ure,
            self.bootstrap_resamples,
            self.seed.wrapping_add(1),
            self.ci_alpha,
        );

        Ok(StudyModel {
            sigma_ure_m: sigma_ure,
            n_sweep,
            crossing_n,
            planned_lo: 4,
            planned_hi: 6,
            map_cells,
            map_bands,
            before,
            after,
        })
    }

    /// The committed, byte-stable `lunar_dop_summary.csv` content. A long/tidy table
    /// with a `section` discriminator; fixed-precision numeric formatting so the bytes
    /// are reproducible.
    fn csv_string(&self, m: &StudyModel) -> String {
        let f = |x: Option<f64>| -> String {
            match x {
                Some(v) => format!("{v:.6}"),
                None => String::new(),
            }
        };
        let mut s = String::new();
        s.push_str(
            "section,label,n_sats,user_lat_deg,gdop_median,hdop_median,vdop_median,coverage_fraction,frac_below_gdop6\n",
        );
        // GDOP-vs-N sweep.
        for r in &m.n_sweep {
            s.push_str(&format!(
                "gdop_vs_n,N={n},{n},,{g},,,{cov:.6},{f6:.6}\n",
                n = r.n_sats,
                g = f(r.gdop_median),
                cov = r.coverage_fraction,
                f6 = r.frac_below_gdop6,
            ));
        }
        // Before/after.
        s.push_str(&format!(
            "before_after,orbit_only,{n},{lat:.1},{g},{h},{v},,\n",
            n = self.n_fleet,
            lat = self.headline_lat_deg,
            g = f(m.before.gdop_median),
            h = f(m.before.hdop_median),
            v = f(m.before.vdop_median),
        ));
        s.push_str(&format!(
            "before_after,beacons_{nb},{n},{lat:.1},{g},{h},{v},,\n",
            nb = self.headline_beacons,
            n = self.n_fleet,
            lat = self.headline_lat_deg,
            g = f(m.after.gdop_median),
            h = f(m.after.hdop_median),
            v = f(m.after.vdop_median),
        ));
        // Spatial map, per-latitude band.
        for b in &m.map_bands {
            s.push_str(&format!(
                "map_band,lat={lat:.1},,{lat:.1},{g},{h},{v},,\n",
                lat = b.lat_deg,
                g = f(b.gdop_median),
                h = f(b.hdop_median),
                v = f(b.vdop_median),
            ));
        }
        s
    }

    fn json(&self, m: &StudyModel) -> Value {
        let n_sweep: Vec<Value> = m
            .n_sweep
            .iter()
            .map(|r| {
                json!({
                    "n_sats": r.n_sats,
                    "gdop_median": tagged(onum(r.gdop_median), Tag::Validated),
                    "coverage_fraction": tagged(json!(r.coverage_fraction), Tag::Validated),
                    "frac_below_gdop6": tagged(json!(r.frac_below_gdop6), Tag::Validated),
                })
            })
            .collect();
        let map_bands: Vec<Value> = m
            .map_bands
            .iter()
            .map(|b| {
                json!({
                    "lat_deg": b.lat_deg,
                    "gdop_median": tagged(onum(b.gdop_median), Tag::Validated),
                    "hdop_median": tagged(onum(b.hdop_median), Tag::Validated),
                    "vdop_median": tagged(onum(b.vdop_median), Tag::Validated),
                })
            })
            .collect();
        json!({
            "gdop_vs_n": {
                "planned_fleet_band": [m.planned_lo, m.planned_hi],
                "gdop6_crossing_n": tagged(
                    m.crossing_n.map(|n| json!(n)).unwrap_or(Value::Null),
                    Tag::Validated,
                ),
                "rows": n_sweep,
            },
            "spatial_gdop_map": {
                "n_cells": m.map_cells.len(),
                "band_lat_deg": [self.band_lat_min_deg, self.band_lat_max_deg],
                "per_latitude_bands": map_bands,
            },
            "beacon_before_after": {
                "user_lat_deg": self.headline_lat_deg,
                "beacon_count": self.headline_beacons,
                "sigma_ure_m": tagged(json!(m.sigma_ure_m), Tag::Modelled),
                "orbit_only": config_json(&m.before),
                "with_beacons": config_json(&m.after),
            },
            "csv": self.csv_string(m),
            "csv_filename": "lunar_dop_summary.csv",
        })
    }

    /// Write the P2 Figure-1 artifacts (the committed CSV + the three SVGs + the HTML
    /// report) into `papers/surface-beacon-dop/` (a NEW dir — the founder's other paper
    /// dirs are untouched). Deterministic; the CSV is the byte-stable, committed record.
    /// Invoked out-of-band (the `#[ignore]`d `emit_paper_artifacts` test), never from
    /// `run` (which stays pure and returns the same bytes in its JSON envelope).
    pub fn emit_paper_artifacts(&self) -> std::io::Result<std::path::PathBuf> {
        let m = self
            .compute()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("papers/surface-beacon-dop");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("lunar_dop_summary.csv"), self.csv_string(&m))?;
        std::fs::write(dir.join("fig1_gdop_map.svg"), fig_gdop_map_svg(self, &m))?;
        std::fs::write(dir.join("fig1_gdop_vs_n.svg"), fig_gdop_vs_n_svg(self, &m))?;
        std::fs::write(
            dir.join("fig1_beacon_before_after.svg"),
            fig_before_after_svg(&m),
        )?;
        std::fs::write(dir.join("report.html"), study_report_html(self, &m))?;
        Ok(dir)
    }
}

impl Scenario for SurfaceBeaconDopStudyScenario {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let m = self.compute()?;
        let fig_map = fig_gdop_map_svg(self, &m);
        let fig_curve = fig_gdop_vs_n_svg(self, &m);
        let fig_ba = fig_before_after_svg(&m);
        let report_html = study_report_html(self, &m);
        let model_json = self.json(&m);

        let doc = json!({
            "kind": Self::KIND,
            "overlay": "kshana-pro",
            "overlay_version": crate::version(),
            "label": SURFACE_BEACON_DOP_STUDY_LABEL,
            "paper": "P2 — Surface-beacon-augmented lunar DOP, Figure 1 (L46)",
            "provenance_legend": {
                "Validated": "every per-sample GDOP/HDOP/VDOP inherits the kshana::orbit::dop kernel (cross-checked vs gnss_lib_py/NumPy); medians/coverage are deterministic order statistics. A computational check, NOT flight validation or certification.",
                "Modelled": "the illustrative constellation, relay set, beacon placement and error budget are representative scenario inputs.",
            },
            "model": model_json,
            "figures": {
                "fig1_gdop_vs_n_svg": fig_curve,
                "fig1_spatial_gdop_map_svg": fig_map,
                "fig1_beacon_before_after_svg": fig_ba,
            },
            "report_html": report_html,
        })
        .to_string();

        let before = m.before.gdop_median.unwrap_or(f64::NAN);
        let after = m.after.gdop_median.unwrap_or(f64::NAN);
        let summary = format!(
            "Kshana Pro P2 surface-beacon-DOP study (L46) | GDOP-vs-N crosses 6 at N={cross} (~2× the {lo}–{hi} planned fleet) [Validated] | \
             polar map {cells} cells | beacon before/after {bef:.1} → {aft:.1} at {lat:.0}° [Validated] | \
             CSV lunar_dop_summary.csv emitted (byte-stable)",
            cross = m.crossing_n.map(|n| n.to_string()).unwrap_or_else(|| ">n_max".into()),
            lo = m.planned_lo,
            hi = m.planned_hi,
            cells = m.map_cells.len(),
            bef = before,
            aft = after,
            lat = self.headline_lat_deg,
        );

        Ok(RunOutput {
            json: doc,
            svg: fig_curve,
            summary,
            csv: None,
        })
    }
}

impl ExternalPack for SurfaceBeaconDopStudyScenario {
    fn kind_name(&self) -> &'static str {
        Self::KIND
    }
    fn meta(&self) -> ScenarioMeta {
        Self::meta_static()
    }
    fn register_into(reg: &mut kshana::registry::PackRegistry) {
        reg.register(crate::registry::pro_factory::<Self>(Self::KIND));
    }
}

// ── L46 figures ─────────────────────────────────────────────────────────────

/// Fig 1 (curve) — median GDOP vs constellation size N, with the planned-fleet band
/// and the GDOP-6 usable-geometry line + crossing.
fn fig_gdop_vs_n_svg(scn: &SurfaceBeaconDopStudyScenario, m: &StudyModel) -> String {
    let (w, h) = (640.0_f64, 380.0_f64);
    let (ml, mr, mt, mb) = (60.0_f64, 30.0_f64, 62.0_f64, 46.0_f64);
    let n_lo = scn.n_min.max(1) as f64;
    let n_hi = scn.n_max.min(24).max(scn.n_min) as f64;
    let px = |n: f64| {
        let f = if (n_hi - n_lo).abs() < 1e-9 {
            0.5
        } else {
            (n - n_lo) / (n_hi - n_lo)
        };
        ml + (w - ml - mr) * f
    };
    let gmax = m
        .n_sweep
        .iter()
        .filter_map(|r| r.gdop_median)
        .fold(8.0_f64, f64::max)
        * 1.1;
    let py = |g: f64| mt + (h - mt - mb) * (1.0 - (g / gmax).clamp(0.0, 1.0));

    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" font-family=\"{FONT_BODY}\" font-size=\"11\">"
    );
    s += &format!("<rect width=\"100%\" height=\"100%\" fill=\"{NAVY}\"/>");
    s += &format!("<text x=\"20\" y=\"26\" fill=\"{BRASS}\" font-family=\"{FONT_HEAD}\" font-size=\"17\" font-weight=\"bold\">Figure 1 — median GDOP vs constellation size</text>");
    s += &format!("<text x=\"20\" y=\"44\" fill=\"{IVORY}\">South-polar band · orbit-only <tspan fill=\"#2dd4bf\">[Validated kernel]</tspan></text>");

    let (x0, x1) = (ml, w - mr);
    let (yb, yt) = (h - mb, mt);
    // Planned-fleet band shading.
    let (bx0, bx1) = (px(m.planned_lo as f64), px(m.planned_hi as f64));
    s += &format!("<rect x=\"{bx0:.1}\" y=\"{yt}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"#233047\" opacity=\"0.6\"/>", (bx1 - bx0).max(1.0), yb - yt);
    s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{BRASS_MUTED}\" font-size=\"9\" text-anchor=\"middle\">planned {}–{}</text>", (bx0 + bx1) / 2.0, yt - 3.0, m.planned_lo, m.planned_hi);
    // Axes.
    s += &format!("<line x1=\"{x0}\" y1=\"{yb}\" x2=\"{x1}\" y2=\"{yb}\" stroke=\"{BRASS_MUTED}\" stroke-width=\"1\"/>");
    s += &format!("<line x1=\"{x0}\" y1=\"{yt}\" x2=\"{x0}\" y2=\"{yb}\" stroke=\"{BRASS_MUTED}\" stroke-width=\"1\"/>");
    // GDOP=6 line.
    let y6 = py(6.0);
    s += &format!("<line x1=\"{x0}\" y1=\"{y6:.1}\" x2=\"{x1}\" y2=\"{y6:.1}\" stroke=\"#7f6b3a\" stroke-dasharray=\"4 3\" stroke-width=\"1\"/>");
    s += &format!(
        "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{BRASS_MUTED}\" font-size=\"9\">GDOP 6</text>",
        x0 + 4.0,
        y6 - 3.0
    );
    // x labels every 4.
    let mut n = (n_lo as usize).max(1);
    while (n as f64) <= n_hi {
        let x = px(n as f64);
        s += &format!(
            "<text x=\"{x:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" text-anchor=\"middle\">{n}</text>",
            yb + 16.0
        );
        n += 4;
    }
    s += &format!(
        "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{BRASS}\" text-anchor=\"middle\">satellites N</text>",
        (x0 + x1) / 2.0,
        h - 8.0
    );
    for k in 0..=4 {
        let g = gmax * k as f64 / 4.0;
        let y = py(g);
        s += &format!("<line x1=\"{x0}\" y1=\"{y:.1}\" x2=\"{x1}\" y2=\"{y:.1}\" stroke=\"#1c2637\" stroke-width=\"1\"/>");
        s += &format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" text-anchor=\"end\">{g:.0}</text>",
            x0 - 6.0,
            y + 4.0
        );
    }
    // Curve.
    let pts: String = m
        .n_sweep
        .iter()
        .filter_map(|r| {
            r.gdop_median
                .map(|g| format!("{:.1},{:.1}", px(r.n_sats as f64), py(g)))
        })
        .collect::<Vec<_>>()
        .join(" ");
    s += &format!(
        "<polyline points=\"{pts}\" fill=\"none\" stroke=\"#f0a868\" stroke-width=\"2.4\"/>"
    );
    for r in &m.n_sweep {
        if let Some(g) = r.gdop_median {
            s += &format!(
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"2.6\" fill=\"#f0a868\"/>",
                px(r.n_sats as f64),
                py(g)
            );
        }
    }
    // Crossing marker.
    if let Some(nc) = m.crossing_n {
        let x = px(nc as f64);
        s += &format!("<line x1=\"{x:.1}\" y1=\"{yt}\" x2=\"{x:.1}\" y2=\"{yb}\" stroke=\"#2dd4bf\" stroke-dasharray=\"3 3\" stroke-width=\"1.2\"/>");
        s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"#2dd4bf\" font-size=\"10\">crosses 6 @ N={nc}</text>", x + 4.0, yt + 12.0);
    }
    s += "</svg>";
    s
}

/// Linear interpolation between two hex colours by `t` in [0,1].
fn heat(t: f64) -> String {
    let t = t.clamp(0.0, 1.0);
    // teal (#2dd4bf, low GDOP) → brass/orange (#f0a868, high GDOP).
    let (r0, g0, b0) = (0x2d, 0xd4, 0xbf);
    let (r1, g1, b1) = (0xf0, 0xa8, 0x68);
    let lerp = |a: i32, b: i32| (a as f64 + (b as f64 - a as f64) * t).round() as i32;
    format!(
        "#{:02x}{:02x}{:02x}",
        lerp(r0, r1),
        lerp(g0, g1),
        lerp(b0, b1)
    )
}

/// Fig 1 (map) — the polar spatial GDOP heatmap (lat × lon).
fn fig_gdop_map_svg(scn: &SurfaceBeaconDopStudyScenario, m: &StudyModel) -> String {
    let (w, h) = (640.0_f64, 380.0_f64);
    let (ml, mr, mt, mb) = (60.0_f64, 90.0_f64, 62.0_f64, 46.0_f64);
    let nlat = scn.map_n_lat.max(1);
    let nlon = scn.map_n_lon.max(1);
    let cell_w = (w - ml - mr) / nlon as f64;
    let cell_h = (h - mt - mb) / nlat as f64;
    // Colour scale bounds.
    let gvals: Vec<f64> = m.map_cells.iter().filter_map(|c| c.gdop_median).collect();
    let gmin = gvals.iter().cloned().fold(f64::INFINITY, f64::min);
    let gmax = gvals
        .iter()
        .cloned()
        .fold(0.0_f64, f64::max)
        .max(gmin + 1e-6);

    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" font-family=\"{FONT_BODY}\" font-size=\"11\">"
    );
    s += &format!("<rect width=\"100%\" height=\"100%\" fill=\"{NAVY}\"/>");
    s += &format!("<text x=\"20\" y=\"26\" fill=\"{BRASS}\" font-family=\"{FONT_HEAD}\" font-size=\"17\" font-weight=\"bold\">Figure 1 — polar spatial GDOP map</text>");
    s += &format!("<text x=\"20\" y=\"44\" fill=\"{IVORY}\">Median GDOP over epochs, lat×lon <tspan fill=\"#2dd4bf\">[Validated kernel]</tspan></text>");

    // Cells: index into the row-major map (lat outer, lon inner).
    for i in 0..nlat {
        for j in 0..nlon {
            let idx = i * nlon + j;
            if idx >= m.map_cells.len() {
                continue;
            }
            let c = &m.map_cells[idx];
            let x = ml + j as f64 * cell_w;
            let y = mt + i as f64 * cell_h;
            let fill = match c.gdop_median {
                Some(g) => heat((g - gmin) / (gmax - gmin)),
                None => "#141b28".to_string(),
            };
            s += &format!(
                "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"{fill}\" stroke=\"{NAVY}\" stroke-width=\"0.5\"/>",
                cell_w + 0.6,
                cell_h + 0.6
            );
        }
    }
    // Axis ticks: lat on the left, lon on the bottom.
    s += &format!(
        "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" text-anchor=\"end\">{:.0}°</text>",
        ml - 6.0,
        mt + 8.0,
        scn.band_lat_max_deg
    );
    s += &format!(
        "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" text-anchor=\"end\">{:.0}°</text>",
        ml - 6.0,
        h - mb,
        scn.band_lat_min_deg
    );
    s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{BRASS}\" text-anchor=\"middle\">longitude −180°…+180°</text>", ml + (w - ml - mr) / 2.0, h - 8.0);
    // Colour legend.
    let lx = w - mr + 14.0;
    for k in 0..=10 {
        let t = k as f64 / 10.0;
        let yy = mt + (h - mt - mb) * (1.0 - t);
        s += &format!(
            "<rect x=\"{lx:.1}\" y=\"{yy:.1}\" width=\"12\" height=\"{:.1}\" fill=\"{}\"/>",
            (h - mt - mb) / 11.0 + 1.0,
            heat(t)
        );
    }
    s += &format!(
        "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" font-size=\"9\">{gmax:.0}</text>",
        lx + 16.0,
        mt + 8.0
    );
    s += &format!(
        "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" font-size=\"9\">{gmin:.0}</text>",
        lx + 16.0,
        h - mb
    );
    s += &format!(
        "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{BRASS_MUTED}\" font-size=\"9\">GDOP</text>",
        lx + 14.0,
        mt - 6.0
    );

    s += "</svg>";
    s
}

/// Fig 1 (before/after) — grouped bars: GDOP/HDOP/VDOP orbit-only vs +beacons.
fn fig_before_after_svg(m: &StudyModel) -> String {
    let (w, h) = (640.0_f64, 340.0_f64);
    let (ml, _mr, mt, mb) = (60.0_f64, 30.0_f64, 62.0_f64, 46.0_f64);
    let groups = [
        ("GDOP", m.before.gdop_median, m.after.gdop_median),
        ("HDOP", m.before.hdop_median, m.after.hdop_median),
        ("VDOP", m.before.vdop_median, m.after.vdop_median),
    ];
    let vmax = groups
        .iter()
        .flat_map(|(_, a, b)| [*a, *b])
        .flatten()
        .fold(1.0_f64, f64::max)
        * 1.15;
    let plot_h = h - mt - mb;
    let py = |v: f64| mt + plot_h * (1.0 - (v / vmax).clamp(0.0, 1.0));
    let group_w = 150.0_f64;
    let bar_w = 46.0_f64;

    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" font-family=\"{FONT_BODY}\" font-size=\"11\">"
    );
    s += &format!("<rect width=\"100%\" height=\"100%\" fill=\"{NAVY}\"/>");
    s += &format!("<text x=\"20\" y=\"26\" fill=\"{BRASS}\" font-family=\"{FONT_HEAD}\" font-size=\"17\" font-weight=\"bold\">Figure 1 — beacon before / after</text>");
    s += &format!("<text x=\"20\" y=\"44\" fill=\"{IVORY}\">Median DOP, orbit-only vs surface-beacon-augmented <tspan fill=\"#2dd4bf\">[Validated]</tspan></text>");
    let yb = h - mb;
    s += &format!("<line x1=\"{ml}\" y1=\"{yb}\" x2=\"{:.1}\" y2=\"{yb}\" stroke=\"{BRASS_MUTED}\" stroke-width=\"1\"/>", w - 20.0);

    for (gi, (label, a, b)) in groups.iter().enumerate() {
        let gx = ml + 20.0 + gi as f64 * group_w;
        if let Some(v) = a {
            let y = py(*v);
            s += &format!("<rect x=\"{gx:.1}\" y=\"{y:.1}\" width=\"{bar_w}\" height=\"{:.1}\" fill=\"#f0a868\"/>", yb - y);
            s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" font-size=\"10\" text-anchor=\"middle\">{v:.1}</text>", gx + bar_w / 2.0, y - 4.0);
        }
        let bx = gx + bar_w + 8.0;
        if let Some(v) = b {
            let y = py(*v);
            s += &format!("<rect x=\"{bx:.1}\" y=\"{y:.1}\" width=\"{bar_w}\" height=\"{:.1}\" fill=\"#2dd4bf\"/>", yb - y);
            s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" font-size=\"10\" text-anchor=\"middle\">{v:.1}</text>", bx + bar_w / 2.0, y - 4.0);
        }
        s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" font-size=\"11\" text-anchor=\"middle\">{label}</text>", gx + bar_w + 4.0, yb + 16.0);
    }
    // Legend.
    s += &format!(
        "<rect x=\"{:.1}\" y=\"58\" width=\"12\" height=\"12\" fill=\"#f0a868\"/>",
        w - 190.0
    );
    s += &format!(
        "<text x=\"{:.1}\" y=\"68\" fill=\"{IVORY}\" font-size=\"10\">orbit-only</text>",
        w - 174.0
    );
    s += &format!(
        "<rect x=\"{:.1}\" y=\"58\" width=\"12\" height=\"12\" fill=\"#2dd4bf\"/>",
        w - 100.0
    );
    s += &format!(
        "<text x=\"{:.1}\" y=\"68\" fill=\"{IVORY}\" font-size=\"10\">+beacons</text>",
        w - 84.0
    );
    s += "</svg>";
    s
}

// ── L46 branded HTML report ─────────────────────────────────────────────────

fn study_report_html(scn: &SurfaceBeaconDopStudyScenario, m: &StudyModel) -> String {
    let n_rows: String = m
        .n_sweep
        .iter()
        .map(|r| {
            format!(
                "<tr>{}{}{}{}</tr>",
                td(&r.n_sats.to_string()),
                td(&fmt_opt(r.gdop_median, 2)),
                td(&format!("{:.3}", r.coverage_fraction)),
                td(&format!("{:.3}", r.frac_below_gdop6)),
            )
        })
        .collect();
    let band_rows: String = m
        .map_bands
        .iter()
        .map(|b| {
            format!(
                "<tr>{}{}{}{}</tr>",
                td(&format!("{:.0}°", b.lat_deg)),
                td(&fmt_opt(b.gdop_median, 2)),
                td(&fmt_opt(b.hdop_median, 2)),
                td(&fmt_opt(b.vdop_median, 2)),
            )
        })
        .collect();

    let cross = m
        .crossing_n
        .map(|n| n.to_string())
        .unwrap_or_else(|| ">n_max".into());

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <title>Kshana Pro — P2 Surface-Beacon-DOP Study (L46)</title></head>\
         <body style=\"margin:0;background:{NAVY};color:{IVORY};font-family:{FONT_BODY};\
         max-width:920px;margin:0 auto;padding:0 28px 60px\">\
         {cover}{banner}\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Figure 1 — three views</h2>\
         <p>The orbit-only median GDOP crosses the usable-geometry value 6 at <strong>N={cross}</strong> {bv} — \
         about twice the {lo}–{hi} planned fleet. Three surface beacons collapse the {lat:.0}° user's median GDOP \
         from <strong>{gb}</strong> to <strong>{ga}</strong> {bv}, with HDOP {hb}→{ha} and VDOP {vb}→{va} \
         (reported separately). σ_URE {sig:.2} m.</p>\
         <h3 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">GDOP vs N</h3>{fig_curve}\
         <h3 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Polar spatial GDOP map</h3>{fig_map}\
         <h3 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Beacon before / after</h3>{fig_ba}\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">GDOP-vs-N table (committed CSV)</h2>\
         <table style=\"border-collapse:collapse;width:100%\"><tr>{h_n}{h_g}{h_cov}{h_f6}</tr>{n_rows}</table>\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Spatial map, per-latitude band</h2>\
         <table style=\"border-collapse:collapse;width:100%\"><tr>{h_lat}{h_g}{h_h}{h_v}</tr>{band_rows}</table>\
         <p style=\"font-size:12px;opacity:.8\">The full table is committed to \
         <code>papers/surface-beacon-dop/lunar_dop_summary.csv</code> (byte-stable). All DOP figures {bv}.</p>\
         </body></html>",
        cover = cover_html(
            "Surface-Beacon Lunar DOP — Figure 1",
            "P2 / L46 — GDOP-vs-N, polar map, and beacon before/after"
        ),
        banner = modelled_banner_html(
            "every per-sample DOP inherits the cross-checked open kernel; the illustrative constellation, \
             relay set, beacon placement and error budget are Modelled."
        ),
        cross = cross,
        bv = badge(Tag::Validated),
        lo = m.planned_lo,
        hi = m.planned_hi,
        lat = scn.headline_lat_deg,
        gb = fmt_opt(m.before.gdop_median, 2),
        ga = fmt_opt(m.after.gdop_median, 2),
        hb = fmt_opt(m.before.hdop_median, 2),
        ha = fmt_opt(m.after.hdop_median, 2),
        vb = fmt_opt(m.before.vdop_median, 2),
        va = fmt_opt(m.after.vdop_median, 2),
        sig = m.sigma_ure_m,
        fig_curve = fig_gdop_vs_n_svg(scn, m),
        fig_map = fig_gdop_map_svg(scn, m),
        fig_ba = fig_before_after_svg(m),
        h_n = th("N"),
        h_g = th("GDOP median"),
        h_cov = th("Coverage"),
        h_f6 = th("Frac &lt; 6"),
        n_rows = n_rows,
        h_lat = th("Lat"),
        h_h = th("HDOP"),
        h_v = th("VDOP"),
        band_rows = band_rows,
    )
}

// ═════════════════════════════════════════════════════════════════════════════
// Tests
// ═════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ---- L45 ----------------------------------------------------------------

    fn l45_run() -> (BeaconDopModel, Value) {
        let scn = BeaconDopScenario::default();
        let m = scn.compute().expect("L45 compute");
        let out = scn.run().expect("L45 run");
        let v: Value = serde_json::from_str(&out.json).unwrap();
        (m, v)
    }

    #[test]
    fn l45_beacons_cut_polar_gdop_substantially() {
        let (m, _) = l45_run();
        let before = m
            .headline_before
            .gdop_median
            .expect("orbit-only GDOP defined");
        let after = m.headline_after.gdop_median.expect("beacon GDOP defined");
        assert!(before.is_finite() && after.is_finite());
        assert!(after < before, "beacons must cut GDOP: {before} -> {after}");
        assert!(
            before / after > 2.0,
            "expected a substantial reduction (>2×): {before} -> {after}"
        );
    }

    #[test]
    fn l45_hdop_and_vdop_reported_separately_and_finite() {
        let (m, v) = l45_run();
        for c in [&m.headline_before, &m.headline_after] {
            let hd = c.hdop_median.expect("HDOP defined");
            let vd = c.vdop_median.expect("VDOP defined");
            assert!(hd.is_finite() && hd > 0.0, "HDOP {hd}");
            assert!(vd.is_finite() && vd > 0.0, "VDOP {vd}");
        }
        // Both surfaces expose HDOP and VDOP keys distinctly (never a lone GDOP).
        let after = &v["model"]["headline_before_after"]["with_beacons"];
        assert!(after["hdop_median"]["value"].is_number());
        assert!(after["vdop_median"]["value"].is_number());
        assert_ne!(after["hdop_median"]["value"], after["vdop_median"]["value"]);
    }

    #[test]
    fn l45_realized_accuracy_is_dop_times_uere() {
        // Oracle: σ = DOP · σ_URE per component; σ_URE = RSS of the Modelled budget.
        let scn = BeaconDopScenario::default();
        let m = scn.compute().unwrap();
        let sigma =
            (scn.clock_sync_m.powi(2) + scn.multipath_m.powi(2) + scn.survey_m.powi(2)).sqrt();
        assert!(
            (m.sigma_ure_m - sigma).abs() < 1e-12,
            "σ_URE {} vs {}",
            m.sigma_ure_m,
            sigma
        );
        let c = &m.headline_after;
        let a = c.accuracy.expect("accuracy defined");
        assert!((a.horizontal_m - c.hdop_median.unwrap() * sigma).abs() < 1e-9);
        assert!((a.vertical_m - c.vdop_median.unwrap() * sigma).abs() < 1e-9);
        assert!((a.pos_3d_m - c.pdop_median.unwrap() * sigma).abs() < 1e-9);
        assert!((a.time_m - c.tdop_median.unwrap() * sigma).abs() < 1e-9);
    }

    #[test]
    fn l45_bootstrap_ci_is_seeded_deterministic_and_brackets_median() {
        let scn = BeaconDopScenario::default();
        let a = scn.compute().unwrap();
        let b = scn.compute().unwrap();
        // Identical bootstrap CIs across independent computes (fixed seed, no clock).
        let (a_lo, a_hi) = a.headline_after.gdop_ci.expect("CI");
        let (b_lo, b_hi) = b.headline_after.gdop_ci.expect("CI");
        assert_eq!(
            (a_lo, a_hi),
            (b_lo, b_hi),
            "seeded bootstrap must be deterministic"
        );
        // The CI brackets the point median (percentile bootstrap property).
        let med = a.headline_after.gdop_median.unwrap();
        assert!(
            a_lo <= med + 1e-9 && med <= a_hi + 1e-9,
            "CI [{a_lo},{a_hi}] vs median {med}"
        );
        assert!(a_lo <= a_hi);
    }

    #[test]
    fn l45_latitude_sweep_covers_band_and_beacons_help_everywhere() {
        let (m, _) = l45_run();
        assert!(m.lat_rows.len() >= 3, "latitude band sampled");
        for r in &m.lat_rows {
            assert!(r.lat_deg >= -90.0 - 1e-9 && r.lat_deg <= -70.0 + 1e-9);
            if let (Some(o), Some(b)) = (r.orbit_only.gdop_median, r.with_beacons.gdop_median) {
                assert!(
                    b <= o + 1e-9,
                    "beacons never worsen GDOP at {}: {o} -> {b}",
                    r.lat_deg
                );
            }
        }
    }

    #[test]
    fn l45_output_is_deterministic() {
        let scn = BeaconDopScenario::default();
        let a = scn.run().unwrap();
        let b = scn.run().unwrap();
        assert_eq!(a.json, b.json);
        assert_eq!(a.svg, b.svg);
        assert_eq!(a.summary, b.summary);
    }

    #[test]
    fn l45_envelope_and_contract() {
        let (_, v) = l45_run();
        let scn = BeaconDopScenario::default();
        assert_eq!(scn.kind_name(), "pro-beacon-dop");
        assert_eq!(scn.meta().name, "pro-beacon-dop");
        assert_eq!(v["kind"], "pro-beacon-dop");
        let out = scn.run().unwrap();
        assert!(out.svg.contains("<svg"));
        assert!(v["report_html"]
            .as_str()
            .unwrap()
            .contains("<!doctype html>"));
        assert!(out.summary.contains("Validated"));
    }

    #[test]
    fn l45_empty_toml_reproduces_baseline() {
        let scn: BeaconDopScenario = toml::from_str("").unwrap();
        let m = scn.compute().unwrap();
        assert!(m.headline_before.gdop_median.unwrap() > m.headline_after.gdop_median.unwrap());
    }

    // ---- L46 ----------------------------------------------------------------

    fn l46_run() -> (StudyModel, Value) {
        let scn = SurfaceBeaconDopStudyScenario::default();
        let m = scn.compute().expect("L46 compute");
        let out = scn.run().expect("L46 run");
        let v: Value = serde_json::from_str(&out.json).unwrap();
        (m, v)
    }

    #[test]
    fn l46_sweep_reaches_n24_and_gdop_decreases_with_n() {
        let (m, _) = l46_run();
        assert_eq!(m.n_sweep.first().unwrap().n_sats, 1);
        assert_eq!(
            m.n_sweep.last().unwrap().n_sats,
            24,
            "sweep must reach N=24"
        );
        // Compare a small-N vs the largest N (median GDOP decreases with N).
        let g4 = m
            .n_sweep
            .iter()
            .find(|r| r.n_sats == 4)
            .unwrap()
            .gdop_median
            .unwrap();
        let g24 = m
            .n_sweep
            .iter()
            .find(|r| r.n_sats == 24)
            .unwrap()
            .gdop_median
            .unwrap();
        assert!(
            g24 < g4,
            "median GDOP should fall with N: N4={g4} N24={g24}"
        );
    }

    #[test]
    fn l46_gdop6_crossing_near_twice_planned_fleet() {
        let (m, _) = l46_run();
        let nc = m.crossing_n.expect("GDOP crosses 6 within the sweep");
        // The crossing is above the planned fleet (the paper's ~2× headline) and within N≤24.
        assert!(
            nc > m.planned_hi,
            "crossing N={nc} should exceed the planned {}",
            m.planned_hi
        );
        assert!(nc <= 24, "crossing N={nc} within the sweep");
    }

    #[test]
    fn l46_beacons_cut_gdop_and_hdop_vdop_reported() {
        let (m, _) = l46_run();
        let before = m.before.gdop_median.expect("orbit-only GDOP");
        let after = m.after.gdop_median.expect("beacon GDOP");
        assert!(
            after < before && before / after > 2.0,
            "before {before} after {after}"
        );
        assert!(m.after.hdop_median.unwrap().is_finite());
        assert!(m.after.vdop_median.unwrap().is_finite());
        // Realized accuracy is DOP × σ_URE.
        let a = m.after.accuracy.unwrap();
        assert!((a.horizontal_m - m.after.hdop_median.unwrap() * m.sigma_ure_m).abs() < 1e-9);
    }

    #[test]
    fn l46_map_covers_band_and_is_defined() {
        let (m, _) = l46_run();
        assert_eq!(m.map_cells.len(), 9 * 12, "map grid");
        for c in &m.map_cells {
            assert!(c.lat_deg >= -90.0 - 1e-9 && c.lat_deg <= -70.0 + 1e-9);
            if let Some(g) = c.gdop_median {
                assert!(g >= 1.0, "GDOP {g} below the DOP floor");
            }
        }
        assert!(!m.map_bands.is_empty());
    }

    #[test]
    fn l46_output_and_csv_are_deterministic() {
        let scn = SurfaceBeaconDopStudyScenario::default();
        let a = scn.run().unwrap();
        let b = scn.run().unwrap();
        assert_eq!(a.json, b.json);
        assert_eq!(a.svg, b.svg);
        assert_eq!(a.summary, b.summary);
        // The CSV is byte-identical on repeat.
        let ma = scn.compute().unwrap();
        let mb = scn.compute().unwrap();
        assert_eq!(scn.csv_string(&ma), scn.csv_string(&mb));
        // And it is embedded in the JSON envelope.
        let v: Value = serde_json::from_str(&a.json).unwrap();
        assert_eq!(v["model"]["csv_filename"], "lunar_dop_summary.csv");
        assert!(v["model"]["csv"]
            .as_str()
            .unwrap()
            .starts_with("section,label,n_sats"));
    }

    #[test]
    fn l46_committed_csv_matches_regeneration() {
        // Drift guard: the committed golden CSV must equal a fresh regeneration byte
        // for byte. Founder re-runs the ignored `emit_paper_artifacts` test to refresh
        // it if the deterministic content ever legitimately changes.
        let scn = SurfaceBeaconDopStudyScenario::default();
        let m = scn.compute().unwrap();
        let fresh = scn.csv_string(&m);
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/papers/surface-beacon-dop/lunar_dop_summary.csv"
        );
        let committed = std::fs::read_to_string(path)
            .expect("committed lunar_dop_summary.csv must exist (run emit_paper_artifacts)");
        assert_eq!(fresh, committed, "committed CSV drifted from regeneration");
    }

    #[test]
    fn l46_envelope_and_contract() {
        let (_, v) = l46_run();
        let scn = SurfaceBeaconDopStudyScenario::default();
        assert_eq!(scn.kind_name(), "pro-surface-beacon-dop-study");
        assert_eq!(scn.meta().name, "pro-surface-beacon-dop-study");
        assert_eq!(v["kind"], "pro-surface-beacon-dop-study");
        let out = scn.run().unwrap();
        assert!(out.svg.contains("<svg"));
        assert!(v["figures"]["fig1_spatial_gdop_map_svg"]
            .as_str()
            .unwrap()
            .contains("Figure 1"));
        assert!(v["report_html"]
            .as_str()
            .unwrap()
            .contains("<!doctype html>"));
    }

    // Opt-in: (re)write the committed P2 Figure-1 artifacts. Ignored in the normal
    // run so `cargo test` never dirties the tree; run explicitly with
    // `cargo test -p kshana-pro emit_paper_artifacts -- --ignored`.
    #[test]
    #[ignore]
    fn emit_paper_artifacts() {
        let dir = SurfaceBeaconDopStudyScenario::default()
            .emit_paper_artifacts()
            .expect("emit artifacts");
        assert!(dir.join("lunar_dop_summary.csv").exists());
    }
}
