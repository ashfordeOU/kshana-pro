// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # UTM / U-space positioning-integrity services (research backlog E18, B1804)
//!
//! Four products for drone operators, U-space service providers (USSPs) and the authorities
//! that review their risk assessments, built on the open engine's interference maps
//! (`kshana-interference-map/v1`, read with `kshana::interference_map::route`):
//!
//! * [`sora`]: a SORA-ready GNSS-interference input for one operation: the route's exposure on
//!   every day map supplied, how often a day was affected, the longest stretch flown through
//!   degraded cells, the dead-reckoning error over that stretch for the UAS class, and the
//!   containment margin against the contingency buffer; filled into a Markdown template
//!   (`sora-input.md`) for the operator's own assessment (`kshana-pro utm sora`).
//! * [`feed`]: a USSP integrity feed: per map cell in an area, the latest state, how many of
//!   the recent days were degraded, an advisory class and a validity window, as JSON Lines
//!   (`utm feed`, Enterprise).
//! * [`rid_check`]: Remote ID position-authenticity checks from broadcast records alone:
//!   reported speed against the speed between reports, implied speed and climb limits, time
//!   order, one identity in two places, operator distance. Nothing here locates an emitter.
//! * [`attest`]: a per-flight positioning-integrity attestation from the flight's own
//!   telemetry (fix gaps, receiver flags, jumps, dead-reckoning error over the longest gap),
//!   the day's map exposure and, when given, the Remote ID check; written as a signed directory
//!   that `kshana-pro dossier verify` or `kshana-pro utm check` verifies offline.
//!
//! Every result is labelled MODELLED and is **decision support** ([`DECISION_SUPPORT`]): never
//! a flight or operational authorisation, a SORA approval or a means of compliance.

pub mod synth;

use std::path::Path;

use kshana::interference_map::grid::{haversine_m, CellId, Grid};
use kshana::interference_map::route::{exposure, load_map};
use serde::{Deserialize, Serialize};

use crate::aviation::{rnd, utc, wilson95, Inputs};

/// `utm sora`, `utm rid-check` and `utm attest` (Pro).
pub const FEATURE: &str = "pro-utm-integrity";
/// `utm feed` (Enterprise: a service feed).
pub const FEATURE_FEED: &str = "pro-ussp-feed";
/// Evidence class of every result.
pub const EVIDENCE: &str = "MODELLED";

/// What every result says about itself.
pub const DECISION_SUPPORT: &str = "Decision support only. Not a flight authorisation, not an \
operational authorisation or a SORA approval, not a means of compliance and not a substitute for \
the operator's assessment, the USSP's procedures or the authority's review. Interference maps show \
where aircraft reported degraded navigation, never a source position.";

// ---------------------------------------------------------------------------------------------
// Request
// ---------------------------------------------------------------------------------------------

/// Dead-reckoning error terms of the UAS (velocity-aided dead reckoning:
/// `σ(t)² = (ψ·d)² + (s·d)² + (w·t)² + 2·σ_v²·τ·t`, `d = v·t`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeadReckoning {
    /// Heading error ψ, degrees.
    pub heading_err_deg: f64,
    /// Scale error s, fraction.
    pub scale_err: f64,
    /// Unknown constant velocity error w (wind estimate), m/s.
    pub velocity_bias_m_s: f64,
    /// Velocity noise σ_v, m/s.
    pub velocity_noise_m_s: f64,
    /// Velocity noise correlation time τ, s.
    pub noise_corr_s: f64,
}

impl DeadReckoning {
    /// Class bands (not a product datasheet): `multirotor` (optical flow), `fixed-wing` and
    /// `vtol` (airspeed and heading).
    pub fn for_class(class: &str) -> Result<Self, String> {
        let d = |psi, s, w, sv, tau| DeadReckoning {
            heading_err_deg: psi,
            scale_err: s,
            velocity_bias_m_s: w,
            velocity_noise_m_s: sv,
            noise_corr_s: tau,
        };
        match class {
            "multirotor" => Ok(d(3.0, 0.03, 0.0, 0.1, 1.0)),
            "fixed-wing" | "vtol" => Ok(d(2.0, 0.05, 1.0, 0.5, 10.0)),
            other => Err(format!(
                "unknown UAS class '{other}' (multirotor, fixed-wing, vtol; or give [operation.dead_reckoning])"
            )),
        }
    }

    /// The error after coasting `t` seconds at `speed` m/s, metres.
    pub fn error_m(&self, speed: f64, t: f64) -> f64 {
        let d = speed * t;
        let psi = self.heading_err_deg.to_radians();
        ((psi * d).powi(2)
            + (self.scale_err * d).powi(2)
            + (self.velocity_bias_m_s * t).powi(2)
            + 2.0 * self.velocity_noise_m_s.powi(2) * self.noise_corr_s * t)
            .sqrt()
    }
}

/// The operation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Operation {
    /// Operation identifier.
    pub id: String,
    /// UAS class (`multirotor`, `fixed-wing`, `vtol`).
    pub uas_class: String,
    /// Planned ground speed, m/s.
    pub speed_m_s: f64,
    /// Planned height above ground, metres.
    #[serde(default)]
    pub height_agl_m: f64,
    /// Waypoints `[lat, lon]`.
    pub waypoints: Vec<[f64; 2]>,
    /// Contingency buffer around the flight geography, metres.
    pub contingency_m: f64,
    /// Ground-risk buffer, metres.
    #[serde(default)]
    pub ground_risk_buffer_m: f64,
    /// Override of the class dead-reckoning terms.
    #[serde(default)]
    pub dead_reckoning: Option<DeadReckoning>,
}

