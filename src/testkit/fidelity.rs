// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! Replay fidelity: what a record-and-replay test bench cannot reproduce, as a scored
//! statement attached to every verdict made on replayed data (method [`METHOD_ID`]).
//!
//! The capture is described by the tester (format [`CAPTURE_FORMAT`], TOML):
//!
//! ```toml
//! format = "kshana-testkit-capture/1"
//! recorder = "RF recorder, 2-bit I/Q"
//! replayer = "Same unit, playback"
//! bit_depth = 2
//! bandwidth_hz = 10e6
//! clipping_fraction = 0.0005      # share of samples at full scale in the recording
//! agc = "fixed"                   # fixed | active (during the recording)
//! reference = "ocxo"              # tcxo | ocxo | rubidium | gpsdo | external-10mhz
//! pll_bandwidth_hz = 15           # receiver carrier loop under test
//! sensors_recorded = ["imu"]
//! sensors_synchronised = true
//! [[recorder_phase_noise]]        # single-sideband L(f) at the GNSS carrier, dBc/Hz
//! offset_hz = 10
//! dbc_hz = -60
//! [[replayer_phase_noise]]
//! offset_hz = 10
//! dbc_hz = -60
//! ```
//!
//! Five limits, each scored in [0, 1] with a severity (`ok`, `limited`, `invalidating`):
//!
//! 1. **Quantisation**: the C/N0 loss of a `b`-bit uniform quantiser at the optimal
//!    threshold (1 bit 1.96 dB, 2 bits 0.55 dB, 3 bits 0.16 dB, 4 bits 0.04 dB; textbook
//!    values, Van Dierendonck in Parkinson and Spilker 1996, ch. 8), and the headroom for
//!    the scenario's strongest jammer: J/N = J/S + S/N (S/N = 45 dB-Hz − 10·log10 B) must
//!    fit under the ADC dynamic range 6.02·b + 1.76 dB less a 10 dB crest-factor margin.
//! 2. **Front-end saturation and AGC**: clipped samples above 0.1 % limit the replay, above
//!    1 % invalidate it; an AGC active during recording bakes the recorder's gain response
//!    into the file, so the receiver's own AGC and J/N indicators see the replayer's gain,
//!    not the field's (limited under a jamming scenario).
//! 3. **LO phase-noise doubling**: the recorder's and the replayer's oscillator phase noise
//!    add (powers), so a replay carries both: σ² = 2∫L(f)df over [B_PLL, B/2] for each,
//!    combined in quadrature, in degrees at the carrier. The penalty is 10·log10(σ²_total /
//!    σ²_recorder) (3 dB for identical units). Above 5° (of the 15° 1-σ PLL rule of thumb,
//!    Kaplan and Hegarty 2017) it limits, above 10° it invalidates carrier-phase and timing
//!    results. No phase-noise data: not assessed, limited.
//! 4. **Missing synchronised sensors**: every sensor the scenario needs
//!    (`[environment] sensors`) must be in the capture and time-synchronised with it;
//!    otherwise an integrated receiver cannot be exercised (invalidating).
//! 5. **Reference oscillator** (timing scenarios only): a free-running reference adds up to
//!    `y·T` of time error over the run (tcxo 1e-6, ocxo 1e-8, rubidium 1e-10; disciplined
//!    1e-12); beyond a tenth of the tightest time criterion it limits, beyond it invalidates.
//!
//! Score = 100 × the product of the five; grade `faithful` (no limit), `limited` or
//! `not-faithful` (any invalidating limit). **MODELLED**: rules of thumb stated above, not
//! a calibration of the bench; the statement says what the replay cannot show, it does not
//! certify what it can.

use serde::{Deserialize, Serialize};

use super::spec::{Metric, Spec, ThreatType};

pub const CAPTURE_FORMAT: &str = "kshana-testkit-capture/1";
pub const METHOD_ID: &str = "kshana-pro/testkit-replay-fidelity/v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseNoisePoint {
    pub offset_hz: f64,
    pub dbc_hz: f64,
}

