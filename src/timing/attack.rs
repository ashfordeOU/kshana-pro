// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # Holdover under attack: telecom PRTC/ePRTC and substation PTP (W1119 C8; research 02 gaps
//! 17, 18, 19)
//!
//! A GNSS-disciplined clock is the timing source of a telecom PRTC or ePRTC and of a substation
//! PTP grandmaster. A spoofer that **drags** GNSS time slowly and then **steps** it moves the
//! clock with it unless a detector notices and the clock leaves GNSS for holdover or for an
//! independent reference (eLoran). This module runs that scenario and reports the time error
//! (TE) against configurable masks.
//!
//! * The oscillator is the eLoran crate's `sync::edo` model ([`kshana_eloran::sync::edo::free_run`]:
//!   initial frequency offset, ageing, white PM, white FM and random-walk FM), and the discipline
//!   is the same type-2 loop as `sync::edo` (ω_n = 1/τ_loop, k_p = 2ζω_n, k_i = ω_n²).
//! * The spoofer adds `drag_ns_per_s` from `start_s` to `step_s`, then a step of `step_ns`.
//! * Detectors ([`Detector`]): `none`; `rate_guard` (the loop's own phase error averaged over a
//!   window exceeds a threshold: catches steps, not drags a type-2 loop tracks); `cross_check`
//!   (GNSS time against an independent reference, eLoran, after a calibration window: catches a
//!   drag once its accumulated offset passes the threshold).
//! * On alarm the clock goes to `holdover` (the steering frozen at its value `rollback_s` before
//!   the alarm, so the drag's frequency is not carried into holdover) or to `eloran` (the loop
//!   continues on the eLoran reference, its uncalibrated bias included).
//! * Masks ([`Mask`]): max |TE| limits. Defaults cite ITU-T G.8272 (PRTC-A 100 ns, PRTC-B 40 ns)
//!   and G.8272.1 (ePRTC 30 ns), a G.8271-class end-application budget (1.5 µs) and the IEEE
//!   C37.238 example budget (±1 µs); all are **illustrative defaults, configurable, and to be
//!   confirmed against the Recommendations and the standard**. The masks here are a single
//!   max |TE| bar each, not the full MTIE/TDEV masks of the Recommendations (MTIE and TDEV are
//!   reported alongside).
//! * The substation case adds the C37.238 time-inaccuracy view: what the grandmaster would
//!   advertise (locked inaccuracy, plus a holdover growth model once it is in holdover) against
//!   the actual TE; an advertised value below the actual TE is **misleading**.
//!
//! Evidence: **MODELLED**. Checked against closed forms (an undetected drag and step add their
//! sum to TE; a cross-check alarms when drag × time passes its threshold; an eLoran fallback
//! settles to the eLoran bias) in `tests/holdover_attack.rs`. Not run on a real clock or
//! receiver.

use kshana_eloran::sync::edo::{free_run, Oscillator};
use kshana_eloran::sync::measure::{mtie_curve, StabilityPoint};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

pub const FORMAT: &str = "kshana-pro-holdover-attack/1";
pub const EVIDENCE: &str = "MODELLED: checked against closed forms; not run on a real clock or \
receiver. Masks are illustrative defaults citing ITU-T G.8272/G.8272.1/G.8271 and IEEE C37.238, \
to be confirmed against them.";

/// Oscillator presets.
pub fn preset(name: &str) -> Option<Oscillator> {
    Some(match name {
        "ocxo" => Oscillator::default(),
        "rubidium" => Oscillator {
            y0: 1e-11,
            ageing_per_day: 2e-12,
            adev1_wpm: 1e-12,
            adev1_wfm: 3e-12,
            adev1_rwfm: 1e-14,
        },
        "cesium" => Oscillator {
            y0: 0.0,
            ageing_per_day: 0.0,
            adev1_wpm: 1e-12,
            adev1_wfm: 5e-12,
            adev1_rwfm: 1e-15,
        },
        _ => return None,
    })
}

/// The spoofer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Spoof {
    pub enabled: bool,
    pub start_s: f64,
    /// Slow time push (ns per s = fractional frequency × 1e9).
    pub drag_ns_per_s: f64,
    /// Time of the step (the drag stops there).
    pub step_s: f64,
    pub step_ns: f64,
}

