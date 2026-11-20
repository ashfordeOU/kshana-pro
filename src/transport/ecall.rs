// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # eCall GNSS tests, emergency-location reliability and tracker duty cycling (backlog B3003)
//!
//! Research 2026-11-20 gaps 07:I11, K02, K03 and 08:D7-D9, E5, E9, E12.
//!
//! * [`SUITE`]: the GNSS receiver tests of Commission Delegated Regulation (EU) 2017/79,
//!   Annex VI, section 2.2 (NMEA output, static, dynamic and shadow-area accuracy, cold-start
//!   time to first fix at −130 and −140 dBm, re-acquisition after a 60 s block-out, and the
//!   three sensitivity checks of 2.2.7), each with the limit of Annex VI section 1.2 it
//!   checks. Read from the consolidated text on legislation.gov.uk (retrieved 20 Nov 2026,
//!   [`SOURCE_URL`]). [`MAPPING`] maps every Annex VI requirement to its tests, with the
//!   corresponding UN Regulation No 144 test (Annex 8) and the ERA-GLONASS note. Two
//!   interference tests are Kshana's own extensions and say so; they are not in the
//!   regulation.
//! * [`score`]: laboratory results per eCall sample (NMEA logs, horizontal error series,
//!   timed runs) against each test's limit and statistic: within the stated limit, beyond it,
//!   or not assessed (no data, or fewer samples or repeats than the method asks for).
//!   Wording is "checked against 2017/79 Annex VI §x"; this is a pre-assessment for the
//!   manufacturer and its technical service, never a type-approval.
//! * [`location`]: emergency-location (AML/E112, PSAP) reliability: per positioning source,
//!   with and without an interference flag, how often the verified error was within the
//!   reported uncertainty radius against the confidence the handset declared (Wilson
//!   interval): consistent, over-confident or under-confident.
//! * [`duty`]: detection of a duty-cycled GNSS receiver in a tracker log (regular off periods
//!   and on bursts), and [`battery`]: the IoT tracker battery against fix interval trade-off
//!   (start type by off time, energy per fix, average power, life, position age).
//!
//! **Label: MODELLED.** The scoring is arithmetic on the results as supplied; the limits
//! and methods are those of the text cited, row by row.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{dist_m, mean, quantile, wilson95, Table};

pub const FEATURE: &str = "pro-ecall-tests";
pub const FEATURE_LOCATION: &str = "pro-emergency-location";
pub const FORMAT: &str = "kshana-pro-ecall-gnss-tests/1";
pub const LOCATION_FORMAT: &str = "kshana-pro-emergency-location/1";
pub const LABEL: &str = "MODELLED: arithmetic on the laboratory results as supplied, against limits and statistics read from the text cited per row. A pre-assessment for the manufacturer and its technical service, not a type-approval and not a statement that any system meets the regulation.";
pub const SOURCE: &str = "Commission Delegated Regulation (EU) 2017/79, Annex VI (eCall compatibility with the positioning services of Galileo and EGNOS)";
pub const SOURCE_URL: &str = "https://www.legislation.gov.uk/eur/2017/79/data.htm?view=plain";
pub const SOURCE_READ: &str = "Annex VI sections 1 and 2 read from the legislation.gov.uk consolidated text on 20 Nov 2026 (paragraphs 1.2.1-1.2.10, 2.1, 2.2.1-2.2.7.13). The EUR-Lex text is authoritative; confirm before submission.";
pub const R144_NOTE: &str = "UN Regulation No 144 (AECS/AECD) carries GNSS receiver test methods in its Annex 8 aligned with these; the Annex 8 paragraph numbers were not read in this work and are to be confirmed against the R144 text.";
pub const ERA_GLONASS_NOTE: &str = "ERA-GLONASS (GOST 33471/33464) needs GLONASS reception and has its own limits; not assessed here. Screen any such work against sanctions before starting.";

/// How a test's values are reduced.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Statistic {
    /// NMEA presence and rate.
    Nmea,
    /// 95th percentile of horizontal error per sample, every sample at most the limit.
    P95PerSample,
    /// Mean over all runs of all samples, at most the limit.
    MeanAllRuns,
    /// Every value of every sample at most the limit.
    MaxEach,
    /// Every value of every sample at least the limit.
    MinEach,
    /// Share of epochs with error beyond the limit while the position was reported trusted.
    TrustedBeyond,
}

/// One test.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Test {
    pub id: &'static str,
    /// Annex VI test paragraph, or `Kshana extension`.
    pub para: &'static str,
    /// The requirement paragraph(s) of Annex VI section 1.2 it checks.
    pub requirement: &'static str,
    pub title: &'static str,
    pub statistic: Statistic,
    /// The limit; `None` for the extensions, whose limit the applicant states.
    pub limit: Option<f64>,
    pub unit: &'static str,
    /// Minimum repeats per sample the method asks for.
    pub min_runs: usize,
    pub conditions: &'static str,
}

/// Annex VI 2.1: at least three eCall samples.
pub const MIN_SAMPLES: usize = 3;

