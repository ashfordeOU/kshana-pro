// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Time-distribution chain verifier (E19 B1904)
//!
//! Research 05 (#114, #149), 06 (H7), 07b (T10), 08 (C8), 09 (D7) and 12 (the national time
//! services of #1661 to #1762): operators that take time from a national laboratory over White
//! Rabbit, PTP or fibre, or from GNSS with a common-view link, need one statement of the chain
//! from UTC to their end point: every hop's bias and uncertainty, whether every calibration
//! was current on the date, and the laboratory's offset from UTC.
//!
//! * **Reference**: `UTC − UTC(k)` from the operator's transcription of BIPM Circular T rows
//!   (MJD, value, uncertainty), interpolated to the date; a date after the last row is
//!   extrapolated (held) for at most `max_extrapolation_days` with an added uncertainty per day,
//!   and marked provisional. A stated value is accepted too.
//! * **Hops**: `wr` (White Rabbit link model: round trip `δMM = Δtxm + Δrxm + Δtxs + Δrxs +
//!   δms + δsm`, fibre asymmetry `α = δms/δsm − 1`, so `δms = (1+α)/(2+α)·(δMM − ΣΔ)`); the
//!   bias of a configured `α` against the calibrated one, and the GUM components of `α`, the
//!   four fixed delays and the round trip; `α` can be **estimated** from a calibration
//!   measurement (the slave's PPS against the master's over a spool, with the `α` the link was
//!   configured with during the measurement) by [`wr_alpha_from_calibration`]); `ptp` (a
//!   stated asymmetry and the cTE of the equipment class as a rectangular bound), `fibre-twtt`,
//!   `gnss-cv` (a common-view comparison) and `stated`.
//! * **Traceability**: every hop needs a calibration record (id, date, validity) current on
//!   the statement date; the chain is `unbroken`, `provisional` (Circular T extrapolated) or
//!   `broken` (with the hops that break it).
//! * **Onboarding test** (a national time service): the delivered time against the operator's
//!   own reference over a window; its mean must agree with the chain's predicted offset within
//!   `k·sqrt(u_chain² + u_ref² + s²/n)`.
//!
//! The statement directory is signed ([`crate::signed_dossier`]); verify with
//! `kshana-pro dossier verify` (free). **Label: MODELLED**: the arithmetic of the stated
//! measurements and calibration records; nothing here measures a link.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::selfcheck::{self, SourceSpec};
use super::sig9;

pub const FEATURE: &str = "pro-timing-chain";
pub const FORMAT: &str = "kshana-pro-timing-chain/1";
pub const LABEL: &str = "MODELLED: the arithmetic of the stated measurements, calibration records and Circular T rows, combined by the GUM; nothing here measures a link. A traceability statement for the operator's own records, to be reviewed with the laboratory that calibrated the chain.";