impl Default for Spoof {
    fn default() -> Self {
        Spoof {
            enabled: true,
            start_s: 3600.0,
            drag_ns_per_s: 0.2,
            step_s: 5400.0,
            step_ns: 1000.0,
        }
    }
}

impl Spoof {
    /// Offset the spoofer adds to GNSS time at `t` (ns).
    pub fn offset_ns(&self, t: f64) -> f64 {
        if !self.enabled || t < self.start_s {
            return 0.0;
        }
        self.drag_ns_per_s * (t.min(self.step_s) - self.start_s)
            + if t >= self.step_s { self.step_ns } else { 0.0 }
    }
}

/// Detector kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectorKind {
    None,
    RateGuard,
    CrossCheck,
}

/// Detector settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Detector {
    pub kind: DetectorKind,
    /// Alarm threshold (ns) on the window mean.
    pub threshold_ns: f64,
    /// Averaging window (s).
    pub window_s: f64,
    /// Cross-check calibration window [from, to] (s): the mean of the cross-check difference
    /// `(eLoran reference) − (GNSS reference)` learned there and removed afterwards.
    pub calib_from_s: f64,
    pub calib_to_s: f64,
}

impl Default for Detector {
    fn default() -> Self {
        Detector {
            kind: DetectorKind::RateGuard,
            threshold_ns: 50.0,
            window_s: 10.0,
            calib_from_s: 1800.0,
            calib_to_s: 3000.0,
        }
    }
}

/// What the clock does on alarm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fallback {
    Holdover,
    Eloran,
}

/// A reference as observed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Reference {
    pub noise_ns: f64,
    pub bias_ns: f64,
}

impl Default for Reference {
    fn default() -> Self {
        Reference {
            noise_ns: 5.0,
            bias_ns: 0.0,
        }
    }
}

/// A max |TE| mask.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mask {
    pub name: String,
    pub limit_ns: f64,
    pub source: String,
}

/// The default masks (illustrative).
pub fn default_masks() -> Vec<Mask> {
    let m = |n: &str, l: f64, s: &str| Mask {
        name: n.into(),
        limit_ns: l,
        source: s.into(),
    };
    vec![
        m(
            "PRTC-B",
            40.0,
            "ITU-T G.8272 (illustrative, confirm against the Recommendation)",
        ),
        m(
            "PRTC-A",
            100.0,
            "ITU-T G.8272 (illustrative, confirm against the Recommendation)",
        ),
        m(
            "ePRTC locked",
            30.0,
            "ITU-T G.8272.1 (illustrative, confirm against the Recommendation)",
        ),
        m(
            "TDD end application",
            1500.0,
            "ITU-T G.8271 class (illustrative)",
        ),
    ]
}

/// C37.238 time-inaccuracy view (substation case).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PtpBudget {
    pub enabled: bool,
    /// The C37.238 example end-to-end budget (ns), illustrative.
    pub budget_ns: f64,
    /// Share of the budget taken by the PTP network (transparent clocks, asymmetry) (ns).
    pub network_ns: f64,
    /// Inaccuracy the grandmaster advertises when locked (ns).
    pub locked_inaccuracy_ns: f64,
    /// Holdover growth the grandmaster adds to its advertised inaccuracy (ns per s).
    pub holdover_growth_ns_per_s: f64,
}

impl Default for PtpBudget {
    fn default() -> Self {
        PtpBudget {
            enabled: false,
            budget_ns: 1000.0,
            network_ns: 200.0,
            locked_inaccuracy_ns: 100.0,
            holdover_growth_ns_per_s: 0.1,
        }
    }
}

/// A scenario.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AttackScenario {
    pub title: String,
    pub tau0_s: f64,
    pub duration_s: f64,
    /// Oscillator preset (`ocxo`, `rubidium`, `cesium`); `oscillator` overrides it when given.
    pub preset: String,
    pub oscillator: Option<Oscillator>,
    pub loop_tau_s: f64,
    pub damping: f64,
    pub gnss: Reference,
    pub eloran: Reference,
    pub spoof: Spoof,
    pub detector: Detector,
    pub fallback: Fallback,
    /// Holdover uses the steering from this long before the alarm (s).
    pub rollback_s: f64,
    pub masks: Vec<Mask>,
    pub ptp: PtpBudget,
    pub seed: u64,
}