/// The test suite.
pub const SUITE: &[Test] = &[
    Test { id: "nmea", para: "2.2.1", requirement: "1.2.1, 1.2.10, 1.1", title: "NMEA-0183 output (RMC, GGA, VTG, GSA, GSV) at 1 Hz", statistic: Statistic::Nmea, limit: Some(1.0), unit: "s between GGA", min_runs: 1, conditions: "1 h static, Galileo + GPS + SBAS, PDOP 2.0-2.5, Galileo -135 dBm, GPS -138.5 dBm, at least 6 + 6 + 2 satellites" },
    Test { id: "static-combined", para: "2.2.2", requirement: "1.2.3, 1.2.5", title: "Horizontal error, static, combined Galileo + GPS + SBAS", statistic: Statistic::P95PerSample, limit: Some(15.0), unit: "m (95 %)", min_runs: 1, conditions: "open sky, PDOP 2.0-2.5, as 2.2.1" },
    Test { id: "static-galileo", para: "2.2.2", requirement: "1.2.2, 1.2.5, 1.1", title: "Horizontal error, static, Galileo only", statistic: Statistic::P95PerSample, limit: Some(15.0), unit: "m (95 %)", min_runs: 1, conditions: "open sky, PDOP 2.0-2.5" },
    Test { id: "static-gps", para: "2.2.2", requirement: "1.2.2, 1.2.5", title: "Horizontal error, static, GPS only", statistic: Statistic::P95PerSample, limit: Some(15.0), unit: "m (95 %)", min_runs: 1, conditions: "open sky, PDOP 2.0-2.5" },
    Test { id: "dynamic", para: "2.2.3", requirement: "1.2.5, 1.2.6", title: "Horizontal error, dynamic", statistic: Statistic::P95PerSample, limit: Some(15.0), unit: "m (95 %)", min_runs: 1, conditions: "1 h manoeuvring at 140 km/h, 500 m turn radius, 0.2 m/s² turning acceleration, PDOP 2.0-2.5" },
    Test { id: "shadow", para: "2.2.4", requirement: "1.2.5", title: "Horizontal error, shadow areas and urban canyons", statistic: Statistic::P95PerSample, limit: Some(40.0), unit: "m (95 %)", min_runs: 1, conditions: "visibility 300 s on, 600 s off; PDOP 3.5-4.0; zones B and C -40 dB, zone A -100 dB or off" },
    Test { id: "ttff-130", para: "2.2.5.7", requirement: "1.2.7", title: "Cold-start time to first fix at -130 dBm", statistic: Statistic::MeanAllRuns, limit: Some(60.0), unit: "s (mean)", min_runs: 10, conditions: "cold start, at least 10 repeats per sample (2.2.5.5)" },
    Test { id: "ttff-140", para: "2.2.5.9", requirement: "1.2.7", title: "Cold-start time to first fix at -140 dBm", statistic: Statistic::MeanAllRuns, limit: Some(300.0), unit: "s (mean)", min_runs: 10, conditions: "as 2.2.5 at -140 dBm (2.2.5.8)" },
    Test { id: "reacq", para: "2.2.6.7", requirement: "1.2.8", title: "Re-acquisition after a 60 s block-out at -130 dBm", statistic: Statistic::MeanAllRuns, limit: Some(20.0), unit: "s (mean)", min_runs: 10, conditions: "antenna disconnected 60 s after 15 min navigation, at least 10 repeats (2.2.6.5)" },
    Test { id: "sens-cold-144", para: "2.2.7.13", requirement: "1.2.9", title: "Acquisition sensitivity: cold-start fix at -144 dBm", statistic: Statistic::MaxEach, limit: Some(3600.0), unit: "s (each)", min_runs: 1, conditions: "2.2.7.5-2.2.7.7" },
    Test { id: "sens-track-155", para: "2.2.7.13", requirement: "1.2.9", title: "Tracking sensitivity: navigation held at -155 dBm", statistic: Statistic::MinEach, limit: Some(600.0), unit: "s held (each)", min_runs: 1, conditions: "2.2.7.8-2.2.7.9" },
    Test { id: "sens-reacq-150", para: "2.2.7.13", requirement: "1.2.9", title: "Re-acquisition sensitivity at -150 dBm after a 20 s block-out", statistic: Statistic::MaxEach, limit: Some(60.0), unit: "s (each)", min_runs: 1, conditions: "2.2.7.10-2.2.7.12" },
    Test { id: "ks-jam-recovery", para: "Kshana extension", requirement: "none (not in the regulation)", title: "Time to first fix after a jamming episode ends", statistic: Statistic::MeanAllRuns, limit: None, unit: "s (mean)", min_runs: 10, conditions: "conducted jamming per the applicant's plan; limit stated by the applicant" },
    Test { id: "ks-trusted-error", para: "Kshana extension", requirement: "none (not in the regulation)", title: "Positions reported trusted (MSD positionCanBeTrusted) with error beyond the limit, under interference", statistic: Statistic::TrustedBeyond, limit: None, unit: "share of trusted epochs", min_runs: 1, conditions: "conducted jamming or spoofing per the applicant's plan; error limit stated by the applicant (trusted_error_m)" },
];

