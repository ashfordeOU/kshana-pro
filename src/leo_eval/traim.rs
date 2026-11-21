// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! T-RAIM fault-injection study for LEO onboard GNSS (B2107, evaluation only).
//!
//! A LEO satellite that takes position and time from GNSS needs to know when a GNSS satellite
//! is faulty. This study runs the open engine's snapshot RAIM with single-fault exclusion
//! ([`kshana::raim::snapshot_raim_fde`], position and clock solved together, the time-RAIM
//! case) on a stated geometry with injected faults, and reports for each number of satellites
//! in view (the redundancy sweep):
//!
//! * the false-alarm rate on fault-free epochs against the stated `p_fa`;
//! * per injected fault (one satellite, or several together: the multi-fault case that low
//!   redundancy cannot resolve), the detection rate, the share of epochs where exactly the faulty
//!   satellite was excluded, and the **recovery time**: epochs after the fault ends until the
//!   test passes without exclusion;
//!
//! Geometry: the LEO at the stated altitude above the reference point; GNSS satellites at the
//! stated orbit radius, in directions spread deterministically over azimuth and elevation above
//! the LEO's local horizon; static over the study. Noise is Gaussian with the stated sigma from a
//! seeded generator. Evaluation of a published algorithm on stated fixtures; no new onboard
//! integrity method is built here. MODELLED.

use serde::{Deserialize, Serialize};

/// Earth radius (m) and GNSS orbit radius used when none is stated (m).
const RE: f64 = 6_371_000.0;

/// One injected fault.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fault {
    pub id: String,
    /// Indices of the faulty satellites (0-based, in the generated order).
    pub sats: Vec<usize>,
    pub bias_m: f64,
    pub start: usize,
    pub end: usize,
}

/// The study request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Study {
    pub altitude_km: f64,
    #[serde(default = "d_gnss")]
    pub gnss_radius_km: f64,
    pub in_view: Vec<usize>,
    pub sigma_m: f64,
    pub p_fa: f64,
    pub p_md: f64,
    pub epochs: usize,
    pub seed: u64,
    #[serde(rename = "fault")]
    pub faults: Vec<Fault>,
}

fn d_gnss() -> f64 {
    26_560.0
}

/// One fault's outcome at one redundancy.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FaultOutcome {
    pub fault: String,
    pub detection_rate: f64,
    pub correct_exclusion_rate: f64,
    /// Epochs after the fault ends until the first clean epoch; `None` if never in the study.
    pub recovery_epochs: Option<usize>,
}

/// Outcome at one number of satellites in view.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Row {
    pub in_view: usize,
    pub redundancy: usize,
    pub fault_free_epochs: usize,
    pub false_alarm_rate: f64,
    pub faults: Vec<FaultOutcome>,
}

/// The study result.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Result_ {
    pub format: &'static str,
    pub study: Study,
    pub rows: Vec<Row>,
    pub statement: &'static str,
}

/// Format tag.
pub const FORMAT: &str = "kshana-pro-leo-traim-study/1";

