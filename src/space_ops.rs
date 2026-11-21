// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Space-operations signed evidence packs (research 2026-11-20 gaps, B2205)
//!
//! Built from the operator's own data and the open engine's orbit tools:
//!
//! * [`ca_record`]: a **collision-avoidance decision record**: the conjunction data messages
//!   (ids, creation times, time of closest approach), the collision probability recomputed from
//!   each message's relative state, covariances and hard-body radius beside the value the message
//!   reports, the operator's decision and rationale, the timeline (decision lead time before TCA)
//!   and a check of the decision against the operator's **own stated policy** (probability
//!   threshold, decision deadline);
//! * [`identify`]: **identification evidence**: the operator's ephemeris of its object against
//!   candidate catalogue TLEs propagated with the open SGP4; RMS separation per candidate and the
//!   margin between the best and the next;
//! * [`disposal`]: a **disposal-plan check**: orbital lifetime of a circular orbit at the stated
//!   altitude under drag in the open engine's US Standard Atmosphere 1976, against the deadline
//!   the operator's plan states;
//! * an **operator evidence pack** (CLI): the three, with an index, signed together.
//!
//! Collision probability: the two-dimensional encounter-plane method (Foster): the combined
//! position covariance projected on the plane normal to the relative velocity, the Gaussian of
//! the projected miss integrated over the disk of the hard-body radius (closed-form inner
//! integral, adaptive quadrature). Object 2's covariance is rotated from its own RTN frame when
//! object states are given; with a relative state both covariances must be in object 1's RTN
//! frame. It assumes a short, rectilinear encounter. MODELLED throughout; no record states that an operator meets a
//! regulation, and the regulatory article an operator pack answers to is to confirm.

use serde::{Deserialize, Serialize};

use crate::forensics::inputs::parse_utc;

/// Pro feature.
pub const FEATURE: &str = "pro-space-ops-evidence";
/// Formats.
pub const CA_FORMAT: &str = "kshana-pro-ca-decision-record/1";
pub const ID_FORMAT: &str = "kshana-pro-identification-evidence/1";
pub const DISPOSAL_FORMAT: &str = "kshana-pro-disposal-check/1";
pub const PACK_FORMAT: &str = "kshana-pro-operator-evidence-pack/1";

type M3 = [[f64; 3]; 3];

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}
fn scale(a: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn quad(c: &M3, u: [f64; 3], v: [f64; 3]) -> f64 {
    let mut s = 0.0;
    for i in 0..3 {
        for j in 0..3 {
            s += u[i] * c[i][j] * v[j];
        }
    }
    s
}

/// Lower-triangle RTN position covariance `[rr, tr, tt, nr, nt, nn]` (m²) to a matrix.
fn cov(l: [f64; 6]) -> M3 {
    [[l[0], l[1], l[3]], [l[1], l[2], l[4]], [l[3], l[4], l[5]]]
}

/// One conjunction data message (the fields the record uses).
///
/// Two input forms, as the operator has them:
///
/// * **object states** (`state1_m`, `state2_m`): each object's position and velocity at TCA in
///   one common inertial frame (m, m/s), with each object's position covariance in **its own**
///   RTN frame, as a CCSDS CDM gives them. Object 2's covariance is rotated from object 2's RTN
///   and object 1's from object 1's into the common frame before they are summed;
/// * **relative state** (`rel_pos_rtn_m`, `rel_vel_rtn_mps`): object 2 relative to object 1,
///   expressed in object 1's RTN axes. Both covariances must then already be in object 1's RTN
///   frame: declare it with `cov2_frame = "rtn1"`. Without the declaration the record computes
///   under that assumption and flags it. The relative velocity must be the **inertial** relative
///   velocity expressed in object 1's RTN axes, not the rate of change of the RTN components
///   (which differs by ω×r); no path here converts between the two.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cdm {
    pub message_id: String,
    pub creation_utc: String,
    pub tca_utc: String,
    /// Object 2 relative to object 1 in object 1's RTN axes (m, inertial m/s).
    #[serde(default)]
    pub rel_pos_rtn_m: Option<[f64; 3]>,
    #[serde(default)]
    pub rel_vel_rtn_mps: Option<[f64; 3]>,
    /// Each object's inertial state at TCA in one common frame `[x, y, z, vx, vy, vz]` (m, m/s).
    #[serde(default)]
    pub state1_m: Option<[f64; 6]>,
    #[serde(default)]
    pub state2_m: Option<[f64; 6]>,
    /// Position covariances, lower triangle `[rr, tr, tt, nr, nt, nn]` (m²): object 1's in its
    /// RTN frame; object 2's in its own RTN frame with object states, in the frame `cov2_frame`
    /// declares with a relative state.
    pub cov1_rtn_m2: [f64; 6],
    pub cov2_rtn_m2: [f64; 6],
    /// With a relative state: `rtn1` (object 2's covariance is in object 1's RTN frame).
    #[serde(default)]
    pub cov2_frame: Option<String>,
    pub hbr_m: f64,
    #[serde(default)]
    pub pc_reported: Option<f64>,
}