/// One Annex VI requirement and its tests.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Requirement {
    pub para: &'static str,
    pub text: &'static str,
    /// Test ids; empty when the requirement is a declaration Kshana does not test.
    pub tests: &'static [&'static str],
    pub note: &'static str,
}

/// Every requirement of Annex VI sections 1.1 and 1.2.
pub const MAPPING: &[Requirement] = &[
    Requirement { para: "1.1", text: "Galileo compatibility (Open Service signals used in the final position) and EGNOS compatibility (corrections applied)", tests: &["static-galileo", "static-combined", "nmea"], note: "EGNOS use is shown by GGA fix quality 2 in the NMEA log (2.2.1)" },
    Requirement { para: "1.2.1", text: "NMEA-0183 output: RMC, GGA, VTG, GSA, GSV", tests: &["nmea"], note: "" },
    Requirement { para: "1.2.2", text: "Individual L1/E1 signals of at least two GNSS including Galileo and GPS", tests: &["static-galileo", "static-gps"], note: "" },
    Requirement { para: "1.2.3", text: "Combined L1/E1 signals of at least two GNSS including Galileo and GPS, plus SBAS", tests: &["static-combined"], note: "" },
    Requirement { para: "1.2.4", text: "Coordinates in WGS-84", tests: &[], note: "a declaration; not tested by Kshana" },
    Requirement { para: "1.2.5", text: "Horizontal error (95 %): 15 m open sky (PDOP 2.0-2.5), 40 m urban canyon (PDOP 3.5-4.0)", tests: &["static-combined", "static-galileo", "static-gps", "dynamic", "shadow"], note: "" },
    Requirement { para: "1.2.6", text: "Operating range: speed 0-140 km/h, linear acceleration 0-2 g", tests: &["dynamic"], note: "2.2.3 exercises 140 km/h; the 2 g acceleration bound is not exercised by the method" },
    Requirement { para: "1.2.7", text: "Cold-start time to first fix: 60 s at -130 dBm, 300 s at -140 dBm", tests: &["ttff-130", "ttff-140"], note: "" },
    Requirement { para: "1.2.8", text: "Re-acquisition within 20 s after a 60 s block-out at -130 dBm", tests: &["reacq"], note: "" },
    Requirement { para: "1.2.9", text: "Sensitivity: cold-start fix within 3600 s at -144 dBm; navigation for at least 600 s at -155 dBm; re-acquisition within 60 s at -150 dBm", tests: &["sens-cold-144", "sens-track-155", "sens-reacq-150"], note: "" },
    Requirement { para: "1.2.10", text: "A position fix at least every second", tests: &["nmea"], note: "checked as the GGA interval of the 2.2.1 log" },
];

/// The test by id.
pub fn test(id: &str) -> Option<&'static Test> {
    SUITE.iter().find(|t| t.id == id)
}

/// Requirements with no test and no declaration note, and mapped test ids that do not exist.
pub fn mapping_problems() -> Vec<String> {
    let mut p = Vec::new();
    for r in MAPPING {
        if r.tests.is_empty() && r.note.is_empty() {
            p.push(format!("{}: no test", r.para));
        }
        for t in r.tests {
            if test(t).is_none() {
                p.push(format!("{}: unknown test {t}", r.para));
            }
        }
    }
    for t in SUITE.iter().filter(|t| t.para != "Kshana extension") {
        if !MAPPING.iter().any(|r| r.tests.contains(&t.id)) {
            p.push(format!("test {} checks no requirement", t.id));
        }
    }
    p
}

/// One eCall sample's results.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleResults {
    pub id: String,
    /// The NMEA log of 2.2.1.
    pub nmea: Option<String>,
    /// Test id -> CSV of horizontal error (`err_h_m`, or `lat,lon,truth_lat,truth_lon`;
    /// with a `trusted` column for `ks-trusted-error`).
    #[serde(default)]
    pub errors: BTreeMap<String, String>,
    /// Test id -> timed values (s).
    #[serde(default)]
    pub values: BTreeMap<String, Vec<f64>>,
}

/// The results request.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Results {
    pub title: String,
    /// The system under test as the manufacturer names it.
    pub unit: String,
    pub laboratory: Option<String>,
    /// Limits of the Kshana extensions: `ks-jam-recovery` (s), `ks-trusted-error` (share).
    #[serde(default)]
    pub extension_limits: BTreeMap<String, f64>,
    /// Error limit (m) for `ks-trusted-error`.
    pub trusted_error_m: Option<f64>,
    pub sample: Vec<SampleResults>,
}

/// One test's result.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TestResult {
    pub id: String,
    pub para: String,
    pub requirement: String,
    pub title: String,
    pub limit: Option<f64>,
    pub unit: String,
    pub value: Option<f64>,
    pub per_sample: BTreeMap<String, f64>,
    pub samples: usize,
    pub runs: usize,
    /// `within the stated limit`, `beyond the stated limit` or `not assessed: <why>`.
    pub status: String,
    pub checked_against: String,
}