/// cTE of the PTP equipment classes (ITU-T G.8273.2 as transcribed: class A 50 ns, B 20 ns,
/// C 10 ns, D not given for cTE), taken as a rectangular bound.
pub const PTP_CLASSES: [(&str, f64); 3] =
    [("class-a", 50e-9), ("class-b", 20e-9), ("class-c", 10e-9)];

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Calibration {
    pub id: String,
    /// `YYYY-MM-DD`.
    pub date: String,
    pub valid_days: i64,
    #[serde(default)]
    pub by: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixedDelays {
    pub tx_m: f64,
    pub rx_m: f64,
    pub tx_s: f64,
    pub rx_s: f64,
}

/// A White Rabbit asymmetry calibration measurement.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlphaCalibration {
    /// Round trip measured over the calibration fibre, s.
    pub round_trip_s: f64,
    pub u_round_trip_s: f64,
    pub fixed_delays_s: FixedDelays,
    /// Standard uncertainty of each fixed delay, s.
    pub u_fixed_s: f64,
    /// Time interval from the master's PPS to the slave's PPS measured during the
    /// calibration (positive: the slave's PPS is later), s.
    pub pps_offset_s: f64,
    pub u_pps_offset_s: f64,
    /// The α the link was configured with during the measurement.
    #[serde(default)]
    pub alpha_configured: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hop {
    pub name: String,
    /// `wr`, `ptp`, `fibre-twtt`, `gnss-cv` or `stated`.
    pub r#type: String,
    #[serde(default)]
    pub calibration: Option<Calibration>,
    // White Rabbit.
    #[serde(default)]
    pub round_trip_s: Option<f64>,
    #[serde(default)]
    pub u_round_trip_s: f64,
    #[serde(default)]
    pub fixed_delays_s: Option<FixedDelays>,
    #[serde(default)]
    pub u_fixed_s: f64,
    /// The calibrated α (or estimated from `alpha_calibration`).
    #[serde(default)]
    pub alpha: Option<f64>,
    #[serde(default)]
    pub u_alpha: f64,
    /// The α configured in the device (default: the calibrated one, no bias).
    #[serde(default)]
    pub alpha_configured: Option<f64>,
    #[serde(default)]
    pub alpha_calibration: Option<AlphaCalibration>,
    // PTP, fibre, GNSS common view, stated.
    #[serde(default)]
    pub te_class: Option<String>,
    /// Known uncorrected asymmetry (half the path difference), s: a bias.
    #[serde(default)]
    pub asymmetry_s: f64,
    #[serde(default)]
    pub u_asymmetry_s: f64,
    /// Measured or stated offset the hop adds (output minus input), s: a bias.
    #[serde(default)]
    pub offset_s: f64,
    #[serde(default)]
    pub u_s: f64,
    #[serde(default)]
    pub basis: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    /// The laboratory's time scale, e.g. `UTC(PTB)`.
    pub lab: String,
    /// CSV `mjd,utc_minus_utck_ns,u_ns` transcribed from Circular T, relative to the request.
    #[serde(default)]
    pub circular_t: Option<String>,
    #[serde(default)]
    pub utc_minus_utck_s: Option<f64>,
    #[serde(default)]
    pub u_s: Option<f64>,
    #[serde(default = "d_extra")]
    pub max_extrapolation_days: f64,
    /// Added standard uncertainty per day of extrapolation, s.
    #[serde(default = "d_extra_u")]
    pub extrapolation_u_s_per_day: f64,
}

fn d_extra() -> f64 {
    45.0
}
fn d_extra_u() -> f64 {
    0.5e-9
}
fn d_k() -> f64 {
    2.0
}

/// The onboarding test of a delivered time service.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Onboarding {
    /// Delivered time minus the operator's reference; any self-check format.
    pub format: String,
    pub path: String,
    /// Standard uncertainty of the operator's reference against UTC, s.
    pub u_ref_s: f64,
    #[serde(default)]
    pub sign: Option<f64>,
    #[serde(default)]
    pub select: Option<String>,
    /// Largest acceptable excursion about the mean, s.
    #[serde(default)]
    pub max_wander_s: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChainSpec {
    pub title: String,
    pub end_point: String,
    /// `YYYY-MM-DD`: the date the statement is for.
    pub date: String,
    #[serde(default = "d_k")]
    pub coverage_k: f64,
    #[serde(default)]
    pub limit_s: Option<f64>,
    pub reference: Reference,
    pub hops: Vec<Hop>,
    #[serde(default)]
    pub onboarding: Option<Onboarding>,
}

impl ChainSpec {
    pub fn parse(src: &str) -> Result<ChainSpec, String> {
        let s: ChainSpec = toml::from_str(src).map_err(|e| format!("chain: {e}"))?;
        if s.hops.is_empty() {
            return Err("chain: at least one hop".into());
        }
        if s.coverage_k.is_nan() || s.coverage_k <= 0.0 {
            return Err("chain: coverage_k must be positive".into());
        }
        Ok(s)
    }
}

/// One component of the budget.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Component {
    pub hop: String,
    pub name: String,
    /// `A` or `B`.
    pub kind: &'static str,
    pub u_s: f64,
    pub basis: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HopResult {
    pub name: String,
    pub r#type: String,
    pub bias_s: f64,
    pub u_s: f64,
    /// `current`, `expired`, `not-yet-valid`, `missing`.
    pub calibration: String,
    pub calibration_detail: String,
    /// For a WR hop: α used, its uncertainty, master-to-slave fibre delay and the fibre
    /// asymmetry `δms − δsm`.
    pub wr: Option<WrResult>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WrResult {
    pub alpha: f64,
    pub u_alpha: f64,
    pub alpha_source: String,
    pub delay_ms_s: f64,
    pub asymmetry_s: f64,
    pub u_asymmetry_s: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct OnboardingResult {
    pub n: usize,
    pub mean_s: f64,
    pub std_s: f64,
    pub max_excursion_s: f64,
    pub predicted_s: f64,
    pub bound_s: f64,
    /// `consistent`, `inconsistent`, `wander-exceeded`.
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChainStatement {
    pub format: &'static str,
    pub label: &'static str,
    pub title: String,
    pub end_point: String,
    pub date: String,
    pub utc: String,
    pub lab: String,
    pub utc_minus_utck_s: f64,
    pub u_reference_s: f64,
    pub reference_status: String,
    pub hops: Vec<HopResult>,
    pub budget: Vec<Component>,
    /// End point minus UTC: every hop's bias (output minus input, from UTC(k)) plus the lab's
    /// offset UTC(k) − UTC, which is minus the Circular T value `utc_minus_utck_s`.
    pub offset_from_utc_s: f64,
    pub u_c_s: f64,
    pub coverage_k: f64,
    pub expanded_s: f64,
    pub limit_s: Option<f64>,
    pub within_limit: Option<bool>,
    /// `unbroken`, `provisional`, `broken`.
    pub traceability: String,
    pub breaks: Vec<String>,
    pub onboarding: Option<OnboardingResult>,
    pub verdict: String,
}

/// Days since 1970-01-01 of `YYYY-MM-DD`.
fn days(s: &str) -> Result<i64, String> {
    super::timebomb::Label::parse_iso(s)
        .map(|l| l.days)
        .map_err(|e| format!("date '{s}': {e}"))
}

/// The WR master-to-slave one-way delay for `alpha`.
pub fn wr_delay_ms(round_trip: f64, d: &FixedDelays, alpha: f64) -> f64 {
    let fibre = round_trip - (d.tx_m + d.rx_m + d.tx_s + d.rx_s);
    d.tx_m + d.rx_s + (1.0 + alpha) / (2.0 + alpha) * fibre
}

/// α from a calibration measurement, with its standard uncertainty (finite-difference GUM
/// propagation over the round trip, the four fixed delays and the PPS offset).
pub fn wr_alpha_from_calibration(c: &AlphaCalibration) -> Result<(f64, f64), String> {
    let f = |rt: f64, d: &FixedDelays, pps: f64| -> f64 {
        let est = wr_delay_ms(rt, d, c.alpha_configured);
        // The slave's clock reads behind by (true − estimated) delay, so its PPS comes that
        // much later than the master's.
        let truth = est + pps;
        let fibre = rt - (d.tx_m + d.rx_m + d.tx_s + d.rx_s);
        let ms = truth - d.tx_m - d.rx_s;
        ms / (fibre - ms) - 1.0
    };
    let d0 = &c.fixed_delays_s;
    let fibre = c.round_trip_s - (d0.tx_m + d0.rx_m + d0.tx_s + d0.rx_s);
    if fibre.is_nan() || fibre <= 0.0 {
        return Err("alpha calibration: the round trip must exceed the fixed delays".into());
    }
    let a = f(c.round_trip_s, d0, c.pps_offset_s);
    let mut var = 0.0;
    let h = 1e-12;
    let mut add = |g: &dyn Fn(f64) -> f64, u: f64| {
        let dd = (g(h) - g(-h)) / (2.0 * h);
        var += (dd * u).powi(2);
    };
    add(
        &|e| f(c.round_trip_s + e, d0, c.pps_offset_s),
        c.u_round_trip_s,
    );
    add(
        &|e| f(c.round_trip_s, d0, c.pps_offset_s + e),
        c.u_pps_offset_s,
    );
    for which in 0..4 {
        add(
            &|e| {
                let mut d = d0.clone();
                match which {
                    0 => d.tx_m += e,
                    1 => d.rx_m += e,
                    2 => d.tx_s += e,
                    _ => d.rx_s += e,
                }
                f(c.round_trip_s, &d, c.pps_offset_s)
            },
            c.u_fixed_s,
        );
    }
    Ok((a, var.sqrt()))
}

fn cal_status(c: &Option<Calibration>, date: i64) -> Result<(String, String), String> {
    Ok(match c {
        None => ("missing".into(), "no calibration record".into()),
        Some(c) => {
            let d = days(&c.date)?;
            let until = d + c.valid_days;
            let by =
                c.by.as_deref()
                    .map(|b| format!(" by {b}"))
                    .unwrap_or_default();
            if date < d {
                (
                    "not-yet-valid".into(),
                    format!("{} of {}{by}: after the statement date", c.id, c.date),
                )
            } else if date > until {
                (
                    "expired".into(),
                    format!(
                        "{} of {}{by}: expired {} days before the statement date",
                        c.id,
                        c.date,
                        date - until
                    ),
                )
            } else {
                (
                    "current".into(),
                    format!(
                        "{} of {}{by}: valid for {} more days",
                        c.id,
                        c.date,
                        until - date
                    ),
                )
            }
        }
    })
}

/// Read Circular T rows `mjd,value_ns,u_ns`.
pub fn read_circular_t(text: &str) -> Result<Vec<(f64, f64, f64)>, String> {
    let mut v = Vec::new();
    for (i, l) in text.lines().enumerate() {
        let l = l.trim();
        if l.is_empty() || l.starts_with('#') || l.starts_with("mjd") {
            continue;
        }
        let c: Vec<f64> = l
            .split([',', ';'])
            .map(|x| x.trim().parse::<f64>())
            .collect::<Result<_, _>>()
            .map_err(|_| format!("circular T line {}: expected mjd,value_ns,u_ns", i + 1))?;
        if c.len() < 3 {
            return Err(format!(
                "circular T line {}: expected mjd,value_ns,u_ns",
                i + 1
            ));
        }
        v.push((c[0], c[1] * 1e-9, c[2] * 1e-9));
    }
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    if v.is_empty() {
        return Err("circular T: no rows".into());
    }
    Ok(v)
}

/// The reference at `mjd`: (value, u, status, detail).
pub fn reference_at(
    r: &Reference,
    rows: Option<&[(f64, f64, f64)]>,
    mjd: f64,
) -> Result<(f64, f64, String, String), String> {
    if let Some(rows) = rows {
        let first = rows[0];
        let last = rows[rows.len() - 1];
        if mjd < first.0 {
            return Err(format!(
                "the statement date (MJD {mjd}) is before the first Circular T row (MJD {})",
                first.0
            ));
        }
        if mjd <= last.0 {
            let i = rows.iter().position(|x| x.0 >= mjd).unwrap_or(0);
            let (v, u) = if rows[i].0 == mjd || i == 0 {
                (rows[i].1, rows[i].2)
            } else {
                let (a, b) = (rows[i - 1], rows[i]);
                let w = (mjd - a.0) / (b.0 - a.0);
                (a.1 + w * (b.1 - a.1), a.2.max(b.2))
            };
            return Ok((
                v,
                u,
                "published".into(),
                format!(
                    "interpolated from Circular T rows (MJD {} to {})",
                    first.0, last.0
                ),
            ));
        }
        let gap = mjd - last.0;
        if gap > r.max_extrapolation_days {
            return Ok((
                last.1,
                last.2,
                "missing".into(),
                format!(
                    "the last Circular T row (MJD {}) is {gap:.0} days before the statement date, beyond the {} allowed",
                    last.0, r.max_extrapolation_days
                ),
            ));
        }
        let u = (last.2.powi(2) + (gap * r.extrapolation_u_s_per_day).powi(2)).sqrt();
        return Ok((
            last.1,
            u,
            "extrapolated".into(),
            format!(
                "held from the last Circular T row (MJD {}) for {gap:.0} days, uncertainty grown by {:e} s per day",
                last.0, r.extrapolation_u_s_per_day
            ),
        ));
    }
    match (r.utc_minus_utck_s, r.u_s) {
        (Some(v), Some(u)) => Ok((v, u, "stated".into(), "stated in the request".into())),
        _ => Err("reference: give circular_t, or utc_minus_utck_s and u_s".into()),
    }
}

/// Assess the chain. `base` resolves the request's relative paths.
pub fn assess(spec: &ChainSpec, base: &Path, utc: &str) -> Result<ChainStatement, String> {
    let date = days(&spec.date)?;
    let mjd = date as f64 + 40_587.0;
    let rows = match &spec.reference.circular_t {
        Some(p) => {
            let path = base.join(p);
            let t =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            Some(read_circular_t(&t)?)
        }
        None => None,
    };
    let (rv, ru, rstat, rdetail) = reference_at(&spec.reference, rows.as_deref(), mjd)?;
    let mut budget = vec![Component {
        hop: spec.reference.lab.clone(),
        name: "UTC − UTC(k)".into(),
        kind: "B",
        u_s: sig9(ru),
        basis: rdetail.clone(),
    }];
    let mut hops = Vec::new();
    let mut breaks = Vec::new();
    if rstat == "missing" {
        breaks.push(format!("reference {}: {rdetail}", spec.reference.lab));
    }
    let mut names = std::collections::BTreeSet::new();
    for h in &spec.hops {
        if !names.insert(h.name.as_str()) {
            return Err(format!("hop name '{}' is repeated", h.name));
        }
        let (cs, cd) = cal_status(&h.calibration, date)?;
        if cs != "current" {
            breaks.push(format!("hop {}: calibration {cs} ({cd})", h.name));
        }
        let mut comps: Vec<Component> = Vec::new();
        let mut comp = |name: &str, kind: &'static str, u: f64, basis: String| {
            if u > 0.0 {
                comps.push(Component {
                    hop: h.name.clone(),
                    name: name.into(),
                    kind,
                    u_s: sig9(u),
                    basis,
                });
            }
        };
        let mut bias = h.offset_s;
        let mut wr = None;
        match h.r#type.as_str() {
            "wr" => {
                let (Some(rt), Some(d)) = (h.round_trip_s, &h.fixed_delays_s) else {
                    return Err(format!(
                        "hop {}: a wr hop needs round_trip_s and fixed_delays_s",
                        h.name
                    ));
                };
                let (alpha, ua, src) = match (&h.alpha_calibration, h.alpha) {
                    (Some(c), _) => {
                        let (a, u) = wr_alpha_from_calibration(c)?;
                        (
                            a,
                            u,
                            "estimated from the calibration measurement".to_string(),
                        )
                    }
                    (None, Some(a)) => (a, h.u_alpha, "stated".to_string()),
                    (None, None) => {
                        return Err(format!(
                            "hop {}: a wr hop needs alpha or alpha_calibration",
                            h.name
                        ))
                    }
                };
                let fibre = rt - (d.tx_m + d.rx_m + d.tx_s + d.rx_s);
                if fibre.is_nan() || fibre <= 0.0 {
                    return Err(format!(
                        "hop {}: the round trip must exceed the fixed delays",
                        h.name
                    ));
                }
                let cfg = h.alpha_configured.unwrap_or(alpha);
                // The slave's clock reads behind by (true − configured estimate) delay.
                bias += wr_delay_ms(rt, d, cfg) - wr_delay_ms(rt, d, alpha);
                let g = 2.0 + alpha;
                let da = fibre / (g * g);
                comp(
                    "fibre asymmetry α",
                    "B",
                    da * ua,
                    format!("u(α) = {ua:.3e} ({src}) × δfibre/(2+α)²"),
                );
                comp(
                    "fixed delays (Δtxm, Δrxs)",
                    "B",
                    (2.0f64).sqrt() * h.u_fixed_s / g,
                    "two delays entering with 1/(2+α)".into(),
                );
                comp(
                    "fixed delays (Δtxs, Δrxm)",
                    "B",
                    (2.0f64).sqrt() * h.u_fixed_s * (1.0 + alpha) / g,
                    "two delays entering with (1+α)/(2+α)".into(),
                );
                comp(
                    "round trip δMM",
                    "A",
                    h.u_round_trip_s * (1.0 + alpha) / g,
                    "measured round trip".into(),
                );
                let sm = fibre / g;
                wr = Some(WrResult {
                    alpha: sig9(alpha),
                    u_alpha: sig9(ua),
                    alpha_source: src,
                    delay_ms_s: sig9(wr_delay_ms(rt, d, alpha)),
                    asymmetry_s: sig9(alpha * sm),
                    u_asymmetry_s: sig9(da * ua * 2.0),
                });
            }
            "ptp" => {
                if let Some(c) = &h.te_class {
                    let (_, v) = PTP_CLASSES.iter().find(|x| x.0 == c).ok_or_else(|| {
                        format!("hop {}: te_class one of class-a, class-b, class-c", h.name)
                    })?;
                    comp(
                        "cTE of the equipment class",
                        "B",
                        v / 3f64.sqrt(),
                        format!("±{v:e} s rectangular (ITU-T G.8273.2 {c}, as transcribed)"),
                    );
                }
                bias += h.asymmetry_s;
                comp("link asymmetry", "B", h.u_asymmetry_s, "stated".into());
            }
            "fibre-twtt" => {
                bias += h.asymmetry_s;
                comp("fibre asymmetry", "B", h.u_asymmetry_s, "stated".into());
            }
            "gnss-cv" | "stated" => {}
            t => {
                return Err(format!(
                    "hop {}: type '{t}' is not wr, ptp, fibre-twtt, gnss-cv or stated",
                    h.name
                ))
            }
        }
        comp(
            "hop",
            if h.r#type == "gnss-cv" { "A" } else { "B" },
            h.u_s,
            h.basis.clone().unwrap_or_else(|| "stated".into()),
        );
        let u = comps.iter().map(|c| c.u_s * c.u_s).sum::<f64>().sqrt();
        budget.extend(comps);
        hops.push(HopResult {
            name: h.name.clone(),
            r#type: h.r#type.clone(),
            bias_s: sig9(bias),
            u_s: sig9(u),
            calibration: cs,
            calibration_detail: cd,
            wr,
        });
    }
    // End − UTC = (End − UTC(k)) + (UTC(k) − UTC) = Σ hop biases − (UTC − UTC(k)).
    let offset = hops.iter().map(|h| h.bias_s).sum::<f64>() - rv;
    let uc = budget.iter().map(|c| c.u_s * c.u_s).sum::<f64>().sqrt();
    let big_u = spec.coverage_k * uc;
    let within = spec.limit_s.map(|l| offset.abs() + big_u <= l);
    let onboarding = match &spec.onboarding {
        None => None,
        Some(o) => {
            let p = base.join(&o.path);
            let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            let rs = selfcheck::read_source(
                &SourceSpec {
                    name: "onboarding".into(),
                    kind: None,
                    format: o.format.clone(),
                    path: o.path.clone(),
                    u_s: 1.0,
                    disciplining: false,
                    sign: o.sign,
                    time_offset_s: 0.0,
                    select: o.select.clone(),
                },
                &text,
            )?;
            let n = rs.len();
            let mean = rs.iter().map(|r| r.offset).sum::<f64>() / n as f64;
            let sd = if n > 1 {
                (rs.iter().map(|r| (r.offset - mean).powi(2)).sum::<f64>() / (n - 1) as f64).sqrt()
            } else {
                0.0
            };
            let exc = rs
                .iter()
                .map(|r| (r.offset - mean).abs())
                .fold(0.0, f64::max);
            // The delivered time minus the operator's reference is predicted to be the chain's
            // offset from UTC (the reference taken as UTC within u_ref).
            let bound =
                spec.coverage_k * (uc * uc + o.u_ref_s * o.u_ref_s + sd * sd / n as f64).sqrt();
            let status = if (mean - offset).abs() > bound {
                "inconsistent"
            } else if o.max_wander_s.is_some_and(|w| exc > w) {
                "wander-exceeded"
            } else {
                "consistent"
            };
            Some(OnboardingResult {
                n,
                mean_s: sig9(mean),
                std_s: sig9(sd),
                max_excursion_s: sig9(exc),
                predicted_s: sig9(offset),
                bound_s: sig9(bound),
                status: status.into(),
            })
        }
    };
    let traceability = if !breaks.is_empty() {
        "broken"
    } else if rstat == "extrapolated" {
        "provisional"
    } else {
        "unbroken"
    };
    let h = super::fleet::human_time;
    let mut verdict = format!(
        "{}: end point minus UTC {} ± {} (k = {}), traceability {}",
        spec.end_point,
        h(offset),
        h(big_u),
        spec.coverage_k,
        traceability
    );
    if let (Some(l), Some(w)) = (spec.limit_s, within) {
        verdict.push_str(&format!(
            "; |offset| + U {} the limit {}",
            if w { "within" } else { "exceeds" },
            h(l)
        ));
    }
    if let Some(o) = &onboarding {
        verdict.push_str(&format!("; onboarding test {}", o.status));
    }
    Ok(ChainStatement {
        format: FORMAT,
        label: LABEL,
        title: spec.title.clone(),
        end_point: spec.end_point.clone(),
        date: spec.date.clone(),
        utc: utc.into(),
        lab: spec.reference.lab.clone(),
        utc_minus_utck_s: sig9(rv),
        u_reference_s: sig9(ru),
        reference_status: rstat,
        hops,
        budget,
        offset_from_utc_s: sig9(offset),
        u_c_s: sig9(uc),
        coverage_k: spec.coverage_k,
        expanded_s: sig9(big_u),
        limit_s: spec.limit_s,
        within_limit: within,
        traceability: traceability.into(),
        breaks,
        onboarding,
        verdict,
    })
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The statement as HTML.
pub fn html(r: &ChainStatement) -> String {
    let h = super::fleet::human_time;
    let mut o = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Traceability statement: {}</title>\
<style>body{{font-family:system-ui,sans-serif;max-width:60rem;margin:2rem auto;padding:0 1rem;color:#111;background:#fff}}\
table{{border-collapse:collapse;width:100%}}td,th{{border:1px solid #ccc;padding:.3rem;text-align:left}}small{{color:#555}}</style></head><body>\
<h1>Traceability statement: {}</h1><p>{}</p><p><strong>{}</strong></p><p>{} · date {} · reference {} ({}, {})</p>",
        esc(&r.end_point),
        esc(&r.title),
        esc(r.label),
        esc(&r.verdict),
        esc(&r.end_point),
        esc(&r.date),
        esc(&r.lab),
        h(r.utc_minus_utck_s),
        esc(&r.reference_status)
    );
    o.push_str("<h2>Chain</h2><table><tr><th>Hop</th><th>Type</th><th>Bias</th><th>u</th><th>Calibration</th></tr>");
    for x in &r.hops {
        o.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{} <small>{}</small></td></tr>",
            esc(&x.name),
            esc(&x.r#type),
            h(x.bias_s),
            h(x.u_s),
            esc(&x.calibration),
            esc(&x.calibration_detail)
        ));
    }
    o.push_str("</table><h2>Uncertainty budget</h2><table><tr><th>Hop</th><th>Component</th><th>Type</th><th>u</th><th>Basis</th></tr>");
    for c in &r.budget {
        o.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td><small>{}</small></td></tr>",
            esc(&c.hop),
            esc(&c.name),
            c.kind,
            h(c.u_s),
            esc(&c.basis)
        ));
    }
    o.push_str(&format!(
        "</table><p>u<sub>c</sub> {} · U = {}·u<sub>c</sub> {}</p>",
        h(r.u_c_s),
        r.coverage_k,
        h(r.expanded_s)
    ));
    if !r.breaks.is_empty() {
        o.push_str("<h2>Breaks in the chain</h2><ul>");
        for b in &r.breaks {
            o.push_str(&format!("<li>{}</li>", esc(b)));
        }
        o.push_str("</ul>");
    }
    o.push_str("</body></html>\n");
    o
}

/// Write the statement directory (sign it afterwards).
pub fn write_dir(
    out: &Path,
    r: &ChainStatement,
    src: &str,
    spec: &ChainSpec,
    base: &Path,
) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", out.display());
    std::fs::create_dir_all(out.join("inputs")).map_err(io)?;
    std::fs::write(
        out.join("statement.json"),
        serde_json::to_string_pretty(r).expect("serialises"),
    )
    .map_err(io)?;
    std::fs::write(out.join("statement.html"), html(r)).map_err(io)?;
    let mut b = String::from("hop,component,type,u_s,basis\n");
    for c in &r.budget {
        b.push_str(&format!(
            "\"{}\",\"{}\",{},{:e},\"{}\"\n",
            c.hop.replace('"', "'"),
            c.name.replace('"', "'"),
            c.kind,
            c.u_s,
            c.basis.replace('"', "'")
        ));
    }
    std::fs::write(out.join("budget.csv"), b).map_err(io)?;
    std::fs::write(out.join("inputs").join("chain.toml"), src).map_err(io)?;
    let mut extra: Vec<&str> = Vec::new();
    if let Some(p) = &spec.reference.circular_t {
        extra.push(p);
    }
    if let Some(o) = &spec.onboarding {
        extra.push(&o.path);
    }
    for p in extra {
        let name = Path::new(p)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("input");
        std::fs::copy(base.join(p), out.join("inputs").join(name))
            .map_err(|e| format!("{p}: {e}"))?;
    }
    Ok(())
}

/// A request template.
pub const TEMPLATE: &str = r#"title = "Exchange time feed"
end_point = "PoP-1 PTP output"
date = "2026-11-15"
coverage_k = 2
limit_s = 100e-9

[reference]
lab = "UTC(k)"
circular_t = "circular-t.csv"   # mjd,utc_minus_utck_ns,u_ns as transcribed from BIPM Circular T

[[hops]]
name = "lab to PoP White Rabbit"
type = "wr"
round_trip_s = 490.2e-6
u_round_trip_s = 10e-12
fixed_delays_s = { tx_m = 300e-9, rx_m = 250e-9, tx_s = 310e-9, rx_s = 260e-9 }
u_fixed_s = 0.1e-9
alpha_configured = 2.6e-4
calibration = { id = "CAL-WR-01", date = "2026-07-01", valid_days = 365, by = "the laboratory" }
[hops.alpha_calibration]          # or: alpha = ..., u_alpha = ...
round_trip_s = 50.0e-6
u_round_trip_s = 10e-12
fixed_delays_s = { tx_m = 300e-9, rx_m = 250e-9, tx_s = 310e-9, rx_s = 260e-9 }
u_fixed_s = 0.1e-9
pps_offset_s = 3.2e-9
u_pps_offset_s = 20e-12
alpha_configured = 0.0

[[hops]]
name = "PoP boundary clock"
type = "ptp"
te_class = "class-b"
asymmetry_s = 0
u_asymmetry_s = 2e-9
calibration = { id = "CAL-BC-01", date = "2026-09-01", valid_days = 365 }
"#;