/// The geometry a probability is computed from, in one frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    pub rel_pos_m: [f64; 3],
    pub rel_vel_mps: [f64; 3],
    pub cov1_m2: M3,
    pub cov2_m2: M3,
    /// How the covariances were brought into one frame.
    pub frames: String,
    /// False when a relative state did not declare the frame of object 2's covariance.
    pub declared: bool,
}

/// The RTN axes of an inertial state, as rows R, T, N (CCSDS 508.0-B-1 annex E: R radial, N along
/// the angular momentum, T = N × R).
pub fn rtn_axes(state: &[f64; 6]) -> Result<M3, String> {
    let r = [state[0], state[1], state[2]];
    let v = [state[3], state[4], state[5]];
    let h = cross(r, v);
    if !(norm(r) > 0.0 && norm(h) > 0.0) {
        return Err("a state with no position or no angular momentum has no RTN frame".into());
    }
    let rr = scale(r, 1.0 / norm(r));
    let n = scale(h, 1.0 / norm(h));
    Ok([rr, cross(n, rr), n])
}

/// An RTN position covariance expressed in the inertial frame of `state`: `Aᵀ C A`.
pub fn rtn_to_inertial(state: &[f64; 6], c: &M3) -> Result<M3, String> {
    let a = rtn_axes(state)?;
    let mut out = [[0.0; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..3)
                .flat_map(|k| (0..3).map(move |l| (k, l)))
                .map(|(k, l)| a[k][i] * c[k][l] * a[l][j])
                .sum();
        }
    }
    Ok(out)
}

/// The geometry of a message in one frame.
pub fn geometry(c: &Cdm) -> Result<Geometry, String> {
    let id = &c.message_id;
    let (c1, c2) = (cov(c.cov1_rtn_m2), cov(c.cov2_rtn_m2));
    match (c.state1_m, c.state2_m, c.rel_pos_rtn_m, c.rel_vel_rtn_mps) {
        (Some(s1), Some(s2), None, None) => {
            if c.cov2_frame.as_deref().is_some_and(|f| f != "rtn2") {
                return Err(format!(
                    "{id}: with object states each covariance is in its own object's RTN frame (cov2_frame may only be \"rtn2\")"
                ));
            }
            let g1 = rtn_to_inertial(&s1, &c1).map_err(|e| format!("{id}: object 1: {e}"))?;
            let g2 = rtn_to_inertial(&s2, &c2).map_err(|e| format!("{id}: object 2: {e}"))?;
            Ok(Geometry {
                rel_pos_m: [s2[0] - s1[0], s2[1] - s1[1], s2[2] - s1[2]],
                rel_vel_mps: [s2[3] - s1[3], s2[4] - s1[4], s2[5] - s1[5]],
                cov1_m2: g1,
                cov2_m2: g2,
                frames: "object states in a common inertial frame; each covariance rotated from its own object's RTN frame into it".into(),
                declared: true,
            })
        }
        (None, None, Some(r), Some(v)) => {
            let declared = match c.cov2_frame.as_deref() {
                Some("rtn1") => true,
                None => false,
                Some(f) => {
                    return Err(format!(
                        "{id}: with a relative state object 2's covariance must be in object 1's RTN frame (cov2_frame = \"rtn1\"), not '{f}'; give both object states to have it rotated"
                    ))
                }
            };
            Ok(Geometry {
                rel_pos_m: r,
                rel_vel_mps: v,
                cov1_m2: c1,
                cov2_m2: c2,
                frames: if declared {
                    "relative state in object 1's RTN axes; both covariances in object 1's RTN frame (declared)".into()
                } else {
                    "relative state in object 1's RTN axes; both covariances ASSUMED in object 1's RTN frame (not declared)".into()
                },
                declared,
            })
        }
        _ => Err(format!(
            "{id}: give either both object states (state1_m, state2_m) or the relative state (rel_pos_rtn_m, rel_vel_rtn_mps), not a mix"
        )),
    }
}

/// Two-dimensional encounter-plane collision probability of a message (Foster), from its
/// geometry in one frame.
pub fn pc_2d(c: &Cdm) -> Result<f64, String> {
    let g = geometry(c)?;
    pc_foster(&g, c.hbr_m).map_err(|e| format!("{}: {e}", c.message_id))
}

