// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # 5G TDD timing, PRS accuracy and A-GNSS integrity vectors (backlog B3004, first slice)
//!
//! Research 2026-11-20 gaps 07:K10 and 08:A11, E13, E14, E15, G23, G24.
//!
//! * [`tdd`]: what GNSS loss at a 5G TDD gNB does to its frame timing. The holdover
//!   evidence model ([`crate::timing::holdover_evidence`]) gives the time-error bound
//!   against time; this function reads off when it crosses the cell phase-synchronisation
//!   budget (±1.5 µs to a common reference, the 3 µs cell-to-cell requirement of 3GPP
//!   TS 38.133, to confirm against the release in use) and when two neighbouring gNBs
//!   drifting in opposite directions use up the guard period of the special slot, less the
//!   propagation delay to the interfering cell: the onset of downlink-to-uplink interference.
//! * [`prs`]: a downlink TDOA accuracy model for positioning reference signals: ranging
//!   standard deviation per gNB from the Cramér-Rao bound for a flat spectrum of the PRS
//!   bandwidth (`β_rms = B/√12`) and the SNR after combining, plus the inter-gNB
//!   synchronisation error, through the TDOA geometry to a horizontal error at a user
//!   position. It ties PRS accuracy to the network's timing: a gNB in GNSS holdover adds its
//!   time error to every range. A performance model of a standard civil method, for known
//!   transmitter sites; it does not locate transmitters.
//! * [`vectors`] and [`check_vectors`]: integrity test vectors for GNSS positioning with
//!   integrity (introduced for UE-based positioning in 3GPP Release 18): each case states a
//!   geometry, per-satellite error overbounds (sigma and bias), a target integrity risk and an
//!   alert limit, and the expected horizontal protection level and availability from a
//!   Gaussian-overbound reference formula written out in [`hpl`]. The vectors are
//!   independent of any test-system vendor; they are **not** the 3GPP conformance cases of
//!   TS 37.571-5 or TS 38.171, which this work did not read.
//!
//! The shared-risk test-tolerance calculator is [`super::tolerance`] and the A-GNSS vector-set
//! scaffold (placeholders only; values transcribed by the user) is [`super::agnss`]. Not
//! built: the vector values themselves (specification text not read), PRS carrier-phase and
//! PRU calibration models, and a gNB timing-alarm adapter.
//!
//! **Label: MODELLED.**

use serde::{Deserialize, Serialize};

use crate::timing::holdover_evidence::{self, HoldoverSpec};

use super::{norm_quantile, Table};

pub const FEATURE: &str = "pro-nr-timing";
pub const FEATURE_VECTORS: &str = "pro-agnss-integrity";
pub const TDD_FORMAT: &str = "kshana-pro-nr-tdd-timing/1";
pub const PRS_FORMAT: &str = "kshana-pro-nr-prs-accuracy/1";
pub const VECTORS_FORMAT: &str = "kshana-pro-agnss-integrity-vectors/1";
pub const CHECK_FORMAT: &str = "kshana-pro-agnss-integrity-check/1";
pub const LABEL: &str =
    "MODELLED: analytic models with stated parameters; not measured on a network or a device.";
/// Speed of light (m/s).
pub const C: f64 = 299_792_458.0;
/// Half the 3 µs cell phase-synchronisation requirement: the budget to a common reference.
pub const PHASE_BUDGET_S: f64 = 1.5e-6;
pub const PHASE_BASIS: &str = "3GPP TS 38.133 cell phase synchronisation accuracy (3 µs between TDD cells, ±1.5 µs to a common reference, as in ITU-T G.8271 class 4); to confirm against the release in use.";

/// OFDM symbol duration including the normal cyclic prefix, on average (s), numerology `mu`.
pub fn symbol_s(mu: u8) -> f64 {
    1e-3 / (14.0 * f64::from(1u32 << mu))
}

/// The TDD request.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TddRequest {
    pub title: String,
    /// Subcarrier spacing numerology (0-4; 1 is 30 kHz).
    pub numerology: u8,
    /// Guard-period symbols of the special slot.
    pub guard_symbols: u32,
    /// Distance to the nearest interfering cell (km).
    pub interferer_km: f64,
    /// Phase budget to a common reference (s); default [`PHASE_BUDGET_S`].
    pub budget_s: Option<f64>,
    /// The gNB's holdover: oscillator, time error at the loss, horizon (`period_h`).
    pub holdover: HoldoverSpec,
}