struct Rng(u64);
impl Rng {
    fn u(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }
    fn n(&mut self) -> f64 {
        let (a, b) = (self.u(), self.u());
        (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos()
    }
}

/// The LEO position and the GNSS satellite positions (ECEF-like, metres).
pub fn geometry(altitude_km: f64, gnss_radius_km: f64, n: usize) -> ([f64; 3], Vec<[f64; 3]>) {
    let user = [0.0, 0.0, RE + altitude_km * 1000.0];
    let rg = gnss_radius_km * 1000.0;
    let sats = (0..n)
        .map(|i| {
            let az = std::f64::consts::TAU * (i as f64 / n as f64) + 0.3;
            let frac = (i as f64 * 0.618_033_988_75).fract();
            let el = (10.0 + 75.0 * frac).to_radians();
            let u = [el.cos() * az.cos(), el.cos() * az.sin(), el.sin()];
            // |user + d·u| = rg.
            let b = 2.0 * (user[2] * u[2]);
            let c = user[2] * user[2] - rg * rg;
            let d = (-b + (b * b - 4.0 * c).sqrt()) / 2.0;
            [user[0] + d * u[0], user[1] + d * u[1], user[2] + d * u[2]]
        })
        .collect();
    (user, sats)
}

/// Run the study.
pub fn run(q: &Study) -> Result<Result_, String> {
    if !(q.altitude_km > 0.0 && q.gnss_radius_km * 1000.0 > RE + q.altitude_km * 1000.0) {
        return Err("the GNSS orbit radius must exceed the LEO orbit radius".into());
    }
    if !(q.sigma_m > 0.0 && q.p_fa > 0.0 && q.p_fa < 1.0 && q.p_md > 0.0 && q.p_md < 1.0) {
        return Err("sigma_m must be positive, p_fa and p_md in (0, 1)".into());
    }
    if q.in_view.iter().any(|n| *n < 5) || q.in_view.is_empty() || q.epochs == 0 {
        return Err(
            "in_view needs at least 5 satellites per case (RAIM redundancy) and epochs > 0".into(),
        );
    }
    for f in &q.faults {
        if f.start >= f.end || f.end > q.epochs || f.sats.is_empty() {
            return Err(format!(
                "fault '{}': needs satellites and start < end <= epochs",
                f.id
            ));
        }
    }
    let mut rows = Vec::new();
    for &n in &q.in_view {
        let (user, sats) = geometry(q.altitude_km, q.gnss_radius_km, n);
        let mut rng = Rng(q.seed ^ n as u64);
        let mut free = 0usize;
        let mut fa = 0usize;
        let mut per: Vec<(usize, usize, usize, Option<usize>)> =
            vec![(0, 0, 0, None); q.faults.len()];
        let mut clean_after: Vec<bool> = vec![false; q.faults.len()];
        for k in 0..q.epochs {
            let mut res: Vec<f64> = (0..n).map(|_| q.sigma_m * rng.n()).collect();
            let active: Vec<usize> = q
                .faults
                .iter()
                .enumerate()
                .filter(|(_, f)| k >= f.start && k < f.end)
                .map(|(i, _)| i)
                .collect();
            for &i in &active {
                for &s in q.faults[i].sats.iter().filter(|s| **s < n) {
                    res[s] += q.faults[i].bias_m;
                }
            }
            let Some(r) =
                kshana::raim::snapshot_raim_fde(user, &sats, &res, q.sigma_m, q.p_fa, q.p_md)
            else {
                return Err(format!("{n} satellites: singular geometry"));
            };
            let det = r.all_in_view.fault_detected;
            if active.is_empty() {
                free += 1;
                if det {
                    fa += 1;
                }
            }
            for &i in &active {
                let f = &q.faults[i];
                per[i].0 += 1;
                if det {
                    per[i].1 += 1;
                }
                if f.sats.len() == 1 && r.excluded == Some(f.sats[0]) {
                    per[i].2 += 1;
                }
            }
            for (i, f) in q.faults.iter().enumerate() {
                if k >= f.end && !clean_after[i] && active.is_empty() && !det {
                    clean_after[i] = true;
                    per[i].3 = Some(k - f.end);
                }
            }
        }
        rows.push(Row {
            in_view: n,
            redundancy: n - 4,
            fault_free_epochs: free,
            false_alarm_rate: if free > 0 {
                fa as f64 / free as f64
            } else {
                f64::NAN
            },
            faults: q
                .faults
                .iter()
                .zip(&per)
                .map(|(f, p)| FaultOutcome {
                    fault: f.id.clone(),
                    detection_rate: if p.0 > 0 {
                        p.1 as f64 / p.0 as f64
                    } else {
                        f64::NAN
                    },
                    correct_exclusion_rate: if p.0 > 0 {
                        p.2 as f64 / p.0 as f64
                    } else {
                        f64::NAN
                    },
                    recovery_epochs: p.3,
                })
                .collect(),
        });
    }
    Ok(Result_ {
        format: FORMAT,
        study: q.clone(),
        rows,
        statement: "Evaluation of the open engine's snapshot RAIM with single-fault exclusion on stated fixtures; not a new onboard integrity method. MODELLED.",
    })
}