/// The encounter-plane probability: the combined covariance projected on the plane normal to
/// the relative velocity, in its principal axes; the inner integral across the disc of the
/// hard-body radius in closed form (erf), the outer one with `x = R sin t` by adaptive Simpson
/// quadrature to a relative tolerance of 1e-10. Mirrors the open engine's encounter-plane
/// routine (`orbit_data::pc::foster_2d`, not yet in the pinned release).
pub fn pc_foster(g: &Geometry, hbr_m: f64) -> Result<f64, String> {
    let v = g.rel_vel_mps;
    if !(norm(v) > 0.0 && hbr_m > 0.0 && hbr_m.is_finite()) {
        return Err("needs a relative velocity and a positive hard-body radius".into());
    }
    let all = g
        .rel_pos_m
        .iter()
        .chain(&v)
        .chain(g.cov1_m2.iter().flatten())
        .chain(g.cov2_m2.iter().flatten());
    if all.clone().any(|x| !x.is_finite()) {
        return Err("an input is not finite".into());
    }
    let z = scale(v, 1.0 / norm(v));
    let r = g.rel_pos_m;
    let rp = sub(r, scale(z, dot(r, z)));
    let x = if norm(rp) > 1e-12 * norm(r).max(1.0) {
        scale(rp, 1.0 / norm(rp))
    } else {
        let t = if z[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let p = cross(z, t);
        scale(p, 1.0 / norm(p))
    };
    let y = cross(z, x);
    let (c1, c2) = (&g.cov1_m2, &g.cov2_m2);
    let mut cc = [[0.0; 3]; 3];
    for (i, row) in cc.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = 0.5 * (c1[i][j] + c1[j][i]) + 0.5 * (c2[i][j] + c2[j][i]);
        }
    }
    let (cxx, cxy, cyy) = (quad(&cc, x, x), quad(&cc, x, y), quad(&cc, y, y));
    let tr = cxx + cyy;
    let disc = (((cxx - cyy) / 2.0).powi(2) + cxy * cxy).sqrt();
    let (l1, l2) = (tr / 2.0 + disc, tr / 2.0 - disc);
    if l2.is_nan() || l2 <= 0.0 {
        return Err(
            "the combined covariance is not positive definite in the encounter plane".into(),
        );
    }
    let th = 0.5 * (2.0 * cxy).atan2(cxx - cyy);
    let (s, co) = th.sin_cos();
    let m = dot(r, x);
    Ok(integrate(hbr_m, l1.sqrt(), l2.sqrt(), m * co, -m * s))
}

fn integrate(r: f64, sx: f64, sy: f64, mx: f64, my: f64) -> f64 {
    let inv_sqrt_2pi = 1.0 / (2.0 * std::f64::consts::PI).sqrt();
    let g = |t: f64| {
        let (st, ct) = t.sin_cos();
        let x = r * st;
        let h = (r * ct).max(0.0);
        let px = inv_sqrt_2pi / sx * (-0.5 * ((x - mx) / sx).powi(2)).exp();
        let s2 = std::f64::consts::SQRT_2 * sy;
        let inner = 0.5 * (libm::erf((my + h) / s2) - libm::erf((my - h) / s2));
        px * inner * r * ct
    };
    let (a, b) = (-std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
    let n = 64;
    let w = (b - a) / n as f64;
    let mut total = 0.0;
    for k in 0..n {
        let (l, u) = (a + k as f64 * w, a + (k + 1) as f64 * w);
        let (fl, fm, fu) = (g(l), g(0.5 * (l + u)), g(u));
        let whole = (u - l) / 6.0 * (fl + 4.0 * fm + fu);
        total += simpson(&g, (l, u), (fl, fm, fu), whole, 1e-13, 40);
    }
    total.clamp(0.0, 1.0)
}

fn simpson(
    g: &dyn Fn(f64) -> f64,
    (a, b): (f64, f64),
    (fa, fm, fb): (f64, f64, f64),
    whole: f64,
    tol: f64,
    depth: u32,
) -> f64 {
    let m = 0.5 * (a + b);
    let (lm, rm) = (0.5 * (a + m), 0.5 * (m + b));
    let (flm, frm) = (g(lm), g(rm));
    let left = (m - a) / 6.0 * (fa + 4.0 * flm + fm);
    let right = (b - m) / 6.0 * (fm + 4.0 * frm + fb);
    let delta = left + right - whole;
    if depth == 0 || delta.abs() <= 15.0 * tol.max(1e-10 * (left + right).abs()) {
        return left + right + delta / 15.0;
    }
    simpson(g, (a, m), (fa, flm, fm), left, tol / 2.0, depth - 1)
        + simpson(g, (m, b), (fm, frm, fb), right, tol / 2.0, depth - 1)
}

/// The operator's own policy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    /// Manoeuvre when the probability is at or above this.
    pub pc_threshold: f64,
    /// Decide at least this long before TCA (h).
    pub decide_by_h: f64,
    pub source: String,
}

/// The decision.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub utc: String,
    /// `maneuver`, `no-maneuver` or `monitor`.
    pub action: String,
    pub rationale: String,
    #[serde(default)]
    pub decided_by: String,
}