/// The TDD report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TddReport {
    pub format: &'static str,
    pub label: &'static str,
    pub title: String,
    pub budget_s: f64,
    pub budget_basis: &'static str,
    pub guard_period_s: f64,
    pub propagation_s: f64,
    /// Per-gNB time error at which two opposite drifts close the guard period.
    pub interference_te_s: f64,
    /// Longest holdover with the bound within the budget (s after the loss), `None` beyond
    /// the horizon (ten times the holdover period).
    pub budget_exceeded_after_s: Option<f64>,
    pub interference_after_s: Option<f64>,
    pub horizon_s: f64,
    pub holdover_verdict: String,
    pub verdict: String,
}

/// TDD frame-timing failure under GNSS loss.
pub fn tdd(req: &TddRequest, utc: &str) -> Result<TddReport, String> {
    if req.numerology > 4 {
        return Err("numerology must be 0-4".into());
    }
    if req.guard_symbols == 0 || !super::nonneg(req.interferer_km) {
        return Err("guard_symbols must be > 0 and interferer_km >= 0".into());
    }
    let budget = req.budget_s.unwrap_or(PHASE_BUDGET_S);
    if !super::pos(budget) {
        return Err("budget_s must be > 0".into());
    }
    let gp = f64::from(req.guard_symbols) * symbol_s(req.numerology);
    let prop = req.interferer_km * 1000.0 / C;
    let te_int = ((gp - prop) / 2.0).max(0.0);
    let mut spec = req.holdover.clone();
    spec.limit_s = Some(budget);
    spec.preset = None;
    let h = holdover_evidence::assess(&spec, utc)?;
    // The model's own crossing search (up to ten times the period) for both limits.
    let b_after = h.max_supported_holdover_s;
    let i_after = if te_int == 0.0 {
        Some(0.0)
    } else {
        let mut si = spec.clone();
        si.limit_s = Some(te_int);
        holdover_evidence::assess(&si, utc)?.max_supported_holdover_s
    };
    let horizon = 10.0 * h.period_s;
    let hh = |x: Option<f64>| {
        x.map(|s| format!("after {}", crate::timing::fleet::human_duration(Some(s))))
            .unwrap_or_else(|| "not within ten times the period".into())
    };
    let verdict = format!(
        "guard period {:.1} µs, propagation {:.1} µs; phase budget ±{:.2} µs passed {}; downlink-to-uplink interference with a neighbour drifting the other way {}",
        gp * 1e6,
        prop * 1e6,
        budget * 1e6,
        hh(b_after),
        hh(i_after)
    );
    Ok(TddReport {
        format: TDD_FORMAT,
        label: LABEL,
        title: req.title.clone(),
        budget_s: budget,
        budget_basis: PHASE_BASIS,
        guard_period_s: gp,
        propagation_s: prop,
        interference_te_s: te_int,
        budget_exceeded_after_s: b_after,
        interference_after_s: i_after,
        horizon_s: horizon,
        holdover_verdict: h.verdict,
        verdict,
    })
}

// ---------------------------------------------------------------------------------------
// PRS accuracy.

/// One gNB.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Gnb {
    pub name: String,
    pub east_m: f64,
    pub north_m: f64,
    /// SNR of one PRS symbol at the user (dB).
    pub snr_db: f64,
    /// This gNB's synchronisation error standard deviation (s); default the request's.
    pub sync_sigma_s: Option<f64>,
}

/// The PRS request.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrsRequest {
    pub title: String,
    /// PRS bandwidth (MHz); FR1 up to 100.
    pub bandwidth_mhz: f64,
    /// PRS symbols combined per measurement.
    pub symbols: u32,
    /// Synchronisation error standard deviation of every gNB (s).
    #[serde(default)]
    pub sync_sigma_s: f64,
    /// Extra ranging error (multipath, NLOS) per gNB, standard deviation (m).
    #[serde(default)]
    pub extra_sigma_m: f64,
    pub gnb: Vec<Gnb>,
    /// User positions (east, north, m).
    pub users: Vec<[f64; 2]>,
}

/// One user position's result.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PrsPoint {
    pub east_m: f64,
    pub north_m: f64,
    pub reference: String,
    pub drms_m: Option<f64>,
    pub two_drms_m: Option<f64>,
}

/// The PRS report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PrsReport {
    pub format: &'static str,
    pub label: &'static str,
    pub title: String,
    pub ranging_sigma_m: Vec<(String, f64)>,
    pub points: Vec<PrsPoint>,
    pub verdict: String,
}

