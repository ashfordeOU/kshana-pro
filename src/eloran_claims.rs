// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # eLoran timing claims, interoperability and safety-of-life evidence (B3702)
//!
//! Research 2026-11-20 gaps, E37 B3702 (07b T11, T12, R13):
//!
//! * [`verify_claim`]: does a measured series support a timing claim such as "within 20 ns of
//!   UTC"? Readings of the receiver's 1PPS against a local UTC(k) realisation are analysed with
//!   the eLoran crate's autocorrelation-aware statistics
//!   ([`kshana_eloran::sync::measure::analyse_offsets`]); the offset UTC(k) − UTC comes from
//!   BIPM Circular T (interpolated between its five-day points, with the stated interpolation
//!   uncertainty) or is entered; every other correction is a GUM Type B entry. Two decisions,
//!   guard-banded with the expanded uncertainty `U = k·u_c`: the **mean offset**
//!   (`|mean| + U ≤ limit` supported, `|mean| − U > limit` not supported, otherwise
//!   inconclusive) and the **instantaneous** claim on the 95th percentile of `|x_i|` with the
//!   Type B part of `U` (each reading carries its own noise, so the Type A term of the mean
//!   does not apply to it).
//! * [`interop`]: a European eLoran Alliance style interoperability test. Stations steered to
//!   different UTC(k) (for example UTC(OP) and UTC(NPL)) each give the receiver clock against
//!   UTC once their laboratory offset and published steering residual are applied; every pair
//!   is compared with the normalised error `E_n = |d| / U` (ISO/IEC 17043 and ISO 13528 use
//!   the same statistic; `|E_n| ≤ 1` is consistent), where a laboratory offset shared by both
//!   stations cancels and is left out of `U`. A weighted mean and its Birge ratio summarise.
//! * [`sol_index`]: a safety-of-life evidence index. The UK National PNT Office guideline asks
//!   for "a clear pathway to certification for safety of life services" and publishes no
//!   checklist, so the categories here are **Kshana's own** (named in [`SOL_CATEGORIES`]),
//!   not those of any certification standard: each category lists the supplied artefacts by
//!   SHA-256 with the headline of any Kshana report, or states that no evidence was supplied.
//!   The CLI copies the artefacts and signs the directory.
//!
//! Evidence: **MODELLED** for the tests' synthetic series; with real readings the result is
//! as good as the counter calibration and the budget entries the user states.

use std::collections::BTreeMap;
use std::path::Path;

use kshana_eloran::hw::calib::TicUnit;
use kshana_eloran::sync::measure::{analyse_offsets, MeasureConfig, TicSeries};
use kshana_eloran::timeref::circular_t::CircularT;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const CLAIM_FORMAT: &str = "kshana-pro-eloran-claim/1";
pub const INTEROP_FORMAT: &str = "kshana-pro-eloran-interop/1";
pub const SOL_FORMAT: &str = "kshana-pro-eloran-sol-evidence/1";

/// One GUM budget entry (a correction with its uncertainty).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetEntry {
    pub name: String,
    /// Correction added to `receiver − UTC` (ns).
    #[serde(default)]
    pub value_ns: f64,
    /// Standard uncertainty (ns) for `normal`; half-width (ns) for `rectangular`.
    pub u_ns: f64,
    /// `normal` (default) or `rectangular` (u = a/√3).
    #[serde(default)]
    pub distribution: Option<String>,
}

impl BudgetEntry {
    fn standard_u(&self) -> Result<f64, String> {
        match self.distribution.as_deref().unwrap_or("normal") {
            "normal" => Ok(self.u_ns),
            "rectangular" => Ok(self.u_ns / 3f64.sqrt()),
            d => Err(format!("budget {}: unknown distribution {d:?}", self.name)),
        }
    }
}