/// The collision-avoidance request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaRequest {
    pub title: String,
    pub primary: String,
    pub secondary: String,
    pub policy: Policy,
    #[serde(rename = "cdm")]
    pub cdms: Vec<Cdm>,
    pub decision: Decision,
    /// The message id of a CDM issued after the manoeuvre, if any.
    #[serde(default)]
    pub post_maneuver_cdm: Option<String>,
}

/// One CDM line of the record.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CdmLine {
    pub message_id: String,
    pub creation_utc: String,
    pub tca_utc: String,
    pub miss_m: f64,
    pub pc_computed: f64,
    /// How the two covariances were brought into one frame.
    pub covariance_frames: String,
    pub pc_reported: Option<f64>,
    pub before_decision: bool,
}

/// The collision-avoidance decision record.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CaRecord {
    pub format: &'static str,
    pub title: String,
    pub primary: String,
    pub secondary: String,
    pub policy: Policy,
    pub cdms: Vec<CdmLine>,
    pub decision: Decision,
    pub basis_cdm: Option<String>,
    pub basis_pc: Option<f64>,
    pub lead_time_h: f64,
    pub post_maneuver_pc: Option<f64>,
    pub policy_findings: Vec<String>,
    pub method: &'static str,
    pub verdict: String,
}

/// Build the record.
pub fn ca_record(q: &CaRequest) -> Result<CaRecord, String> {
    if q.cdms.is_empty() {
        return Err("at least one [[cdm]] is needed".into());
    }
    if !["maneuver", "no-maneuver", "monitor"].contains(&q.decision.action.as_str()) {
        return Err(format!("decision.action '{}'", q.decision.action));
    }
    if q.decision.rationale.trim().is_empty() || q.policy.source.trim().is_empty() {
        return Err("the decision needs a rationale and the policy its source".into());
    }
    let td = parse_utc(&q.decision.utc).ok_or("decision.utc is not a UTC time")?;
    let mut lines = Vec::new();
    let mut by_id = std::collections::BTreeMap::new();
    let mut undeclared = Vec::new();
    for c in &q.cdms {
        let tc = parse_utc(&c.creation_utc)
            .ok_or_else(|| format!("{}: bad creation_utc", c.message_id))?;
        parse_utc(&c.tca_utc).ok_or_else(|| format!("{}: bad tca_utc", c.message_id))?;
        let g = geometry(c)?;
        let pc = pc_foster(&g, c.hbr_m).map_err(|e| format!("{}: {e}", c.message_id))?;
        if !g.declared {
            undeclared.push(c.message_id.clone());
        }
        by_id.insert(c.message_id.clone(), pc);
        lines.push(CdmLine {
            message_id: c.message_id.clone(),
            creation_utc: c.creation_utc.clone(),
            tca_utc: c.tca_utc.clone(),
            miss_m: norm(g.rel_pos_m),
            pc_computed: pc,
            covariance_frames: g.frames.clone(),
            pc_reported: c.pc_reported,
            before_decision: tc <= td,
        });
    }
    lines.sort_by(|a, b| a.creation_utc.cmp(&b.creation_utc));
    let basis = lines.iter().rfind(|l| l.before_decision).cloned();
    let tca = basis
        .as_ref()
        .and_then(|b| parse_utc(&b.tca_utc))
        .or_else(|| parse_utc(&lines[0].tca_utc))
        .unwrap_or(td);
    let lead = (tca - td) / 3600.0;
    let mut f = Vec::new();
    match &basis {
        None => f.push("no CDM was available before the decision".into()),
        Some(b) => {
            let over = b.pc_computed >= q.policy.pc_threshold;
            if over && q.decision.action != "maneuver" {
                f.push(format!("the probability before the decision ({:.2e}) was at or above the stated threshold ({:.1e}) and the action was '{}'", b.pc_computed, q.policy.pc_threshold, q.decision.action));
            }
            if !over && q.decision.action == "maneuver" {
                f.push(format!("manoeuvre below the stated threshold ({:.2e} < {:.1e}): the rationale should say why", b.pc_computed, q.policy.pc_threshold));
            }
            if let Some(r) = b.pc_reported {
                if r > 0.0 && (b.pc_computed / r).log10().abs() > 1.0 {
                    f.push(format!("recomputed probability {:.2e} differs from the reported {:.2e} by more than a factor of ten", b.pc_computed, r));
                }
            }
        }
    }
    if !undeclared.is_empty() {
        f.push(format!(
            "{} does not declare the frame of object 2's covariance (cov2_frame); it was taken as object 1's RTN frame, which is wrong if it is object 2's own",
            undeclared.join(", ")
        ));
    }
    if lead < q.policy.decide_by_h {
        f.push(format!(
            "decision {lead:.1} h before TCA, later than the stated {} h",
            q.policy.decide_by_h
        ));
    }
    let post = q
        .post_maneuver_cdm
        .as_ref()
        .and_then(|id| by_id.get(id).copied());
    if let Some(p) = post {
        if p >= q.policy.pc_threshold {
            f.push(format!(
                "the post-manoeuvre probability {p:.2e} is still at or above the threshold"
            ));
        }
    }
    let verdict = if f.is_empty() {
        format!("Decision '{}' {:.1} h before TCA is consistent with the operator's stated policy on the CDMs supplied.", q.decision.action, lead)
    } else {
        format!("Decision '{}' {:.1} h before TCA; {} point(s) for review against the operator's stated policy.", q.decision.action, lead, f.len())
    };
    Ok(CaRecord {
        format: CA_FORMAT,
        title: q.title.clone(),
        primary: q.primary.clone(),
        secondary: q.secondary.clone(),
        policy: q.policy.clone(),
        cdms: lines,
        decision: q.decision.clone(),
        basis_cdm: basis.as_ref().map(|b| b.message_id.clone()),
        basis_pc: basis.as_ref().map(|b| b.pc_computed),
        lead_time_h: lead,
        post_maneuver_pc: post,
        policy_findings: f,
        method: "Two-dimensional encounter-plane probability (Foster 1992): both covariances in one frame (each rotated from its own object's RTN frame when object states are given), summed, projected on the plane normal to the inertial relative velocity, integrated over the hard-body disc (closed-form inner integral, adaptive quadrature); short rectilinear encounter; MODELLED.",
        verdict,
    })
}