impl Operation {
    fn dr(&self) -> Result<DeadReckoning, String> {
        match &self.dead_reckoning {
            Some(d) => Ok(d.clone()),
            None => DeadReckoning::for_class(&self.uas_class),
        }
    }

    fn check(&self) -> Result<(), String> {
        let ok = self.waypoints.len() >= 2
            && self.waypoints.iter().all(|w| {
                w[0].is_finite() && w[1].is_finite() && w[0].abs() <= 90.0 && w[1].abs() <= 180.0
            })
            && self.speed_m_s.is_finite()
            && self.speed_m_s > 0.0
            && self.contingency_m.is_finite()
            && self.contingency_m >= 0.0;
        if ok {
            Ok(())
        } else {
            Err("operation needs two or more valid waypoints, a positive speed_m_s and a non-negative contingency_m".into())
        }
    }

    fn route(&self) -> Vec<(f64, f64)> {
        self.waypoints.iter().map(|w| (w[0], w[1])).collect()
    }
}

/// USSP feed parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FeedParams {
    /// Area `[south, west, north, east]`, degrees.
    pub area: [f64; 4],
    /// Hours the feed is valid after the end of the latest map day.
    #[serde(default = "default_valid_hours")]
    pub valid_hours: f64,
}

fn default_valid_hours() -> f64 {
    24.0
}

/// Remote ID check parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RidParams {
    /// Largest plausible ground speed, m/s.
    pub max_speed_m_s: f64,
    /// Largest plausible climb or descent rate, m/s.
    pub max_climb_m_s: f64,
    /// Reported-speed disagreement allowed: max(abs, rel × speed), m/s.
    pub speed_tol_abs_m_s: f64,
    /// Relative part of the speed tolerance.
    pub speed_tol_rel: f64,
    /// Operator distance above which a report is flagged, metres.
    pub max_operator_m: f64,
    /// Two reports of one identity closer in time than this but farther apart than
    /// `max_speed_m_s` allows are a clone, seconds.
    pub clone_window_s: f64,
}

impl Default for RidParams {
    fn default() -> Self {
        RidParams {
            max_speed_m_s: 50.0,
            max_climb_m_s: 20.0,
            speed_tol_abs_m_s: 5.0,
            speed_tol_rel: 0.5,
            max_operator_m: 15_000.0,
            clone_window_s: 1.0,
        }
    }
}

/// A `utm` request (TOML).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Request {
    /// Title.
    pub title: String,
    /// The operation.
    pub operation: Operation,
    /// Interference maps (`kshana-interference-map/v1` GeoJSON), one per day.
    #[serde(default)]
    pub maps: Vec<String>,
    /// Remote ID records (CSV), optional.
    #[serde(default)]
    pub rid: Option<String>,
    /// Flight telemetry (CSV), for `attest`.
    #[serde(default)]
    pub telemetry: Option<String>,
    /// Feed parameters, for `feed`.
    #[serde(default)]
    pub feed: Option<FeedParams>,
    /// Remote ID check parameters.
    #[serde(default)]
    pub rid_params: RidParams,
}

/// A request with its inputs read.
pub struct Loaded {
    /// The request.
    pub req: Request,
    /// Day maps: (file, text).
    pub maps: Vec<(String, String)>,
    /// Remote ID CSV text.
    pub rid: Option<String>,
    /// Telemetry CSV text.
    pub telemetry: Option<String>,
    /// Every input, for the result directory.
    pub inputs: Inputs,
}

/// Read a request file and the files it names (relative to the request).
pub fn load(path: &Path) -> Result<Loaded, String> {
    let src = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let req: Request = toml::from_str(&src).map_err(|e| format!("{}: {e}", path.display()))?;
    req.operation.check()?;
    let base = path.parent().unwrap_or(Path::new("."));
    let mut inputs = Inputs::default();
    inputs.add_text("request.toml", "request", &src);
    let mut maps = Vec::new();
    for m in &req.maps {
        let t = inputs.read(base, m, "interference-map")?;
        maps.push((m.clone(), t));
    }
    let rid = match &req.rid {
        Some(r) => Some(inputs.read(base, r, "remote-id")?),
        None => None,
    };
    let telemetry = match &req.telemetry {
        Some(r) => Some(inputs.read(base, r, "telemetry")?),
        None => None,
    };
    Ok(Loaded {
        req,
        maps,
        rid,
        telemetry,
        inputs,
    })
}

// ---------------------------------------------------------------------------------------------
// SORA input
// ---------------------------------------------------------------------------------------------

/// One day map's exposure of the route.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MapExposure {
    /// Map file.
    pub file: String,
    /// Map day.
    pub date: String,
    /// Share of route length in degraded cells.
    pub share_degraded: f64,
    /// Share in cells without a call.
    pub share_unassessed: f64,
    /// Share in cells not observed.
    pub share_not_observed: f64,
    /// Route length in degraded cells, metres.
    pub degraded_m: f64,
}

