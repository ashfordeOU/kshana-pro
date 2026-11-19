// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! PNT-DP test vectors: input sequences with the integrity flags expected of them.
//!
//! A ship steams north-east at 5 m/s for 240 s from 59.40 N 24.70 E. The sources and their
//! errors are synthetic: each position source's error is uniform per axis within
//! `+/- sqrt(3) sigma` (so a fault-free source can never exceed `sqrt(6) sigma < 4 sigma` and
//! the vectors are free of chance false alarms), the inertial/log velocity carries a 0.02 m/s
//! bias. Deterministic per seed.
//!
//! The expected values are written from the scenario's own geometry, not from the
//! processor's output: an epoch whose outcome depends on noise during a transition is marked
//! `*` (not asserted), and every other epoch is pinned.
//!
//! * `nominal`: every source healthy; high integrity on DGNSS from the second epoch (the
//!   first has no dead reckoning yet, so two groups: medium).
//! * `gnss-spoof`: from 60 s every GNSS service (GPS, Galileo, DGNSS; one L-band group) is
//!   dragged east at 2 m/s. Once the drag exceeds what eLoran's consistency bound can absorb
//!   (asserted from 130 s), the GNSS group is `inconsistent` and eLoran is selected with
//!   medium integrity (eLoran and dead reckoning agree: two groups).
//! * `jump`: GPS jumps 300 m north at 80 s and stays there: `jump` at 80 s, `inconsistent`
//!   after; Galileo is selected throughout from 80 s, integrity high.
//! * `frozen`: Galileo repeats its 100 s output from then on: `frozen` from 104 s (the fifth
//!   identical sample); GPS stays selected with high integrity.
//! * `disagreement`: only GPS and eLoran; from 50 s eLoran is 400 m off. Before: medium on
//!   GPS. At 50 s eLoran's step fails its rate check (`jump`; GPS alone: low). After: an
//!   unresolved `disagreement` on both, integrity none.

use std::path::Path;

use super::{run, Config, Integrity, Output, Sample};
use crate::pics::log::metres_per_degree;
use crate::timing::model::SplitMix;

pub const CONFIG_FULL: &str = r#"# PNT-DP test-vector configuration (SYNTHETIC)
[[source]]
id = "gps"
kind = "gnss"
group = "lband"
sigma_m = 3.0

[[source]]
id = "galileo"
kind = "gnss"
group = "lband"
sigma_m = 3.0

[[source]]
id = "dgnss"
kind = "dgnss"
group = "lband"
sigma_m = 1.0

[[source]]
id = "eloran"
kind = "eloran"
group = "terrestrial"
sigma_m = 20.0

[[source]]
id = "ins"
kind = "inertial"
group = "inertial"
sigma_mps = 0.05

[limits]
k = 4.0
max_speed_mps = 25.0
max_age_s = 3.0
frozen_epochs = 5
reanchor_sigma_m = 5.0
alert_limit_m = 100.0
"#;

/// One expected epoch; `None` in a field means "not asserted" (`*` in the file).
#[derive(Debug, Clone, PartialEq)]
pub struct Expected {
    pub t_s: f64,
    pub selected: Option<String>,
    pub integrity: Option<Integrity>,
    pub flags: Option<String>,
}

pub struct Vector {
    pub name: &'static str,
    pub description: &'static str,
    pub config: String,
    pub samples: Vec<Sample>,
    pub expected: Vec<Expected>,
    /// The true position each epoch (lat, lon), for the protection-level checks.
    pub truth: Vec<(f64, f64)>,
}

pub const DURATION_S: usize = 240;
const LAT0: f64 = 59.40;
const LON0: f64 = 24.70;
const SPEED: f64 = 5.0;

fn truth_en(t: f64) -> (f64, f64) {
    let v = SPEED / 2f64.sqrt();
    (v * t, v * t)
}

/// Which faults a vector applies.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Case {
    Nominal,
    Spoof,
    Jump,
    Frozen,
    Disagreement,
}

/// Noise model: `Uniform` keeps every fault-free error within sqrt(6) sigma; `Gaussian` is
/// used by the Monte Carlo test of the protection level.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Noise {
    Uniform,
    Gaussian,
}

