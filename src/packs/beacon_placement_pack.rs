// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The `pro-beacon-placement` (L47) pack — treats P2's surface-beacon **siting** as
//! the genuine optimization variable it always was.
//!
//! Where the L45/L46 beacon-DOP packs *fix* a beacon geometry and sweep it, this pack
//! **optimizes placement over terrain**. It composes the open substrate:
//!
//! * a DEM tile ([`kshana::realdata::lola_dem::DemTile`]) and its Validated closed-form
//!   terrain line-of-sight ([`terrain_los_clear`]);
//! * the airless-Moon beacon horizon ([`kshana::lunar_beacon::beacon_visible`]) and the
//!   beacon-augmented DOP kernel ([`dop_with_beacons`], [`realized_accuracy`]);
//! * the general multi-objective machinery ([`kshana::mcda::pareto`] +
//!   [`kshana::mcda::topsis`]).
//!
//! ## What it does
//! 1. A small south-polar **served-user** grid and a grid of **candidate** beacon sites
//!    on the DEM (each placed at its terrain height + a mast). A candidate *serves* a
//!    user only if the beacon clears the horizon **and** the straight line of sight
//!    stays above the real relief — so a crater rim can shadow an otherwise-visible
//!    beacon, and a rim-top vantage can reach a user the bare sphere would hide.
//! 2. For every size-`k` subset of candidate sites (a **full enumeration** = the brute
//!    force), it scores the served-user geometry: the median served-user GDOP and HDOP
//!    (separately) through the Validated DOP kernel, and the served-user fraction.
//! 3. It builds the **Pareto front** over `{minimize median GDOP, minimize median HDOP,
//!    maximize served fraction}` (with the knee point), and picks a single recommended
//!    siting with **TOPSIS**. A greedy-add path is reported as a cheaper alternative.
//! 4. It emits the recommended beacon lat/lon/height set, the served-user GDOP/HDOP
//!    before/after, the full Pareto front, a branded HTML report and a DEM SVG with the
//!    candidate + chosen beacons.
//!
//! ## Validated vs Modelled (honesty discipline — load-bearing)
//! * **Validated** — the horizon + terrain-LOS masking (which candidate serves which
//!   user) inherits the exact airless horizon and the closed-form terrain-LOS geometry;
//!   the per-config DOP inherits the cross-checked [`kshana::orbit::dop`] kernel and the
//!   medians are deterministic order statistics; the Pareto dominance/front is an exact
//!   relation; and the optimizer **agrees with an independent brute-force enumeration**
//!   (the recommended config is on the true front, and under a single-objective weight
//!   the TOPSIS pick equals the brute-force argmin). These are *computational* checks —
//!   NOT flight validation or certification.
//! * **Modelled** — the DEM tile is [`DemTile::representative_south_polar`], a
//!   representative synthetic relief, **not** an actual LOLA PDS product; the TOPSIS
//!   ranking of scenario configs is a preference model; and every scenario input (user
//!   grid, candidate grid, constellation, `k`, error budget, weights) is a
//!   representative choice, never a fielded measurement.
//!
//! Deterministic end-to-end: no wall-clock, no RNG — order statistics and a lexicographic
//! subset enumeration only.

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};
use kshana::lunar::{selenographic_to_mcmf, Selenographic};
use kshana::lunar_beacon::{
    beacon_visible, dop_with_beacons, realized_accuracy, BeaconErrorBudget,
};
use kshana::lunar_service::LunarConstellation;
use kshana::mcda::pareto::{knee_point, pareto_front, KneePoint};
use kshana::mcda::topsis::topsis;
use kshana::mcda::Objective;
use kshana::realdata::lola_dem::{terrain_los_clear, DemTile};

use serde_json::{json, Value};

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
            Tag::Validated => "#2dd4bf",
            Tag::Modelled => BRASS,
        }
    }
}

// ── Geometry & statistics helpers ────────────────────────────────────────────

/// MCMF position of a site *on the terrain*: its radius is `R_moon + DEM height at the
/// ground point + mast/antenna height`. So a beacon on a crater rim sits high (a long
/// horizon) and a user in a basin sits low — exactly the terrain dependence the siting
/// optimizer exploits.
fn terrain_site(dem: &DemTile, lat_deg: f64, lon_deg: f64, mast_m: f64) -> Vec3 {
    let alt = dem.height_at(lat_deg, lon_deg) + mast_m;
    selenographic_to_mcmf(Selenographic {
        lat_rad: lat_deg.to_radians(),
        lon_rad: lon_deg.to_radians(),
        alt_m: alt,
    })
}