/// Cramér-Rao ranging standard deviation (m) for a flat PRS spectrum of `bw_hz` and linear
/// post-combining SNR `snr`.
pub fn crlb_range_m(bw_hz: f64, snr: f64) -> f64 {
    let beta = bw_hz / 12f64.sqrt();
    C / (2.0 * std::f64::consts::PI * beta * (2.0 * snr).sqrt())
}

fn inv2(a: [[f64; 2]; 2]) -> Option<[[f64; 2]; 2]> {
    let d = a[0][0] * a[1][1] - a[0][1] * a[1][0];
    (d.abs() > 1e-30).then(|| [[a[1][1] / d, -a[0][1] / d], [-a[1][0] / d, a[0][0] / d]])
}

/// Horizontal DRMS of TDOA positioning at `(e, n)` with reference gNB `r` (index).
pub fn tdoa_drms(gnbs: &[(f64, f64)], sigma_m: &[f64], r: usize, e: f64, n: f64) -> Option<f64> {
    let unit = |k: usize| {
        let (de, dn) = (e - gnbs[k].0, n - gnbs[k].1);
        let d = (de * de + dn * dn).sqrt().max(1e-6);
        [de / d, dn / d]
    };
    let ur = unit(r);
    let others: Vec<usize> = (0..gnbs.len()).filter(|k| *k != r).collect();
    if others.len() < 2 {
        return None;
    }
    let m = others.len();
    let h: Vec<[f64; 2]> = others
        .iter()
        .map(|&k| {
            let u = unit(k);
            [u[0] - ur[0], u[1] - ur[1]]
        })
        .collect();
    // R = diag(σk²) + σr²·11ᵀ; solve with the full matrix by Gauss-Jordan.
    let mut rm = vec![vec![sigma_m[r].powi(2); m]; m];
    for (i, &k) in others.iter().enumerate() {
        rm[i][i] += sigma_m[k].powi(2);
    }
    let rinv = super::rail::invert(&rm)?;
    let mut nmat = [[0.0; 2]; 2];
    for a in 0..2 {
        for b in 0..2 {
            let mut s = 0.0;
            for i in 0..m {
                for j in 0..m {
                    s += h[i][a] * rinv[i][j] * h[j][b];
                }
            }
            nmat[a][b] = s;
        }
    }
    let q = inv2(nmat)?;
    Some((q[0][0] + q[1][1]).max(0.0).sqrt())
}

/// The PRS accuracy model.
pub fn prs(req: &PrsRequest) -> Result<PrsReport, String> {
    if !(req.bandwidth_mhz > 0.0 && req.bandwidth_mhz <= 400.0) || req.symbols == 0 {
        return Err("bandwidth_mhz must be in (0, 400] and symbols > 0".into());
    }
    if req.gnb.len() < 3 {
        return Err("at least three gNBs".into());
    }
    let sig: Vec<f64> = req
        .gnb
        .iter()
        .map(|g| {
            let snr = 10f64.powf(g.snr_db / 10.0) * f64::from(req.symbols);
            let r = crlb_range_m(req.bandwidth_mhz * 1e6, snr);
            let s = g.sync_sigma_s.unwrap_or(req.sync_sigma_s) * C;
            (r * r + s * s + req.extra_sigma_m.powi(2)).sqrt()
        })
        .collect();
    let pos: Vec<(f64, f64)> = req.gnb.iter().map(|g| (g.east_m, g.north_m)).collect();
    let points: Vec<PrsPoint> = req
        .users
        .iter()
        .map(|u| {
            // Reference: the gNB with the smallest ranging sigma (strongest).
            let r = (0..sig.len())
                .min_by(|a, b| sig[*a].total_cmp(&sig[*b]))
                .unwrap_or(0);
            let d = tdoa_drms(&pos, &sig, r, u[0], u[1]);
            PrsPoint {
                east_m: u[0],
                north_m: u[1],
                reference: req.gnb[r].name.clone(),
                drms_m: d,
                two_drms_m: d.map(|x| 2.0 * x),
            }
        })
        .collect();
    let worst = points
        .iter()
        .filter_map(|p| p.two_drms_m)
        .fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| m.max(x))));
    let verdict = format!(
        "{} MHz, {} symbols, sync σ {:.0} ns: worst 2DRMS over {} user position(s) {}",
        req.bandwidth_mhz,
        req.symbols,
        req.sync_sigma_s * 1e9,
        points.len(),
        worst
            .map(|w| format!("{w:.2} m"))
            .unwrap_or_else(|| "undefined (geometry)".into())
    );
    Ok(PrsReport {
        format: PRS_FORMAT,
        label: LABEL,
        title: req.title.clone(),
        ranging_sigma_m: req.gnb.iter().map(|g| g.name.clone()).zip(sig).collect(),
        points,
        verdict,
    })
}

