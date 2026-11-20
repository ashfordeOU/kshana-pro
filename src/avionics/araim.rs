// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! ARAIM behaviour checks on the open engine's reference airborne algorithm.
//!
//! The protection levels come from `kshana::araim_reference` (open engine 0.41.0): the WG-C
//! ARAIM Technical Subgroup's reference airborne algorithm, which the open engine checks
//! against the subgroup's own published worked examples. This module adds:
//!
//! * `published_vectors`: the worked examples run now, each output against the tolerance
//!   the source document states (evidence that this build reproduces them);
//! * `cases`: geometry and integrity-support-message inputs per constellation, giving the
//!   reference VPL, HPL, EMT and vertical accuracy sigma; a constellation can be declared
//!   unusable (an ICAO-style "institutional" case: the ISM removes it) and the change from
//!   leaving a constellation out can be reported (for example LEO ranging sources added to
//!   GPS and Galileo);
//! * availability against the applicant's alert limits (parameters, never built in);
//! * the equipment's own outputs for the same cases compared to the reference within a
//!   stated tolerance, and its availability decision compared to the reference decision.
//!
//! H-ARAIM (horizontal) and vertical (V-ARAIM readiness) are the same computation with
//! different limits; V-ARAIM equipment standards are not issued, so vertical results are
//! readiness evidence only.

use kshana::araim_reference::{
    add_v42_protection_levels, araim_reference_protection_levels, published_vectors, AddV42Case,
    ReferenceConstants, ReferenceSatellite,
};
use serde::{Deserialize, Serialize};

use super::procedure::{Check, DutLog, Limit, Params};
use super::sig9;
use crate::scorecards::levels::Verdict;

/// What an ARAIM procedure checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AraimCheck {
    PublishedVectors,
    Cases,
}

fn d_fc_thres() -> f64 {
    0.01
}

/// The navigation constants (default: the published LPV-200 column).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Constants {
    #[serde(default)]
    pub p_hmi_vert: Option<f64>,
    #[serde(default)]
    pub p_hmi_horz: Option<f64>,
    #[serde(default)]
    pub p_fa_vert: Option<f64>,
    #[serde(default)]
    pub p_fa_horz: Option<f64>,
    #[serde(default)]
    pub p_emt: Option<f64>,
    #[serde(default)]
    pub p_thres: Option<f64>,
    #[serde(default = "d_fc_thres")]
    pub fc_thres: f64,
}

impl Default for Constants {
    fn default() -> Self {
        Constants {
            p_hmi_vert: None,
            p_hmi_horz: None,
            p_fa_vert: None,
            p_fa_horz: None,
            p_emt: None,
            p_thres: None,
            fc_thres: d_fc_thres(),
        }
    }
}

impl Constants {
    fn resolve(&self) -> ReferenceConstants {
        let b = ReferenceConstants::LPV_200;
        ReferenceConstants {
            p_hmi_vert: self.p_hmi_vert.unwrap_or(b.p_hmi_vert),
            p_hmi_horz: self.p_hmi_horz.unwrap_or(b.p_hmi_horz),
            p_fa_vert: self.p_fa_vert.unwrap_or(b.p_fa_vert),
            p_fa_horz: self.p_fa_horz.unwrap_or(b.p_fa_horz),
            p_emt: self.p_emt.unwrap_or(b.p_emt),
            p_thres: self.p_thres.unwrap_or(b.p_thres),
            tol_pl_m: b.tol_pl_m,
        }
    }
}

/// One constellation of a case: its ISM values, error model and satellites (elevation,
/// azimuth in degrees).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstellationSpec {
    pub name: String,
    pub p_const: f64,
    pub p_sat: f64,
    pub sigma_ura_m: f64,
    pub sigma_ure_m: f64,
    pub b_nom_m: f64,
    /// Residual troposphere and airborne (receiver plus multipath) sigmas, constant per
    /// satellite here; the applicant's elevation-dependent models are their input.
    #[serde(default)]
    pub sigma_tropo_m: f64,
    #[serde(default)]
    pub sigma_user_m: f64,
    pub el_az_deg: Vec<[f64; 2]>,
    /// Declared unusable by the integrity support message (an "institutional" case).
    #[serde(default)]
    pub unusable: bool,
}