/// A candidate beacon *serves* a user iff the beacon clears the airless horizon
/// ([`beacon_visible`]) **and** the straight line of sight stays above the DEM relief
/// ([`terrain_los_clear`]). Validated closed-form geometry — the terrain masking is the
/// extra constraint the bare-sphere horizon misses.
fn serves(dem: &DemTile, user_mcmf: Vec3, beacon_mcmf: Vec3, n_steps: usize) -> bool {
    beacon_visible(user_mcmf, beacon_mcmf)
        && terrain_los_clear(dem, user_mcmf, beacon_mcmf, n_steps)
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

/// `C(n, k)` with saturation (returns `usize::MAX` on overflow) — used only to guard the
/// enumeration size before it is run.
fn binomial(n: usize, k: usize) -> usize {
    if k > n {
        return 0;
    }
    let k = k.min(n - k);
    let mut num: u128 = 1;
    for i in 0..k as u128 {
        num = num.saturating_mul((n as u128) - i) / (i + 1);
    }
    if num > usize::MAX as u128 {
        usize::MAX
    } else {
        num as usize
    }
}

/// All size-`k` subsets of `0..n`, in ascending lexicographic order (deterministic).
fn combinations(n: usize, k: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    if k == 0 || k > n {
        return out;
    }
    let mut idx: Vec<usize> = (0..k).collect();
    loop {
        out.push(idx.clone());
        // Advance the lexicographic combination.
        let mut i = k;
        while i > 0 {
            i -= 1;
            if idx[i] != i + n - k {
                idx[i] += 1;
                for j in i + 1..k {
                    idx[j] = idx[j - 1] + 1;
                }
                break;
            }
            if i == 0 {
                return out;
            }
        }
    }
}

/// Satellite MCMF positions for the illustrative fleet at each requested epoch.
fn sats_per_epoch(n_fleet: usize, n_epochs: usize, step_s: f64) -> Vec<Vec<Vec3>> {
    let con = LunarConstellation::illustrative_lcns(n_fleet);
    (0..n_epochs)
        .map(|k| con.positions_mcmf(k as f64 * step_s))
        .collect()
}

/// Median-over-epochs DOP components for one user against a (static) beacon set.
#[derive(Clone, Copy, Debug, Default)]
struct UDop {
    gdop: Option<f64>,
    hdop: Option<f64>,
    vdop: Option<f64>,
}

/// Evaluate one user's beacon-augmented DOP over all epochs, reducing each defined
/// component to its median (Validated order statistic over the Validated kernel).
fn user_dop_over_epochs(
    user: Vec3,
    sats_per_epoch: &[Vec<Vec3>],
    beacons: &[Vec3],
    mask_rad: f64,
) -> UDop {
    let (mut g, mut h, mut v) = (Vec::new(), Vec::new(), Vec::new());
    for sats in sats_per_epoch {
        if let Some(d) = dop_with_beacons(user, sats, beacons, mask_rad) {
            g.push(d.gdop);
            h.push(d.hdop);
            v.push(d.vdop);
        }
    }
    UDop {
        gdop: median_of(g),
        hdop: median_of(h),
        vdop: median_of(v),
    }
}

// ── Scenario ─────────────────────────────────────────────────────────────────

// The baseline is a *local surface-beacon network* on the rim/flank of the
// representative polar crater: masts a few tens of km apart across the raised rim, so
// the DEM relief genuinely gates which candidate can serve which user (a beacon on the
// far flank is shadowed by the crest). Users and candidates sit on interleaved latitude
// rings (never co-located) spanning the rim crest (colat ≈ 5°, lat −85°).
fn d_user_lat_min() -> f64 {
    -85.5
}
fn d_user_lat_max() -> f64 {
    -84.5
}
fn d_user_n_lat() -> usize {
    2
}
fn d_user_lon_min() -> f64 {
    -6.0
}
fn d_user_lon_max() -> f64 {
    6.0
}
fn d_user_n_lon() -> usize {
    3
}
fn d_user_antenna_h() -> f64 {
    2.0
}
fn d_cand_lat_min() -> f64 {
    -85.0
}
fn d_cand_lat_max() -> f64 {
    -84.0
}
fn d_cand_n_lat() -> usize {
    2
}
fn d_cand_lon_min() -> f64 {
    -6.0
}
fn d_cand_lon_max() -> f64 {
    6.0
}
fn d_cand_n_lon() -> usize {
    3
}
fn d_cand_mast_h() -> f64 {
    100.0
}
fn d_k_beacons() -> usize {
    3
}
fn d_max_configs() -> usize {
    50_000
}
fn d_n_fleet() -> usize {
    6
}
fn d_n_epochs() -> usize {
    6
}
fn d_epoch_step() -> f64 {
    3_600.0
}
fn d_mask_deg() -> f64 {
    5.0
}
fn d_terrain_steps() -> usize {
    48
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
fn d_w_gdop() -> f64 {
    0.5
}
fn d_w_hdop() -> f64 {
    0.2
}
fn d_w_served() -> f64 {
    0.3
}

/// A `pro-beacon-placement` scenario. Every field defaults to the L47 baseline, so an
/// empty scenario (`kind = "pro-beacon-placement"`) reproduces the headline siting study.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct BeaconPlacementScenario {
    #[serde(default = "d_user_lat_min")]
    pub user_lat_min_deg: f64,
    #[serde(default = "d_user_lat_max")]
    pub user_lat_max_deg: f64,
    #[serde(default = "d_user_n_lat")]
    pub user_n_lat: usize,
    #[serde(default = "d_user_lon_min")]
    pub user_lon_min_deg: f64,
    #[serde(default = "d_user_lon_max")]
    pub user_lon_max_deg: f64,
    #[serde(default = "d_user_n_lon")]
    pub user_n_lon: usize,
    #[serde(default = "d_user_antenna_h")]
    pub user_antenna_h_m: f64,
    #[serde(default = "d_cand_lat_min")]
    pub cand_lat_min_deg: f64,
    #[serde(default = "d_cand_lat_max")]
    pub cand_lat_max_deg: f64,
    #[serde(default = "d_cand_n_lat")]
    pub cand_n_lat: usize,
    #[serde(default = "d_cand_lon_min")]
    pub cand_lon_min_deg: f64,
    #[serde(default = "d_cand_lon_max")]
    pub cand_lon_max_deg: f64,
    #[serde(default = "d_cand_n_lon")]
    pub cand_n_lon: usize,
    #[serde(default = "d_cand_mast_h")]
    pub cand_mast_h_m: f64,
    #[serde(default = "d_k_beacons")]
    pub k_beacons: usize,
    #[serde(default = "d_max_configs")]
    pub max_configs: usize,
    #[serde(default = "d_n_fleet")]
    pub n_fleet: usize,
    #[serde(default = "d_n_epochs")]
    pub n_epochs: usize,
    #[serde(default = "d_epoch_step")]
    pub epoch_step_s: f64,
    #[serde(default = "d_mask_deg")]
    pub mask_deg: f64,
    #[serde(default = "d_terrain_steps")]
    pub terrain_steps: usize,
    #[serde(default = "d_clock_sync")]
    pub clock_sync_m: f64,
    #[serde(default = "d_multipath")]
    pub multipath_m: f64,
    #[serde(default = "d_survey")]
    pub survey_m: f64,
    #[serde(default = "d_w_gdop")]
    pub w_gdop: f64,
    #[serde(default = "d_w_hdop")]
    pub w_hdop: f64,
    #[serde(default = "d_w_served")]
    pub w_served: f64,
}

impl Default for BeaconPlacementScenario {
    fn default() -> Self {
        Self {
            user_lat_min_deg: d_user_lat_min(),
            user_lat_max_deg: d_user_lat_max(),
            user_n_lat: d_user_n_lat(),
            user_lon_min_deg: d_user_lon_min(),
            user_lon_max_deg: d_user_lon_max(),
            user_n_lon: d_user_n_lon(),
            user_antenna_h_m: d_user_antenna_h(),
            cand_lat_min_deg: d_cand_lat_min(),
            cand_lat_max_deg: d_cand_lat_max(),
            cand_n_lat: d_cand_n_lat(),
            cand_lon_min_deg: d_cand_lon_min(),
            cand_lon_max_deg: d_cand_lon_max(),
            cand_n_lon: d_cand_n_lon(),
            cand_mast_h_m: d_cand_mast_h(),
            k_beacons: d_k_beacons(),
            max_configs: d_max_configs(),
            n_fleet: d_n_fleet(),
            n_epochs: d_n_epochs(),
            epoch_step_s: d_epoch_step(),
            mask_deg: d_mask_deg(),
            terrain_steps: d_terrain_steps(),
            clock_sync_m: d_clock_sync(),
            multipath_m: d_multipath(),
            survey_m: d_survey(),
            w_gdop: d_w_gdop(),
            w_hdop: d_w_hdop(),
            w_served: d_w_served(),
        }
    }
}

/// One candidate beacon site on the DEM.
#[derive(Clone, Debug)]
struct Cand {
    lat_deg: f64,
    lon_deg: f64,
    /// Radius above the mean sphere (terrain height + mast), for display/provenance.
    height_m: f64,
    mcmf: Vec3,
}

/// One served-user grid point.
#[derive(Clone, Debug)]
struct Usr {
    lat_deg: f64,
    lon_deg: f64,
    mcmf: Vec3,
}

/// The evaluation of one candidate configuration (a size-`k` subset).
#[derive(Clone, Debug)]
struct ConfigEval {
    /// Candidate indices in the subset (ascending).
    sites: Vec<usize>,
    n_served: usize,
    served_fraction: f64,
    /// Median over served users of their median-over-epochs GDOP (Validated).
    gdop_median: f64,
    hdop_median: f64,
    on_front: bool,
}

/// All-user aggregate DOP for a beacon set (used for the headline before/after).
#[derive(Clone, Copy, Debug, Default)]
struct AllUserDop {
    gdop: Option<f64>,
    hdop: Option<f64>,
    vdop: Option<f64>,
    horizontal_m: Option<f64>,
}

/// Everything the pack derives, computed once and shared by JSON / figure / HTML.
struct PlacementModel {
    sigma_ure_m: f64,
    users: Vec<Usr>,
    cands: Vec<Cand>,
    k: usize,
    n_configs_total: usize,
    /// Every *evaluable* config (serves ≥1 user with defined medians).
    evals: Vec<ConfigEval>,
    /// Indices into `evals` on the Pareto front (ascending).
    front_idx: Vec<usize>,
    /// Knee point (index into `evals`), if defined.
    knee_eval: Option<usize>,
    /// Recommended config: index into `evals` (always a front member).
    recommended: usize,
    /// TOPSIS ranking of the front, as `evals` indices (best first).
    topsis_ranking: Vec<usize>,
    /// Greedy-add path: candidate indices chosen in order.
    greedy_sites: Vec<usize>,
    /// Whether greedy reached the brute-force/TOPSIS recommended siting.
    greedy_matches_optimum: bool,
    /// Headline before/after over ALL users (orbit-only vs recommended siting).
    before_all: AllUserDop,
    after_all: AllUserDop,
    /// Served-user-restricted before/after for the recommended config.
    rec_before_gdop: Option<f64>,
    rec_after_gdop: Option<f64>,
    rec_before_hdop: Option<f64>,
    rec_after_hdop: Option<f64>,
}

fn linspace(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    if n <= 1 {
        return vec![lo];
    }
    (0..n)
        .map(|i| lo + (hi - lo) * (i as f64) / ((n - 1) as f64))
        .collect()
}

impl BeaconPlacementScenario {
    pub const KIND: &'static str = "pro-beacon-placement";

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: Self::KIND,
            description: "Kshana Pro P2/L47 pack: optimize surface-beacon PLACEMENT over terrain. A served-user polar grid and a grid of candidate beacon sites on a DEM (kshana::realdata::lola_dem); a candidate serves a user only if it clears the airless horizon (kshana::lunar_beacon::beacon_visible) AND the line of sight stays above the real relief (terrain_los_clear) — so a crater rim can shadow a beacon the bare sphere would allow. Full size-k-subset enumeration (brute force) scores served-user median GDOP/HDOP (separately) via the Validated dop_with_beacons kernel plus served fraction; kshana::mcda pareto_front + topsis pick the recommended siting on the true Pareto front. Terrain-LOS masking and per-config DOP are Validated; the representative DEM tile and the mcda ranking are Modelled. An engineering study, not a certification.",
            required_fields: &[],
            optional_fields: &[
                "user_lat_min_deg",
                "user_lat_max_deg",
                "user_n_lat",
                "user_lon_min_deg",
                "user_lon_max_deg",
                "user_n_lon",
                "user_antenna_h_m",
                "cand_lat_min_deg",
                "cand_lat_max_deg",
                "cand_n_lat",
                "cand_lon_min_deg",
                "cand_lon_max_deg",
                "cand_n_lon",
                "cand_mast_h_m",
                "k_beacons",
                "max_configs",
                "n_fleet",
                "n_epochs",
                "epoch_step_s",
                "mask_deg",
                "terrain_steps",
                "clock_sync_m",
                "multipath_m",
                "survey_m",
                "w_gdop",
                "w_hdop",
                "w_served",
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

    fn build_users(&self, dem: &DemTile) -> Vec<Usr> {
        let mut v = Vec::new();
        for lat in linspace(
            self.user_lat_min_deg,
            self.user_lat_max_deg,
            self.user_n_lat,
        ) {
            for lon in linspace(
                self.user_lon_min_deg,
                self.user_lon_max_deg,
                self.user_n_lon,
            ) {
                v.push(Usr {
                    lat_deg: lat,
                    lon_deg: lon,
                    mcmf: terrain_site(dem, lat, lon, self.user_antenna_h_m),
                });
            }
        }
        v
    }

    fn build_cands(&self, dem: &DemTile) -> Vec<Cand> {
        let mut v = Vec::new();
        for lat in linspace(
            self.cand_lat_min_deg,
            self.cand_lat_max_deg,
            self.cand_n_lat,
        ) {
            for lon in linspace(
                self.cand_lon_min_deg,
                self.cand_lon_max_deg,
                self.cand_n_lon,
            ) {
                let height_m = dem.height_at(lat, lon) + self.cand_mast_h_m;
                v.push(Cand {
                    lat_deg: lat,
                    lon_deg: lon,
                    height_m,
                    mcmf: terrain_site(dem, lat, lon, self.cand_mast_h_m),
                });
            }
        }
        v
    }

    /// Served-geometry matrix over the DEM: `vis[u][c]` iff candidate `c` serves user `u`.
    fn visibility(&self, dem: &DemTile, users: &[Usr], cands: &[Cand]) -> Vec<Vec<bool>> {
        users
            .iter()
            .map(|u| {
                cands
                    .iter()
                    .map(|c| serves(dem, u.mcmf, c.mcmf, self.terrain_steps))
                    .collect()
            })
            .collect()
    }

    /// The beacon MCMF positions in `sites` that serve user `ui` (terrain + horizon).
    fn served_beacons(
        &self,
        ui: usize,
        sites: &[usize],
        vis: &[Vec<bool>],
        cands: &[Cand],
    ) -> Vec<Vec3> {
        sites
            .iter()
            .filter(|&&c| vis[ui][c])
            .map(|&c| cands[c].mcmf)
            .collect()
    }

    /// Score a config on the served users: median served-user GDOP/HDOP + served fraction.
    /// `None` if no user is served or the medians are undefined (not evaluable).
    fn score_config(
        &self,
        sites: &[usize],
        users: &[Usr],
        cands: &[Cand],
        vis: &[Vec<bool>],
        sats: &[Vec<Vec3>],
    ) -> Option<ConfigEval> {
        let mut n_served = 0usize;
        let (mut gs, mut hs) = (Vec::new(), Vec::new());
        for (ui, u) in users.iter().enumerate() {
            let beacons = self.served_beacons(ui, sites, vis, cands);
            if beacons.is_empty() {
                continue;
            }
            n_served += 1;
            let d = user_dop_over_epochs(u.mcmf, sats, &beacons, self.mask_rad());
            if let Some(g) = d.gdop {
                gs.push(g);
            }
            if let Some(h) = d.hdop {
                hs.push(h);
            }
        }
        if n_served == 0 {
            return None;
        }
        let (gm, hm) = (median_of(gs), median_of(hs));
        match (gm, hm) {
            (Some(gdop_median), Some(hdop_median)) => Some(ConfigEval {
                sites: sites.to_vec(),
                n_served,
                served_fraction: n_served as f64 / users.len() as f64,
                gdop_median,
                hdop_median,
                on_front: false,
            }),
            _ => None,
        }
    }

    /// All-user aggregate DOP for a beacon set: each user uses its served beacons (empty
    /// = orbit-only). The honest headline metric — a user with no served beacon simply
    /// keeps its orbit-only geometry.
    fn all_user_dop(
        &self,
        sites: &[usize],
        users: &[Usr],
        cands: &[Cand],
        vis: &[Vec<bool>],
        sats: &[Vec<Vec3>],
        sigma_ure: f64,
    ) -> AllUserDop {
        let (mut gs, mut hs, mut vs) = (Vec::new(), Vec::new(), Vec::new());
        for (ui, u) in users.iter().enumerate() {
            let beacons = self.served_beacons(ui, sites, vis, cands);
            let d = user_dop_over_epochs(u.mcmf, sats, &beacons, self.mask_rad());
            if let Some(g) = d.gdop {
                gs.push(g);
            }
            if let Some(h) = d.hdop {
                hs.push(h);
            }
            if let Some(v) = d.vdop {
                vs.push(v);
            }
        }
        let hdop = median_of(hs);
        AllUserDop {
            gdop: median_of(gs),
            hdop,
            vdop: median_of(vs),
            horizontal_m: hdop.map(|h| realized_accuracy(&hdop_only(h), sigma_ure).horizontal_m),
        }
    }

    /// The objective vector of a config: `[median GDOP, median HDOP, served fraction]`,
    /// matched by `objectives()` below.
    fn objective_of(e: &ConfigEval) -> Vec<f64> {
        vec![e.gdop_median, e.hdop_median, e.served_fraction]
    }

    fn objectives() -> [Objective; 3] {
        // minimize GDOP, minimize HDOP, maximize served fraction.
        [Objective::Min, Objective::Min, Objective::Max]
    }

    /// Greedy add: repeatedly add the candidate that most reduces the all-user median
    /// GDOP, until `k` sites are chosen. Deterministic (ascending-index tie-break).
    fn greedy(
        &self,
        users: &[Usr],
        cands: &[Cand],
        vis: &[Vec<bool>],
        sats: &[Vec<Vec3>],
        sigma_ure: f64,
    ) -> Vec<usize> {
        let mut chosen: Vec<usize> = Vec::new();
        while chosen.len() < self.k_beacons && chosen.len() < cands.len() {
            let mut best: Option<(usize, f64)> = None;
            for c in 0..cands.len() {
                if chosen.contains(&c) {
                    continue;
                }
                let mut trial = chosen.clone();
                trial.push(c);
                trial.sort_unstable();
                let g = self
                    .all_user_dop(&trial, users, cands, vis, sats, sigma_ure)
                    .gdop
                    .unwrap_or(f64::INFINITY);
                let better = match best {
                    None => true,
                    Some((_, bg)) => g < bg - 1e-12,
                };
                if better {
                    best = Some((c, g));
                }
            }
            match best {
                Some((c, _)) => chosen.push(c),
                None => break,
            }
        }
        chosen.sort_unstable();
        chosen
    }

    fn compute(&self) -> Result<PlacementModel, KshanaError> {
        if self.n_epochs == 0 {
            return Err(KshanaError::InvalidInput(
                "n_epochs must be positive".into(),
            ));
        }
        let dem = DemTile::representative_south_polar();
        let users = self.build_users(&dem);
        let cands = self.build_cands(&dem);
        if users.is_empty() || cands.is_empty() {
            return Err(KshanaError::InvalidInput(
                "user and candidate grids must be non-empty".into(),
            ));
        }
        if self.k_beacons == 0 || self.k_beacons > cands.len() {
            return Err(KshanaError::InvalidInput(format!(
                "k_beacons must be in 1..={} (candidate count), got {}",
                cands.len(),
                self.k_beacons
            )));
        }
        let n_configs_total = binomial(cands.len(), self.k_beacons);
        if n_configs_total > self.max_configs {
            return Err(KshanaError::InvalidInput(format!(
                "C({}, {}) = {} exceeds max_configs {} — reduce the candidate grid or k, or raise max_configs",
                cands.len(),
                self.k_beacons,
                n_configs_total,
                self.max_configs
            )));
        }

        let sigma_ure = self.sigma_ure();
        let vis = self.visibility(&dem, &users, &cands);
        let sats = sats_per_epoch(self.n_fleet, self.n_epochs, self.epoch_step_s);

        // Full enumeration = brute force over every size-k subset.
        let mut evals: Vec<ConfigEval> = combinations(cands.len(), self.k_beacons)
            .into_iter()
            .filter_map(|sites| self.score_config(&sites, &users, &cands, &vis, &sats))
            .collect();
        if evals.is_empty() {
            return Err(KshanaError::InvalidInput(
                "no candidate configuration serves any user (widen the grids, raise the mast, or relax the mask)".into(),
            ));
        }

        // Pareto front + knee over the evaluable configs.
        let points: Vec<Vec<f64>> = evals.iter().map(Self::objective_of).collect();
        let objs = Self::objectives();
        let front_idx = pareto_front(&points, &objs);
        for &i in &front_idx {
            evals[i].on_front = true;
        }
        let knee_eval = knee_point(&points, &objs).map(|kp: KneePoint| kp.index);

        // Recommended config: TOPSIS over the front only ⇒ always a front member.
        let front_points: Vec<Vec<f64>> = front_idx.iter().map(|&i| points[i].clone()).collect();
        let weights = [self.w_gdop, self.w_hdop, self.w_served];
        let tr = topsis(&front_points, &weights, &objs)
            .map_err(|e| KshanaError::InvalidInput(format!("TOPSIS failed: {e}")))?;
        let winner_in_front = tr.winner().unwrap_or(0);
        let recommended = front_idx[winner_in_front];
        let topsis_ranking: Vec<usize> = tr.ranking.iter().map(|&r| front_idx[r]).collect();

        // Greedy alternative.
        let greedy_sites = self.greedy(&users, &cands, &vis, &sats, sigma_ure);
        let greedy_matches_optimum = greedy_sites == evals[recommended].sites;

        // Headline before/after over ALL users.
        let before_all = self.all_user_dop(&[], &users, &cands, &vis, &sats, sigma_ure);
        let after_all = self.all_user_dop(
            &evals[recommended].sites,
            &users,
            &cands,
            &vis,
            &sats,
            sigma_ure,
        );

        // Served-user-restricted before/after for the recommended config: the same users
        // it serves, orbit-only vs augmented. Adding ranging rows can only lower DOP, so
        // this is a clean, monotone improvement statement.
        let rec_sites = evals[recommended].sites.clone();
        let (mut bg, mut ag, mut bh, mut ah) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for (ui, u) in users.iter().enumerate() {
            let beacons = self.served_beacons(ui, &rec_sites, &vis, &cands);
            if beacons.is_empty() {
                continue;
            }
            let before = user_dop_over_epochs(u.mcmf, &sats, &[], self.mask_rad());
            let after = user_dop_over_epochs(u.mcmf, &sats, &beacons, self.mask_rad());
            if let Some(g) = before.gdop {
                bg.push(g);
            }
            if let Some(g) = after.gdop {
                ag.push(g);
            }
            if let Some(h) = before.hdop {
                bh.push(h);
            }
            if let Some(h) = after.hdop {
                ah.push(h);
            }
        }

        Ok(PlacementModel {
            sigma_ure_m: sigma_ure,
            users,
            cands,
            k: self.k_beacons,
            n_configs_total,
            evals,
            front_idx,
            knee_eval,
            recommended,
            topsis_ranking,
            greedy_sites,
            greedy_matches_optimum,
            before_all,
            after_all,
            rec_before_gdop: median_of(bg),
            rec_after_gdop: median_of(ag),
            rec_before_hdop: median_of(bh),
            rec_after_hdop: median_of(ah),
        })
    }

    fn sites_json(&self, m: &PlacementModel, sites: &[usize]) -> Value {
        json!(sites
            .iter()
            .map(|&c| {
                let cd = &m.cands[c];
                json!({
                    "cand_index": c,
                    "lat_deg": cd.lat_deg,
                    "lon_deg": cd.lon_deg,
                    "height_m": cd.height_m,
                })
            })
            .collect::<Vec<_>>())
    }

    fn json(&self, m: &PlacementModel) -> Value {
        let rec = &m.evals[m.recommended];
        let front: Vec<Value> = m
            .front_idx
            .iter()
            .map(|&i| {
                let e = &m.evals[i];
                json!({
                    "sites": self.sites_json(m, &e.sites),
                    "n_served": e.n_served,
                    "served_fraction": tagged(json!(e.served_fraction), Tag::Validated),
                    "gdop_median": tagged(json!(e.gdop_median), Tag::Validated),
                    "hdop_median": tagged(json!(e.hdop_median), Tag::Validated),
                    "recommended": i == m.recommended,
                    "knee": Some(i) == m.knee_eval,
                })
            })
            .collect();

        let ba = |a: &AllUserDop| {
            json!({
                "gdop_median": tagged(onum(a.gdop), Tag::Validated),
                "hdop_median": tagged(onum(a.hdop), Tag::Validated),
                "vdop_median": tagged(onum(a.vdop), Tag::Validated),
                "horizontal_1sigma_m": tagged(onum(a.horizontal_m), Tag::Validated),
            })
        };

        json!({
            "dem": {
                "source": tagged(json!("representative_south_polar"), Tag::Modelled),
                "note": "representative synthetic polar relief (raised rim + basin + massif) — NOT an actual LOLA PDS product",
                "lat_deg": [-90.0, -80.0],
                "lon_deg": [-180.0, 180.0],
            },
            "sigma_ure_m": tagged(json!(m.sigma_ure_m), Tag::Modelled),
            "grids": {
                "n_users": m.users.len(),
                "n_candidates": m.cands.len(),
                "k_beacons": m.k,
                "n_configs_enumerated": m.n_configs_total,
                "n_configs_evaluable": m.evals.len(),
            },
            "recommended_siting": {
                "sites": self.sites_json(m, &rec.sites),
                "served_fraction": tagged(json!(rec.served_fraction), Tag::Validated),
                "served_user_gdop_median": tagged(json!(rec.gdop_median), Tag::Validated),
                "served_user_hdop_median": tagged(json!(rec.hdop_median), Tag::Validated),
                "on_pareto_front": tagged(json!(rec.on_front), Tag::Validated),
                "selection": tagged(json!("TOPSIS over the Pareto front"), Tag::Modelled),
                "weights": { "gdop": self.w_gdop, "hdop": self.w_hdop, "served": self.w_served },
            },
            "served_user_before_after": {
                "note": "the recommended config's served users, orbit-only vs augmented; adding ranging rows is monotone (DOP cannot increase)",
                "gdop_before": tagged(onum(m.rec_before_gdop), Tag::Validated),
                "gdop_after": tagged(onum(m.rec_after_gdop), Tag::Validated),
                "hdop_before": tagged(onum(m.rec_before_hdop), Tag::Validated),
                "hdop_after": tagged(onum(m.rec_after_hdop), Tag::Validated),
            },
            "all_user_before_after": {
                "orbit_only": ba(&m.before_all),
                "with_recommended_beacons": ba(&m.after_all),
            },
            "pareto_front": {
                "n_front": m.front_idx.len(),
                "objectives": ["min median GDOP", "min median HDOP", "max served fraction"],
                "knee_eval_index": m.knee_eval,
                "members": front,
            },
            "topsis_ranking_eval_indices": m.topsis_ranking,
            "greedy": {
                "sites": self.sites_json(m, &m.greedy_sites),
                "matches_optimum": tagged(json!(m.greedy_matches_optimum), Tag::Validated),
                "note": "greedy add minimizing all-user median GDOP — a cheaper heuristic; may not equal the enumerated optimum (Modelled heuristic)",
            },
            "provenance_legend": {
                "Validated": "terrain-LOS + horizon masking (which candidate serves which user) is closed-form geometry; per-config DOP inherits the cross-checked kshana::orbit::dop kernel; Pareto dominance/front is exact; the recommended config is on the true front and agrees with an independent brute-force enumeration. A computational check, NOT flight validation or certification.",
                "Modelled": "the representative DEM tile (not real LOLA), the TOPSIS ranking (a preference model), and every scenario input (grids, constellation, k, error budget, weights).",
            },
        })
    }
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
/// A [`Dop`](kshana::orbit::Dop) carrying only HDOP, so `realized_accuracy(..).horizontal_m`
/// is `HDOP · σ_URE` (the other components are irrelevant to the horizontal read-out).
fn hdop_only(hdop: f64) -> kshana::orbit::Dop {
    kshana::orbit::Dop {
        gdop: hdop,
        pdop: hdop,
        hdop,
        vdop: hdop,
        tdop: hdop,
    }
}

impl Scenario for BeaconPlacementScenario {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let m = self.compute()?;
        let fig = placement_svg(self, &m);
        let report_html = placement_report_html(self, &m, &fig);
        let model_json = self.json(&m);

        let doc = json!({
            "kind": Self::KIND,
            "overlay": "kshana-pro",
            "overlay_version": crate::version(),
            "paper": "P2 — Surface-beacon PLACEMENT optimization over terrain (L47)",
            "model": model_json,
            "figure": { "dem_siting_svg": fig },
            "report_html": report_html,
        })
        .to_string();

        let rec = &m.evals[m.recommended];
        let summary = format!(
            "Kshana Pro P2 beacon PLACEMENT (L47) | optimized {} of {} candidate sites (k={}), {} configs enumerated | \
             recommended siting serves {:.0}% of users, median served GDOP {:.2}, HDOP {:.2} [Validated] | \
             served-user GDOP {} → {} with beacons [Validated] | on true Pareto front [Validated] | \
             DEM representative (not LOLA) + TOPSIS ranking [Modelled]",
            rec.sites.len(),
            m.cands.len(),
            m.k,
            m.n_configs_total,
            rec.served_fraction * 100.0,
            rec.gdop_median,
            rec.hdop_median,
            fmt_opt(m.rec_before_gdop, 2),
            fmt_opt(m.rec_after_gdop, 2),
        );

        Ok(RunOutput {
            json: doc,
            svg: fig,
            summary,
            csv: None,
        })
    }
}

impl ExternalPack for BeaconPlacementScenario {
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

// ── Figure: DEM tile with candidate + chosen beacons and users ───────────────

fn placement_svg(_scn: &BeaconPlacementScenario, m: &PlacementModel) -> String {
    let (w, h) = (680.0_f64, 420.0_f64);
    let (ml, mr, mt, mb) = (54.0_f64, 150.0_f64, 62.0_f64, 46.0_f64);
    // Map lon∈[-180,180] → x, lat∈[-90,-80] → y (north up, so -80 at the top).
    let px = |lon: f64| ml + (w - ml - mr) * (lon + 180.0) / 360.0;
    let py = |lat: f64| mt + (h - mt - mb) * (lat - (-80.0)) / (-90.0 - (-80.0));

    // Background DEM heat: sample the representative tile on a coarse grid.
    let dem = DemTile::representative_south_polar();
    let (nx, ny) = (48usize, 20usize);
    let mut hmin = f64::INFINITY;
    let mut hmax = f64::NEG_INFINITY;
    let mut cells = Vec::with_capacity(nx * ny);
    for iy in 0..ny {
        let lat = -80.0 - 10.0 * (iy as f64 + 0.5) / ny as f64;
        for ix in 0..nx {
            let lon = -180.0 + 360.0 * (ix as f64 + 0.5) / nx as f64;
            let z = dem.height_at(lat, lon);
            hmin = hmin.min(z);
            hmax = hmax.max(z);
            cells.push((lon, lat, z));
        }
    }
    let span = (hmax - hmin).max(1e-6);

    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" font-family=\"{FONT_BODY}\" font-size=\"11\">"
    );
    s += &format!("<rect width=\"100%\" height=\"100%\" fill=\"{NAVY}\"/>");
    s += &format!("<text x=\"20\" y=\"26\" fill=\"{BRASS}\" font-family=\"{FONT_HEAD}\" font-size=\"17\" font-weight=\"bold\">Beacon placement over terrain</text>");
    s += &format!("<text x=\"20\" y=\"44\" fill=\"{IVORY}\">DEM relief · candidate + chosen beacons · served users <tspan fill=\"#2dd4bf\">[terrain-LOS Validated]</tspan> <tspan fill=\"{BRASS}\">[DEM Modelled]</tspan></text>");

    let cw = (w - ml - mr) / nx as f64;
    let ch = (h - mt - mb) / ny as f64;
    for (lon, lat, z) in &cells {
        let t = (z - hmin) / span;
        s += &format!(
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"{}\"/>",
            px(*lon) - cw / 2.0,
            py(*lat) - ch / 2.0,
            cw + 0.6,
            ch + 0.6,
            relief(t)
        );
    }
    // Frame.
    s += &format!(
        "<rect x=\"{ml}\" y=\"{mt}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"none\" stroke=\"{BRASS_MUTED}\"/>",
        w - ml - mr,
        h - mt - mb
    );

    // Users (small ivory squares).
    for u in &m.users {
        s += &format!(
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"7\" height=\"7\" fill=\"{IVORY}\" opacity=\"0.9\"/>",
            px(u.lon_deg) - 3.5,
            py(u.lat_deg) - 3.5
        );
    }
    // Candidate sites (hollow brass rings); recommended (filled teal); knee (brass ring).
    let rec_sites = &m.evals[m.recommended].sites;
    for (ci, c) in m.cands.iter().enumerate() {
        let (x, y) = (px(c.lon_deg), py(c.lat_deg));
        if rec_sites.contains(&ci) {
            s += &format!("<circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"7\" fill=\"#2dd4bf\" stroke=\"{INK}\" stroke-width=\"1\"/>");
        } else {
            s += &format!("<circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"5\" fill=\"none\" stroke=\"{BRASS}\" stroke-width=\"1.6\"/>");
        }
    }

    // Axes labels.
    s += &format!(
        "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" text-anchor=\"end\">−80°</text>",
        ml - 6.0,
        mt + 4.0
    );
    s += &format!(
        "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" text-anchor=\"end\">−90°</text>",
        ml - 6.0,
        h - mb
    );
    s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{BRASS}\" text-anchor=\"middle\">longitude −180°…+180°</text>", ml + (w - ml - mr) / 2.0, h - 8.0);

    // Legend.
    let lx = w - mr + 12.0;
    s += &format!(
        "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"7\" fill=\"#2dd4bf\"/>",
        lx + 6.0,
        mt + 12.0
    );
    s += &format!(
        "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" font-size=\"10\">chosen beacon</text>",
        lx + 18.0,
        mt + 16.0
    );
    s += &format!("<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"5\" fill=\"none\" stroke=\"{BRASS}\" stroke-width=\"1.6\"/>", lx + 6.0, mt + 34.0);
    s += &format!(
        "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" font-size=\"10\">candidate</text>",
        lx + 18.0,
        mt + 38.0
    );
    s += &format!(
        "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"7\" height=\"7\" fill=\"{IVORY}\"/>",
        lx + 3.0,
        mt + 52.0
    );
    s += &format!(
        "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" font-size=\"10\">served user</text>",
        lx + 18.0,
        mt + 60.0
    );
    s += &format!("<text x=\"{lx:.1}\" y=\"{:.1}\" fill=\"{BRASS_MUTED}\" font-size=\"9\">dark→light = low→high relief</text>", mt + 84.0);

    s += "</svg>";
    s
}

/// Relief colour ramp: dark navy (low) → brass (high).
fn relief(t: f64) -> String {
    let t = t.clamp(0.0, 1.0);
    let (r0, g0, b0) = (0x12, 0x1b, 0x2a);
    let (r1, g1, b1) = (0xc7, 0xaa, 0x66);
    let lerp = |a: i32, b: i32| (a as f64 + (b as f64 - a as f64) * t).round() as i32;
    format!(
        "#{:02x}{:02x}{:02x}",
        lerp(r0, r1),
        lerp(g0, g1),
        lerp(b0, b1)
    )
}

// ── Branded HTML report ──────────────────────────────────────────────────────

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
fn sites_label(m: &PlacementModel, sites: &[usize]) -> String {
    sites
        .iter()
        .map(|&c| {
            let cd = &m.cands[c];
            format!(
                "({:.1}°, {:.0}°, {:.0} m)",
                cd.lat_deg, cd.lon_deg, cd.height_m
            )
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

fn placement_report_html(_scn: &BeaconPlacementScenario, m: &PlacementModel, fig: &str) -> String {
    let rec = &m.evals[m.recommended];

    let front_rows: String = m
        .front_idx
        .iter()
        .map(|&i| {
            let e = &m.evals[i];
            let flag = if i == m.recommended {
                " ★ recommended"
            } else if Some(i) == m.knee_eval {
                " ◇ knee"
            } else {
                ""
            };
            format!(
                "<tr>{}{}{}{}</tr>",
                td(&format!("{}{}", sites_label(m, &e.sites), flag)),
                td(&format!("{:.2}", e.gdop_median)),
                td(&format!("{:.2}", e.hdop_median)),
                td(&format!("{:.0}%", e.served_fraction * 100.0)),
            )
        })
        .collect();

    let greedy_note = if m.greedy_matches_optimum {
        format!(
            "Greedy add reached the same siting {}.",
            badge(Tag::Validated)
        )
    } else {
        format!(
            "Greedy add ({}) did <em>not</em> reach the enumerated optimum — a cheaper heuristic {}.",
            sites_label(m, &m.greedy_sites),
            badge(Tag::Modelled)
        )
    };

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <title>Kshana Pro — P2 Beacon Placement over Terrain (L47)</title></head>\
         <body style=\"margin:0;background:{NAVY};color:{IVORY};font-family:{FONT_BODY};\
         max-width:940px;margin:0 auto;padding:0 28px 60px\">\
         {cover}{banner}\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Recommended siting — {k} of {ncand} candidate sites</h2>\
         <p>Of <strong>{ncfg}</strong> enumerated size-{k} configurations, TOPSIS on the Pareto front selects \
         <strong>{rec_sites}</strong> {bv}. It serves <strong>{sf:.0}%</strong> of the user grid with a median \
         served-user GDOP of <strong>{g:.2}</strong> and HDOP <strong>{hh:.2}</strong> (reported separately) {bv}. \
         Over the served users the augmented GDOP falls {gb} → {ga} and HDOP {hb} → {ha} {bv} — adding ranging rows \
         is monotone, so beacons never worsen a user's geometry. σ_URE {sig:.2} m ⇒ served-user horizontal 1σ \
         {hbm} → {ham} m.</p>\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Terrain siting map</h2>{fig}\
         <p style=\"font-size:12px;opacity:.85\">A candidate serves a user only when it clears the airless horizon \
         <em>and</em> the line of sight stays above the DEM relief {bv}; the representative tile itself is {mv}.</p>\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Pareto front — the siting trade-off</h2>\
         <p>{nfront} of {neval} evaluable configurations are non-dominated over {{min GDOP, min HDOP, max served}}. \
         The recommended config is on the front by construction {bv}; {greedy_note}</p>\
         <table style=\"border-collapse:collapse;width:100%\"><tr>{h_s}{h_g}{h_h}{h_sf}</tr>{front_rows}</table>\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">All-user headline (orbit-only vs recommended beacons)</h2>\
         <table style=\"border-collapse:collapse;width:100%\">\
         <tr>{h_case}{h_g}{h_h}{h_v}{h_acc}</tr>\
         <tr>{c_o}{go}{ho}{vo}{ao}</tr>\
         <tr>{c_b}{gbn}{hbn}{vbn}{abn}</tr></table>\
         <p style=\"font-size:12px;opacity:.8\">DEM: representative south-polar relief {mv} (raised rim + basin + massif); \
         replace with a parsed LOLA tile for operational siting. GDOP/HDOP/served-fraction inherit the Validated kernel and \
         closed-form terrain geometry {bv}; the single recommended config is a TOPSIS preference {mv}.</p>\
         </body></html>",
        cover = cover_html(
            "Surface-Beacon Placement over Terrain",
            "P2 / L47 — siting as the optimization variable"
        ),
        banner = modelled_banner_html(
            "the terrain-LOS masking and per-config DOP inherit the cross-checked open kernel and closed-form geometry; \
             the representative DEM tile and the TOPSIS ranking are Modelled."
        ),
        k = m.k,
        ncand = m.cands.len(),
        ncfg = m.n_configs_total,
        rec_sites = sites_label(m, &rec.sites),
        sf = rec.served_fraction * 100.0,
        g = rec.gdop_median,
        hh = rec.hdop_median,
        gb = fmt_opt(m.rec_before_gdop, 2),
        ga = fmt_opt(m.rec_after_gdop, 2),
        hb = fmt_opt(m.rec_before_hdop, 2),
        ha = fmt_opt(m.rec_after_hdop, 2),
        sig = m.sigma_ure_m,
        hbm = fmt_opt(m.rec_before_hdop.map(|h| h * m.sigma_ure_m), 1),
        ham = fmt_opt(m.rec_after_hdop.map(|h| h * m.sigma_ure_m), 1),
        bv = badge(Tag::Validated),
        mv = badge(Tag::Modelled),
        fig = fig,
        nfront = m.front_idx.len(),
        neval = m.evals.len(),
        greedy_note = greedy_note,
        h_s = th("Siting (lat, lon, height)"),
        h_g = th("GDOP"),
        h_h = th("HDOP"),
        h_sf = th("Served"),
        front_rows = front_rows,
        h_case = th("Case"),
        h_v = th("VDOP"),
        h_acc = th("Horiz σ (m)"),
        c_o = td("orbit-only"),
        go = td(&fmt_opt(m.before_all.gdop, 2)),
        ho = td(&fmt_opt(m.before_all.hdop, 2)),
        vo = td(&fmt_opt(m.before_all.vdop, 2)),
        ao = td(&fmt_opt(m.before_all.horizontal_m, 1)),
        c_b = td("+ recommended beacons"),
        gbn = td(&fmt_opt(m.after_all.gdop, 2)),
        hbn = td(&fmt_opt(m.after_all.hdop, 2)),
        vbn = td(&fmt_opt(m.after_all.vdop, 2)),
        abn = td(&fmt_opt(m.after_all.horizontal_m, 1)),
    )
}

// ═════════════════════════════════════════════════════════════════════════════
// Tests
// ═════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use kshana::lunar::R_MOON_M;
    use kshana::mcda::pareto::dominates;

    fn run_default() -> (PlacementModel, Value) {
        let scn = BeaconPlacementScenario::default();
        let m = scn.compute().expect("compute");
        let out = scn.run().expect("run");
        let v: Value = serde_json::from_str(&out.json).unwrap();
        (m, v)
    }

    /// A raw surface MCMF site (no terrain), matching the kshana beacon convention.
    fn bare_site(lat_deg: f64, lon_deg: f64, height_m: f64) -> Vec3 {
        let lat = lat_deg.to_radians();
        let lon = lon_deg.to_radians();
        let r = R_MOON_M + height_m;
        [
            r * lat.cos() * lon.cos(),
            r * lat.cos() * lon.sin(),
            r * lat.sin(),
        ]
    }

    // ── Validated: terrain masking (closed-form geometry) ────────────────────

    #[test]
    fn beacon_behind_a_rim_does_not_serve_but_serves_when_rim_removed() {
        // Two surface points ~6 km apart at 2 m, with a 3000 m wall at the midpoint.
        let a = bare_site(-85.0, 0.0, 2.0);
        let b = bare_site(-85.0, 0.2, 2.0);
        let wall = DemTile {
            lat_min_deg: -86.0,
            lat_max_deg: -84.0,
            lon_min_deg: -0.1,
            lon_max_deg: 0.3,
            n_lat: 3,
            n_lon: 3,
            heights_m: vec![0.0, 3000.0, 0.0, 0.0, 3000.0, 0.0, 0.0, 3000.0, 0.0],
        };
        // The pair is within the airless horizon (masking is purely the terrain).
        assert!(beacon_visible(a, b), "the pair must be within the horizon");
        assert!(
            !serves(&wall, a, b, 64),
            "a beacon behind the 3 km rim must NOT serve the shadowed user"
        );
        let flat = DemTile {
            heights_m: vec![0.0; 9],
            ..wall.clone()
        };
        assert!(
            serves(&flat, a, b, 64),
            "removing the rim must let the same beacon serve the user"
        );
    }

    #[test]
    fn representative_rim_shadows_a_within_horizon_pair() {
        // On the REAL representative tile: two points on opposite flanks of the polar
        // rim (colat 4 and colat 6, both ~2000 m) are within the horizon, but the
        // 2500 m rim crest at colat 5 sits between them and blocks the LOS.
        let dem = DemTile::representative_south_polar();
        let u = terrain_site(&dem, -86.0, 0.0, 2.0); // colat 4 flank
        let c = terrain_site(&dem, -84.0, 0.0, 2.0); // colat 6 flank
        assert!(
            beacon_visible(u, c),
            "flank pair is within the airless horizon"
        );
        assert!(
            !serves(&dem, u, c, 64),
            "the rim crest must shadow the cross-rim pair on the representative tile"
        );
        // Flatten the tile: the same pair is then served (relief was the only blocker).
        let flat = DemTile {
            heights_m: vec![0.0; dem.heights_m.len()],
            ..dem.clone()
        };
        assert!(
            serves(&flat, u, c, 64),
            "a flat tile clears the same cross-rim pair"
        );
    }

    #[test]
    fn removing_relief_only_adds_lines_of_sight() {
        // Monotonicity of the terrain-LOS masking, with the SAME site positions held
        // fixed: relaxing the occlusion surface (real relief → flat) can only ADD lines
        // of sight, never remove them. (Rebuilding the sites on a flat tile would ALSO
        // lower each beacon's vantage/horizon, which is a different effect — here we
        // isolate the pure terrain-LOS claim.)
        let scn = BeaconPlacementScenario::default();
        let dem = DemTile::representative_south_polar();
        let users = scn.build_users(&dem);
        let cands = scn.build_cands(&dem);
        // Same (real-relief) site positions; only the DEM used for the LOS check changes.
        let flat = DemTile {
            heights_m: vec![0.0; dem.heights_m.len()],
            ..dem.clone()
        };
        let real = scn.visibility(&dem, &users, &cands);
        let flatv = scn.visibility(&flat, &users, &cands);
        let real_count: usize = real.iter().flatten().filter(|&&b| b).count();
        let flat_count: usize = flatv.iter().flatten().filter(|&&b| b).count();
        assert!(
            flat_count >= real_count,
            "relaxing relief must not remove LOS: flat {flat_count} < real {real_count}"
        );
        // Every real-relief LOS is still clear on the flat tile (pairwise monotone).
        for (rr, ff) in real.iter().flatten().zip(flatv.iter().flatten()) {
            assert!(
                !(*rr) || *ff,
                "a real-relief LOS must remain clear when relief is removed"
            );
        }
        // And relief strictly blocks at least one otherwise-clear pair here.
        assert!(
            flat_count > real_count,
            "the representative rim must block ≥1 pair"
        );
    }

    // ── Validated: optimizer agrees with brute force ─────────────────────────

    /// Independent brute-force argmin of median served-user GDOP over ALL size-k
    /// subsets — the reference the mcda pipeline must agree with. Returns the winning
    /// site set (sorted).
    fn brute_force_argmin_gdop(scn: &BeaconPlacementScenario) -> Vec<usize> {
        let dem = DemTile::representative_south_polar();
        let users = scn.build_users(&dem);
        let cands = scn.build_cands(&dem);
        let vis = scn.visibility(&dem, &users, &cands);
        let sats = sats_per_epoch(scn.n_fleet, scn.n_epochs, scn.epoch_step_s);
        let mut best: Option<(Vec<usize>, f64)> = None;
        for sites in combinations(cands.len(), scn.k_beacons) {
            if let Some(e) = scn.score_config(&sites, &users, &cands, &vis, &sats) {
                let better = match &best {
                    None => true,
                    Some((_, bg)) => e.gdop_median < bg - 1e-12,
                };
                if better {
                    best = Some((sites, e.gdop_median));
                }
            }
        }
        best.expect("at least one evaluable config").0
    }

    #[test]
    fn recommended_config_is_on_the_true_pareto_front() {
        let (m, v) = run_default();
        let rec = &m.evals[m.recommended];
        assert!(
            rec.on_front,
            "recommended config must be on the Pareto front"
        );
        assert!(m.front_idx.contains(&m.recommended));
        assert_eq!(
            v["model"]["recommended_siting"]["on_pareto_front"]["value"],
            true
        );
    }

    #[test]
    fn pareto_front_matches_independent_dominance_recomputation() {
        let (m, _) = run_default();
        let points: Vec<Vec<f64>> = m
            .evals
            .iter()
            .map(BeaconPlacementScenario::objective_of)
            .collect();
        let objs = BeaconPlacementScenario::objectives();
        // Every front member is non-dominated; every non-front member IS dominated.
        for (i, e) in m.evals.iter().enumerate() {
            let dominated =
                (0..m.evals.len()).any(|j| j != i && dominates(&points[j], &points[i], &objs));
            assert_eq!(
                e.on_front, !dominated,
                "front membership must equal non-domination for eval {i}"
            );
        }
    }

    #[test]
    fn topsis_gdop_only_weight_matches_brute_force_argmin() {
        // Oracle: a single-objective (GDOP-only) TOPSIS weighting must select the exact
        // config an independent brute-force enumeration finds as the min-median-GDOP
        // optimum. Uses a SMALL controlled case (few candidates, small k) with a unique
        // optimum so the comparison is unambiguous.
        let scn = BeaconPlacementScenario {
            cand_n_lat: 1,
            cand_n_lon: 4,
            cand_lat_min_deg: -84.5,
            cand_lat_max_deg: -84.5,
            k_beacons: 2,
            user_n_lat: 1,
            user_n_lon: 4,
            user_lat_min_deg: -85.0,
            user_lat_max_deg: -85.0,
            w_gdop: 1.0,
            w_hdop: 0.0,
            w_served: 0.0,
            ..Default::default()
        };
        let m = scn.compute().expect("small-case compute");
        let brute = brute_force_argmin_gdop(&scn);
        assert_eq!(
            m.evals[m.recommended].sites, brute,
            "GDOP-only TOPSIS pick {:?} must equal brute-force argmin {:?}",
            m.evals[m.recommended].sites, brute
        );
        // And the recommended is on the front (a min-GDOP point cannot be dominated on GDOP).
        assert!(m.evals[m.recommended].on_front);
    }

    // ── Placement behaviour ──────────────────────────────────────────────────

    #[test]
    fn beacons_help_served_users_monotonically() {
        let (m, _) = run_default();
        let before = m.rec_before_gdop.expect("served-user orbit-only GDOP");
        let after = m.rec_after_gdop.expect("served-user beacon GDOP");
        assert!(before.is_finite() && after.is_finite());
        // Adding ranging rows can only lower DOP (never raise it).
        assert!(
            after <= before + 1e-9,
            "beacons must not worsen served GDOP: {before} -> {after}"
        );
        // And on this polar geometry they strictly help.
        assert!(
            after < before,
            "beacons should strictly cut served-user GDOP: {before} -> {after}"
        );
    }

    #[test]
    fn served_fraction_in_unit_interval_and_positive() {
        let (m, _) = run_default();
        let sf = m.evals[m.recommended].served_fraction;
        assert!(sf > 0.0 && sf <= 1.0, "served fraction {sf}");
        for e in &m.evals {
            assert!(e.served_fraction > 0.0 && e.served_fraction <= 1.0);
            assert!(e.gdop_median.is_finite() && e.hdop_median.is_finite());
        }
    }

    #[test]
    fn hdop_reported_separately_from_gdop() {
        let (m, v) = run_default();
        let rec = &m.evals[m.recommended];
        assert!(rec.hdop_median.is_finite() && rec.hdop_median > 0.0);
        let node = &v["model"]["recommended_siting"];
        assert!(node["served_user_gdop_median"]["value"].is_number());
        assert!(node["served_user_hdop_median"]["value"].is_number());
    }

    #[test]
    fn all_user_headline_beacons_do_not_worsen_gdop() {
        let (m, _) = run_default();
        let before = m.before_all.gdop.expect("orbit-only all-user GDOP");
        let after = m.after_all.gdop.expect("beacon all-user GDOP");
        assert!(after <= before + 1e-9, "all-user GDOP: {before} -> {after}");
        // Realized horizontal accuracy is HDOP × σ_URE.
        let hd = m.after_all.hdop.unwrap();
        let horiz = m.after_all.horizontal_m.unwrap();
        assert!((horiz - hd * m.sigma_ure_m).abs() < 1e-9);
    }

    // ── Determinism & contract ───────────────────────────────────────────────

    #[test]
    fn output_is_deterministic() {
        let scn = BeaconPlacementScenario::default();
        let a = scn.run().unwrap();
        let b = scn.run().unwrap();
        assert_eq!(a.json, b.json);
        assert_eq!(a.svg, b.svg);
        assert_eq!(a.summary, b.summary);
    }

    #[test]
    fn envelope_and_contract() {
        let (_, v) = run_default();
        let scn = BeaconPlacementScenario::default();
        assert_eq!(scn.kind_name(), "pro-beacon-placement");
        assert_eq!(scn.meta().name, "pro-beacon-placement");
        assert_eq!(v["kind"], "pro-beacon-placement");
        let out = scn.run().unwrap();
        assert!(out.svg.contains("<svg"));
        assert!(v["report_html"]
            .as_str()
            .unwrap()
            .contains("<!doctype html>"));
        assert!(out.summary.contains("Validated"));
        assert!(out.summary.contains("PLACEMENT"));
    }

    #[test]
    fn empty_toml_reproduces_baseline() {
        let scn: BeaconPlacementScenario = toml::from_str("").unwrap();
        let m = scn.compute().unwrap();
        assert!(m.evals[m.recommended].on_front);
        assert!(m.rec_after_gdop.unwrap() <= m.rec_before_gdop.unwrap() + 1e-9);
    }

    #[test]
    fn combinations_are_lexicographic_and_complete() {
        assert_eq!(
            combinations(4, 2),
            vec![
                vec![0, 1],
                vec![0, 2],
                vec![0, 3],
                vec![1, 2],
                vec![1, 3],
                vec![2, 3]
            ]
        );
        assert_eq!(binomial(8, 3), 56);
        assert_eq!(binomial(4, 2), 6);
        assert_eq!(combinations(4, 2).len(), binomial(4, 2));
    }

    #[test]
    fn config_count_guard_rejects_a_blowup() {
        let scn = BeaconPlacementScenario {
            max_configs: 5,
            cand_n_lat: 2,
            cand_n_lon: 4, // 8 candidates, k=3 -> 56 > 5
            k_beacons: 3,
            ..Default::default()
        };
        assert!(scn.compute().is_err());
    }
}