// ---------------------------------------------------------------------------------------
// Integrity vectors.

/// One satellite of a vector.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sat {
    pub az_deg: f64,
    pub el_deg: f64,
    /// Overbounding standard deviation of the range error (m).
    pub sigma_m: f64,
    /// Overbounding bias (m).
    pub bias_m: f64,
}

/// One vector case.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub id: String,
    pub sats: Vec<Sat>,
    /// Target integrity risk (per epoch).
    pub tir: f64,
    pub al_m: f64,
    pub expected_hpl_m: f64,
    pub expected_available: bool,
}

/// The vector set.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VectorSet {
    pub format: String,
    pub basis: String,
    pub formula: String,
    pub tolerance_m: f64,
    pub cases: Vec<Case>,
}

pub const VECTOR_BASIS: &str = "Kshana reference vectors for GNSS positioning with integrity (3GPP Release 18 introduced integrity for UE-based positioning). Independent of any test-system vendor. Not the 3GPP conformance cases of TS 37.571-5 or TS 38.171.";
pub const FORMULA: &str = "Weighted least squares (W = diag(1/σ²)) over east, north, up and clock; S = (GᵀWG)⁻¹GᵀW; HPL = K·sqrt(λmax(P_EN)) + sqrt((Σ|S_e,i|·b_i)² + (Σ|S_n,i|·b_i)²), K = Φ⁻¹(1 − TIR/2); available when HPL < AL.";

/// Invert a small square matrix (Gauss-Jordan with partial pivoting).
pub fn invert_m(a: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    super::rail::invert(a)
}

/// The horizontal protection level of a case's inputs.
pub fn hpl(sats: &[Sat], tir: f64) -> Option<f64> {
    if sats.len() < 4 || !(tir > 0.0 && tir < 1.0) {
        return None;
    }
    let g: Vec<[f64; 4]> = sats
        .iter()
        .map(|s| {
            let (a, e) = (s.az_deg.to_radians(), s.el_deg.to_radians());
            [-e.cos() * a.sin(), -e.cos() * a.cos(), -e.sin(), 1.0]
        })
        .collect();
    let w: Vec<f64> = sats.iter().map(|s| 1.0 / s.sigma_m.powi(2)).collect();
    let mut n = vec![vec![0.0; 4]; 4];
    for (gi, wi) in g.iter().zip(&w) {
        for a in 0..4 {
            for b in 0..4 {
                n[a][b] += gi[a] * wi * gi[b];
            }
        }
    }
    let p = invert_m(&n)?;
    // S = P Gᵀ W: rows east and north.
    let s_row = |r: usize| -> Vec<f64> {
        g.iter()
            .zip(&w)
            .map(|(gi, wi)| (0..4).map(|k| p[r][k] * gi[k]).sum::<f64>() * wi)
            .collect()
    };
    let (se, sn) = (s_row(0), s_row(1));
    let (a, b, c) = (p[0][0], p[0][1], p[1][1]);
    let lmax = 0.5 * (a + c) + ((0.5 * (a - c)).powi(2) + b * b).sqrt();
    let k = norm_quantile(1.0 - tir / 2.0);
    let be: f64 = se.iter().zip(sats).map(|(x, s)| x.abs() * s.bias_m).sum();
    let bn: f64 = sn.iter().zip(sats).map(|(x, s)| x.abs() * s.bias_m).sum();
    Some(k * lmax.sqrt() + (be * be + bn * bn).sqrt())
}

/// The built-in vector set: deterministic geometries from 5 to 12 satellites, three
/// overbound levels, three integrity risks and two alert limits.
pub fn vectors() -> VectorSet {
    let mut cases = Vec::new();
    let mut id = 0;
    for nsat in [5usize, 7, 9, 12] {
        for (sig, bias) in [(0.5, 0.1), (2.0, 0.5), (6.0, 2.0)] {
            for tir in [1e-5, 1e-7] {
                for al in [3.0, 25.0] {
                    id += 1;
                    let sats: Vec<Sat> = (0..nsat)
                        .map(|i| {
                            let el = 15.0 + (i * 37 % 70) as f64;
                            Sat {
                                az_deg: (i as f64 * 360.0 / nsat as f64 + 11.0 * nsat as f64)
                                    % 360.0,
                                el_deg: el,
                                sigma_m: sig / (el.to_radians().sin()).sqrt(),
                                bias_m: bias,
                            }
                        })
                        .collect();
                    let h = hpl(&sats, tir).unwrap_or(f64::INFINITY);
                    let hr = (h * 1000.0).round() / 1000.0;
                    cases.push(Case {
                        id: format!("INT-{id:03}"),
                        sats,
                        tir,
                        al_m: al,
                        expected_hpl_m: hr,
                        expected_available: h < al,
                    });
                }
            }
        }
    }
    VectorSet {
        format: VECTORS_FORMAT.into(),
        basis: VECTOR_BASIS.into(),
        formula: FORMULA.into(),
        tolerance_m: 0.01,
        cases,
    }
}