fn d_pll() -> f64 {
    15.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capture {
    pub format: String,
    #[serde(default)]
    pub recorder: String,
    #[serde(default)]
    pub replayer: String,
    pub bit_depth: u8,
    pub bandwidth_hz: f64,
    #[serde(default)]
    pub clipping_fraction: Option<f64>,
    /// `fixed` or `active`.
    #[serde(default)]
    pub agc: Option<String>,
    /// `tcxo`, `ocxo`, `rubidium`, `gpsdo` or `external-10mhz`.
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default = "d_pll")]
    pub pll_bandwidth_hz: f64,
    #[serde(default)]
    pub sensors_recorded: Vec<String>,
    #[serde(default)]
    pub sensors_synchronised: bool,
    #[serde(default)]
    pub recorder_phase_noise: Vec<PhaseNoisePoint>,
    #[serde(default)]
    pub replayer_phase_noise: Vec<PhaseNoisePoint>,
}

impl Capture {
    pub fn parse(src: &str) -> Result<Capture, String> {
        let c: Capture = toml::from_str(src).map_err(|e| format!("capture: {e}"))?;
        if c.format != CAPTURE_FORMAT {
            return Err(format!(
                "capture format '{}' (expected {CAPTURE_FORMAT})",
                c.format
            ));
        }
        if c.bit_depth == 0 || c.bit_depth > 24 {
            return Err("capture: bit_depth must be 1 to 24".into());
        }
        if !(c.bandwidth_hz > 0.0 && c.pll_bandwidth_hz > 0.0) {
            return Err("capture: bandwidth_hz and pll_bandwidth_hz must be > 0".into());
        }
        if let Some(a) = &c.agc {
            if a != "fixed" && a != "active" {
                return Err("capture: agc must be fixed or active".into());
            }
        }
        if let Some(r) = &c.reference {
            if frac_stability(r).is_none() {
                return Err(format!("capture: unknown reference '{r}'"));
            }
        }
        for p in c.recorder_phase_noise.iter().chain(&c.replayer_phase_noise) {
            if !(p.offset_hz > 0.0 && p.dbc_hz.is_finite()) {
                return Err(
                    "capture: phase-noise points need offset_hz > 0 and a finite dbc_hz".into(),
                );
            }
        }
        Ok(c)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    Ok,
    Limited,
    Invalidating,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Limit {
    pub id: String,
    pub score: f64,
    pub severity: Severity,
    pub value: Option<f64>,
    pub unit: String,
    pub detail: String,
}

/// The scored statement attached to a verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FidelityStatement {
    pub method: String,
    pub label: String,
    pub score: u32,
    /// `faithful`, `limited` or `not-faithful`.
    pub grade: String,
    pub statement: String,
    pub limits: Vec<Limit>,
}

/// Quantisation C/N0 loss of a `b`-bit quantiser, dB.
pub fn quantisation_loss_db(bits: u8) -> f64 {
    match bits {
        1 => 1.96,
        2 => 0.55,
        3 => 0.16,
        4 => 0.04,
        _ => 0.01,
    }
}

/// Fractional frequency stability of a reference over a test run (representative).
pub fn frac_stability(reference: &str) -> Option<f64> {
    Some(match reference {
        "tcxo" => 1e-6,
        "ocxo" => 1e-8,
        "rubidium" => 1e-10,
        "gpsdo" | "external-10mhz" => 1e-12,
        _ => return None,
    })
}

/// RMS phase jitter (degrees) of an SSB phase-noise curve over [f1, f2]: σ² = 2∫L(f)df,
/// piecewise power-law between the points, flat outside them.
pub fn phase_jitter_deg(points: &[PhaseNoisePoint], f1: f64, f2: f64) -> Option<f64> {
    if points.is_empty() || f2 <= f1 {
        return None;
    }
    let mut p: Vec<(f64, f64)> = points.iter().map(|x| (x.offset_hz, x.dbc_hz)).collect();
    p.sort_by(|a, b| a.0.total_cmp(&b.0));
    let l_at = |f: f64| -> f64 {
        if f <= p[0].0 {
            return p[0].1;
        }
        if f >= p[p.len() - 1].0 {
            return p[p.len() - 1].1;
        }
        let k = p.partition_point(|x| x.0 <= f) - 1;
        let (a, b) = (p[k], p[k + 1]);
        let r = (f.log10() - a.0.log10()) / (b.0.log10() - a.0.log10());
        a.1 + r * (b.1 - a.1)
    };
    // numerical integration on a log grid
    let n = 400;
    let (lf1, lf2) = (f1.log10(), f2.log10());
    let mut s = 0.0;
    for i in 0..n {
        let fa = 10f64.powf(lf1 + (lf2 - lf1) * i as f64 / n as f64);
        let fb = 10f64.powf(lf1 + (lf2 - lf1) * (i + 1) as f64 / n as f64);
        let la = 10f64.powf(l_at(fa) / 10.0);
        let lb = 10f64.powf(l_at(fb) / 10.0);
        s += 0.5 * (la + lb) * (fb - fa);
    }
    Some((2.0 * s).sqrt().to_degrees())
}

fn lim(
    id: &str,
    score: f64,
    severity: Severity,
    value: Option<f64>,
    unit: &str,
    detail: String,
) -> Limit {
    Limit {
        id: id.into(),
        score,
        severity,
        value,
        unit: unit.into(),
        detail,
    }
}

/// The fidelity statement of replaying `cap` for `spec`.
pub fn assess(cap: &Capture, spec: &Spec) -> FidelityStatement {
    let mut v = Vec::new();
    // 1. Quantisation and jamming headroom.
    let loss = quantisation_loss_db(cap.bit_depth);
    let dr = 6.02 * cap.bit_depth as f64 + 1.76;
    let snr = 45.0 - 10.0 * cap.bandwidth_hz.log10();
    let max_js = spec
        .threats
        .iter()
        .filter(|t| t.kind == Some(ThreatType::Jamming))
        .filter_map(|t| t.js_db)
        .fold(f64::NEG_INFINITY, f64::max);
    let mut q = lim(
        "quantisation",
        if loss <= 0.2 {
            1.0
        } else if loss <= 0.6 {
            0.85
        } else {
            0.6
        },
        if loss <= 0.6 {
            Severity::Ok
        } else {
            Severity::Limited
        },
        Some(loss),
        "dB",
        format!(
            "{}-bit capture: {loss:.2} dB C/N0 loss; ADC dynamic range {dr:.1} dB",
            cap.bit_depth
        ),
    );
    if max_js.is_finite() {
        let jn = max_js + snr;
        let room = dr - 10.0;
        if jn > room {
            q.score = q.score.min(0.2);
            q.severity = Severity::Invalidating;
            q.detail.push_str(&format!(
                "; strongest jammer J/N {jn:.1} dB exceeds the usable range {room:.1} dB: the recording cannot hold the interference the scenario needs"
            ));
        } else {
            q.detail.push_str(&format!(
                "; strongest jammer J/N {jn:.1} dB fits ({room:.1} dB usable)"
            ));
        }
    }
    v.push(q);
    // 2. Saturation and AGC.
    let jam = max_js.is_finite();
    let (mut s, mut sev) = (1.0, Severity::Ok);
    let mut d = match cap.clipping_fraction {
        Some(c) if c > 0.01 => {
            (s, sev) = (0.2, Severity::Invalidating);
            format!("{:.2} % of samples clipped: front end saturated", c * 100.0)
        }
        Some(c) if c > 0.001 => {
            (s, sev) = (0.7, Severity::Limited);
            format!("{:.2} % of samples clipped", c * 100.0)
        }
        Some(c) => format!("{:.3} % of samples clipped", c * 100.0),
        None => {
            (s, sev) = (0.9, Severity::Limited);
            "clipping not stated: not assessed".into()
        }
    };
    match cap.agc.as_deref() {
        Some("active") if jam => {
            s = f64::min(s, 0.6);
            if sev == Severity::Ok {
                sev = Severity::Limited;
            }
            d.push_str("; AGC active during recording under a jamming scenario: the receiver's AGC and J/N indicators see the replayer's gain, not the field's");
        }
        Some(a) => d.push_str(&format!("; AGC {a}")),
        None => d.push_str("; AGC mode not stated"),
    }
    v.push(lim(
        "saturation-agc",
        s,
        sev,
        cap.clipping_fraction,
        "fraction",
        d,
    ));
    // 3. Phase-noise doubling.
    let f2 = (cap.bandwidth_hz / 2.0).min(1e6);
    let rec = phase_jitter_deg(&cap.recorder_phase_noise, cap.pll_bandwidth_hz, f2);
    let rep = phase_jitter_deg(&cap.replayer_phase_noise, cap.pll_bandwidth_hz, f2);
    v.push(match (rec, rep) {
        (Some(a), Some(b)) => {
            let tot = (a * a + b * b).sqrt();
            let pen = 10.0 * (tot * tot / (a * a).max(1e-30)).log10();
            let (sc, sv) = if tot > 10.0 {
                (0.3, Severity::Invalidating)
            } else if tot > 5.0 {
                (0.7, Severity::Limited)
            } else {
                (1.0 - 0.02 * pen.max(0.0), Severity::Ok)
            };
            lim("lo-phase-noise", sc, sv, Some(tot), "deg", format!(
                "recorder {a:.2}°, replayer {b:.2}° RMS over {:.0} Hz–{:.0} kHz: {tot:.2}° combined, +{pen:.1} dB over the field (doubling)",
                cap.pll_bandwidth_hz, f2 / 1e3
            ))
        }
        _ => lim("lo-phase-noise", 0.9, Severity::Limited, None, "deg", "phase noise of the recorder or the replayer not stated: the doubling is not assessed".into()),
    });
    // 4. Sensors.
    let missing: Vec<&String> = spec
        .environment
        .sensors
        .iter()
        .filter(|s| !cap.sensors_synchronised || !cap.sensors_recorded.contains(s))
        .collect();
    v.push(if spec.environment.sensors.is_empty() {
        lim("sensors", 1.0, Severity::Ok, Some(0.0), "count", "the scenario needs no synchronised sensor".into())
    } else if missing.is_empty() {
        lim("sensors", 1.0, Severity::Ok, Some(0.0), "count", format!("{} recorded and synchronised", spec.environment.sensors.join(", ")))
    } else {
        lim("sensors", 0.3, Severity::Invalidating, Some(missing.len() as f64), "count", format!(
            "missing or not synchronised: {}; an integrated receiver cannot be exercised by the RF replay alone",
            missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        ))
    });
    // 5. Reference oscillator (timing scenarios).
    if spec.has_timing_criteria() {
        let tight = spec
            .criteria
            .iter()
            .filter(|c| {
                matches!(
                    c.metric,
                    Metric::MaxUnflaggedTimeErrorNs | Metric::MaxTimeErrorNs
                )
            })
            .map(|c| c.value)
            .fold(f64::INFINITY, f64::min);
        v.push(match cap.reference.as_deref().and_then(frac_stability) {
            Some(y) => {
                let ns = y * spec.scene.duration_s * 1e9;
                let (sc, sv) = if ns > tight {
                    (0.3, Severity::Invalidating)
                } else if ns > tight / 10.0 {
                    (0.7, Severity::Limited)
                } else {
                    (1.0, Severity::Ok)
                };
                lim(
                    "reference",
                    sc,
                    sv,
                    Some(ns),
                    "ns",
                    format!(
                    "{} reference: up to {ns:.1} ns over {:.0} s against a {tight:.0} ns criterion",
                    cap.reference.as_deref().unwrap_or("?"), spec.scene.duration_s
                ),
                )
            }
            None => lim(
                "reference",
                0.7,
                Severity::Limited,
                None,
                "ns",
                "reference oscillator not stated: the replay's own time error is not assessed"
                    .into(),
            ),
        });
    }
    let score = (100.0 * v.iter().map(|l| l.score).product::<f64>()).round() as u32;
    let grade = if v.iter().any(|l| l.severity == Severity::Invalidating) {
        "not-faithful"
    } else if v.iter().any(|l| l.severity == Severity::Limited) {
        "limited"
    } else {
        "faithful"
    };
    let worst: Vec<String> = v
        .iter()
        .filter(|l| l.severity != Severity::Ok)
        .map(|l| {
            format!(
                "{} ({})",
                l.id,
                serde_json::to_value(l.severity).unwrap().as_str().unwrap()
            )
        })
        .collect();
    FidelityStatement {
        method: METHOD_ID.into(),
        label: "MODELLED: rules of thumb, not a calibration of the bench.".into(),
        score,
        grade: grade.into(),
        statement: format!(
            "Replay fidelity {score}/100 ({grade}) for scenario {}{}.",
            spec.id,
            if worst.is_empty() {
                String::new()
            } else {
                format!("; limits: {}", worst.join(", "))
            }
        ),
        limits: v,
    }
}

/// The qualification a fidelity statement puts on a verdict.
pub fn qualification(f: &FidelityStatement) -> String {
    match f.grade.as_str() {
        "not-faithful" => format!("Verdict made on a replay that cannot reproduce part of the scenario ({}); a pass here is not evidence for the limited aspects.", f.statement),
        "limited" => format!("Verdict made on a replay with stated fidelity limits ({}).", f.statement),
        _ => String::new(),
    }
}