impl Default for AttackScenario {
    fn default() -> Self {
        AttackScenario {
            title: "PRTC under a drag-and-step spoof".into(),
            tau0_s: 1.0,
            duration_s: 10_800.0,
            preset: "rubidium".into(),
            oscillator: None,
            loop_tau_s: 300.0,
            damping: 0.707,
            gnss: Reference {
                noise_ns: 5.0,
                bias_ns: 0.0,
            },
            eloran: Reference {
                noise_ns: 30.0,
                bias_ns: 20.0,
            },
            spoof: Spoof::default(),
            detector: Detector::default(),
            fallback: Fallback::Holdover,
            rollback_s: 600.0,
            masks: default_masks(),
            ptp: PtpBudget::default(),
            seed: 1,
        }
    }
}

impl AttackScenario {
    /// The substation preset: an OCXO grandmaster under a 2 µs step, C37.238 budget.
    pub fn substation() -> Self {
        AttackScenario {
            title: "Substation PTP grandmaster under a GNSS spoof (C37.238 budget)".into(),
            preset: "ocxo".into(),
            loop_tau_s: 100.0,
            spoof: Spoof { enabled: true, start_s: 3600.0, drag_ns_per_s: 0.0, step_s: 3600.0, step_ns: 2000.0 },
            masks: vec![Mask {
                name: "C37.238 budget after network allowance".into(),
                limit_ns: 800.0,
                source: "IEEE C37.238 example ±1 µs budget less a 200 ns network allowance (illustrative)".into(),
            }],
            ptp: PtpBudget { enabled: true, ..PtpBudget::default() },
            ..AttackScenario::default()
        }
    }
}

/// Result per mask.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MaskResult {
    pub name: String,
    pub limit_ns: f64,
    pub source: String,
    pub max_abs_te_ns: f64,
    pub met: bool,
    pub first_exceeded_s: Option<f64>,
}

/// The C37.238 view.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PtpResult {
    pub budget_ns: f64,
    pub network_ns: f64,
    pub grandmaster_allowance_ns: f64,
    pub max_abs_te_ns: f64,
    pub within_budget: bool,
    /// Epochs where the advertised inaccuracy was below the actual |TE|.
    pub misleading_epochs: usize,
    pub first_misleading_s: Option<f64>,
}

/// The report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttackReport {
    pub format: String,
    pub evidence: String,
    pub scenario: AttackScenario,
    pub alarm_s: Option<f64>,
    /// Alarm time minus spoof start, and minus step time (s).
    pub latency_from_start_s: Option<f64>,
    pub latency_from_step_s: Option<f64>,
    pub spoof_offset_at_alarm_ns: Option<f64>,
    pub te_at_alarm_ns: Option<f64>,
    pub max_abs_te_before_attack_ns: f64,
    pub max_abs_te_after_attack_ns: f64,
    pub final_te_ns: f64,
    pub masks: Vec<MaskResult>,
    pub ptp: Option<PtpResult>,
    pub mtie: Vec<StabilityPoint>,
    /// TE decimated to one value per 10 s (ns), and the mode per sample.
    pub te_10s_ns: Vec<f64>,
    pub mode_10s: Vec<String>,
    pub limits: Vec<String>,
}

fn gauss(r: &mut ChaCha8Rng) -> f64 {
    let (u1, u2): (f64, f64) = (r.gen::<f64>().max(1e-300), r.gen());
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
}

