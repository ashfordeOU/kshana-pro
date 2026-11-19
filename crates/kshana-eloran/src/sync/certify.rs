// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Sync-Verify: an alignment certificate for an eLoran simulator coupled to any GNSS
//! simulator (any vendor), from time-interval-counter readings.
//!
//! The certificate states the measured `t_eLoran − t_GNSS` offset with its expanded
//! uncertainty (k = 2, combining the autocorrelation-aware Type A term with the cable and
//! TIC Type B terms of [`super::measure`]), TDEV at the standard averaging times, the drift,
//! and the verdict against the stated tolerance: the offset passes only when
//! `|offset − target| + U ≤ tolerance` (a guard-banded decision, so the certificate never
//! passes a set-up whose uncertainty straddles the limit). It names the simulators, the
//! counter and its calibration due date, the cable delays applied, and the method.
//!
//! The readings are stored beside the certificate and the certificate carries their SHA-256
//! and a binding hash, so [`verify_certificate`] recomputes the whole analysis from the raw
//! readings; signing the directory (kshana-pro `dossier sign`) makes it attributable.
//!
//! Evidence: the statistics are those of [`super::measure`] (MODELLED, internal tests);
//! the certificate is only as good as the counter's calibration, which it records but cannot
//! check.

use super::measure::{analyse, MeasureConfig, SyncMeasurementReport, TicSeries};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const FORMAT: &str = "kshana-eloran-sync-certificate/1";
pub const READINGS_FILE: &str = "tic_readings_ns.csv";

/// Who and what was measured.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Setup {
    pub eloran_simulator: String,
    pub gnss_simulator: String,
    pub counter: String,
    pub counter_serial: String,
    pub counter_cal_due: Option<String>,
    pub reference: String,
    pub laboratory: String,
    pub notes: String,
}

/// TDEV at one standard τ.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TdevAt {
    pub tau_s: f64,
    pub tdev_ns: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// |offset − target| + U ≤ tolerance and TDEV within its limit.
    Conforms,
    /// |offset − target| − U > tolerance, or TDEV over its limit: out for certain.
    DoesNotConform,
    /// The uncertainty interval straddles the tolerance: no decision.
    Inconclusive,
}

/// The certificate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Certificate {
    pub format: String,
    pub setup: Setup,
    pub config: MeasureConfig,
    pub readings: usize,
    pub duration_s: f64,
    pub readings_sha256: String,
    /// Mean `t_eLoran − t_GNSS` (ns) after cable delays and polarity.
    pub offset_ns: f64,
    pub target_ns: f64,
    /// Expanded uncertainty, k = 2 (ns).
    pub expanded_u_ns: f64,
    pub tolerance_ns: f64,
    pub tdev: Vec<TdevAt>,
    pub drift_ns_per_day: Option<f64>,
    pub decision: Decision,
    pub statement: String,
    pub method: String,
    pub binding_sha256: String,
}

