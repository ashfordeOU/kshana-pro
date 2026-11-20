// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Recorder-chain register (format [`FORMAT`]): what a recording chain did to the signal,
//! written down once per configuration so every recording can name it.
//!
//! ```toml
//! format = "kshana-simlab-recorder-chain/1"
//! chain_id = "REC-A"
//! date = "2026-11-18"
//! operator = "A. Operator"
//! sample_rate_hz = 20e6          # complex I/Q sample rate
//! signal_bandwidth_hz = 2.046e6  # main lobe of the narrowest signal recorded (GPS L1 C/A)
//! pll_bandwidth_hz = 15
//! [[stage]]
//! kind = "lna"                   # lna | filter | amplifier | cable | mixer | adc | agc | reference
//! name = "Active antenna LNA"
//! gain_db = 28
//! noise_figure_db = 1.8
//! group_delay_ns = 12
//! group_delay_u_ns = 2           # standard uncertainty
//! ```
//!
//! The cascade, per stage in order: gain sums in dB; the noise figure by Friis,
//! `F = F1 + (F2 − 1)/G1 + (F3 − 1)/(G1·G2) + …` (a passive loss of L dB has NF = L dB);
//! the bandwidth is the narrowest filter's (or the ADC's complex sample rate); group delays
//! add and their standard uncertainties add in quadrature (GUM, uncorrelated); the bit depth
//! and AGC mode are the ADC's and AGC's. Findings (each a rule of thumb):
//!
//! * the bandwidth below the signal bandwidth truncates the main lobe (limiting);
//! * a complex sample rate below the bandwidth aliases (invalidating);
//! * a 1-bit ADC loses 1.96 dB of C/N0 (limiting; same table as `testkit fidelity`);
//! * an AGC active during recording bakes its gain response into the file (limiting);
//! * a free-running TCXO reference (limiting for timing work);
//! * a stage after the first contributing more than 0.5 dB to the cascade noise figure.
//!
//! Change detection: against the previous registration of the same chain (from the register),
//! any registered quantity that moved beyond its stated tolerance is listed. The result also
//! carries a `capture.toml` for `kshana-pro testkit fidelity`, so a replay's fidelity
//! statement is derived from the registered chain, not retyped.

use serde::{Deserialize, Serialize};

use kshana_eloran::calreg::{EntryBody, Register, SubjectKind};

use super::r;
use crate::testkit::fidelity::{self, PhaseNoisePoint};

pub const FORMAT: &str = "kshana-simlab-recorder-chain/1";
pub const METHOD: &str = "kshana-pro/simlab-recorder-chain/v1";

fn d_pll() -> f64 {
    15.0
}