/// The SORA-ready interference input.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SoraInput {
    /// Format tag.
    pub format: &'static str,
    /// Evidence class.
    pub evidence: &'static str,
    /// Title.
    pub title: String,
    /// The operation.
    pub operation: Operation,
    /// Route length, km.
    pub route_km: f64,
    /// Planned flight time, s.
    pub flight_time_s: f64,
    /// Exposure per day map.
    pub maps: Vec<MapExposure>,
    /// Days with any degraded route length.
    pub days_affected: usize,
    /// Share of days affected with its Wilson 95 % interval.
    pub day_affected_rate: Option<[f64; 3]>,
    /// Mean share of route length not observed.
    pub mean_share_not_observed: f64,
    /// Longest route length in degraded cells on one day, metres.
    pub worst_degraded_m: f64,
    /// Time to fly it, s.
    pub worst_coast_s: f64,
    /// Dead-reckoning error after it, metres.
    pub worst_dr_error_m: f64,
    /// The dead-reckoning terms used.
    pub dead_reckoning: DeadReckoning,
    /// Contingency buffer minus the dead-reckoning error, metres.
    pub containment_margin_m: f64,
    /// The contingency buffer that would cover the error, metres.
    pub suggested_contingency_m: f64,
    /// `insufficient-data`, `none-observed`, `low`, `medium` or `high`.
    pub interference_class: &'static str,
    /// Decision-support statement.
    pub decision_support: &'static str,
}

/// Build the SORA input from loaded maps.
pub fn sora(req: &Request, maps: &[(String, String)]) -> Result<SoraInput, String> {
    req.operation.check()?;
    let route = req.operation.route();
    let route_m: f64 = route
        .windows(2)
        .map(|w| haversine_m(w[0].0, w[0].1, w[1].0, w[1].1))
        .sum();
    let mut rows = Vec::new();
    for (file, text) in maps {
        let m = load_map(text).map_err(|e| format!("{file}: {e}"))?;
        let e = exposure(&route, &m);
        rows.push(MapExposure {
            file: file.clone(),
            date: m.date.clone(),
            share_degraded: rnd(e.share_degraded, 4),
            share_unassessed: rnd(e.share_unassessed, 4),
            share_not_observed: rnd(e.share_not_observed, 4),
            degraded_m: rnd(e.share_degraded * e.route_km * 1000.0, 1),
        });
    }
    rows.sort_by(|a, b| a.date.cmp(&b.date).then(a.file.cmp(&b.file)));
    let n = rows.len();
    let k = rows.iter().filter(|r| r.share_degraded > 0.0).count();
    let rate = wilson95(k, n).map(|(lo, hi)| [rnd(k as f64 / n as f64, 4), lo, hi]);
    let not_obs = if n == 0 {
        1.0
    } else {
        rows.iter().map(|r| r.share_not_observed).sum::<f64>() / n as f64
    };
    let worst = rows.iter().map(|r| r.degraded_m).fold(0.0, f64::max);
    let op = &req.operation;
    let dr = op.dr()?;
    let coast = worst / op.speed_m_s;
    let err = dr.error_m(op.speed_m_s, coast);
    let margin = op.contingency_m - err;
    let p = k as f64 / n.max(1) as f64;
    let class = if n == 0 || not_obs >= 0.5 {
        "insufficient-data"
    } else if k == 0 {
        "none-observed"
    } else if p >= 0.5 || margin < 0.0 {
        "high"
    } else if p >= 0.1 {
        "medium"
    } else {
        "low"
    };
    Ok(SoraInput {
        format: "kshana-pro-utm-sora/1",
        evidence: EVIDENCE,
        title: req.title.clone(),
        operation: op.clone(),
        route_km: rnd(route_m / 1000.0, 3),
        flight_time_s: rnd(route_m / op.speed_m_s, 1),
        maps: rows,
        days_affected: k,
        day_affected_rate: rate,
        mean_share_not_observed: rnd(not_obs, 4),
        worst_degraded_m: rnd(worst, 1),
        worst_coast_s: rnd(coast, 1),
        worst_dr_error_m: rnd(err, 2),
        dead_reckoning: dr,
        containment_margin_m: rnd(margin, 2),
        suggested_contingency_m: rnd(op.contingency_m.max(err), 1),
        interference_class: class,
        decision_support: DECISION_SUPPORT,
    })
}

/// The Markdown template `sora-input.md` is filled from. Placeholders are `{{name}}`.
pub const SORA_TEMPLATE: &str = "# GNSS interference input for the SORA: {{operation}}\n\
\n\
{{title}}\n\
\n\
Prepared with Kshana Pro `utm sora` (format kshana-pro-utm-sora/1). Evidence class: {{evidence}}.\n\
\n\
## 1. Operation\n\
\n\
| Item | Value |\n\
|---|---|\n\
| UAS class | {{uas_class}} |\n\
| Planned ground speed | {{speed}} m/s |\n\
| Height above ground | {{height}} m |\n\
| Route length | {{route_km}} km |\n\
| Planned flight time | {{flight_time}} s |\n\
| Contingency buffer | {{contingency}} m |\n\
| Ground-risk buffer | {{grb}} m |\n\
\n\
## 2. GNSS interference exposure of the route\n\
\n\
Day maps examined: {{days}}. Days with degraded cells on the route: {{days_affected}} \
(rate {{rate}}). Mean share of the route not observed by the maps: {{not_observed}}.\n\
\n\
| Map day | Degraded share | Unassessed share | Not observed share | Degraded length (m) |\n\
|---|---|---|---|---|\n\
{{map_rows}}\n\
\n\
## 3. Containment under GNSS loss\n\
\n\
Longest route length in degraded cells on one day: {{worst_m}} m, flown in {{coast}} s. \
Dead-reckoning error after it ({{dr_terms}}): {{dr_err}} m. Containment margin against the \
contingency buffer: {{margin}} m. A contingency buffer of {{suggested}} m would cover the error.\n\
\n\
## 4. Inputs to the assessment\n\
\n\
- Interference class for this operation: **{{class}}**.\n\
- External services: GNSS is an external service the operation relies on; this input states \
how often the route was affected and what the UAS's dead reckoning leaves when it is.\n\
- Operational volume: the containment margin above is an input to sizing the contingency \
volume; it is not a sizing rule.\n\
- Data gaps: route length not observed by the maps carries no information; it is not clear \
airspace.\n\
\n\
{{decision_support}}\n";