/// The score report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EcallReport {
    pub format: &'static str,
    pub label: &'static str,
    pub title: String,
    pub unit: String,
    pub laboratory: Option<String>,
    pub utc: String,
    pub source: &'static str,
    pub source_url: &'static str,
    pub source_read: &'static str,
    pub r144_note: &'static str,
    pub era_glonass_note: &'static str,
    pub tests: Vec<TestResult>,
    pub requirements: Vec<RequirementStatus>,
    pub verdict: String,
}

/// A requirement's status from its tests.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RequirementStatus {
    pub para: String,
    pub text: String,
    pub tests: Vec<String>,
    pub status: String,
    pub note: String,
}

/// NMEA log summary.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct NmeaSummary {
    pub sentences: BTreeMap<String, usize>,
    pub missing: Vec<String>,
    pub gga_median_interval_s: Option<f64>,
    pub gga_max_interval_s: Option<f64>,
    pub gga_quality2_share: Option<f64>,
    pub bad_checksum: usize,
}

fn nmea_checksum_ok(line: &str) -> Option<bool> {
    let body = line.strip_prefix('$')?;
    let (data, cs) = body.rsplit_once('*')?;
    let want = u8::from_str_radix(cs.trim().get(..2)?, 16).ok()?;
    Some(data.bytes().fold(0u8, |a, b| a ^ b) == want)
}

/// Summarise an NMEA-0183 log.
pub fn nmea_summary(text: &str) -> NmeaSummary {
    let mut s = NmeaSummary::default();
    let mut gga_t = Vec::new();
    let (mut q2, mut ngga) = (0usize, 0usize);
    for l in text.lines().map(str::trim).filter(|l| l.starts_with('$')) {
        if nmea_checksum_ok(l) == Some(false) {
            s.bad_checksum += 1;
            continue;
        }
        let data = l.split('*').next().unwrap_or("");
        let f: Vec<&str> = data.split(',').collect();
        let Some(ty) = f[0].get(3..6) else { continue };
        *s.sentences.entry(ty.to_string()).or_insert(0) += 1;
        if ty == "GGA" {
            ngga += 1;
            if f.get(6) == Some(&"2") {
                q2 += 1;
            }
            if let Some(t) = f.get(1).filter(|t| t.len() >= 6) {
                let p = |a: usize, b: usize| t.get(a..b).and_then(|x| x.parse::<f64>().ok());
                if let (Some(h), Some(m), Some(sec)) = (
                    p(0, 2),
                    p(2, 4),
                    t.get(4..).and_then(|x| x.parse::<f64>().ok()),
                ) {
                    gga_t.push(h * 3600.0 + m * 60.0 + sec);
                }
            }
        }
    }
    for need in ["RMC", "GGA", "VTG", "GSA", "GSV"] {
        if !s.sentences.contains_key(need) {
            s.missing.push(need.into());
        }
    }
    let iv: Vec<f64> = gga_t
        .windows(2)
        .map(|w| {
            let d = w[1] - w[0];
            if d < -43_200.0 {
                d + 86_400.0
            } else {
                d
            }
        })
        .filter(|d| *d > 0.0)
        .collect();
    s.gga_median_interval_s = quantile(&iv, 0.5);
    s.gga_max_interval_s = iv.iter().copied().reduce(f64::max);
    s.gga_quality2_share = (ngga > 0).then(|| q2 as f64 / ngga as f64);
    s
}

/// Horizontal errors (m) of an error CSV, and the `trusted` flags if present.
pub fn read_errors(t: &Table) -> Result<(Vec<f64>, Option<Vec<bool>>), String> {
    let mut e = Vec::new();
    let tr = t.col("trusted");
    let mut flags = tr.map(|_| Vec::new());
    for i in 0..t.rows.len() {
        let v = if let Some(c) = t.col("err_h_m") {
            t.num(i, Some(c))?
        } else {
            let g = |n: &str| t.num(i, t.col(n));
            match (g("lat")?, g("lon")?, g("truth_lat")?, g("truth_lon")?) {
                (Some(a), Some(b), Some(c), Some(d)) => Some(dist_m(a, b, c, d)),
                (None, _, _, _) | (_, None, _, _) => None,
                _ => {
                    return Err(
                        "an error CSV needs err_h_m or lat, lon, truth_lat, truth_lon".into(),
                    )
                }
            }
        };
        if let Some(v) = v {
            e.push(v);
            if let Some(f) = flags.as_mut() {
                f.push(matches!(t.text(i, tr), "1" | "true" | "yes"));
            }
        }
    }
    Ok((e, flags))
}

fn status(v: Option<f64>, limit: Option<f64>, at_least: bool) -> String {
    match (v, limit) {
        (_, None) => "not assessed: no limit stated by the applicant".into(),
        (None, _) => "not assessed: no data".into(),
        (Some(v), Some(l)) => {
            let ok = if at_least { v >= l } else { v <= l };
            if ok {
                "within the stated limit".into()
            } else {
                "beyond the stated limit".into()
            }
        }
    }
}