/// One identification candidate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub name: String,
    pub line1: String,
    pub line2: String,
}

/// One candidate's fit.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Fit {
    pub name: String,
    pub norad_id: String,
    pub rms_km: f64,
    pub max_km: f64,
}

/// Identification evidence.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Identification {
    pub format: &'static str,
    pub points: usize,
    pub fits: Vec<Fit>,
    pub best: String,
    /// Next-best RMS over best RMS.
    pub margin: f64,
    /// The frame and time scale of the ephemeris: declared (`frame`, `time_scale` columns) or
    /// assumed.
    pub ephemeris_frame: String,
    pub verdict: String,
}

/// The operator's ephemeris (`timestamp,x_km,y_km,z_km[,frame][,time_scale]`) against candidate
/// TLEs. SGP4 gives TEME positions on UTC, so the ephemeris must be TEME with UTC timestamps: a
/// `frame` or `time_scale` column with any other value is refused (an EME2000 or ITRF ephemeris,
/// or GPS-time stamps, would be tens to thousands of km off and could change the best match).
/// Without the columns, TEME on UTC is assumed and the verdict says so.
pub fn identify(ephem: &str, cands: &[Candidate]) -> Result<Identification, String> {
    if cands.len() < 2 {
        return Err("identification needs at least two candidates".into());
    }
    let t = crate::transport::Table::parse(ephem)?;
    let (ct, cx, cy, cz) = (
        t.need("timestamp")?,
        t.need("x_km")?,
        t.need("y_km")?,
        t.need("z_km")?,
    );
    let (cf, cs) = (t.col("frame"), t.col("time_scale"));
    let mut pts = Vec::new();
    for i in 0..t.rows.len() {
        if cf.is_some() && !t.text(i, cf).eq_ignore_ascii_case("TEME") {
            return Err(format!(
                "row {}: frame '{}': the ephemeris must be TEME (convert it first)",
                i + 2,
                t.text(i, cf)
            ));
        }
        if cs.is_some() && !t.text(i, cs).eq_ignore_ascii_case("UTC") {
            return Err(format!(
                "row {}: time_scale '{}': the timestamps must be UTC (convert them first)",
                i + 2,
                t.text(i, cs)
            ));
        }
        let ts = t.text(i, Some(ct));
        let tt = parse_utc(ts).ok_or_else(|| format!("row {}: bad timestamp", i + 2))?;
        let n = |c| {
            t.num(i, Some(c))?
                .ok_or_else(|| format!("row {}: empty", i + 2))
        };
        pts.push((tt / 86_400.0 + 2_440_587.5, [n(cx)?, n(cy)?, n(cz)?]));
    }
    if pts.is_empty() {
        return Err("no ephemeris point".into());
    }
    let mut fits = Vec::new();
    for c in cands {
        let tle =
            kshana::tle::parse_tle(&c.line1, &c.line2).map_err(|e| format!("{}: {e}", c.name))?;
        let s = tle.to_sgp4(kshana::sgp4::wgs72(), false);
        let mut d2 = Vec::new();
        for (jd, p) in &pts {
            let tsince = (jd - s.epoch_jd_utc()) * 1440.0;
            let (r, _) = s
                .propagate(tsince)
                .map_err(|e| format!("{}: SGP4 error {e}", c.name))?;
            d2.push(norm(sub(r, *p)));
        }
        fits.push(Fit {
            name: c.name.clone(),
            norad_id: c.line1.get(2..7).unwrap_or("").trim().to_string(),
            rms_km: (d2.iter().map(|x| x * x).sum::<f64>() / d2.len() as f64).sqrt(),
            max_km: d2.iter().cloned().fold(0.0, f64::max),
        });
    }
    fits.sort_by(|a, b| a.rms_km.total_cmp(&b.rms_km));
    let margin = fits[1].rms_km / fits[0].rms_km.max(1e-9);
    let declared = cf.is_some() && cs.is_some();
    let ephemeris_frame = if declared {
        "TEME on UTC (declared)".to_string()
    } else {
        "TEME on UTC (ASSUMED: no frame and time_scale columns)".to_string()
    };
    Ok(Identification {
        format: ID_FORMAT,
        points: pts.len(),
        best: fits[0].name.clone(),
        margin,
        verdict: format!(
            "Best match {} (NORAD {}) at {:.3} km RMS; the next candidate is {:.1} times further. Ephemeris {}. Identification evidence from the operator's ephemeris; MODELLED.",
            fits[0].name, fits[0].norad_id, fits[0].rms_km, margin, ephemeris_frame
        ),
        ephemeris_frame,
        fits,
    })
}