fn d_sigbw() -> f64 {
    2.046e6
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stage {
    pub kind: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub gain_db: f64,
    #[serde(default)]
    pub noise_figure_db: Option<f64>,
    #[serde(default)]
    pub bandwidth_hz: Option<f64>,
    #[serde(default)]
    pub group_delay_ns: f64,
    #[serde(default)]
    pub group_delay_u_ns: f64,
    /// ADC only.
    #[serde(default)]
    pub bits: Option<u8>,
    /// AGC only: `fixed` or `active`.
    #[serde(default)]
    pub mode: Option<String>,
    /// Reference only: `tcxo`, `ocxo`, `rubidium`, `gpsdo`, `external-10mhz`.
    #[serde(default)]
    pub reference: Option<String>,
    /// Reference or LO: SSB phase noise at the GNSS carrier.
    #[serde(default)]
    pub phase_noise: Vec<PhaseNoisePoint>,
    #[serde(default)]
    pub serial: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chain {
    pub format: String,
    pub chain_id: String,
    pub date: String,
    #[serde(default)]
    pub operator: String,
    pub sample_rate_hz: f64,
    #[serde(default = "d_sigbw")]
    pub signal_bandwidth_hz: f64,
    #[serde(default = "d_pll")]
    pub pll_bandwidth_hz: f64,
    /// Registration interval (days) for the due date.
    #[serde(default)]
    pub interval_days: Option<u32>,
    #[serde(rename = "stage")]
    pub stages: Vec<Stage>,
}

const KINDS: [&str; 8] = [
    "lna",
    "filter",
    "amplifier",
    "cable",
    "mixer",
    "adc",
    "agc",
    "reference",
];

impl Chain {
    pub fn parse(src: &str) -> Result<Chain, String> {
        let c: Chain = toml::from_str(src).map_err(|e| format!("recorder chain: {e}"))?;
        if c.format != FORMAT {
            return Err(format!(
                "recorder chain: format '{}' (expected {FORMAT})",
                c.format
            ));
        }
        if c.chain_id.trim().is_empty() {
            return Err("recorder chain: chain_id is empty".into());
        }
        kshana_eloran::calreg::day_number(&c.date).map_err(|e| format!("recorder chain: {e}"))?;
        if !(c.sample_rate_hz > 0.0 && c.signal_bandwidth_hz > 0.0 && c.pll_bandwidth_hz > 0.0) {
            return Err("recorder chain: rates and bandwidths must be > 0".into());
        }
        if c.stages.is_empty() {
            return Err("recorder chain: no [[stage]]".into());
        }
        let mut adcs = 0;
        for (i, s) in c.stages.iter().enumerate() {
            if !KINDS.contains(&s.kind.as_str()) {
                return Err(format!("stage {}: unknown kind '{}'", i + 1, s.kind));
            }
            if s.kind == "adc" {
                adcs += 1;
                match s.bits {
                    Some(b) if (1..=24).contains(&b) => {}
                    _ => return Err(format!("stage {}: an adc needs bits 1 to 24", i + 1)),
                }
            }
            if let Some(m) = &s.mode {
                if m != "fixed" && m != "active" {
                    return Err(format!("stage {}: mode must be fixed or active", i + 1));
                }
            }
            if let Some(rf) = &s.reference {
                if fidelity::frac_stability(rf).is_none() {
                    return Err(format!("stage {}: unknown reference '{rf}'", i + 1));
                }
            }
            if s.bandwidth_hz.is_some_and(|b| b <= 0.0)
                || s.noise_figure_db.is_some_and(|n| n < 0.0)
                || s.group_delay_u_ns < 0.0
            {
                return Err(format!(
                    "stage {}: bandwidth, NF and uncertainty must be ≥ 0",
                    i + 1
                ));
            }
        }
        if adcs != 1 {
            return Err(format!(
                "recorder chain: exactly one adc stage needed ({adcs} given)"
            ));
        }
        Ok(c)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    /// `ok`, `limited` or `invalidating` (as in `testkit fidelity`).
    pub severity: String,
    pub id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Change {
    pub quantity: String,
    pub previous: f64,
    pub now: f64,
    pub tolerance: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChainReport {
    pub method: &'static str,
    pub label: &'static str,
    pub evidence: &'static str,
    pub chain_id: String,
    pub date: String,
    pub chain_sha256: String,
    pub cascade_gain_db: f64,
    pub cascade_nf_db: f64,
    /// Contribution (dB) of each stage to the cascade noise figure.
    pub nf_contribution_db: Vec<(String, f64)>,
    pub bandwidth_hz: f64,
    pub bit_depth: u8,
    pub agc: String,
    pub reference: Option<String>,
    pub group_delay_ns: f64,
    pub group_delay_u_ns: f64,
    pub findings: Vec<Finding>,
    /// Quantities changed beyond tolerance since the previous registration (empty when
    /// there is none or nothing moved).
    pub changes_since_previous: Vec<Change>,
    pub previous_registration: Option<String>,
    /// The register's head hash after this registration (keep a copy elsewhere).
    pub register_head: Option<String>,
    /// The `kshana-testkit-capture/1` file for `testkit fidelity`.
    pub capture_toml: String,
}

fn db(x: f64) -> f64 {
    10.0 * x.log10()
}
fn lin(x: f64) -> f64 {
    10f64.powf(x / 10.0)
}

/// Assess a chain.
pub fn assess(c: &Chain, chain_sha256: &str) -> ChainReport {
    // Friis cascade; a stage without an NF is a passive loss when its gain is negative,
    // otherwise noiseless (recorded as a finding).
    let mut f_total = 1.0;
    let mut g_before = 1.0;
    let mut contrib = Vec::new();
    let mut gain_db = 0.0;
    let mut missing_nf = Vec::new();
    for s in &c.stages {
        if matches!(s.kind.as_str(), "adc" | "agc" | "reference") {
            continue;
        }
        let nf = match s.noise_figure_db {
            Some(n) => n,
            None if s.gain_db < 0.0 => -s.gain_db,
            None => {
                missing_nf.push(s.name.clone());
                0.0
            }
        };
        let add = (lin(nf) - 1.0) / g_before;
        let before = db(f_total);
        f_total += add;
        contrib.push((stage_name(s), r(db(f_total) - before, 3)));
        g_before *= lin(s.gain_db);
        gain_db += s.gain_db;
    }
    let adc = c.stages.iter().find(|s| s.kind == "adc").unwrap();
    let bits = adc.bits.unwrap_or(1);
    let filt_bw = c
        .stages
        .iter()
        .filter_map(|s| s.bandwidth_hz)
        .fold(f64::INFINITY, f64::min);
    let bandwidth = filt_bw.min(c.sample_rate_hz);
    let agc = c
        .stages
        .iter()
        .find(|s| s.kind == "agc")
        .and_then(|s| s.mode.clone())
        .unwrap_or_else(|| "fixed".into());
    let refst = c.stages.iter().find(|s| s.kind == "reference");
    let reference = refst.and_then(|s| s.reference.clone());
    let phase_noise: Vec<PhaseNoisePoint> = c
        .stages
        .iter()
        .flat_map(|s| s.phase_noise.iter().cloned())
        .collect();
    let gd: f64 = c.stages.iter().map(|s| s.group_delay_ns).sum();
    let gd_u = c
        .stages
        .iter()
        .map(|s| s.group_delay_u_ns.powi(2))
        .sum::<f64>()
        .sqrt();

    let mut findings = Vec::new();
    let mut f = |sev: &str, id: &str, detail: String| {
        findings.push(Finding {
            severity: sev.into(),
            id: id.into(),
            detail,
        })
    };
    if c.sample_rate_hz < filt_bw {
        f(
            "invalidating",
            "aliasing",
            format!(
                "complex sample rate {:.3} MHz is below the analogue bandwidth {:.3} MHz: out-of-band energy aliases into the recording",
                c.sample_rate_hz / 1e6,
                filt_bw / 1e6
            ),
        );
    }
    if bandwidth < c.signal_bandwidth_hz {
        f(
            "limited",
            "bandwidth",
            format!(
                "recorded bandwidth {:.3} MHz is narrower than the signal bandwidth {:.3} MHz: the correlation peak is rounded and multipath and spoofing structure near the peak is lost",
                bandwidth / 1e6,
                c.signal_bandwidth_hz / 1e6
            ),
        );
    }
    let ql = fidelity::quantisation_loss_db(bits);
    f(
        if bits <= 1 { "limited" } else { "ok" },
        "quantisation",
        format!(
            "{bits}-bit ADC: about {ql} dB C/N0 loss (textbook value for the optimal threshold)"
        ),
    );
    if agc == "active" {
        f(
            "limited",
            "agc",
            "AGC active during recording: its gain response is in the file and a replay shows the recorder's AGC, not the field's".into(),
        );
    }
    match reference.as_deref() {
        None => f(
            "limited",
            "reference",
            "no reference stage: the recording's time base is not stated".into(),
        ),
        Some("tcxo") => f(
            "limited",
            "reference",
            "free-running TCXO (about 1e-6): timing results from this recording drift by up to 1 µs per second of run".into(),
        ),
        Some(rf) => f("ok", "reference", format!("reference {rf}")),
    }
    for (name, dbc) in contrib.iter().skip(1) {
        if *dbc > 0.5 {
            f(
                "limited",
                "noise-figure",
                format!("{name} adds {dbc:.2} dB to the cascade noise figure: raise the gain ahead of it"),
            );
        }
    }
    if !missing_nf.is_empty() {
        f(
            "limited",
            "noise-figure",
            format!(
                "no noise figure stated for {}: taken as noiseless, so the cascade NF is a lower bound",
                missing_nf.join(", ")
            ),
        );
    }

    let mut cap = format!(
        "# Derived from recorder chain {} ({}), sha256 {}\nformat = \"{}\"\nrecorder = \"{}\"\nbit_depth = {bits}\nbandwidth_hz = {bandwidth}\nagc = \"{agc}\"\npll_bandwidth_hz = {}\n",
        c.chain_id,
        c.date,
        chain_sha256,
        fidelity::CAPTURE_FORMAT,
        c.chain_id.replace('"', "'"),
        c.pll_bandwidth_hz
    );
    if let Some(rf) = &reference {
        cap.push_str(&format!("reference = \"{rf}\"\n"));
    }
    for p in &phase_noise {
        cap.push_str(&format!(
            "[[recorder_phase_noise]]\noffset_hz = {}\ndbc_hz = {}\n",
            p.offset_hz, p.dbc_hz
        ));
    }
    ChainReport {
        method: METHOD,
        label: super::LABEL,
        evidence: super::EVIDENCE,
        chain_id: c.chain_id.clone(),
        date: c.date.clone(),
        chain_sha256: chain_sha256.into(),
        cascade_gain_db: r(gain_db, 3),
        cascade_nf_db: r(db(f_total), 3),
        nf_contribution_db: contrib,
        bandwidth_hz: bandwidth,
        bit_depth: bits,
        agc,
        reference,
        group_delay_ns: r(gd, 3),
        group_delay_u_ns: r(gd_u, 3),
        findings,
        changes_since_previous: Vec::new(),
        previous_registration: None,
        register_head: None,
        capture_toml: cap,
    }
}

fn stage_name(s: &Stage) -> String {
    if s.name.is_empty() {
        s.kind.clone()
    } else {
        s.name.clone()
    }
}

/// Registered quantities with their change tolerance.
fn quantities(rep: &ChainReport) -> Vec<(&'static str, f64, &'static str, f64)> {
    vec![
        ("cascade_gain_db", rep.cascade_gain_db, "dB", 1.0),
        ("cascade_nf_db", rep.cascade_nf_db, "dB", 0.3),
        (
            "bandwidth_hz",
            rep.bandwidth_hz,
            "Hz",
            rep.bandwidth_hz * 0.01,
        ),
        ("bit_depth", rep.bit_depth as f64, "bit", 0.0),
        (
            "group_delay_ns",
            rep.group_delay_ns,
            "ns",
            2.0 * rep.group_delay_u_ns.max(0.5),
        ),
    ]
}

/// Compare with the previous registration of the chain in `reg` and append this one.
/// Returns the register head after appending.
pub fn register(
    rep: &mut ChainReport,
    reg: &Register,
    operator: &str,
    interval_days: Option<u32>,
) -> Result<String, String> {
    let v = reg.verify()?;
    if !v.ok() {
        return Err(format!(
            "register {}: hash chain broken: {}",
            reg.path().display(),
            v.problems.join("; ")
        ));
    }
    let mut prev_date = None;
    for (q, now, _, tol) in quantities(rep) {
        if let Some(e) = reg.latest(&rep.chain_id, Some(q))? {
            prev_date = Some(e.body.date.clone());
            if (e.body.value - now).abs() > tol {
                rep.changes_since_previous.push(Change {
                    quantity: q.into(),
                    previous: e.body.value,
                    now,
                    tolerance: r(tol, 3),
                });
            }
        }
    }
    rep.previous_registration = prev_date;
    let mut head = v.head;
    for (q, value, unit, _) in quantities(rep) {
        let u = if q == "group_delay_ns" {
            2.0 * rep.group_delay_u_ns
        } else {
            0.0
        };
        let e = reg.append(EntryBody {
            seq: 0,
            subject_id: rep.chain_id.clone(),
            subject_kind: SubjectKind::Instrument,
            date: rep.date.clone(),
            quantity: q.into(),
            value,
            unit: unit.into(),
            expanded_u: u,
            k: 2.0,
            operator: operator.into(),
            report_sha256: rep.chain_sha256.clone(),
            interval_days,
            notes: format!("{METHOD}; MODELLED cascade from the declared stages"),
        })?;
        head = e.hash;
    }
    rep.register_head = Some(head.clone());
    Ok(head)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friis_cascade_of_lna_then_cable() {
        let c = Chain::parse(
            r#"format = "kshana-simlab-recorder-chain/1"
chain_id = "T"
date = "2026-11-18"
sample_rate_hz = 4e6
[[stage]]
kind = "lna"
gain_db = 20
noise_figure_db = 2
[[stage]]
kind = "cable"
gain_db = -10
[[stage]]
kind = "adc"
bits = 2
"#,
        )
        .unwrap();
        let rep = assess(&c, "00");
        // F = 1.585 + (10 − 1)/100 = 1.675 → 2.240 dB.
        assert!(
            (rep.cascade_nf_db - 2.240).abs() < 0.002,
            "{}",
            rep.cascade_nf_db
        );
        assert_eq!(rep.cascade_gain_db, 10.0);
        assert_eq!(rep.bandwidth_hz, 4e6);
        assert!(fidelity::Capture::parse(&rep.capture_toml).is_ok());
    }
}