fn fmt(x: f64) -> String {
    let s = format!("{x:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Fill [`SORA_TEMPLATE`] from a result. Every placeholder is replaced.
pub fn sora_markdown(r: &SoraInput) -> String {
    let op = &r.operation;
    let rows: Vec<String> = r
        .maps
        .iter()
        .map(|m| {
            format!(
                "| {} | {} | {} | {} | {} |",
                m.date,
                fmt(m.share_degraded),
                fmt(m.share_unassessed),
                fmt(m.share_not_observed),
                fmt(m.degraded_m)
            )
        })
        .collect();
    let rate = r
        .day_affected_rate
        .map(|[p, lo, hi]| format!("{} (95 % interval {} to {})", fmt(p), fmt(lo), fmt(hi)))
        .unwrap_or_else(|| "not available".into());
    let d = &r.dead_reckoning;
    let terms = format!(
        "heading {} deg, scale {}, velocity bias {} m/s, velocity noise {} m/s over {} s",
        fmt(d.heading_err_deg),
        fmt(d.scale_err),
        fmt(d.velocity_bias_m_s),
        fmt(d.velocity_noise_m_s),
        fmt(d.noise_corr_s)
    );
    let pairs: [(&str, String); 22] = [
        ("operation", op.id.clone()),
        ("title", r.title.clone()),
        ("evidence", r.evidence.into()),
        ("uas_class", op.uas_class.clone()),
        ("speed", fmt(op.speed_m_s)),
        ("height", fmt(op.height_agl_m)),
        ("route_km", fmt(r.route_km)),
        ("flight_time", fmt(r.flight_time_s)),
        ("contingency", fmt(op.contingency_m)),
        ("grb", fmt(op.ground_risk_buffer_m)),
        ("days", r.maps.len().to_string()),
        ("days_affected", r.days_affected.to_string()),
        ("rate", rate),
        ("not_observed", fmt(r.mean_share_not_observed)),
        ("map_rows", rows.join("\n")),
        ("worst_m", fmt(r.worst_degraded_m)),
        ("coast", fmt(r.worst_coast_s)),
        ("dr_terms", terms),
        ("dr_err", fmt(r.worst_dr_error_m)),
        ("margin", fmt(r.containment_margin_m)),
        ("suggested", fmt(r.suggested_contingency_m)),
        ("class", r.interference_class.into()),
    ];
    let mut s = SORA_TEMPLATE.to_string();
    for (k, v) in pairs {
        s = s.replace(&format!("{{{{{k}}}}}"), &v);
    }
    s.replace("{{decision_support}}", r.decision_support)
}

// ---------------------------------------------------------------------------------------------
// USSP feed
// ---------------------------------------------------------------------------------------------

/// One feed record (one cell).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FeedRecord {
    /// Cell row.
    pub cell_i: i32,
    /// Cell column.
    pub cell_j: i32,
    /// `[south, west, north, east]`, degrees.
    pub bbox: [f64; 4],
    /// State on the latest map day that has the cell.
    pub latest_state: &'static str,
    /// Day of that state.
    pub latest_date: String,
    /// Days the cell was degraded.
    pub degraded_days: usize,
    /// Days the cell was in a map.
    pub days_observed: usize,
    /// `persistent` (degraded on half or more of the observed days, and on the latest),
    /// `recurring` (two or more days), `single` (one day) or `none`.
    pub advisory: &'static str,
    /// Valid from (UTC).
    pub valid_from: String,
    /// Valid to (UTC).
    pub valid_to: String,
}

/// The feed.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Feed {
    /// Format tag.
    pub format: &'static str,
    /// Evidence class.
    pub evidence: &'static str,
    /// Area.
    pub area: [f64; 4],
    /// Map days used.
    pub days: Vec<String>,
    /// Records, one per cell in the area present in any map, by cell.
    pub records: Vec<FeedRecord>,
    /// Decision-support statement.
    pub decision_support: &'static str,
}