/// The disposal-plan request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisposalRequest {
    pub title: String,
    /// Mean altitude of the (near-circular) disposal orbit (km).
    pub altitude_km: f64,
    pub mass_kg: f64,
    pub area_m2: f64,
    #[serde(default = "d_cd")]
    pub cd: f64,
    /// The plan's own deadline (years) and where it comes from.
    pub plan_deadline_years: f64,
    pub plan_source: String,
}

fn d_cd() -> f64 {
    2.2
}

/// The disposal check.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Disposal {
    pub format: &'static str,
    pub request: DisposalRequest,
    pub lifetime_years: Option<f64>,
    pub within_plan: Option<bool>,
    pub model: &'static str,
    pub verdict: String,
}

/// Lifetime of a circular orbit by drag in the US76 atmosphere until 120 km (or 200 years).
pub fn disposal(q: &DisposalRequest) -> Result<Disposal, String> {
    if !(q.altitude_km > 120.0
        && q.mass_kg > 0.0
        && q.area_m2 > 0.0
        && q.cd > 0.0
        && q.plan_deadline_years > 0.0)
    {
        return Err(
            "altitude above 120 km and positive mass, area, cd and deadline are needed".into(),
        );
    }
    if q.plan_source.trim().is_empty() {
        return Err("plan_source must say where the deadline comes from".into());
    }
    const GM: f64 = 3.986_004_418e14;
    const RE: f64 = 6_378_137.0;
    let b = q.cd * q.area_m2 / q.mass_kg;
    let mut a = RE + q.altitude_km * 1000.0;
    let mut t = 0.0;
    let limit = 200.0 * 365.25 * 86_400.0;
    while a - RE > 120_000.0 && t < limit {
        let rho = kshana::reentry::us76_density(a - RE);
        let dadt = -(GM * a).sqrt() * rho * b;
        // Step so that a changes by at most 0.5 km, at most 10 days.
        let dt = if dadt < 0.0 {
            (500.0 / -dadt).min(10.0 * 86_400.0)
        } else {
            10.0 * 86_400.0
        };
        a += dadt * dt;
        t += dt;
    }
    let years = (a - RE <= 120_000.0).then_some(t / (365.25 * 86_400.0));
    let within = years.map(|y| y <= q.plan_deadline_years).or(Some(false));
    Ok(Disposal {
        format: DISPOSAL_FORMAT,
        request: q.clone(),
        lifetime_years: years,
        within_plan: within,
        model: "Circular orbit, da/dt = -sqrt(GM a) rho Cd A/m, US Standard Atmosphere 1976 (static: no solar-activity variation), to 120 km. A screening estimate; solar activity can change lifetimes by large factors. MODELLED.",
        verdict: match years {
            Some(y) => format!("Estimated lifetime {y:.1} years against the plan's {} years ({}): {}.", q.plan_deadline_years, q.plan_source, if y <= q.plan_deadline_years { "within the plan's deadline under this model" } else { "beyond the plan's deadline under this model" }),
            None => format!("No decay to 120 km within 200 years under this model; beyond the plan's {} years.", q.plan_deadline_years),
        },
    })
}

/// The candidates file: `[[candidate]]` tables.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidates {
    pub candidate: Vec<Candidate>,
}

/// The operator evidence-pack request (paths relative to the request file).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackRequest {
    pub title: String,
    pub operator: String,
    pub object: String,
    /// The article or framework this pack is organised for, as the operator names it (to confirm).
    #[serde(default)]
    pub framework_note: String,
    #[serde(default)]
    pub ca: Vec<String>,
    #[serde(default)]
    pub ephemeris: Option<String>,
    #[serde(default)]
    pub candidates: Option<String>,
    #[serde(default)]
    pub disposal: Option<String>,
}