/// Score the results (paths relative to `base`).
pub fn score(res: &Results, base: &Path, utc: &str) -> Result<EcallReport, String> {
    if res.sample.is_empty() {
        return Err("no samples".into());
    }
    for s in &res.sample {
        for k in s.errors.keys().chain(s.values.keys()) {
            if test(k).is_none() {
                return Err(format!("sample {}: unknown test '{k}'", s.id));
            }
        }
    }
    let mut out = Vec::new();
    for t in SUITE {
        let limit = if t.para == "Kshana extension" {
            res.extension_limits.get(t.id).copied()
        } else {
            t.limit
        };
        let mut per = BTreeMap::new();
        let mut runs = 0usize;
        let mut all = Vec::new();
        let mut short = Vec::new();
        let mut note = None;
        for s in &res.sample {
            match t.statistic {
                Statistic::Nmea => {
                    let Some(p) = &s.nmea else { continue };
                    let f = base.join(p);
                    let n = nmea_summary(
                        &std::fs::read_to_string(&f)
                            .map_err(|e| format!("{}: {e}", f.display()))?,
                    );
                    runs += 1;
                    // A missing sentence type makes the sample's interval infinite.
                    let v = if n.missing.is_empty() {
                        n.gga_median_interval_s.unwrap_or(f64::INFINITY)
                    } else {
                        note = Some(format!("sample {} lacks {}", s.id, n.missing.join(", ")));
                        f64::INFINITY
                    };
                    per.insert(s.id.clone(), v);
                }
                Statistic::P95PerSample | Statistic::TrustedBeyond => {
                    let Some(p) = s.errors.get(t.id) else {
                        continue;
                    };
                    let (e, flags) = read_errors(&Table::read(&base.join(p))?)?;
                    if e.is_empty() {
                        continue;
                    }
                    runs += e.len();
                    if t.statistic == Statistic::P95PerSample {
                        per.insert(s.id.clone(), quantile(&e, 0.95).unwrap_or(f64::NAN));
                    } else {
                        let Some(lim) = res.trusted_error_m else {
                            note = Some("trusted_error_m not stated".into());
                            continue;
                        };
                        let fl = flags
                            .ok_or(format!("sample {}: {} needs a trusted column", s.id, t.id))?;
                        let trusted = fl.iter().filter(|f| **f).count();
                        let bad = e.iter().zip(&fl).filter(|(x, f)| **f && **x > lim).count();
                        per.insert(
                            s.id.clone(),
                            if trusted > 0 {
                                bad as f64 / trusted as f64
                            } else {
                                0.0
                            },
                        );
                    }
                }
                Statistic::MeanAllRuns | Statistic::MaxEach | Statistic::MinEach => {
                    let Some(v) = s.values.get(t.id).filter(|v| !v.is_empty()) else {
                        continue;
                    };
                    runs += v.len();
                    if v.len() < t.min_runs {
                        short.push(s.id.clone());
                    }
                    all.extend(v.iter().copied());
                    let r = match t.statistic {
                        Statistic::MeanAllRuns => mean(v).unwrap_or(f64::NAN),
                        Statistic::MaxEach => v.iter().copied().fold(f64::MIN, f64::max),
                        _ => v.iter().copied().fold(f64::MAX, f64::min),
                    };
                    per.insert(s.id.clone(), r);
                }
            }
        }
        let worst = |hi: bool| {
            per.values()
                .copied()
                .reduce(|a, b| if hi { a.max(b) } else { a.min(b) })
        };
        let value = match t.statistic {
            Statistic::MeanAllRuns => mean(&all),
            Statistic::MinEach => worst(false),
            _ => worst(true),
        };
        let mut st = status(value, limit, t.statistic == Statistic::MinEach);
        if value.is_some() && limit.is_some() {
            if per.len() < MIN_SAMPLES && t.para != "Kshana extension" {
                st = format!(
                    "not assessed: {} sample(s), the method asks for at least {MIN_SAMPLES} (2.1)",
                    per.len()
                );
            } else if !short.is_empty() {
                st = format!(
                    "not assessed: fewer than {} repeats for {}",
                    t.min_runs,
                    short.join(", ")
                );
            }
        }
        if let (Some(n), true) = (&note, st.starts_with("beyond")) {
            st = format!("{st} ({n})");
        }
        out.push(TestResult {
            id: t.id.into(),
            para: t.para.into(),
            requirement: t.requirement.into(),
            title: t.title.into(),
            limit,
            unit: t.unit.into(),
            value: value.filter(|v| v.is_finite()),
            per_sample: per,
            samples: res.sample.len(),
            runs,
            status: st,
            checked_against: if t.para == "Kshana extension" {
                "Kshana extension; not in the regulation".into()
            } else {
                format!(
                    "checked against 2017/79 Annex VI §{} (limit of §{})",
                    t.para, t.requirement
                )
            },
        });
    }
    let requirements = MAPPING
        .iter()
        .map(|r| {
            let ts: Vec<&TestResult> = out
                .iter()
                .filter(|x| r.tests.contains(&x.id.as_str()))
                .collect();
            let status = if ts.is_empty() {
                "not tested by Kshana".to_string()
            } else if ts.iter().any(|x| x.status.starts_with("beyond")) {
                "a test is beyond its stated limit".into()
            } else if ts.iter().all(|x| x.status.starts_with("within")) {
                "every mapped test within its stated limit".into()
            } else {
                "incomplete: a mapped test was not assessed".into()
            };
            RequirementStatus {
                para: r.para.into(),
                text: r.text.into(),
                tests: r.tests.iter().map(|t| t.to_string()).collect(),
                status,
                note: r.note.into(),
            }
        })
        .collect();
    let c = |p: &str| out.iter().filter(|x| x.status.starts_with(p)).count();
    let verdict = format!(
        "{}: checked against 2017/79 Annex VI: {} test(s) within the stated limit, {} beyond, {} not assessed. Pre-assessment, not a type-approval.",
        res.unit,
        c("within"),
        c("beyond"),
        c("not assessed")
    );
    Ok(EcallReport {
        format: FORMAT,
        label: LABEL,
        title: res.title.clone(),
        unit: res.unit.clone(),
        laboratory: res.laboratory.clone(),
        utc: utc.into(),
        source: SOURCE,
        source_url: SOURCE_URL,
        source_read: SOURCE_READ,
        r144_note: R144_NOTE,
        era_glonass_note: ERA_GLONASS_NOTE,
        tests: out,
        requirements,
        verdict,
    })
}