/// One day map's cells: (date, cell size, cell states).
type DayCells = (String, f64, Vec<(CellId, &'static str)>);

fn cells_of(text: &str) -> Result<DayCells, String> {
    let m = load_map(text).map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for f in v["features"].as_array().into_iter().flatten() {
        let p = &f["properties"];
        let (Some(i), Some(j)) = (p["cell_i"].as_i64(), p["cell_j"].as_i64()) else {
            continue;
        };
        let s = match p["status"].as_str().unwrap_or("") {
            "degraded" | "anomalous" => "degraded",
            "not_degraded" | "not_anomalous" => "not_degraded",
            _ => "unassessed",
        };
        out.push((
            CellId {
                i: i as i32,
                j: j as i32,
            },
            s,
        ));
    }
    Ok((m.date, m.grid.cell_deg, out))
}

/// Build the USSP feed.
pub fn feed(req: &Request, maps: &[(String, String)]) -> Result<Feed, String> {
    let p = req
        .feed
        .as_ref()
        .ok_or("the request has no [feed] table (area, valid_hours)")?;
    let a = p.area;
    let valid =
        a.iter().all(|x| x.is_finite()) && a[0] < a[2] && a[1] < a[3] && p.valid_hours > 0.0;
    if !valid {
        return Err("feed.area must be [south, west, north, east] with south < north, west < east, and valid_hours positive".into());
    }
    let mut days: Vec<DayCells> = Vec::new();
    for (file, text) in maps {
        days.push(cells_of(text).map_err(|e| format!("{file}: {e}"))?);
    }
    days.sort_by(|x, y| x.0.cmp(&y.0));
    let deg = days
        .first()
        .map(|d| d.1)
        .ok_or("feed needs at least one map")?;
    if days.iter().any(|d| (d.1 - deg).abs() > 1e-9) {
        return Err("every map of a feed must use the same cell size".into());
    }
    let grid = Grid::new(deg).ok_or("invalid map grid")?;
    let last = days.last().map(|d| d.0.clone()).unwrap_or_default();
    let t_end = utc(&format!("{last}T00:00:00Z"))? + 86_400.0;
    let mut by: std::collections::BTreeMap<(i32, i32), Vec<(String, &'static str)>> =
        Default::default();
    for (date, _, cells) in &days {
        for (c, s) in cells {
            let (south, west, north, east) = grid.bounds(*c);
            if north <= a[0] || south >= a[2] || east <= a[1] || west >= a[3] {
                continue;
            }
            by.entry((c.i, c.j)).or_default().push((date.clone(), s));
        }
    }
    let records = by
        .into_iter()
        .map(|((i, j), v)| {
            let (s, w, n, e) = grid.bounds(CellId { i, j });
            let deg_days = v.iter().filter(|x| x.1 == "degraded").count();
            let (ld, ls) = v.last().cloned().unwrap_or_default();
            let advisory = if deg_days >= 1 && 2 * deg_days >= v.len() && ls == "degraded" {
                "persistent"
            } else if deg_days >= 2 {
                "recurring"
            } else if deg_days == 1 {
                "single"
            } else {
                "none"
            };
            FeedRecord {
                cell_i: i,
                cell_j: j,
                bbox: [s, w, n, e],
                latest_state: ls,
                latest_date: ld,
                degraded_days: deg_days,
                days_observed: v.len(),
                advisory,
                valid_from: crate::forensics::inputs::iso(t_end),
                valid_to: crate::forensics::inputs::iso(t_end + p.valid_hours * 3600.0),
            }
        })
        .collect();
    Ok(Feed {
        format: "kshana-pro-ussp-feed/1",
        evidence: EVIDENCE,
        area: a,
        days: days.iter().map(|d| d.0.clone()).collect(),
        records,
        decision_support: DECISION_SUPPORT,
    })
}

/// The feed as JSON Lines: the header (without records), then one record per line.
pub fn feed_jsonl(f: &Feed) -> String {
    let mut head = serde_json::to_value(f).unwrap_or_default();
    if let Some(o) = head.as_object_mut() {
        o.remove("records");
    }
    let mut s = head.to_string() + "\n";
    for r in &f.records {
        s += &serde_json::to_string(r).unwrap_or_default();
        s.push('\n');
    }
    s
}

// ---------------------------------------------------------------------------------------------
// Remote ID position authenticity
// ---------------------------------------------------------------------------------------------

/// One Remote ID report (the broadcast's own fields).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RidReport {
    /// Time, Unix seconds.
    pub t: f64,
    /// UAS identity.
    pub uas_id: String,
    /// Latitude, degrees.
    pub lat: f64,
    /// Longitude, degrees.
    pub lon: f64,
    /// Altitude, metres.
    pub alt_m: f64,
    /// Reported ground speed, m/s.
    pub speed_m_s: Option<f64>,
    /// Operator latitude, degrees.
    pub op_lat: Option<f64>,
    /// Operator longitude, degrees.
    pub op_lon: Option<f64>,
}

fn time_of(s: &str) -> Option<f64> {
    s.trim()
        .parse::<f64>()
        .ok()
        .filter(|x| x.is_finite())
        .or_else(|| utc(s.trim()).ok())
}

/// Read Remote ID CSV: a header naming at least `t_utc`, `uas_id`, `lat`, `lon`, `alt_m`, and
/// optionally `speed_m_s`, `op_lat`, `op_lon` (time as ISO-8601 UTC or Unix seconds). Returns
/// the reports and the count of rows skipped.
pub fn parse_rid(text: &str) -> Result<(Vec<RidReport>, usize), String> {
    let mut lines = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let head: Vec<String> = lines
        .next()
        .ok_or("empty Remote ID file")?
        .split(',')
        .map(|h| h.trim().to_ascii_lowercase())
        .collect();
    let col = |n: &str| head.iter().position(|h| h == n);
    let (Some(ct), Some(cid), Some(cla), Some(clo), Some(cal)) = (
        col("t_utc"),
        col("uas_id"),
        col("lat"),
        col("lon"),
        col("alt_m"),
    ) else {
        return Err("Remote ID CSV needs the columns t_utc, uas_id, lat, lon, alt_m".into());
    };
    let (cs, col_lat, col_lon) = (col("speed_m_s"), col("op_lat"), col("op_lon"));
    let (mut out, mut bad) = (Vec::new(), 0);
    for l in lines {
        let f: Vec<&str> = l.split(',').collect();
        let num = |c: Option<usize>| {
            c.and_then(|c| f.get(c))
                .and_then(|v| v.trim().parse::<f64>().ok())
                .filter(|v| v.is_finite())
        };
        let r = (|| {
            let lat = num(Some(cla))?;
            let lon = num(Some(clo))?;
            (lat.abs() <= 90.0 && lon.abs() <= 180.0).then_some(())?;
            Some(RidReport {
                t: time_of(f.get(ct)?)?,
                uas_id: f.get(cid)?.trim().to_string(),
                lat,
                lon,
                alt_m: num(Some(cal))?,
                speed_m_s: num(cs),
                op_lat: num(col_lat),
                op_lon: num(col_lon),
            })
        })();
        match r {
            Some(r) if !r.uas_id.is_empty() => out.push(r),
            _ => bad += 1,
        }
    }
    Ok((out, bad))
}

/// The checks of one identity.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RidIdentity {
    /// UAS identity.
    pub uas_id: String,
    /// Reports.
    pub reports: usize,
    /// Reports out of time order or repeated.
    pub time_order: usize,
    /// Report pairs whose reported speed disagrees with the speed between them.
    pub speed_mismatch: usize,
    /// Report pairs implying more than the largest plausible speed.
    pub implied_speed: usize,
    /// Report pairs implying more than the largest plausible climb rate.
    pub implied_climb: usize,
    /// Reports with the operator farther than the limit.
    pub operator_far: usize,
    /// Report pairs that put the identity in two places at once.
    pub clone_pairs: usize,
    /// Largest implied speed, m/s.
    pub max_implied_speed_m_s: f64,
    /// `CONSISTENT`, `SUSPECT` (soft findings) or `INCONSISTENT` (clone, teleport or time).
    pub verdict: &'static str,
}

/// The Remote ID result.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RidResult {
    /// Format tag.
    pub format: &'static str,
    /// Evidence class.
    pub evidence: &'static str,
    /// Parameters.
    pub params: RidParams,
    /// Rows skipped as malformed.
    pub skipped_rows: usize,
    /// Per identity.
    pub identities: Vec<RidIdentity>,
    /// Decision-support statement.
    pub decision_support: &'static str,
}