/// The operator evidence pack.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Pack {
    pub format: &'static str,
    pub title: String,
    pub operator: String,
    pub object: String,
    pub framework_note: String,
    pub ca_records: Vec<CaRecord>,
    pub identification: Option<Identification>,
    pub disposal: Option<Disposal>,
    pub open_points: Vec<String>,
    pub label: &'static str,
    pub verdict: String,
}

/// Build the pack from its request; `base` is the request's directory. Returns the pack and the
/// inputs read (name, text) for the evidence directory.
pub fn pack(
    q: &PackRequest,
    base: &std::path::Path,
) -> Result<(Pack, Vec<(String, String)>), String> {
    use crate::timing::grid::read_text;
    let mut inputs = Vec::new();
    let mut ca_records = Vec::new();
    for (i, p) in q.ca.iter().enumerate() {
        let s = read_text(base, p)?;
        let r: CaRequest = crate::transport::parse_toml(&s, p)?;
        ca_records.push(ca_record(&r)?);
        inputs.push((format!("ca-{}.toml", i + 1), s));
    }
    let identification = match (&q.ephemeris, &q.candidates) {
        (Some(e), Some(c)) => {
            let es = read_text(base, e)?;
            let cs = read_text(base, c)?;
            let cands: Candidates = crate::transport::parse_toml(&cs, c)?;
            let r = identify(&es, &cands.candidate)?;
            inputs.push(("ephemeris.csv".into(), es));
            inputs.push(("candidates.toml".into(), cs));
            Some(r)
        }
        (None, None) => None,
        _ => return Err("identification needs both ephemeris and candidates".into()),
    };
    let disposal = match &q.disposal {
        Some(d) => {
            let s = read_text(base, d)?;
            let r: DisposalRequest = crate::transport::parse_toml(&s, d)?;
            inputs.push(("disposal.toml".into(), s));
            Some(disposal(&r)?)
        }
        None => None,
    };
    let mut open = Vec::new();
    for r in &ca_records {
        for f in &r.policy_findings {
            open.push(format!("{}: {f}", r.title));
        }
    }
    if ca_records.is_empty() {
        open.push("no collision-avoidance record in this pack".into());
    }
    match &identification {
        None => open.push("no identification evidence in this pack".into()),
        Some(i) if i.margin < 3.0 => open.push(format!(
            "identification margin {:.1} is small: the best two candidates are close",
            i.margin
        )),
        _ => {}
    }
    match &disposal {
        None => open.push("no disposal-plan check in this pack".into()),
        Some(d) if d.within_plan != Some(true) => open.push(format!("disposal: {}", d.verdict)),
        _ => {}
    }
    let verdict = format!(
        "Operator evidence pack for {}: {} collision-avoidance record(s), identification {}, disposal check {}; {} open point(s). Evidence organised for review; not a statement that any requirement is met.",
        q.object,
        ca_records.len(),
        if identification.is_some() { "included" } else { "absent" },
        if disposal.is_some() { "included" } else { "absent" },
        open.len()
    );
    Ok((
        Pack {
            format: PACK_FORMAT,
            title: q.title.clone(),
            operator: q.operator.clone(),
            object: q.object.clone(),
            framework_note: if q.framework_note.trim().is_empty() {
                "Framework article not stated by the operator (to confirm).".into()
            } else {
                format!(
                    "{} (as stated by the operator; to confirm)",
                    q.framework_note
                )
            },
            ca_records,
            identification,
            disposal,
            open_points: open,
            label: "MODELLED: computed from the operator's own data with open orbit tools",
            verdict,
        },
        inputs,
    ))
}

/// TLE checksum digit of a line's first 68 characters.
fn tle_checksum(l: &str) -> char {
    let s: u32 = l
        .chars()
        .take(68)
        .map(|c| {
            if c == '-' {
                1
            } else {
                c.to_digit(10).unwrap_or(0)
            }
        })
        .sum();
    char::from_digit(s % 10, 10).unwrap_or('0')
}

/// A TLE line from its first 68 characters, with the checksum appended.
pub fn tle_line(body: &str) -> String {
    let b = format!("{body:<68}");
    format!("{b}{}", tle_checksum(&b))
}