fn sha(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

fn readings_csv(r: &[f64]) -> String {
    let mut s = String::from("reading_ns\n");
    for v in r {
        s += &format!("{v:?}\n");
    }
    s
}

fn binding(c: &Certificate) -> String {
    let mut x = c.clone();
    x.binding_sha256 = String::new();
    sha(serde_json::to_string(&x).unwrap_or_default().as_bytes())
}

/// Build the certificate from raw readings (ns, as the counter gave them).
pub fn certify(
    readings_ns: &[f64],
    cfg: &MeasureConfig,
    setup: &Setup,
) -> Result<(Certificate, SyncMeasurementReport), String> {
    let rep = analyse(&TicSeries::from_ns(readings_ns), cfg)?;
    let u = 2.0 * rep.correction.u_ns;
    let dev = (rep.mean_ns - cfg.target_ns).abs();
    let tol = cfg.offset_limit_ns;
    let tdev_over = rep
        .tdev
        .iter()
        .filter(|p| cfg.tdev_max_tau_s.is_none_or(|m| p.tau_s <= m))
        .any(|p| p.value_ns > cfg.tdev_limit_ns);
    let decision = if dev - u > tol || tdev_over {
        Decision::DoesNotConform
    } else if dev + u <= tol {
        Decision::Conforms
    } else {
        Decision::Inconclusive
    };
    // TDEV is computed at octave averaging times; report the point nearest each decade.
    let mut tdev: Vec<TdevAt> = vec![];
    for want in [1.0, 10.0, 100.0, 1000.0, 10000.0] {
        if let Some(p) = rep
            .tdev
            .iter()
            .filter(|p| (p.tau_s / want).log2().abs() <= 0.6)
            .min_by(|a, b| {
                (a.tau_s / want)
                    .log2()
                    .abs()
                    .partial_cmp(&(b.tau_s / want).log2().abs())
                    .unwrap()
            })
        {
            if !tdev.iter().any(|t: &TdevAt| t.tau_s == p.tau_s) {
                tdev.push(TdevAt {
                    tau_s: p.tau_s,
                    tdev_ns: p.value_ns,
                });
            }
        }
    }
    let statement = format!(
        "t_eLoran − t_GNSS = {:.2} ns ± {:.2} ns (k = 2) over {} readings ({:.0} s); target {:.2} ns, tolerance ±{:.2} ns: {}",
        rep.mean_ns,
        u,
        rep.n,
        rep.duration_s,
        cfg.target_ns,
        tol,
        match decision {
            Decision::Conforms => "CONFORMS (guard-banded)",
            Decision::DoesNotConform => "DOES NOT CONFORM",
            Decision::Inconclusive => "INCONCLUSIVE (uncertainty straddles the tolerance)",
        }
    );
    let mut c = Certificate {
        format: FORMAT.into(),
        setup: setup.clone(),
        config: cfg.clone(),
        readings: rep.n,
        duration_s: rep.duration_s,
        readings_sha256: sha(readings_csv(readings_ns).as_bytes()),
        offset_ns: rep.mean_ns,
        target_ns: cfg.target_ns,
        expanded_u_ns: u,
        tolerance_ns: tol,
        tdev,
        drift_ns_per_day: rep.drift.as_ref().map(|d| d.slope_ns_per_day),
        decision,
        statement,
        method: "TIC start/stop per the configured polarity; cable delays removed; Type A from the autocorrelation-aware effective sample size; Type B from cable and TIC terms; k = 2; guard-banded decision (ILAC-G8 style)".into(),
        binding_sha256: String::new(),
    };
    c.binding_sha256 = binding(&c);
    Ok((c, rep))
}

/// Human-readable certificate.
pub fn markdown(c: &Certificate) -> String {
    let s = &c.setup;
    let mut m = format!(
        "# eLoran–GNSS synchronisation certificate\n\n**{}**\n\n| | |\n|---|---|\n| eLoran simulator | {} |\n| GNSS simulator | {} |\n| Counter | {} {} (calibration due {}) |\n| Reference | {} |\n| Laboratory | {} |\n| Readings | {} over {:.0} s, SHA-256 `{}` |\n| Cable delays removed | eLoran {:.3} ns, GNSS {:.3} ns (u = {:.3} ns) |\n| Drift | {} |\n\n| τ (s) | TDEV (ns) |\n|---|---|\n",
        c.statement,
        s.eloran_simulator, s.gnss_simulator, s.counter, s.counter_serial,
        s.counter_cal_due.as_deref().unwrap_or("not stated"),
        s.reference, s.laboratory, c.readings, c.duration_s, c.readings_sha256,
        c.config.eloran_cable_ns, c.config.gnss_cable_ns, c.config.cable_u_ns,
        c.drift_ns_per_day.map(|d| format!("{d:.3} ns/day")).unwrap_or_else(|| "not estimated".into())
    );
    for t in &c.tdev {
        m += &format!("| {} | {:.3} |\n", t.tau_s, t.tdev_ns);
    }
    m += &format!("\nMethod: {}.\n\nBinding SHA-256: `{}`. Verify with `kshana-pro eloran sync verify-cert <dir>`.\n", c.method, c.binding_sha256);
    m
}

/// Write `certificate.json`, `certificate.md` and the readings into `dir`.
pub fn write(dir: &Path, c: &Certificate, readings_ns: &[f64]) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    std::fs::write(dir.join(READINGS_FILE), readings_csv(readings_ns))
        .map_err(|e| e.to_string())?;
    std::fs::write(
        dir.join("certificate.json"),
        serde_json::to_string_pretty(c).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(dir.join("certificate.md"), markdown(c)).map_err(|e| e.to_string())
}

/// Recompute the certificate from the stored readings; returns the problems (empty = intact).
pub fn verify_certificate(dir: &Path) -> Result<Vec<String>, String> {
    let c: Certificate = serde_json::from_str(
        &std::fs::read_to_string(dir.join("certificate.json"))
            .map_err(|e| format!("certificate.json: {e}"))?,
    )
    .map_err(|e| format!("certificate.json: {e}"))?;
    let mut bad = vec![];
    if binding(&c) != c.binding_sha256 {
        bad.push("binding hash does not match (the certificate was edited)".into());
    }
    let text = std::fs::read_to_string(dir.join(READINGS_FILE))
        .map_err(|e| format!("{READINGS_FILE}: {e}"))?;
    if sha(text.as_bytes()) != c.readings_sha256 {
        bad.push("the readings file differs from the one certified".into());
        return Ok(bad);
    }
    let r: Vec<f64> = text
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.trim().parse::<f64>().map_err(|e| format!("{l}: {e}")))
        .collect::<Result<_, _>>()?;
    let (again, _) = certify(&r, &c.config, &c.setup)?;
    if again.binding_sha256 != c.binding_sha256 {
        bad.push("recomputing from the readings gives a different certificate".into());
    }
    Ok(bad)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn readings(offset: f64, sigma: f64, n: usize, seed: u64) -> Vec<f64> {
        let mut k = seed;
        (0..n)
            .map(|_| {
                let mut s = 0.0;
                for _ in 0..12 {
                    k = k
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    s += (k >> 11) as f64 / (1u64 << 53) as f64;
                }
                // reading = -(y) for StartEloranStopGnss with zero cables: y = offset + noise
                -(offset + sigma * (s - 6.0))
            })
            .collect()
    }

    #[test]
    fn guard_banded_decisions() {
        let cfg = MeasureConfig {
            offset_limit_ns: 10.0,
            tdev_limit_ns: 5.0,
            tic_u_ns: 0.2,
            ..Default::default()
        };
        let s = Setup {
            eloran_simulator: "Kshana Pro".into(),
            gnss_simulator: "GNSS sim".into(),
            counter: "TIC".into(),
            ..Default::default()
        };
        let (c, _) = certify(&readings(2.0, 0.3, 600, 1), &cfg, &s).unwrap();
        assert_eq!(c.decision, Decision::Conforms, "{}", c.statement);
        assert!((c.offset_ns - 2.0).abs() < 0.1, "{}", c.offset_ns);
        let (c, _) = certify(&readings(30.0, 0.3, 600, 2), &cfg, &s).unwrap();
        assert_eq!(c.decision, Decision::DoesNotConform);
        // Offset right at the tolerance: the interval straddles it.
        let (c, _) = certify(&readings(10.0, 0.3, 600, 3), &cfg, &s).unwrap();
        assert_eq!(c.decision, Decision::Inconclusive, "{}", c.statement);
        assert!(c.tdev.len() >= 3 && c.tdev[0].tau_s == 1.0, "{:?}", c.tdev);
    }

    #[test]
    fn certificate_recomputes_and_detects_edits() {
        let cfg = MeasureConfig {
            tic_u_ns: 0.2,
            ..Default::default()
        };
        let r = readings(1.0, 0.5, 200, 9);
        let (c, _) = certify(&r, &cfg, &Setup::default()).unwrap();
        let dir = std::env::temp_dir().join(format!("kshana-synccert-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write(&dir, &c, &r).unwrap();
        assert!(verify_certificate(&dir).unwrap().is_empty());
        let mut t = std::fs::read_to_string(dir.join(READINGS_FILE)).unwrap();
        t = t.replacen("\n-", "\n-9", 1);
        std::fs::write(dir.join(READINGS_FILE), t).unwrap();
        assert!(verify_certificate(&dir).unwrap()[0].contains("readings file differs"));
        write(&dir, &c, &r).unwrap();
        let mut cc = c.clone();
        cc.decision = Decision::Conforms;
        cc.offset_ns = 0.0;
        std::fs::write(
            dir.join("certificate.json"),
            serde_json::to_string(&cc).unwrap(),
        )
        .unwrap();
        assert!(verify_certificate(&dir).unwrap()[0].contains("binding"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