/// The score page.
pub fn html(r: &EcallReport) -> String {
    use super::fo;
    let tests = r
        .tests
        .iter()
        .map(|t| {
            vec![
                t.id.clone(),
                t.para.clone(),
                t.title.clone(),
                fo(t.limit),
                t.unit.clone(),
                fo(t.value),
                t.runs.to_string(),
                t.status.clone(),
            ]
        })
        .collect();
    let reqs = r
        .requirements
        .iter()
        .map(|q| {
            vec![
                q.para.clone(),
                q.text.clone(),
                q.tests.join(", "),
                q.status.clone(),
                q.note.clone(),
            ]
        })
        .collect();
    super::page(
        &format!("eCall GNSS tests: {}", r.title),
        r.label,
        &r.verdict,
        &[
            format!("{} ({}). {}", r.source, r.source_url, r.source_read),
            r.r144_note.into(),
            r.era_glonass_note.into(),
        ],
        &[
            (
                "Tests".into(),
                vec![
                    "Test",
                    "Annex VI §",
                    "Title",
                    "Limit",
                    "Unit",
                    "Value",
                    "Runs",
                    "Status",
                ],
                tests,
            ),
            (
                "Requirements".into(),
                vec![
                    "Annex VI §",
                    "Requirement (paraphrase)",
                    "Tests",
                    "Status",
                    "Note",
                ],
                reqs,
            ),
        ],
    )
}

// ---------------------------------------------------------------------------------------
// Emergency-location reliability.

/// One group of calls.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LocationGroup {
    pub source: String,
    pub interference: String,
    pub calls: usize,
    pub verified: usize,
    pub within_radius: usize,
    pub within_share: Option<f64>,
    pub within_wilson95: Option<(f64, f64)>,
    pub declared_confidence: Option<f64>,
    /// `consistent with the declared confidence`, `over-confident`, `under-confident` or
    /// `not assessed`.
    pub reliability: String,
    pub median_radius_m: Option<f64>,
    pub median_error_m: Option<f64>,
    pub p95_error_m: Option<f64>,
    pub median_time_to_location_s: Option<f64>,
}

/// The emergency-location report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LocationReport {
    pub format: &'static str,
    pub label: &'static str,
    pub title: String,
    pub groups: Vec<LocationGroup>,
    pub verdict: String,
}