fn build(case: Case, seed: u64, noise: Noise) -> (Vec<Sample>, Vec<(f64, f64)>) {
    let (mlat, mlon) = metres_per_degree(LAT0);
    let mut rng = SplitMix::new(seed);
    let mut draw = |sigma: f64| match noise {
        Noise::Uniform => rng.range(-(3f64.sqrt()) * sigma, 3f64.sqrt() * sigma),
        Noise::Gaussian => sigma * rng.normal(),
    };
    let mut out = Vec::new();
    let mut truth = Vec::new();
    let sources: &[(&str, f64)] = match case {
        Case::Nominal | Case::Spoof => &[
            ("gps", 3.0),
            ("galileo", 3.0),
            ("dgnss", 1.0),
            ("eloran", 20.0),
        ],
        Case::Jump | Case::Frozen => &[("gps", 3.0), ("galileo", 3.0), ("eloran", 20.0)],
        Case::Disagreement => &[("gps", 3.0), ("eloran", 20.0)],
    };
    let mut frozen_at: Option<(f64, f64)> = None;
    for k in 0..=DURATION_S {
        let t = k as f64;
        let (te, tn) = truth_en(t);
        truth.push((LAT0 + tn / mlat, LON0 + te / mlon));
        for &(id, sigma) in sources {
            let (mut e, mut n) = (te + draw(sigma), tn + draw(sigma));
            match case {
                Case::Spoof if t >= 60.0 && id != "eloran" => e += 2.0 * (t - 60.0),
                Case::Jump if t >= 80.0 && id == "gps" => n += 300.0,
                Case::Disagreement if t >= 50.0 && id == "eloran" => n += 400.0,
                _ => {}
            }
            let (mut lat, mut lon) = (LAT0 + n / mlat, LON0 + e / mlon);
            if case == Case::Frozen && id == "galileo" && t >= 100.0 {
                let f = *frozen_at.get_or_insert((lat, lon));
                lat = f.0;
                lon = f.1;
            }
            out.push(Sample {
                t_s: t,
                source: id.into(),
                lat_deg: Some(lat),
                lon_deg: Some(lon),
                sigma_m: None,
                valid: true,
                vn_mps: None,
                ve_mps: None,
            });
        }
        if case != Case::Disagreement {
            let v = SPEED / 2f64.sqrt();
            out.push(Sample {
                t_s: t,
                source: "ins".into(),
                lat_deg: None,
                lon_deg: None,
                sigma_m: None,
                valid: true,
                vn_mps: Some(v + 0.02),
                ve_mps: Some(v + 0.02),
            });
        }
    }
    (out, truth)
}

fn exp(t: f64, sel: &str, integ: &str, flags: &str) -> Expected {
    let o = |s: &str| (s != "*").then(|| s.to_string());
    Expected {
        t_s: t,
        selected: o(sel),
        integrity: (integ != "*").then(|| Integrity::parse(integ).expect("integrity")),
        flags: o(flags),
    }
}

fn expected(case: Case) -> Vec<Expected> {
    (0..=DURATION_S)
        .map(|k| {
            let t = k as f64;
            match case {
                Case::Nominal if k == 0 => exp(t, "dgnss", "medium", ""),
                Case::Nominal => exp(t, "dgnss", "high", ""),
                Case::Spoof if k == 0 => exp(t, "dgnss", "medium", ""),
                Case::Spoof if t < 62.0 => exp(t, "dgnss", "high", ""),
                Case::Spoof if t < 130.0 => exp(t, "*", "*", "*"),
                Case::Spoof => exp(
                    t,
                    "eloran",
                    "medium",
                    "dgnss:inconsistent;galileo:inconsistent;gps:inconsistent",
                ),
                Case::Jump if k == 0 => exp(t, "gps", "medium", ""),
                Case::Jump if t < 80.0 => exp(t, "gps", "high", ""),
                Case::Jump if k == 80 => exp(t, "galileo", "high", "gps:jump"),
                Case::Jump => exp(t, "galileo", "high", "gps:inconsistent"),
                Case::Frozen if k == 0 => exp(t, "gps", "medium", ""),
                Case::Frozen if t < 100.0 => exp(t, "gps", "high", ""),
                Case::Frozen if t < 104.0 => exp(t, "*", "*", "*"),
                Case::Frozen => exp(t, "gps", "high", "galileo:frozen"),
                Case::Disagreement if t < 50.0 => exp(t, "gps", "medium", ""),
                // The 400 m step is first caught by eLoran's own rate check.
                Case::Disagreement if k == 50 => exp(t, "gps", "low", "eloran:jump"),
                Case::Disagreement => exp(t, "gps", "none", "eloran:disagreement;gps:disagreement"),
            }
        })
        .collect()
}

/// Every vector, with the uniform noise of the pinned files.
pub fn all() -> Vec<Vector> {
    let full = CONFIG_FULL.to_string();
    let mk = |name, description, case, seed| {
        let (samples, truth) = build(case, seed, Noise::Uniform);
        Vector {
            name,
            description,
            config: full.clone(),
            samples,
            expected: expected(case),
            truth,
        }
    };
    vec![
        mk("nominal", "All sources healthy.", Case::Nominal, 11),
        mk(
            "gnss-spoof",
            "Every GNSS service dragged east at 2 m/s from 60 s.",
            Case::Spoof,
            12,
        ),
        mk(
            "jump",
            "GPS jumps 300 m north at 80 s and stays.",
            Case::Jump,
            13,
        ),
        mk(
            "frozen",
            "Galileo repeats its 100 s output from then on.",
            Case::Frozen,
            14,
        ),
        mk(
            "disagreement",
            "GPS and eLoran only; eLoran 400 m off from 50 s.",
            Case::Disagreement,
            15,
        ),
    ]
}