/// One checked case.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CaseCheck {
    pub id: String,
    pub expected_hpl_m: f64,
    pub reported_hpl_m: Option<f64>,
    pub expected_available: bool,
    pub reported_available: Option<bool>,
    /// `agrees`, `HPL differs`, `availability differs`, `optimistic HPL` or `not reported`.
    pub result: String,
}

/// The check report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CheckReport {
    pub format: &'static str,
    pub label: &'static str,
    pub basis: &'static str,
    pub cases: Vec<CaseCheck>,
    pub agree: usize,
    pub verdict: String,
}

/// Check a device's outputs (`case,hpl_m,available`) against a vector set.
pub fn check_vectors(set: &VectorSet, out: &Table) -> Result<CheckReport, String> {
    let (cc, ch, ca) = (out.need("case")?, out.need("hpl_m")?, out.col("available"));
    let mut cases = Vec::new();
    for c in &set.cases {
        let row = (0..out.rows.len()).find(|&i| out.rows[i][cc] == c.id);
        let rep = row.map(|i| out.num(i, Some(ch))).transpose()?.flatten();
        let av = row.and_then(|i| match out.text(i, ca) {
            "" => None,
            v => Some(matches!(v, "1" | "true" | "yes")),
        });
        let result = match rep {
            None => "not reported",
            Some(h) if h < c.expected_hpl_m - set.tolerance_m => "optimistic HPL",
            Some(h) if (h - c.expected_hpl_m).abs() > set.tolerance_m => "HPL differs",
            Some(_) if av.is_some_and(|a| a != c.expected_available) => "availability differs",
            Some(_) => "agrees",
        };
        cases.push(CaseCheck {
            id: c.id.clone(),
            expected_hpl_m: c.expected_hpl_m,
            reported_hpl_m: rep,
            expected_available: c.expected_available,
            reported_available: av,
            result: result.into(),
        });
    }
    let agree = cases.iter().filter(|c| c.result == "agrees").count();
    let opt = cases
        .iter()
        .filter(|c| c.result == "optimistic HPL")
        .count();
    Ok(CheckReport {
        format: CHECK_FORMAT,
        label: LABEL,
        basis: VECTOR_BASIS,
        verdict: format!(
            "{agree} of {} cases agree with the reference within {} m; {opt} optimistic",
            cases.len(),
            set.tolerance_m
        ),
        cases,
        agree,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_and_guard() {
        assert!((symbol_s(1) - 35.714e-6).abs() < 1e-8);
    }

    #[test]
    fn crlb_scales_with_bandwidth() {
        let a = crlb_range_m(100e6, 100.0);
        let b = crlb_range_m(20e6, 100.0);
        assert!((b / a - 5.0).abs() < 1e-9);
        assert!(a > 0.01 && a < 1.0, "{a}");
    }

    #[test]
    fn hpl_grows_with_risk_and_sigma() {
        let v = vectors();
        assert_eq!(v.cases.len(), 48);
        let s = &v.cases[0].sats;
        let a = hpl(s, 1e-5).unwrap();
        let b = hpl(s, 1e-7).unwrap();
        assert!(b > a);
        // Single-satellite symmetry: four equal-elevation satellites plus zenith.
        let mut sats: Vec<Sat> = (0..4)
            .map(|i| Sat {
                az_deg: 90.0 * i as f64,
                el_deg: 45.0,
                sigma_m: 1.0,
                bias_m: 0.0,
            })
            .collect();
        sats.push(Sat {
            az_deg: 0.0,
            el_deg: 90.0,
            sigma_m: 1.0,
            bias_m: 0.0,
        });
        let h = hpl(&sats, 1e-7).unwrap();
        // Horizontal variance per axis = 1/(2 cos²45°) = 1; K(1e-7) ≈ 5.33.
        assert!((h - 5.327).abs() < 0.01, "{h}");
    }
}