/// Run the Remote ID checks.
pub fn rid_check(reports: &[RidReport], skipped: usize, p: &RidParams) -> RidResult {
    let mut ids: Vec<&str> = reports.iter().map(|r| r.uas_id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    let identities = ids
        .into_iter()
        .map(|id| {
            let rs: Vec<&RidReport> = reports.iter().filter(|r| r.uas_id == id).collect();
            let time_order = rs.windows(2).filter(|w| w[1].t <= w[0].t).count();
            let mut sorted = rs.clone();
            sorted.sort_by(|a, b| a.t.total_cmp(&b.t));
            let (mut sm, mut is, mut ic, mut cl, mut vmax) = (0, 0, 0, 0, 0.0f64);
            for w in sorted.windows(2) {
                let dt = w[1].t - w[0].t;
                let d = haversine_m(w[0].lat, w[0].lon, w[1].lat, w[1].lon);
                if dt <= 0.0 {
                    if d > p.max_speed_m_s * p.clone_window_s {
                        cl += 1;
                    }
                    continue;
                }
                let v = d / dt;
                vmax = vmax.max(v);
                if dt < p.clone_window_s && d > p.max_speed_m_s * p.clone_window_s {
                    cl += 1;
                } else if v > p.max_speed_m_s {
                    is += 1;
                }
                if ((w[1].alt_m - w[0].alt_m) / dt).abs() > p.max_climb_m_s {
                    ic += 1;
                }
                if let (Some(a), Some(b)) = (w[0].speed_m_s, w[1].speed_m_s) {
                    let rep = 0.5 * (a + b);
                    if (v - rep).abs() > p.speed_tol_abs_m_s.max(p.speed_tol_rel * rep) {
                        sm += 1;
                    }
                }
            }
            let far = rs
                .iter()
                .filter(|r| match (r.op_lat, r.op_lon) {
                    (Some(a), Some(b)) => haversine_m(r.lat, r.lon, a, b) > p.max_operator_m,
                    _ => false,
                })
                .count();
            let verdict = if cl > 0 || is > 0 || time_order > 0 {
                "INCONSISTENT"
            } else if sm > 0 || ic > 0 || far > 0 {
                "SUSPECT"
            } else {
                "CONSISTENT"
            };
            RidIdentity {
                uas_id: id.to_string(),
                reports: rs.len(),
                time_order,
                speed_mismatch: sm,
                implied_speed: is,
                implied_climb: ic,
                operator_far: far,
                clone_pairs: cl,
                max_implied_speed_m_s: rnd(vmax, 2),
                verdict,
            }
        })
        .collect();
    RidResult {
        format: "kshana-pro-rid-check/1",
        evidence: EVIDENCE,
        params: p.clone(),
        skipped_rows: skipped,
        identities,
        decision_support: DECISION_SUPPORT,
    }
}

// ---------------------------------------------------------------------------------------------
// Per-flight attestation
// ---------------------------------------------------------------------------------------------

/// One telemetry epoch.
#[derive(Clone, Debug, PartialEq)]
struct Tel {
    t: f64,
    lat: f64,
    lon: f64,
    fix: u8,
    eph: Option<f64>,
    jam: Option<u8>,
    spoof: Option<u8>,
}

fn parse_telemetry(text: &str) -> Result<(Vec<Tel>, usize), String> {
    let mut lines = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let head: Vec<String> = lines
        .next()
        .ok_or("empty telemetry file")?
        .split(',')
        .map(|h| h.trim().to_ascii_lowercase())
        .collect();
    let col = |n: &str| head.iter().position(|h| h == n);
    let (Some(ct), Some(cla), Some(clo), Some(cf)) =
        (col("t_utc"), col("lat"), col("lon"), col("fix_type"))
    else {
        return Err("telemetry CSV needs the columns t_utc, lat, lon, fix_type".into());
    };
    let (ce, cj, cs) = (col("eph_m"), col("jamming_state"), col("spoofing_state"));
    let (mut out, mut bad) = (Vec::new(), 0);
    for l in lines {
        let f: Vec<&str> = l.split(',').collect();
        let num = |c: Option<usize>| {
            c.and_then(|c| f.get(c))
                .and_then(|v| v.trim().parse::<f64>().ok())
                .filter(|v| v.is_finite())
        };
        let r = (|| {
            Some(Tel {
                t: time_of(f.get(ct)?)?,
                lat: num(Some(cla))?,
                lon: num(Some(clo))?,
                fix: num(Some(cf))?.clamp(0.0, 255.0) as u8,
                eph: num(ce),
                jam: num(cj).map(|v| v.clamp(0.0, 255.0) as u8),
                spoof: num(cs).map(|v| v.clamp(0.0, 255.0) as u8),
            })
        })();
        match r {
            Some(r) => out.push(r),
            None => bad += 1,
        }
    }
    out.sort_by(|a, b| a.t.total_cmp(&b.t));
    Ok((out, bad))
}

/// The per-flight attestation.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Attestation {
    /// Format tag.
    pub format: &'static str,
    /// Evidence class.
    pub evidence: &'static str,
    /// Operation id.
    pub operation: String,
    /// Flight start and end (UTC).
    pub flight_utc: [String; 2],
    /// Telemetry epochs and rows skipped.
    pub epochs: usize,
    /// Rows skipped.
    pub skipped_rows: usize,
    /// Share of epochs without a 3-D fix.
    pub share_no_3d_fix: f64,
    /// Longest stretch without a 3-D fix, s.
    pub longest_gap_s: f64,
    /// Dead-reckoning error over it at the planned speed, metres.
    pub gap_dr_error_m: f64,
    /// Epochs with a receiver jamming state of 2 or more.
    pub jamming_epochs: usize,
    /// Epochs with a receiver spoofing state of 2 or more.
    pub spoofing_epochs: usize,
    /// Steps implying more than twice the planned speed plus 10 m/s.
    pub position_jumps: usize,
    /// Largest reported horizontal accuracy, metres.
    pub max_eph_m: Option<f64>,
    /// Share of the flown track in degraded cells on the flight's day map, when one is given.
    pub track_share_degraded: Option<f64>,
    /// Remote ID verdict of the operation's identity, when Remote ID was given.
    pub remote_id: Option<RidResult>,
    /// `NOMINAL`, `DEGRADED` or `INTEGRITY-SUSPECT`.
    pub statement: &'static str,
    /// Why.
    pub reasons: Vec<String>,
    /// Decision-support statement.
    pub decision_support: &'static str,
}