/// One ARAIM case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseSpec {
    pub id: String,
    #[serde(default)]
    pub description: String,
    #[serde(rename = "constellation")]
    pub constellations: Vec<ConstellationSpec>,
    /// Constellations to leave out for a comparison (the change they make).
    #[serde(default)]
    pub compare_without: Vec<String>,
}

/// The ARAIM part of a procedure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AraimSpec {
    pub check: AraimCheck,
    #[serde(default)]
    pub constants: Constants,
    #[serde(default, rename = "case")]
    pub cases: Vec<CaseSpec>,
}

/// The reference outputs of one case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaseResult {
    pub id: String,
    pub used: Vec<String>,
    pub declared_unusable: Vec<String>,
    pub satellites: usize,
    pub vpl_m: f64,
    pub hpl_m: f64,
    pub emt_m: f64,
    pub sigma_v_acc_m: f64,
    pub p_not_monitored: f64,
    pub comparisons: Vec<Comparison>,
}

/// The protection levels with some constellations left out, and the change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    pub without: String,
    pub vpl_m: Option<f64>,
    pub hpl_m: Option<f64>,
    /// `with − without` (m): negative when the constellation lowers the level.
    pub delta_vpl_m: Option<f64>,
    pub delta_hpl_m: Option<f64>,
    pub note: String,
}

fn validate_prob(what: &str, p: f64) -> Result<(), String> {
    if !(0.0..1.0).contains(&p) {
        return Err(format!("{what}: {p} is not a probability in [0, 1)"));
    }
    Ok(())
}