/// Reliability statistics of emergency-call locations. Columns: `source`, `radius_m`,
/// `confidence` (per cent), optional `err_m` (verified error), `interference` (0/1),
/// `ttl_s` (time to location). Caller identifiers, if present, are ignored.
pub fn location(t: &Table, title: &str) -> Result<LocationReport, String> {
    let (src, rad, conf) = (
        t.need("source")?,
        t.need("radius_m")?,
        t.need("confidence")?,
    );
    let (err, intf, ttl) = (t.col("err_m"), t.col("interference"), t.col("ttl_s"));
    #[derive(Default)]
    struct Acc {
        calls: usize,
        radius: Vec<f64>,
        conf: Vec<f64>,
        errs: Vec<f64>,
        within: usize,
        ttl: Vec<f64>,
    }
    let mut g: BTreeMap<(String, String), Acc> = BTreeMap::new();
    for i in 0..t.rows.len() {
        let flag = match t.text(i, intf) {
            "" => "unknown",
            "1" | "true" | "yes" => "interference flagged",
            _ => "no interference flag",
        };
        let a = g
            .entry((t.text(i, Some(src)).to_string(), flag.to_string()))
            .or_default();
        a.calls += 1;
        let r = t.num(i, Some(rad))?;
        if let Some(r) = r {
            a.radius.push(r);
        }
        if let Some(c) = t.num(i, Some(conf))? {
            a.conf.push(c);
        }
        if let Some(x) = t.num(i, ttl)? {
            a.ttl.push(x);
        }
        if let (Some(e), Some(r)) = (t.num(i, err)?, r) {
            a.errs.push(e);
            if e <= r {
                a.within += 1;
            }
        }
    }
    let groups: Vec<LocationGroup> = g
        .into_iter()
        .map(|((source, interference), a)| {
            let n = a.errs.len();
            let w = wilson95(a.within, n);
            let dc = quantile(&a.conf, 0.5).map(|c| c / 100.0);
            let reliability = match (w, dc) {
                (Some((lo, hi)), Some(c)) if n >= 10 => {
                    if hi < c {
                        "over-confident"
                    } else if lo > c {
                        "under-confident"
                    } else {
                        "consistent with the declared confidence"
                    }
                }
                _ => "not assessed (fewer than 10 verified calls)",
            };
            LocationGroup {
                source,
                interference,
                calls: a.calls,
                verified: n,
                within_radius: a.within,
                within_share: (n > 0).then(|| a.within as f64 / n as f64),
                within_wilson95: w,
                declared_confidence: dc,
                reliability: reliability.into(),
                median_radius_m: quantile(&a.radius, 0.5),
                median_error_m: quantile(&a.errs, 0.5),
                p95_error_m: quantile(&a.errs, 0.95),
                median_time_to_location_s: quantile(&a.ttl, 0.5),
            }
        })
        .collect();
    let over = groups
        .iter()
        .filter(|g| g.reliability == "over-confident")
        .count();
    let verdict = format!(
        "{} call(s) in {} group(s); {over} group(s) over-confident (verified error within the reported radius less often than the declared confidence)",
        groups.iter().map(|g| g.calls).sum::<usize>(),
        groups.len()
    );
    Ok(LocationReport {
        format: LOCATION_FORMAT,
        label: "MODELLED: statistics of the call records as supplied; a verified error is only as good as the PSAP's later verification.",
        title: title.into(),
        groups,
        verdict,
    })
}

// ---------------------------------------------------------------------------------------
// Duty cycling and battery.

/// Duty-cycling detection result.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DutyReport {
    pub fixes: usize,
    pub nominal_interval_s: Option<f64>,
    pub off_periods: usize,
    pub median_off_s: Option<f64>,
    pub off_cv: Option<f64>,
    pub median_on_s: Option<f64>,
    pub median_fixes_per_burst: Option<f64>,
    pub period_s: Option<f64>,
    pub duty_ratio: Option<f64>,
    /// `duty-cycled (regular)`, `irregular gaps`, `continuous` or `too few fixes`.
    pub pattern: String,
}

/// Detect duty cycling from fix times (sorted or not).
pub fn duty(times: &[f64]) -> DutyReport {
    let mut t = times.to_vec();
    t.sort_by(f64::total_cmp);
    let mut r = DutyReport {
        fixes: t.len(),
        nominal_interval_s: None,
        off_periods: 0,
        median_off_s: None,
        off_cv: None,
        median_on_s: None,
        median_fixes_per_burst: None,
        period_s: None,
        duty_ratio: None,
        pattern: "too few fixes".into(),
    };
    if t.len() < 10 {
        return r;
    }
    let iv: Vec<f64> = t
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|d| *d > 0.0)
        .collect();
    let nominal = quantile(&iv, 0.1).unwrap_or(1.0);
    r.nominal_interval_s = Some(nominal);
    let mut offs = Vec::new();
    let mut bursts: Vec<(f64, f64, usize)> = Vec::new();
    let mut b0 = (t[0], 1usize);
    for w in t.windows(2) {
        let d = w[1] - w[0];
        if d > 3.0 * nominal && d > nominal + 5.0 {
            offs.push(d);
            bursts.push((b0.0, w[0], b0.1));
            b0 = (w[1], 1);
        } else {
            b0.1 += 1;
        }
    }
    bursts.push((b0.0, *t.last().expect("non-empty"), b0.1));
    r.off_periods = offs.len();
    if offs.len() < 3 {
        r.pattern = "continuous".into();
        return r;
    }
    let m = mean(&offs).unwrap_or(0.0);
    let sd = (offs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / offs.len() as f64).sqrt();
    let cv = if m > 0.0 { sd / m } else { f64::INFINITY };
    let on: Vec<f64> = bursts.iter().map(|b| b.1 - b.0 + nominal).collect();
    let fpb: Vec<f64> = bursts.iter().map(|b| b.2 as f64).collect();
    r.median_off_s = quantile(&offs, 0.5);
    r.off_cv = Some(cv);
    r.median_on_s = quantile(&on, 0.5);
    r.median_fixes_per_burst = quantile(&fpb, 0.5);
    if let (Some(off), Some(on)) = (r.median_off_s, r.median_on_s) {
        r.period_s = Some(off + on - nominal);
        r.duty_ratio = Some(on / (off + on - nominal));
    }
    r.pattern = if cv < 0.2 {
        "duty-cycled (regular)".into()
    } else {
        "irregular gaps".into()
    };
    r
}