/// UTC(k) − UTC: from Circular T or entered.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct RefOffset {
    /// Circular T text file (relative to the request).
    pub circular_t: Option<String>,
    /// Laboratory code in Circular T (e.g. `NPL`, `OP`).
    pub lab: Option<String>,
    /// MJD of the measurement (midpoint).
    pub mjd: Option<f64>,
    /// Standard uncertainty of interpolating between the five-day points (ns).
    pub u_interp_ns: f64,
    /// Entered value (ns) and standard uncertainty when no circular is given.
    pub value_ns: Option<f64>,
    pub u_ns: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimDef {
    pub statement: String,
    pub limit_ns: f64,
    #[serde(default = "d_k")]
    pub coverage_k: f64,
}

fn d_k() -> f64 {
    2.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Readings {
    /// CSV file of readings (relative to the request).
    pub file: String,
    #[serde(default)]
    pub column: Option<usize>,
    /// `ns` (default) or `s`.
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default = "d_tau")]
    pub tau0_s: f64,
    /// `rx_minus_ref` (default: start = receiver 1PPS) or `ref_minus_rx`.
    #[serde(default)]
    pub sign: Option<String>,
    /// The local reference, e.g. `UTC(NPL)`.
    #[serde(default)]
    pub reference: String,
}

fn d_tau() -> f64 {
    1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimRequest {
    pub claim: ClaimDef,
    pub readings: Readings,
    #[serde(default)]
    pub reference_offset: RefOffset,
    #[serde(default)]
    pub budget: Vec<BudgetEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimDecision {
    Supported,
    NotSupported,
    Inconclusive,
}

fn decide(value: f64, u: f64, limit: f64) -> ClaimDecision {
    if value.abs() + u <= limit {
        ClaimDecision::Supported
    } else if value.abs() - u > limit {
        ClaimDecision::NotSupported
    } else {
        ClaimDecision::Inconclusive
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BudgetLine {
    pub name: String,
    pub kind: String,
    pub value_ns: f64,
    pub u_ns: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClaimReport {
    pub format: String,
    pub statement: String,
    pub limit_ns: f64,
    pub reference: String,
    pub readings: usize,
    pub duration_s: f64,
    pub readings_sha256: String,
    /// Mean of `receiver − UTC(k)` (ns), before corrections.
    pub mean_rx_minus_ref_ns: f64,
    pub std_ns: f64,
    pub n_effective: f64,
    /// UTC(k) − UTC applied (ns) and where it came from.
    pub ref_offset_ns: f64,
    pub ref_offset_source: String,
    /// `receiver − UTC` (ns): mean plus every correction.
    pub offset_ns: f64,
    pub budget: Vec<BudgetLine>,
    pub u_a_ns: f64,
    pub u_b_ns: f64,
    pub u_c_ns: f64,
    pub coverage_k: f64,
    pub expanded_u_ns: f64,
    pub mean_decision: ClaimDecision,
    /// 95th percentile of `|x_i|` after corrections (ns), with `k·u_B` as its guard band.
    pub p95_abs_ns: f64,
    pub max_abs_ns: f64,
    pub instantaneous_decision: ClaimDecision,
    pub tdev_ns: Vec<(f64, f64)>,
    pub statement_of_result: String,
    pub evidence: String,
}

fn sha256_hex(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

/// UTC(k) − UTC at `mjd` from a parsed circular, by linear interpolation; returns
/// `(value, u, source text)`.
pub fn ref_offset_from_circular(
    c: &CircularT,
    lab: &str,
    mjd: f64,
    u_interp_ns: f64,
) -> Result<(f64, f64, String), String> {
    let pts = c.points(lab)?;
    if pts.is_empty() {
        return Err(format!("Circular T: no values for {lab}"));
    }
    let (first, last) = (pts[0].0, pts[pts.len() - 1].0);
    if mjd < first || mjd > last {
        return Err(format!(
            "MJD {mjd} is outside the circular's span {first}..{last} for {lab}: no extrapolation"
        ));
    }
    let i = pts.iter().position(|p| p.0 >= mjd).unwrap();
    let v = if pts[i].0 == mjd || i == 0 {
        pts[i].1
    } else {
        let (a, b) = (pts[i - 1], pts[i]);
        a.1 + (b.1 - a.1) * (mjd - a.0) / (b.0 - a.0)
    };
    let u_lab = c
        .labs
        .get(lab)
        .and_then(|r| r.u_ns)
        .ok_or(format!("Circular T: no uncertainty u for {lab}"))?;
    let u = (u_lab * u_lab + u_interp_ns * u_interp_ns).sqrt();
    let src = format!(
        "Circular T{} UTC({lab}) − UTC at MJD {mjd} (linear between five-day values; u = {u_lab} ns from the circular, interpolation {u_interp_ns} ns)",
        c.number.map(|n| format!(" {n}")).unwrap_or_default()
    );
    Ok((v, u, src))
}

fn resolve_ref(r: &RefOffset, base: &Path) -> Result<(f64, f64, String), String> {
    match (&r.circular_t, &r.lab, r.mjd) {
        (Some(f), Some(lab), Some(mjd)) => {
            let p = base.join(f);
            let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            let c = CircularT::parse(&text)?;
            ref_offset_from_circular(&c, lab, mjd, r.u_interp_ns)
        }
        (Some(_), _, _) => Err("reference_offset: circular_t needs lab and mjd".into()),
        _ => match (r.value_ns, r.u_ns) {
            (Some(v), Some(u)) => Ok((v, u, "entered by the user".into())),
            (None, None) => Ok((
                0.0,
                0.0,
                "none: the reference is taken as UTC itself (no UTC(k) − UTC term)".into(),
            )),
            _ => Err("reference_offset: give value_ns with u_ns".into()),
        },
    }
}

/// Verify a timing claim from a request file's directory.
pub fn verify_claim(req: &ClaimRequest, base: &Path) -> Result<ClaimReport, String> {
    let c = &req.claim;
    if !(c.limit_ns > 0.0 && c.coverage_k > 0.0) {
        return Err("claim: limit_ns and coverage_k must be > 0".into());
    }
    let p = base.join(&req.readings.file);
    let text = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let unit = match req.readings.unit.as_deref().unwrap_or("ns") {
        "ns" => TicUnit::Nanoseconds,
        "s" => TicUnit::Seconds,
        u => return Err(format!("readings.unit {u:?}: ns or s")),
    };
    let series = TicSeries::from_csv(&String::from_utf8_lossy(&text), req.readings.column, unit)?;
    let sign = match req.readings.sign.as_deref().unwrap_or("rx_minus_ref") {
        "rx_minus_ref" => 1.0,
        "ref_minus_rx" => -1.0,
        s => return Err(format!("readings.sign {s:?}: rx_minus_ref or ref_minus_rx")),
    };
    let x: Vec<f64> = series.readings_ns.iter().map(|v| sign * v).collect();
    let cfg = MeasureConfig {
        tau0_s: req.readings.tau0_s,
        offset_limit_ns: c.limit_ns,
        tdev_limit_ns: f64::INFINITY,
        ..Default::default()
    };
    let m = analyse_offsets(&x, &cfg)?;
    let (ref_v, ref_u, ref_src) = resolve_ref(&req.reference_offset, base)?;
    let mut lines = vec![BudgetLine {
        name: "repeatability of the mean (Type A, autocorrelation-aware)".into(),
        kind: "A".into(),
        value_ns: 0.0,
        u_ns: m.correction.u_a_ns,
    }];
    if ref_u > 0.0 || ref_v != 0.0 {
        lines.push(BudgetLine {
            name: if req.readings.reference.trim().is_empty() {
                "UTC(k) − UTC".into()
            } else {
                format!("{} − UTC", req.readings.reference.trim())
            },
            kind: "B".into(),
            value_ns: ref_v,
            u_ns: ref_u,
        });
    }
    let mut corr = ref_v;
    for b in &req.budget {
        let u = b.standard_u()?;
        if u.is_nan() || u < 0.0 {
            return Err(format!("budget {}: u_ns must be ≥ 0", b.name));
        }
        corr += b.value_ns;
        lines.push(BudgetLine {
            name: b.name.clone(),
            kind: "B".into(),
            value_ns: b.value_ns,
            u_ns: u,
        });
    }
    let u_b = lines
        .iter()
        .filter(|l| l.kind == "B")
        .map(|l| l.u_ns * l.u_ns)
        .sum::<f64>()
        .sqrt();
    let u_a = m.correction.u_a_ns;
    let u_c = (u_a * u_a + u_b * u_b).sqrt();
    let big_u = c.coverage_k * u_c;
    let offset = m.mean_ns + corr;
    let mut abs: Vec<f64> = x.iter().map(|v| (v + corr).abs()).collect();
    abs.sort_by(f64::total_cmp);
    let p95 = kshana_eloran::stats::quantile_sorted(&abs, 0.95).unwrap_or(f64::NAN);
    let max_abs = *abs.last().unwrap_or(&f64::NAN);
    let mean_decision = decide(offset, big_u, c.limit_ns);
    let inst = decide(p95, c.coverage_k * u_b, c.limit_ns);
    let word = |d: ClaimDecision| match d {
        ClaimDecision::Supported => "supported by this evidence",
        ClaimDecision::NotSupported => "not supported by this evidence",
        ClaimDecision::Inconclusive => "inconclusive (the uncertainty straddles the limit)",
    };
    Ok(ClaimReport {
        format: CLAIM_FORMAT.into(),
        statement: c.statement.clone(),
        limit_ns: c.limit_ns,
        reference: req.readings.reference.clone(),
        readings: x.len(),
        duration_s: m.duration_s,
        readings_sha256: sha256_hex(&text),
        mean_rx_minus_ref_ns: m.mean_ns,
        std_ns: m.std_ns,
        n_effective: m.correction.n_effective,
        ref_offset_ns: ref_v,
        ref_offset_source: ref_src,
        offset_ns: offset,
        budget: lines,
        u_a_ns: u_a,
        u_b_ns: u_b,
        u_c_ns: u_c,
        coverage_k: c.coverage_k,
        expanded_u_ns: big_u,
        mean_decision,
        p95_abs_ns: p95,
        max_abs_ns: max_abs,
        instantaneous_decision: inst,
        tdev_ns: m.tdev.iter().map(|p| (p.tau_s, p.value_ns)).collect(),
        statement_of_result: format!(
            "receiver − UTC = {offset:.2} ns ± {big_u:.2} ns (k = {}); mean-offset claim {}; 95 % of instants {}",
            c.coverage_k,
            word(mean_decision),
            word(inst)
        ),
        evidence: "MODELLED statistics (eLoran crate sync analysis); GUM budget as stated by the user; Circular T values as published".into(),
    })
}

// ------------------------------------------------------------------ interoperability

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteropStation {
    pub id: String,
    /// The station's time reference as a Circular T laboratory code (e.g. `NPL`, `OP`).
    pub time_ref: String,
    /// Station time scale minus receiver clock after the propagation correction (ns).
    pub ref_minus_rx_ns: f64,
    pub u_meas_ns: f64,
    /// Residual propagation (ASF) uncertainty of this path (ns).
    #[serde(default)]
    pub u_prop_ns: f64,
    /// Published steering residual: station time − UTC(k) (ns).
    #[serde(default)]
    pub steering_ns: f64,
    #[serde(default)]
    pub u_steer_ns: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteropRequest {
    #[serde(rename = "station")]
    pub stations: Vec<InteropStation>,
    #[serde(default = "d_k")]
    pub coverage_k: f64,
    /// Optional tolerance on each pair difference (ns).
    #[serde(default)]
    pub tolerance_ns: Option<f64>,
    /// Circular T file (relative) with the MJD; or `[lab_offsets]` entered.
    #[serde(default)]
    pub circular_t: Option<String>,
    #[serde(default)]
    pub mjd: Option<f64>,
    #[serde(default)]
    pub u_interp_ns: f64,
    /// Entered UTC(k) − UTC per lab: `{ NPL = [value_ns, u_ns] }`.
    #[serde(default)]
    pub lab_offsets: BTreeMap<String, [f64; 2]>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StationEstimate {
    pub id: String,
    pub time_ref: String,
    /// `receiver − UTC` from this station (ns).
    pub rx_minus_utc_ns: f64,
    /// Without the laboratory term (which cancels between stations of one lab).
    pub u_own_ns: f64,
    pub u_lab_ns: f64,
    pub lab_offset_ns: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PairResult {
    pub a: String,
    pub b: String,
    pub same_time_ref: bool,
    pub difference_ns: f64,
    pub expanded_u_ns: f64,
    pub e_n: f64,
    pub consistent: bool,
    /// `within`, `outside` or `inconclusive` against the tolerance, when given.
    pub tolerance: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InteropReport {
    pub format: String,
    pub stations: Vec<StationEstimate>,
    pub pairs: Vec<PairResult>,
    pub weighted_mean_ns: f64,
    pub u_weighted_mean_ns: f64,
    /// √(χ²/(n−1)); near 1 when the stated uncertainties explain the spread.
    pub birge_ratio: Option<f64>,
    pub consistent_pairs: usize,
    pub lab_offset_source: String,
    pub evidence: String,
}

pub fn interop(req: &InteropRequest, base: &Path) -> Result<InteropReport, String> {
    if req.stations.len() < 2 {
        return Err("interop: at least two stations".into());
    }
    let circ = match &req.circular_t {
        Some(f) => {
            let p = base.join(f);
            let t = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            Some(CircularT::parse(&t)?)
        }
        None => None,
    };
    let mut src = String::new();
    let mut est = vec![];
    for s in &req.stations {
        let (lab_v, lab_u) = if let Some([v, u]) = req.lab_offsets.get(&s.time_ref) {
            src = "entered by the user".into();
            (*v, *u)
        } else if let Some(c) = &circ {
            let mjd = req.mjd.ok_or("interop: circular_t needs mjd")?;
            let (v, u, t) = ref_offset_from_circular(c, &s.time_ref, mjd, req.u_interp_ns)?;
            src = t;
            (v, u)
        } else {
            return Err(format!(
                "station {}: no offset for {} (give circular_t + mjd or lab_offsets)",
                s.id, s.time_ref
            ));
        };
        let own = (s.u_meas_ns.powi(2) + s.u_prop_ns.powi(2) + s.u_steer_ns.powi(2)).sqrt();
        est.push(StationEstimate {
            id: s.id.clone(),
            time_ref: s.time_ref.clone(),
            rx_minus_utc_ns: lab_v + s.steering_ns - s.ref_minus_rx_ns,
            u_own_ns: own,
            u_lab_ns: lab_u,
            lab_offset_ns: lab_v,
        });
    }
    let k = req.coverage_k;
    let mut pairs = vec![];
    for i in 0..est.len() {
        for j in i + 1..est.len() {
            let (a, b) = (&est[i], &est[j]);
            let same = a.time_ref == b.time_ref;
            let var = a.u_own_ns.powi(2)
                + b.u_own_ns.powi(2)
                + if same {
                    0.0
                } else {
                    a.u_lab_ns.powi(2) + b.u_lab_ns.powi(2)
                };
            let d = a.rx_minus_utc_ns - b.rx_minus_utc_ns;
            let u = k * var.sqrt();
            let e_n = if u > 0.0 { d.abs() / u } else { f64::INFINITY };
            pairs.push(PairResult {
                a: a.id.clone(),
                b: b.id.clone(),
                same_time_ref: same,
                difference_ns: d,
                expanded_u_ns: u,
                e_n,
                consistent: e_n <= 1.0,
                tolerance: req.tolerance_ns.map(|t| {
                    match decide(d, u, t) {
                        ClaimDecision::Supported => "within",
                        ClaimDecision::NotSupported => "outside",
                        ClaimDecision::Inconclusive => "inconclusive",
                    }
                    .to_string()
                }),
            });
        }
    }
    let w: Vec<f64> = est
        .iter()
        .map(|e| 1.0 / (e.u_own_ns.powi(2) + e.u_lab_ns.powi(2)).max(1e-12))
        .collect();
    let sw: f64 = w.iter().sum();
    let mean = est
        .iter()
        .zip(&w)
        .map(|(e, w)| e.rx_minus_utc_ns * w)
        .sum::<f64>()
        / sw;
    let chi2: f64 = est
        .iter()
        .zip(&w)
        .map(|(e, w)| w * (e.rx_minus_utc_ns - mean).powi(2))
        .sum();
    Ok(InteropReport {
        format: INTEROP_FORMAT.into(),
        consistent_pairs: pairs.iter().filter(|p| p.consistent).count(),
        stations: est,
        pairs,
        weighted_mean_ns: mean,
        u_weighted_mean_ns: (1.0 / sw).sqrt(),
        birge_ratio: (req.stations.len() > 1)
            .then(|| (chi2 / (req.stations.len() - 1) as f64).sqrt()),
        lab_offset_source: src,
        evidence: "MODELLED: normalised-error comparison on the stated uncertainties; laboratory offsets as published or entered".into(),
    })
}

// ------------------------------------------------------------------ safety-of-life index

/// Kshana's own evidence categories for a safety-of-life pathway (not taken from any
/// certification standard): id and description.
pub const SOL_CATEGORIES: &[(&str, &str)] = &[
    ("requirements", "the service requirement claimed (accuracy, availability, continuity, integrity, timing) and its source"),
    ("timing_traceability", "traceability of the timing output to UTC with an uncertainty budget"),
    ("position_performance", "accuracy, availability and continuity statistics with their confidence"),
    ("integrity_fde", "fault detection and exclusion, protection levels, time to alarm"),
    ("interoperability", "consistency across transmitters steered to different UTC(k)"),
    ("failure_modes", "the faults considered and how each is detected or bounded"),
    ("configuration", "identification of the equipment, firmware, antenna and installation tested"),
    ("reproducibility", "raw data, hashes and the method version so a reviewer can recompute"),
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SolArtifact {
    pub path: String,
    pub category: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SolRequest {
    pub title: String,
    pub subject: String,
    #[serde(rename = "artifact", default)]
    pub artifacts: Vec<SolArtifact>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SolEntry {
    pub file: String,
    pub sha256: String,
    pub note: String,
    /// `format` of a Kshana JSON report, when it is one.
    pub report_format: Option<String>,
    /// Its headline (`statement_of_result`, `summary`, or decision counts).
    pub headline: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SolCategory {
    pub id: String,
    pub description: String,
    pub status: String,
    pub entries: Vec<SolEntry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SolIndex {
    pub format: String,
    pub title: String,
    pub subject: String,
    pub categories: Vec<SolCategory>,
    pub covered: usize,
    pub gaps: Vec<String>,
    pub statement: String,
}

fn headline(v: &serde_json::Value) -> Option<String> {
    for k in ["statement_of_result", "summary"] {
        if let Some(s) = v.get(k).and_then(|s| s.as_str()) {
            return Some(s.to_string());
        }
    }
    if v.get("format").and_then(|f| f.as_str()) == Some(INTEROP_FORMAT) {
        return Some(format!(
            "{} of {} pairs consistent (E_n ≤ 1)",
            v.get("consistent_pairs")?.as_u64()?,
            v.get("pairs")?.as_array()?.len()
        ));
    }
    if let (Some(n), Some(a)) = (v.get("nominal"), v.get("alarms")) {
        return Some(format!("{n} nominal epochs, {a} alarms"));
    }
    None
}

/// Build the index from the artefacts (paths relative to `base`); returns the index and the
/// `(source path, file name)` pairs the caller copies.
pub fn sol_index(
    req: &SolRequest,
    base: &Path,
) -> Result<(SolIndex, Vec<(std::path::PathBuf, String)>), String> {
    let mut by_cat: BTreeMap<&str, Vec<SolEntry>> = BTreeMap::new();
    let mut copies = vec![];
    for a in &req.artifacts {
        if !SOL_CATEGORIES.iter().any(|(c, _)| *c == a.category) {
            return Err(format!(
                "artifact {}: unknown category {:?} (known: {})",
                a.path,
                a.category,
                SOL_CATEGORIES
                    .iter()
                    .map(|c| c.0)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        let p = base.join(&a.path);
        let bytes = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .ok_or(format!("{}: not a file", a.path))?;
        let json: Option<serde_json::Value> = serde_json::from_slice(&bytes).ok();
        by_cat
            .entry(a.category.as_str())
            .or_default()
            .push(SolEntry {
                file: format!("artifacts/{name}"),
                sha256: sha256_hex(&bytes),
                note: a.note.clone(),
                report_format: json
                    .as_ref()
                    .and_then(|v| v.get("format"))
                    .and_then(|f| f.as_str())
                    .map(String::from),
                headline: json.as_ref().and_then(headline),
            });
        if !copies
            .iter()
            .any(|(_, n): &(std::path::PathBuf, String)| *n == name)
        {
            copies.push((p, name));
        } else {
            return Err(format!("two artefacts are named {name}"));
        }
    }
    let categories: Vec<SolCategory> = SOL_CATEGORIES
        .iter()
        .map(|(id, d)| {
            let entries = by_cat.remove(id).unwrap_or_default();
            SolCategory {
                id: id.to_string(),
                description: d.to_string(),
                status: if entries.is_empty() {
                    "no evidence supplied".into()
                } else {
                    "evidence supplied".into()
                },
                entries,
            }
        })
        .collect();
    let gaps: Vec<String> = categories
        .iter()
        .filter(|c| c.entries.is_empty())
        .map(|c| c.id.clone())
        .collect();
    Ok((
        SolIndex {
            format: SOL_FORMAT.into(),
            title: req.title.clone(),
            subject: req.subject.clone(),
            covered: categories.len() - gaps.len(),
            categories,
            gaps,
            statement: "Evidence index for a safety-of-life certification pathway. The categories are Kshana's own, not those of a certification standard; the index says what evidence was supplied, not that any requirement is met.".into(),
        },
        copies,
    ))
}

/// The index as Markdown.
pub fn sol_markdown(ix: &SolIndex) -> String {
    let mut s = format!(
        "# {}\n\nSubject: {}\n\n{}\n\n| category | status | evidence |\n|---|---|---|\n",
        ix.title, ix.subject, ix.statement
    );
    for c in &ix.categories {
        let ev = c
            .entries
            .iter()
            .map(|e| {
                format!(
                    "`{}` ({}…){}",
                    e.file,
                    &e.sha256[..12],
                    e.headline
                        .as_ref()
                        .map(|h| format!(": {h}"))
                        .unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join("<br>");
        s += &format!("| {} | {} | {} |\n", c.id, c.status, ev);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decisions_are_guard_banded() {
        assert_eq!(decide(10.0, 5.0, 20.0), ClaimDecision::Supported);
        assert_eq!(decide(-18.0, 5.0, 20.0), ClaimDecision::Inconclusive);
        assert_eq!(decide(30.0, 5.0, 20.0), ClaimDecision::NotSupported);
    }
}