/// Build the attestation for one flight.
pub fn attest(l: &Loaded) -> Result<Attestation, String> {
    let req = &l.req;
    let op = &req.operation;
    let text = l
        .telemetry
        .as_ref()
        .ok_or("attest needs telemetry = \"<file.csv>\" in the request")?;
    let (tel, skipped) = parse_telemetry(text)?;
    if tel.len() < 2 {
        return Err("the telemetry has fewer than two valid epochs".into());
    }
    let n = tel.len();
    let no3d = tel.iter().filter(|e| e.fix < 3).count();
    let mut gap: f64 = 0.0;
    let mut start: Option<f64> = None;
    for e in &tel {
        if e.fix < 3 {
            let s = *start.get_or_insert(e.t);
            gap = gap.max(e.t - s);
        } else if let Some(s) = start.take() {
            gap = gap.max(e.t - s);
        }
    }
    let dr = op.dr()?;
    let gap_err = dr.error_m(op.speed_m_s, gap);
    let jam = tel.iter().filter(|e| e.jam.is_some_and(|j| j >= 2)).count();
    let spoof = tel
        .iter()
        .filter(|e| e.spoof.is_some_and(|s| s >= 2))
        .count();
    let vlim = 2.0 * op.speed_m_s + 10.0;
    let jumps = tel
        .windows(2)
        .filter(|w| w[0].fix >= 3 && w[1].fix >= 3)
        .filter(|w| {
            let dt = (w[1].t - w[0].t).max(1e-3);
            haversine_m(w[0].lat, w[0].lon, w[1].lat, w[1].lon) / dt > vlim
        })
        .count();
    let max_eph = tel.iter().filter_map(|e| e.eph).reduce(f64::max);
    let day = crate::forensics::inputs::iso(tel[0].t)[..10].to_string();
    let mut share = None;
    for (file, t) in &l.maps {
        let m = load_map(t).map_err(|e| format!("{file}: {e}"))?;
        if m.date == day {
            let track: Vec<(f64, f64)> = tel
                .iter()
                .filter(|e| e.fix >= 3)
                .map(|e| (e.lat, e.lon))
                .collect();
            share = Some(rnd(exposure(&track, &m).share_degraded, 4));
        }
    }
    let rid = match &l.rid {
        Some(t) => {
            let (r, bad) = parse_rid(t)?;
            let mine: Vec<RidReport> = r.into_iter().filter(|x| x.uas_id == op.id).collect();
            Some(rid_check(&mine, bad, &req.rid_params))
        }
        None => None,
    };
    let mut reasons = Vec::new();
    let mut suspect = false;
    if spoof > 0 {
        suspect = true;
        reasons.push(format!(
            "receiver spoofing state 2 or more in {spoof} epochs"
        ));
    }
    if jumps > 0 {
        suspect = true;
        reasons.push(format!("{jumps} position steps faster than {vlim} m/s"));
    }
    if gap_err > op.contingency_m {
        suspect = true;
        reasons.push(format!(
            "dead-reckoning error over the longest fix gap ({} m) exceeds the contingency buffer ({} m)",
            rnd(gap_err, 1),
            op.contingency_m
        ));
    }
    if let Some(r) = &rid {
        for i in &r.identities {
            if i.verdict == "INCONSISTENT" {
                suspect = true;
                reasons.push(format!("Remote ID of {} is inconsistent", i.uas_id));
            }
        }
    }
    let degraded = jam > 0 || no3d > 0 || share.is_some_and(|s| s > 0.0);
    if !suspect && degraded {
        if jam > 0 {
            reasons.push(format!("receiver jamming state 2 or more in {jam} epochs"));
        }
        if no3d > 0 {
            reasons.push(format!(
                "{no3d} epochs without a 3-D fix; longest gap {} s within the contingency buffer",
                rnd(gap, 1)
            ));
        }
        if share.is_some_and(|s| s > 0.0) {
            reasons.push("track crosses cells degraded on the day map".into());
        }
    }
    let statement = if suspect {
        "INTEGRITY-SUSPECT"
    } else if degraded {
        "DEGRADED"
    } else {
        "NOMINAL"
    };
    Ok(Attestation {
        format: "kshana-pro-utm-attestation/1",
        evidence: EVIDENCE,
        operation: op.id.clone(),
        flight_utc: [
            crate::forensics::inputs::iso(tel[0].t),
            crate::forensics::inputs::iso(tel[n - 1].t),
        ],
        epochs: n,
        skipped_rows: skipped,
        share_no_3d_fix: rnd(no3d as f64 / n as f64, 4),
        longest_gap_s: rnd(gap, 1),
        gap_dr_error_m: rnd(gap_err, 2),
        jamming_epochs: jam,
        spoofing_epochs: spoof,
        position_jumps: jumps,
        max_eph_m: max_eph.map(|x| rnd(x, 2)),
        track_share_degraded: share,
        remote_id: rid,
        statement,
        reasons,
        decision_support: DECISION_SUPPORT,
    })
}