/// Run a scenario.
pub fn run(s: &AttackScenario) -> Result<AttackReport, String> {
    if s.tau0_s <= 0.0 || s.duration_s <= 10.0 * s.tau0_s || s.loop_tau_s <= s.tau0_s {
        return Err("tau0, duration and loop time constant must be positive and ordered".into());
    }
    let osc = match &s.oscillator {
        Some(o) => o.clone(),
        None => preset(&s.preset).ok_or(format!("unknown oscillator preset '{}'", s.preset))?,
    };
    let n = (s.duration_s / s.tau0_s) as usize;
    let x = free_run(&osc, s.tau0_s, n, s.seed);
    let mut r = ChaCha8Rng::seed_from_u64(s.seed ^ 0x51ed_270b_2764_8f1d);
    let wn = 1.0 / s.loop_tau_s;
    let (kp, ki) = (2.0 * s.damping * wn, wn * wn);
    let (mut steer, mut integ, mut corr) = (0.0f64, 0.0f64, 0.0f64);
    let mut steer_hist: Vec<f64> = Vec::with_capacity(n);
    let (mut te, mut mode) = (Vec::with_capacity(n), Vec::with_capacity(n));
    let win = ((s.detector.window_s / s.tau0_s).round() as usize).max(1);
    let mut buf: std::collections::VecDeque<f64> = Default::default();
    let (mut cal_sum, mut cal_n) = (0.0f64, 0usize);
    let mut alarm: Option<(usize, f64)> = None; // epoch, spoof offset
    let mut hold_steer = 0.0;
    let mut hold_start = 0.0;
    let mut advertised = Vec::with_capacity(n);
    for (k, &xo) in x.iter().enumerate() {
        let t = k as f64 * s.tau0_s;
        let out = xo + corr;
        te.push(out);
        let sp = s.spoof.offset_ns(t);
        let e_g = out - (s.gnss.bias_ns + sp + s.gnss.noise_ns * gauss(&mut r));
        let e_e = out - (s.eloran.bias_ns + s.eloran.noise_ns * gauss(&mut r));
        // Detector (only while locked to GNSS).
        if alarm.is_none() {
            let v = match s.detector.kind {
                DetectorKind::None => None,
                DetectorKind::RateGuard => Some(e_g),
                DetectorKind::CrossCheck => {
                    let d = e_g - e_e; // = (eLoran ref) − (GNSS ref), noisy
                    if t >= s.detector.calib_from_s && t < s.detector.calib_to_s {
                        cal_sum += d;
                        cal_n += 1;
                        None
                    } else if t >= s.detector.calib_to_s && cal_n > 0 {
                        Some(d - cal_sum / cal_n as f64)
                    } else {
                        None
                    }
                }
            };
            // The loop needs ~10 τ to pull in; the guard is armed after that.
            if let Some(v) = v.filter(|_| t >= 10.0 * s.loop_tau_s) {
                buf.push_back(v);
                if buf.len() > win {
                    buf.pop_front();
                }
                if buf.len() == win {
                    let m = buf.iter().sum::<f64>() / win as f64;
                    if m.abs() > s.detector.threshold_ns {
                        alarm = Some((k, sp));
                        let back = ((s.rollback_s / s.tau0_s) as usize).min(k);
                        hold_steer = steer_hist.get(k - back).copied().unwrap_or(steer);
                        hold_start = t;
                    }
                }
            }
        }
        let (meas, m) = match (alarm, s.fallback) {
            (None, _) => (Some(e_g), "gnss"),
            (Some(_), Fallback::Eloran) => (Some(e_e), "eloran"),
            (Some(_), Fallback::Holdover) => (None, "holdover"),
        };
        mode.push(m);
        advertised.push(if m == "holdover" {
            s.ptp.locked_inaccuracy_ns + s.ptp.holdover_growth_ns_per_s * (t - hold_start)
        } else {
            s.ptp.locked_inaccuracy_ns
        });
        if let Some(e) = meas {
            integ += ki * e * s.tau0_s;
            steer = -(kp * e + integ);
        } else {
            steer = hold_steer;
        }
        steer_hist.push(steer);
        corr += steer * s.tau0_s;
    }
    let k_att = if s.spoof.enabled {
        (s.spoof.start_s / s.tau0_s) as usize
    } else {
        n
    };
    let settle = ((10.0 * s.loop_tau_s / s.tau0_s) as usize).min(n);
    let maxabs = |a: usize, b: usize| te[a.min(b)..b].iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let k_att = k_att.clamp(settle, n);
    let masks = s
        .masks
        .iter()
        .map(|m| {
            let first = te[settle..]
                .iter()
                .position(|v| v.abs() > m.limit_ns)
                .map(|i| (i + settle) as f64 * s.tau0_s);
            let mx = maxabs(settle, n);
            MaskResult {
                name: m.name.clone(),
                limit_ns: m.limit_ns,
                source: m.source.clone(),
                max_abs_te_ns: round(mx),
                met: mx <= m.limit_ns,
                first_exceeded_s: first,
            }
        })
        .collect();
    let ptp = s.ptp.enabled.then(|| {
        let allow = s.ptp.budget_ns - s.ptp.network_ns;
        let mis: Vec<usize> = (settle..n)
            .filter(|&k| te[k].abs() > advertised[k])
            .collect();
        // Isolated noise excursions above a locked advertisement are not the point: count
        // epochs where |TE| exceeds the advertised value by more than 3σ of the GNSS noise.
        let mis: Vec<usize> = mis
            .into_iter()
            .filter(|&k| te[k].abs() > advertised[k] + 3.0 * s.gnss.noise_ns)
            .collect();
        PtpResult {
            budget_ns: s.ptp.budget_ns,
            network_ns: s.ptp.network_ns,
            grandmaster_allowance_ns: allow,
            max_abs_te_ns: round(maxabs(settle, n)),
            within_budget: maxabs(settle, n) <= allow,
            misleading_epochs: mis.len(),
            first_misleading_s: mis.first().map(|&k| k as f64 * s.tau0_s),
        }
    });
    let alarm_s = alarm.map(|(k, _)| k as f64 * s.tau0_s);
    let mut limits = vec![
        "The oscillator, references and spoofer are models; a real clock's noise, temperature response and receiver behaviour under capture differ.".to_string(),
        "Masks are single max |TE| bars with illustrative defaults citing the Recommendations and C37.238; the full MTIE/TDEV masks are not applied (MTIE is reported).".into(),
        "A spoofer that drags slower than the cross-check threshold over the calibration and averaging windows, or that also attacks the independent reference, is not detected by these detectors.".into(),
    ];
    if s.fallback == Fallback::Eloran {
        limits.push(format!(
            "The eLoran fallback carries its uncalibrated bias ({:.0} ns here) into the output.",
            s.eloran.bias_ns
        ));
    } else if let Some(a) =
        alarm_s.filter(|a| s.spoof.enabled && a - s.rollback_s > s.spoof.start_s)
    {
        limits.push(format!(
            "Holdover took the steering from {:.0} s before the alarm ({:.0} s), after the drag began ({:.0} s): the drag's frequency is carried into holdover and the time error keeps growing.",
            s.rollback_s, a, s.spoof.start_s
        ));
    }
    Ok(AttackReport {
        format: FORMAT.into(),
        evidence: EVIDENCE.into(),
        scenario: s.clone(),
        alarm_s,
        latency_from_start_s: alarm_s
            .filter(|_| s.spoof.enabled)
            .map(|a| a - s.spoof.start_s),
        latency_from_step_s: alarm_s
            .filter(|a| s.spoof.enabled && *a >= s.spoof.step_s)
            .map(|a| a - s.spoof.step_s),
        spoof_offset_at_alarm_ns: alarm.map(|(_, o)| round(o)),
        te_at_alarm_ns: alarm.map(|(k, _)| round(te[k])),
        max_abs_te_before_attack_ns: round(maxabs(settle, k_att)),
        max_abs_te_after_attack_ns: round(maxabs(k_att, n)),
        final_te_ns: round(*te.last().unwrap_or(&0.0)),
        masks,
        ptp,
        mtie: mtie_curve(&te[settle..], s.tau0_s),
        te_10s_ns: te
            .iter()
            .step_by((10.0 / s.tau0_s).max(1.0) as usize)
            .map(|v| round(*v))
            .collect(),
        mode_10s: mode
            .iter()
            .step_by((10.0 / s.tau0_s).max(1.0) as usize)
            .map(|m| m.to_string())
            .collect(),
        limits,
    })
}

fn round(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}