/// Battery model parameters.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BatteryParams {
    pub capacity_wh: f64,
    /// GNSS-active power (W), receiver plus host while acquiring.
    pub active_w: f64,
    pub sleep_w: f64,
    /// Fix intervals to evaluate (s).
    pub intervals_s: Vec<f64>,
    /// Time to fix by start type (s); defaults are typical values, replace with the module's.
    #[serde(default = "d_hot")]
    pub hot_ttff_s: f64,
    #[serde(default = "d_warm")]
    pub warm_ttff_s: f64,
    /// Off time up to which ephemeris is still valid (hot start), s.
    #[serde(default = "d_hot_max")]
    pub hot_max_off_s: f64,
    /// Time kept on after the first fix (s).
    #[serde(default = "d_hold")]
    pub hold_s: f64,
    /// Assisted start time to fix (s), when assistance data is delivered.
    pub assisted_ttff_s: Option<f64>,
    /// Energy per assistance download (J).
    #[serde(default)]
    pub assist_j: f64,
}

fn d_hot() -> f64 {
    2.0
}
fn d_warm() -> f64 {
    30.0
}
fn d_hot_max() -> f64 {
    7200.0
}
fn d_hold() -> f64 {
    3.0
}

/// One interval's result.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BatteryRow {
    pub interval_s: f64,
    pub start: String,
    pub on_s: f64,
    pub energy_per_fix_j: f64,
    pub avg_power_mw: f64,
    pub life_days: f64,
    pub mean_position_age_s: f64,
}

/// The battery trade-off over the intervals.
pub fn battery(p: &BatteryParams) -> Result<Vec<BatteryRow>, String> {
    if !(p.capacity_wh > 0.0 && p.active_w > 0.0 && p.sleep_w >= 0.0) {
        return Err("capacity_wh and active_w must be > 0, sleep_w >= 0".into());
    }
    let mut out = Vec::new();
    for &iv in &p.intervals_s {
        if !super::pos(iv) {
            return Err(format!("interval {iv} must be > 0"));
        }
        let (start, ttff, extra) = if iv <= p.hot_max_off_s {
            ("hot", p.hot_ttff_s, 0.0)
        } else if let Some(a) = p.assisted_ttff_s {
            ("assisted", a, p.assist_j)
        } else {
            ("warm", p.warm_ttff_s, 0.0)
        };
        let on = (ttff + p.hold_s).min(iv);
        let e = p.active_w * on + p.sleep_w * (iv - on) + extra;
        let avg = e / iv;
        out.push(BatteryRow {
            interval_s: iv,
            start: start.into(),
            on_s: on,
            energy_per_fix_j: e,
            avg_power_mw: avg * 1000.0,
            life_days: p.capacity_wh * 3600.0 / avg / 86_400.0,
            mean_position_age_s: iv / 2.0 + ttff,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_requirement_is_mapped() {
        assert!(mapping_problems().is_empty(), "{:?}", mapping_problems());
        assert_eq!(MAPPING.len(), 11);
        assert_eq!(test("shadow").unwrap().limit, Some(40.0));
    }

    #[test]
    fn nmea_summary_reads_types_and_rate() {
        let mk = |body: &str| {
            let cs = body.bytes().fold(0u8, |a, b| a ^ b);
            format!("${body}*{cs:02X}\n")
        };
        let mut s = String::new();
        for i in 0..5 {
            s.push_str(&mk(&format!(
                "GNGGA,1200{i:02}.00,5200.0,N,01300.0,E,2,12,0.9,40,M,40,M,,"
            )));
            s.push_str(&mk(&format!(
                "GNRMC,1200{i:02}.00,A,5200.0,N,01300.0,E,0,0,201126,,,D"
            )));
        }
        let n = nmea_summary(&s);
        assert_eq!(n.missing, vec!["VTG", "GSA", "GSV"]);
        assert_eq!(n.gga_median_interval_s, Some(1.0));
        assert_eq!(n.gga_quality2_share, Some(1.0));
        assert!(nmea_checksum_ok("$GNGGA,1*00") == Some(false));
    }

    #[test]
    fn duty_cycle_found() {
        let mut t = Vec::new();
        for c in 0..20 {
            for k in 0..5 {
                t.push(c as f64 * 300.0 + k as f64);
            }
        }
        let d = duty(&t);
        assert_eq!(d.pattern, "duty-cycled (regular)");
        assert!((d.period_s.unwrap() - 300.0).abs() < 1.0, "{d:?}");
        let cont: Vec<f64> = (0..100).map(f64::from).collect();
        assert_eq!(duty(&cont).pattern, "continuous");
    }

    #[test]
    fn battery_switches_start_type() {
        let p = BatteryParams {
            capacity_wh: 10.0,
            active_w: 0.1,
            sleep_w: 1e-5,
            intervals_s: vec![60.0, 14_400.0],
            hot_ttff_s: 2.0,
            warm_ttff_s: 30.0,
            hot_max_off_s: 7200.0,
            hold_s: 3.0,
            assisted_ttff_s: None,
            assist_j: 0.0,
        };
        let r = battery(&p).unwrap();
        assert_eq!(r[0].start, "hot");
        assert_eq!(r[1].start, "warm");
        assert!(r[1].life_days > r[0].life_days);
    }
}