/// The request for the hosted per-flight verdict (`kshana-pro verdict flight-integrity`): the
/// operation's speed and contingency buffer, the telemetry epochs, the linearised
/// dead-reckoning growth over the longest gap, and this attestation (whose hash the verdict
/// binds). Other flights and regional reports may be added to the JSON before sending.
pub fn verdict_request(l: &Loaded, a: &Attestation) -> Result<serde_json::Value, String> {
    let text = l
        .telemetry
        .as_ref()
        .ok_or("the verdict request needs telemetry = \"<file.csv>\" in the request")?;
    let (tel, _) = parse_telemetry(text)?;
    let op = &l.req.operation;
    let epochs: Vec<serde_json::Value> = tel
        .iter()
        .map(|e| {
            serde_json::json!({
                "t": e.t,
                "lat_deg": e.lat,
                "lon_deg": e.lon,
                "fix_type": e.fix,
                "eph_m": e.eph,
                "jamming_state": e.jam,
                "spoofing_state": e.spoof,
            })
        })
        .collect();
    let mut operation = serde_json::json!({
        "id": op.id,
        "speed_m_s": op.speed_m_s,
        "contingency_m": op.contingency_m,
    });
    if a.longest_gap_s > 0.0 {
        operation["dr_growth_m_per_s"] = serde_json::json!(a.gap_dr_error_m / a.longest_gap_s);
    }
    Ok(serde_json::json!({
        "operation": operation,
        "telemetry": epochs,
        "attestation": serde_json::to_value(a).map_err(|e| e.to_string())?,
    }))
}

/// A one-paragraph plain-text summary of any result.
pub fn summary(kind: &str, headline: &str) -> String {
    format!("[utm {kind}] {headline}\n{DECISION_SUPPORT}\nEvidence class: {EVIDENCE}.\n")
}