/// SYNTHETIC inputs for every record: `(file name, text)`.
pub fn synth() -> Result<Vec<(&'static str, String)>, String> {
    let l1 = tle_line("1 90001U 26001A   26320.50000000  .00001000  00000-0  50000-4 0  9990");
    let a2 = tle_line("2 90001  97.5000 120.0000 0010000  90.0000 270.0000 15.20000000  1000");
    let b1 = tle_line("1 90002U 26001B   26320.50000000  .00001000  00000-0  50000-4 0  9991");
    let b2 = tle_line("2 90002  97.5000 120.0000 0010000  90.0000 271.5000 15.20000000  1000");
    let c1 = tle_line("1 90003U 26001C   26320.50000000  .00001000  00000-0  50000-4 0  9992");
    let c2 = tle_line("2 90003  97.5000 121.0000 0010000  90.0000 270.0000 15.20000000  1000");
    let tle = kshana::tle::parse_tle(&l1, &a2)?;
    let s = tle.to_sgp4(kshana::sgp4::wgs72(), false);
    let mut eph = String::from("timestamp,x_km,y_km,z_km,frame,time_scale\n");
    let mut st: u64 = 5;
    for k in 0..60 {
        let tsince = 60.0 + k as f64;
        let (r, _) = s.propagate(tsince).map_err(|e| format!("SGP4 error {e}"))?;
        let jd = s.epoch_jd_utc() + tsince / 1440.0;
        let unix = ((jd - 2_440_587.5) * 86_400.0).round();
        let mut n = || {
            st = st
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((st >> 33) as f64 / (1u64 << 31) as f64 - 0.5) * 0.2
        };
        eph.push_str(&format!(
            "{},{:.4},{:.4},{:.4},TEME,UTC\n",
            crate::forensics::inputs::iso(unix),
            r[0] + n(),
            r[1] + n(),
            r[2] + n()
        ));
    }
    let cands = format!(
        "[[candidate]]\nname = \"SYNTH-A\"\nline1 = \"{l1}\"\nline2 = \"{a2}\"\n\n\
         [[candidate]]\nname = \"SYNTH-B\"\nline1 = \"{b1}\"\nline2 = \"{b2}\"\n\n\
         [[candidate]]\nname = \"SYNTH-C\"\nline1 = \"{c1}\"\nline2 = \"{c2}\"\n"
    );
    let cdm = |id: &str, created: &str, r: f64, pc: f64| {
        format!(
            "[[cdm]]\nmessage_id = \"{id}\"\ncreation_utc = \"{created}\"\ntca_utc = \"2026-11-18T06:00:00Z\"\n\
             rel_pos_rtn_m = [{r}, 0.0, 30.0]\nrel_vel_rtn_mps = [0.0, -14000.0, 1200.0]\n\
             cov1_rtn_m2 = [2500.0, 0.0, 250000.0, 0.0, 0.0, 900.0]\n\
             cov2_rtn_m2 = [40000.0, 0.0, 4000000.0, 0.0, 0.0, 10000.0]\ncov2_frame = \"rtn1\"\nhbr_m = 15.0\npc_reported = {pc:e}\n\n"
        )
    };
    let ca = format!(
        "title = \"SYNTHETIC conjunction\"\nprimary = \"SYNTH-A (90001)\"\nsecondary = \"DEBRIS (99999)\"\n\
         decision = {{ utc = \"2026-11-17T08:00:00Z\", action = \"maneuver\", rationale = \"Pc above the policy threshold and rising\", decided_by = \"Flight dynamics on call\" }}\n\
         post_maneuver_cdm = \"CDM-SYNTH-4\"\n\n\
         [policy]\npc_threshold = 1.0e-4\ndecide_by_h = 12.0\nsource = \"SYNTHETIC operations manual section 4\"\n\n{}{}{}{}",
        cdm("CDM-SYNTH-1", "2026-11-15T06:00:00Z", 1500.0, 1.0e-14),
        cdm("CDM-SYNTH-2", "2026-11-16T06:00:00Z", 600.0, 4.0e-5),
        cdm("CDM-SYNTH-3", "2026-11-17T06:00:00Z", 200.0, 1.2e-3),
        cdm("CDM-SYNTH-4", "2026-11-17T18:00:00Z", 3000.0, 1.0e-49),
    );
    let disp = "title = \"SYNTHETIC disposal plan\"\naltitude_km = 450.0\nmass_kg = 120.0\narea_m2 = 1.2\ncd = 2.2\n\
                plan_deadline_years = 5.0\nplan_source = \"SYNTHETIC mission disposal plan v1\"\n"
        .to_string();
    let pk = "title = \"SYNTHETIC operator evidence pack\"\noperator = \"SYNTHETIC operator\"\nobject = \"SYNTH-A (90001)\"\n\
              framework_note = \"Art. 15-style operator evidence\"\nca = [\"ca.toml\"]\nephemeris = \"ephemeris.csv\"\n\
              candidates = \"candidates.toml\"\ndisposal = \"disposal.toml\"\n"
        .to_string();
    Ok(vec![
        ("ca.toml", ca),
        ("ephemeris.csv", eph),
        ("candidates.toml", cands),
        ("disposal.toml", disp),
        (
            "disposal-sensitivity.toml",
            crate::disposal_sensitivity::synth(),
        ),
        ("pack.toml", pk),
    ])
}