impl AraimSpec {
    pub fn validate(&self) -> Result<(), String> {
        match self.check {
            AraimCheck::PublishedVectors => {
                if !self.cases.is_empty() {
                    return Err("araim: published_vectors takes no cases".into());
                }
            }
            AraimCheck::Cases => {
                if self.cases.is_empty() {
                    return Err("araim: cases needs at least one [[araim.case]]".into());
                }
            }
        }
        if !(0.0..1.0).contains(&self.constants.fc_thres) {
            return Err("araim: fc_thres must be in [0, 1)".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for c in &self.cases {
            if !ids.insert(&c.id) {
                return Err(format!("araim: case '{}' twice", c.id));
            }
            if c.constellations.is_empty() {
                return Err(format!("araim case '{}': no constellation", c.id));
            }
            for k in &c.constellations {
                let at = format!("araim case '{}' constellation '{}'", c.id, k.name);
                validate_prob(&format!("{at} p_const"), k.p_const)?;
                validate_prob(&format!("{at} p_sat"), k.p_sat)?;
                for (n, v) in [
                    ("sigma_ura_m", k.sigma_ura_m),
                    ("sigma_ure_m", k.sigma_ure_m),
                    ("b_nom_m", k.b_nom_m),
                    ("sigma_tropo_m", k.sigma_tropo_m),
                    ("sigma_user_m", k.sigma_user_m),
                ] {
                    if !v.is_finite() || v < 0.0 {
                        return Err(format!("{at}: {n} must be finite and not negative"));
                    }
                }
                if k.sigma_ura_m <= 0.0 || k.sigma_ure_m <= 0.0 {
                    return Err(format!(
                        "{at}: sigma_ura_m and sigma_ure_m must be positive"
                    ));
                }
                for [el, az] in &k.el_az_deg {
                    if !(0.0..=90.0).contains(el) || !az.is_finite() {
                        return Err(format!("{at}: elevation {el} outside 0 to 90 degrees"));
                    }
                }
            }
            for w in &c.compare_without {
                if !c.constellations.iter().any(|k| &k.name == w) {
                    return Err(format!(
                        "araim case '{}': compare_without '{w}' is not a constellation",
                        c.id
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Build the engine's case from the constellations not in `leave_out` and not unusable.
fn engine_case(
    c: &CaseSpec,
    constants: &Constants,
    leave_out: &[&str],
) -> Result<(AddV42Case, Vec<String>), String> {
    let used: Vec<&ConstellationSpec> = c
        .constellations
        .iter()
        .filter(|k| !k.unusable && !leave_out.contains(&k.name.as_str()))
        .collect();
    let mut sats = Vec::new();
    let mut b_nom = Vec::new();
    let mut p_sat = Vec::new();
    for (j, k) in used.iter().enumerate() {
        let common = k.sigma_tropo_m.powi(2) + k.sigma_user_m.powi(2);
        for [el, az] in &k.el_az_deg {
            let (el, az) = (el.to_radians(), az.to_radians());
            sats.push(ReferenceSatellite {
                east: -el.cos() * az.sin(),
                north: -el.cos() * az.cos(),
                up: -el.sin(),
                constellation: j,
                c_int_m2: k.sigma_ura_m.powi(2) + common,
                c_acc_m2: k.sigma_ure_m.powi(2) + common,
            });
            b_nom.push(k.b_nom_m);
            p_sat.push(k.p_sat);
        }
    }
    Ok((
        AddV42Case {
            satellites: sats,
            constellations: used.len(),
            b_nom_m: b_nom,
            p_sat,
            p_const: used.iter().map(|k| k.p_const).collect(),
            constants: constants.resolve(),
            fc_thres: constants.fc_thres,
        },
        used.iter().map(|k| k.name.clone()).collect(),
    ))
}

/// The reference outputs of every case.
pub fn run_cases(spec: &AraimSpec) -> Result<Vec<CaseResult>, String> {
    spec.validate()?;
    let mut out = Vec::new();
    for c in &spec.cases {
        let (case, used) = engine_case(c, &spec.constants, &[])?;
        if used.is_empty() {
            return Err(format!(
                "araim case '{}': every constellation is unusable",
                c.id
            ));
        }
        let r = add_v42_protection_levels(&case).map_err(|e| format!("case '{}': {e}", c.id))?;
        let mut comparisons = Vec::new();
        for w in &c.compare_without {
            let (case_w, used_w) = engine_case(c, &spec.constants, &[w.as_str()])?;
            let rw = if used_w.is_empty() {
                Err("no constellation left".to_string())
            } else {
                add_v42_protection_levels(&case_w)
            };
            comparisons.push(match rw {
                Ok(rw) => Comparison {
                    without: w.clone(),
                    vpl_m: Some(sig9(rw.vpl_m)),
                    hpl_m: Some(sig9(rw.hpl_m)),
                    delta_vpl_m: Some(sig9(r.vpl_m - rw.vpl_m)),
                    delta_hpl_m: Some(sig9(r.hpl_m - rw.hpl_m)),
                    note: String::new(),
                },
                Err(e) => Comparison {
                    without: w.clone(),
                    vpl_m: None,
                    hpl_m: None,
                    delta_vpl_m: None,
                    delta_hpl_m: None,
                    note: format!("no solution without '{w}': {e}"),
                },
            });
        }
        out.push(CaseResult {
            id: c.id.clone(),
            used,
            declared_unusable: c
                .constellations
                .iter()
                .filter(|k| k.unusable)
                .map(|k| k.name.clone())
                .collect(),
            satellites: case.satellites.len(),
            vpl_m: sig9(r.vpl_m),
            hpl_m: sig9(r.hpl_m),
            emt_m: sig9(r.emt_m),
            sigma_v_acc_m: sig9(r.sigma_v_acc_m),
            p_not_monitored: sig9(r.p_not_monitored),
            comparisons,
        });
    }
    Ok(out)
}

fn le_check(
    id: String,
    subject: &str,
    measured: Option<f64>,
    limit: &Limit,
    unit: &str,
    what: &str,
) -> Check {
    let (verdict, detail) = match (measured, limit.value) {
        (None, _) => (Verdict::NotAssessed, format!("{what}: no value")),
        (Some(_), None) => (
            Verdict::NotAssessed,
            format!("{what}: limit '{}' not supplied", limit.param),
        ),
        (Some(m), Some(l)) => {
            if m.is_finite() && m <= l {
                (Verdict::Met, format!("{what} {m:.3} {unit} <= {l} {unit}"))
            } else {
                (
                    Verdict::NotMet,
                    format!("{what} {m:.3} {unit} > {l} {unit}"),
                )
            }
        }
    };
    Check {
        id,
        kind: "at_most".into(),
        subject: subject.into(),
        limit_param: limit.param.clone(),
        limit: limit.value,
        limit_source: limit.source.clone(),
        measured: measured.map(sig9),
        verdict,
        detail,
    }
}

/// The reference availability decision of a case under the applicant's limits: `Some(true)`
/// or `Some(false)`, `None` when a needed limit is missing.
fn available(r: &CaseResult, params: &Params, vertical: bool) -> Option<bool> {
    if vertical {
        let val = params.value("val_m")?;
        let mut ok = r.vpl_m <= val;
        if let Some(e) = params.value("emt_limit_m") {
            ok &= r.emt_m <= e;
        }
        if let Some(s) = params.value("sigma_acc_limit_m") {
            ok &= r.sigma_v_acc_m <= s;
        }
        Some(ok)
    } else {
        Some(r.hpl_m <= params.value("hal_m")?)
    }
}

/// The checks of an ARAIM procedure: availability per case against the applicant's limits
/// (reference results), then, with an equipment log, the equipment's outputs against the
/// reference.
pub fn checks(
    spec: &AraimSpec,
    params: &Params,
    log: Option<&DutLog>,
) -> Result<(Vec<Check>, Vec<CaseResult>), String> {
    match spec.check {
        AraimCheck::PublishedVectors => Ok((published_vector_checks()?, vec![])),
        AraimCheck::Cases => {
            let results = run_cases(spec)?;
            let mut out = Vec::new();
            let tol = params.limit("tol_pl_m");
            for r in &results {
                for (vertical, axis) in [(true, "vertical"), (false, "horizontal")] {
                    let a = available(r, params, vertical);
                    out.push(Check {
                        id: format!("{}-reference-{axis}-availability", r.id),
                        kind: "reference".into(),
                        subject: format!("case {}", r.id),
                        limit_param: if vertical { "val_m" } else { "hal_m" }.into(),
                        limit: params.value(if vertical { "val_m" } else { "hal_m" }),
                        limit_source: params.source(if vertical { "val_m" } else { "hal_m" }),
                        measured: Some(if vertical { r.vpl_m } else { r.hpl_m }),
                        verdict: Verdict::NotAssessed,
                        detail: match a {
                            Some(true) => format!("reference: {axis} available"),
                            Some(false) => format!("reference: {axis} not available"),
                            None => format!("reference: {axis} limit not supplied"),
                        },
                    });
                }
                let Some(log) = log else {
                    out.push(Check {
                        id: format!("{}-equipment", r.id),
                        kind: "equipment".into(),
                        subject: format!("case {}", r.id),
                        limit_param: String::new(),
                        limit: None,
                        limit_source: String::new(),
                        measured: None,
                        verdict: Verdict::NotAssessed,
                        detail: "no equipment log supplied: reference results only".into(),
                    });
                    continue;
                };
                let Some(d) = log.araim_cases.iter().find(|d| d.id == r.id) else {
                    out.push(Check {
                        id: format!("{}-equipment", r.id),
                        kind: "equipment".into(),
                        subject: format!("case {}", r.id),
                        limit_param: String::new(),
                        limit: None,
                        limit_source: String::new(),
                        measured: None,
                        verdict: Verdict::NotAssessed,
                        detail: "the equipment log has no output for this case".into(),
                    });
                    continue;
                };
                for (name, dut, refv) in [
                    ("vpl", d.vpl_m, r.vpl_m),
                    ("hpl", d.hpl_m, r.hpl_m),
                    ("emt", d.emt_m, r.emt_m),
                    ("sigma-v-acc", d.sigma_v_acc_m, r.sigma_v_acc_m),
                ] {
                    if dut.is_none() && (name == "emt" || name == "sigma-v-acc") {
                        continue;
                    }
                    let mut c = le_check(
                        format!("{}-equipment-{name}", r.id),
                        &format!("case {}", r.id),
                        dut.map(|x| (x - refv).abs()),
                        &tol,
                        "m",
                        &format!("|equipment {name} − reference {refv:.3}|"),
                    );
                    c.kind = "agreement".into();
                    out.push(c);
                }
                for (vertical, axis, dut) in [
                    (true, "vertical", d.available_vertical),
                    (false, "horizontal", d.available_horizontal),
                ] {
                    let Some(dut) = dut else { continue };
                    let refa = available(r, params, vertical);
                    out.push(Check {
                        id: format!("{}-equipment-{axis}-availability", r.id),
                        kind: "decision".into(),
                        subject: format!("case {}", r.id),
                        limit_param: if vertical { "val_m" } else { "hal_m" }.into(),
                        limit: params.value(if vertical { "val_m" } else { "hal_m" }),
                        limit_source: params.source(if vertical { "val_m" } else { "hal_m" }),
                        measured: None,
                        verdict: match refa {
                            None => Verdict::NotAssessed,
                            Some(x) => Verdict::from_bool(Some(x == dut)),
                        },
                        detail: match refa {
                            None => "limit not supplied".into(),
                            Some(x) => format!(
                                "equipment says {axis} {}, reference says {}",
                                if dut { "available" } else { "not available" },
                                if x { "available" } else { "not available" }
                            ),
                        },
                    });
                }
                if !r.declared_unusable.is_empty() {
                    let excl = &d.excluded_constellations;
                    let ok = r.declared_unusable.iter().all(|u| excl.contains(u));
                    out.push(Check {
                        id: format!("{}-equipment-institutional-exclusion", r.id),
                        kind: "decision".into(),
                        subject: format!("case {}", r.id),
                        limit_param: String::new(),
                        limit: None,
                        limit_source: String::new(),
                        measured: None,
                        verdict: Verdict::from_bool(Some(ok)),
                        detail: format!(
                            "declared unusable {:?}; equipment excluded {:?}",
                            r.declared_unusable, excl
                        ),
                    });
                }
            }
            Ok((out, results))
        }
    }
}

/// The published WG-C worked examples run through the open engine, each output against the
/// tolerance the source states.
pub fn published_vector_checks() -> Result<Vec<Check>, String> {
    let mut out = Vec::new();
    for v in published_vectors() {
        let r = araim_reference_protection_levels(&v.case)?;
        for (name, got, want, pl) in [
            ("vpl", r.vpl_m, v.vpl_m, true),
            ("hpl", r.hpl_m, v.hpl_m, true),
            ("emt", r.emt_m, v.emt_m, true),
            ("sigma-v-acc", r.sigma_v_acc_m, v.sigma_v_acc_m, false),
        ] {
            let diff = (got - want).abs();
            let unreproducible = pl && !v.protection_levels_reproducible;
            out.push(Check {
                id: format!("{}-{name}", v.id),
                kind: "published-vector".into(),
                subject: v.citation.split_whitespace().collect::<Vec<_>>().join(" "),
                limit_param: "tolerance stated by the source".into(),
                limit: Some(v.tolerance_m),
                limit_source: v
                    .tolerance_source
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
                measured: Some(sig9(diff)),
                verdict: if unreproducible {
                    Verdict::NotAssessed
                } else {
                    Verdict::from_bool(Some(diff <= v.tolerance_m))
                },
                detail: if unreproducible {
                    format!(
                        "computed {got:.4} m, printed {want} m; not assessed: the source's own \
                         note says its protection levels are not reproducible together ({})",
                        v.location
                    )
                } else {
                    format!(
                        "computed {got:.4} m, printed {want} m ({}); |difference| {diff:.4} m",
                        v.location
                    )
                },
            });
        }
    }
    Ok(out)
}