/// The nominal vector with Gaussian errors, for the Monte Carlo test.
pub fn nominal_gaussian(seed: u64) -> (Vec<Sample>, Vec<(f64, f64)>) {
    build(Case::Nominal, seed, Noise::Gaussian)
}

/// The spoof vector with Gaussian errors.
pub fn spoof_gaussian(seed: u64) -> (Vec<Sample>, Vec<(f64, f64)>) {
    build(Case::Spoof, seed, Noise::Gaussian)
}

pub const EXPECTED_HEADER: &str = "t_s,selected,integrity,flags";

pub fn expected_csv(e: &[Expected]) -> String {
    let s = |v: &Option<String>| v.clone().unwrap_or_else(|| "*".into());
    let mut o = format!("{EXPECTED_HEADER}\n");
    for x in e {
        o.push_str(&format!(
            "{},{},{},{}\n",
            x.t_s,
            s(&x.selected),
            x.integrity
                .map(|i| i.as_str().to_string())
                .unwrap_or_else(|| "*".into()),
            s(&x.flags)
        ));
    }
    o
}

pub fn parse_expected(text: &str) -> Result<Vec<Expected>, String> {
    let mut out = Vec::new();
    for (n, l) in text.lines().enumerate() {
        if l.starts_with("t_s") || l.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = l.splitn(4, ',').collect();
        if f.len() != 4 {
            return Err(format!("expected line {}: 4 fields", n + 1));
        }
        let t: f64 = f[0].parse().map_err(|e| format!("line {}: {e}", n + 1))?;
        if f[2] != "*" && Integrity::parse(f[2]).is_none() {
            return Err(format!("line {}: integrity '{}'", n + 1, f[2]));
        }
        out.push(exp(t, f[1], f[2], f[3]));
    }
    Ok(out)
}

/// Compare outputs with expectations; each mismatch in words.
pub fn compare(out: &[Output], exp: &[Expected]) -> Vec<String> {
    let mut p = Vec::new();
    if out.len() != exp.len() {
        p.push(format!(
            "{} outputs for {} expected epochs",
            out.len(),
            exp.len()
        ));
    }
    for (o, e) in out.iter().zip(exp) {
        if o.t_s != e.t_s {
            p.push(format!("t = {}: expected t = {}", o.t_s, e.t_s));
            continue;
        }
        if let Some(s) = &e.selected {
            if o.selected.as_deref().unwrap_or("") != s {
                p.push(format!(
                    "t = {}: selected {:?}, expected {s}",
                    o.t_s, o.selected
                ));
            }
        }
        if let Some(i) = e.integrity {
            if o.integrity != i {
                p.push(format!(
                    "t = {}: integrity {}, expected {}",
                    o.t_s,
                    o.integrity.as_str(),
                    i.as_str()
                ));
            }
        }
        if let Some(f) = &e.flags {
            if &o.flags_text() != f {
                p.push(format!(
                    "t = {}: flags '{}', expected '{f}'",
                    o.t_s,
                    o.flags_text()
                ));
            }
        }
    }
    p
}

/// Run a vector and compare.
pub fn check(v: &Vector) -> Result<Vec<String>, String> {
    let cfg = Config::parse(&v.config)?;
    Ok(compare(&run(&cfg, &v.samples)?, &v.expected))
}

/// Write every vector as `<dir>/<name>/{config.toml,inputs.csv,expected.csv,README.txt}`.
pub fn write_all(dir: &Path) -> Result<(), String> {
    for v in all() {
        let d = dir.join(v.name);
        std::fs::create_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
        let w = |f: &str, s: &str| {
            std::fs::write(d.join(f), s).map_err(|e| format!("{}: {e}", d.join(f).display()))
        };
        w("config.toml", &v.config)?;
        w("inputs.csv", &super::inputs_csv(&v.samples))?;
        w("expected.csv", &expected_csv(&v.expected))?;
        w(
            "README.txt",
            &format!(
                "PNT-DP test vector '{}' (SYNTHETIC, MODELLED): {}\n`*` in expected.csv: not asserted (a transition whose exact epoch depends on noise).\nFlags: source:flag[+flag];... sorted. Regenerate with `kshana-pro pntdp vectors --out <dir>`.\n",
                v.name, v.description
            ),
        )?;
    }
    Ok(())
}
