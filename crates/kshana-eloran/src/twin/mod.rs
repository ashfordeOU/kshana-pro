// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! National eLoran network digital twin (stream F6).
//!
//! A time-stepped system model (not signal level) of an eLoran service:
//!
//! * **Transmitters**, each with a clock whose time error against its UTC(k) evolves as
//!   offset + frequency error + seeded white-FM / random-walk-FM noise, plus injected faults
//!   (phase step, frequency step);
//! * **Monitor stations** at known sites, each observing every transmitter's timing error with
//!   white measurement noise and a slowly varying temporal-ASF error (diurnal sinusoid);
//! * a **control centre** that averages the monitors per transmitter, raises an **alarm** when
//!   the estimate leaves `alarm_limit_ns` for `alarm_persistence` consecutive steps (and
//!   issues an LDC integrity warning event), and **steers** the transmitter with a PI loop on
//!   phase (frequency and phase corrections, bounded), clearing the alarm once back inside
//!   `clear_limit_ns`.
//!
//! Outputs an event log and KPIs per transmitter: largest |timing error vs UTC(k)|, share of
//! time within the service limit and not alarmed (availability), time to alarm and time to
//! correct for each fault. Deterministic per seed. MODELLED.

use crate::timebase::gauss;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

/// A fault injected into a transmitter clock.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClockFault {
    /// Phase step (ns) at `at_s`.
    PhaseStep { at_s: f64, step_ns: f64 },
    /// Frequency step (ns per s, i.e. 1e-9 fractional) from `at_s`.
    FrequencyStep { at_s: f64, rate_ns_per_s: f64 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TwinTransmitter {
    pub id: String,
    /// The UTC(k) it is steered to (label only).
    #[serde(default)]
    pub time_ref: String,
    #[serde(default)]
    pub initial_offset_ns: f64,
    /// Free-running frequency error (ns/s).
    #[serde(default)]
    pub frequency_ns_per_s: f64,
    /// White-FM phase noise per step (ns, 1σ) and random-walk-FM (ns/s per √step).
    #[serde(default)]
    pub wfm_ns: f64,
    #[serde(default)]
    pub rwfm_ns_per_s: f64,
    #[serde(default)]
    pub faults: Vec<ClockFault>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TwinMonitor {
    pub id: String,
    /// Measurement noise (ns, 1σ per observation).
    pub noise_ns: f64,
    /// Temporal-ASF error amplitude (ns) of a diurnal sinusoid, and its phase (rad).
    #[serde(default)]
    pub asf_diurnal_ns: f64,
    #[serde(default)]
    pub asf_phase_rad: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ControlConfig {
    /// Service timing limit vs UTC(k) (ns), e.g. 50.
    pub service_limit_ns: f64,
    /// Alarm threshold on the monitored estimate (ns) and persistence (steps).
    pub alarm_limit_ns: f64,
    pub alarm_persistence: usize,
    /// Alarm clears when the estimate is back inside this (ns).
    pub clear_limit_ns: f64,
    /// PI gains on phase: frequency correction = kp·e + ki·Σe (ns/s per ns), bounded.
    pub kp: f64,
    pub ki: f64,
    pub max_freq_correction_ns_per_s: f64,
    /// Phase step applied on alarm (fraction of the estimate removed at once, 0..1).
    pub phase_correction_on_alarm: f64,
}

impl Default for ControlConfig {
    fn default() -> Self {
        ControlConfig {
            service_limit_ns: 50.0,
            alarm_limit_ns: 30.0,
            alarm_persistence: 3,
            clear_limit_ns: 10.0,
            kp: 0.02,
            ki: 0.0005,
            max_freq_correction_ns_per_s: 1.0,
            phase_correction_on_alarm: 0.9,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TwinConfig {
    pub step_s: f64,
    pub duration_s: f64,
    pub seed: u64,
    pub transmitters: Vec<TwinTransmitter>,
    pub monitors: Vec<TwinMonitor>,
    #[serde(default)]
    pub control: ControlConfig,
}

/// An event of the twin's log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TwinEvent {
    pub t_s: f64,
    pub transmitter: String,
    pub kind: String,
    pub value_ns: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FaultOutcome {
    pub fault_at_s: f64,
    /// Time from fault to alarm (s), if an alarm was raised.
    pub time_to_alarm_s: Option<f64>,
    /// Time from fault until the true error is back within the service limit and stays so
    /// for 10 steps (s), if it happened.
    pub time_to_correct_s: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransmitterKpi {
    pub id: String,
    pub max_abs_error_ns: f64,
    pub rms_error_ns: f64,
    /// Share of steps within the service limit and not alarmed.
    pub availability: f64,
    pub alarms: usize,
    pub faults: Vec<FaultOutcome>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TwinResult {
    pub kpis: Vec<TransmitterKpi>,
    pub events: Vec<TwinEvent>,
    /// True error (ns) per step per transmitter, for plotting.
    pub series_ns: Vec<Vec<f64>>,
}

/// Run the twin.
pub fn run(cfg: &TwinConfig) -> Result<TwinResult, String> {
    if !(cfg.step_s > 0.0 && cfg.duration_s > 0.0) {
        return Err("twin: step and duration must be positive".into());
    }
    if cfg.monitors.is_empty() {
        return Err("twin: at least one monitor station".into());
    }
    let c = &cfg.control;
    let steps = (cfg.duration_s / cfg.step_s).round() as usize;
    let mut rng = ChaCha8Rng::seed_from_u64(cfg.seed ^ 0x7417_2026);
    let n = cfg.transmitters.len();
    let mut x: Vec<f64> = cfg.transmitters.iter().map(|t| t.initial_offset_ns).collect();
    let mut f_rw = vec![0.0f64; n];
    let mut f_ctl = vec![0.0f64; n];
    let mut integ = vec![0.0f64; n];
    let mut alarm = vec![false; n];
    let mut over = vec![0usize; n];
    let mut series = vec![Vec::with_capacity(steps); n];
    let mut events = Vec::new();
    let mut ok_steps = vec![0usize; n];
    let mut alarms = vec![0usize; n];
    let mut alarm_times: Vec<Vec<f64>> = vec![vec![]; n];
    for k in 0..steps {
        let t = k as f64 * cfg.step_s;
        for (i, tx) in cfg.transmitters.iter().enumerate() {
            // clock evolution over one step
            let mut f = tx.frequency_ns_per_s + f_rw[i] + f_ctl[i];
            for fault in &tx.faults {
                match fault {
                    ClockFault::PhaseStep { at_s, step_ns } => {
                        if t >= *at_s && t - cfg.step_s < *at_s {
                            x[i] += step_ns;
                            events.push(TwinEvent { t_s: t, transmitter: tx.id.clone(), kind: "fault_phase_step".into(), value_ns: *step_ns });
                        }
                    }
                    ClockFault::FrequencyStep { at_s, rate_ns_per_s } => {
                        if t >= *at_s {
                            f += rate_ns_per_s;
                        }
                        if t >= *at_s && t - cfg.step_s < *at_s {
                            events.push(TwinEvent { t_s: t, transmitter: tx.id.clone(), kind: "fault_frequency_step".into(), value_ns: *rate_ns_per_s });
                        }
                    }
                }
            }
            if k > 0 {
                x[i] += f * cfg.step_s + tx.wfm_ns * gauss(&mut rng);
                f_rw[i] += tx.rwfm_ns_per_s * cfg.step_s.sqrt() * gauss(&mut rng);
            }
            // monitoring
            let mut sum = 0.0;
            for m in &cfg.monitors {
                let asf = m.asf_diurnal_ns * (std::f64::consts::TAU * t / 86_400.0 + m.asf_phase_rad).sin();
                sum += x[i] + asf + m.noise_ns * gauss(&mut rng);
            }
            let est = sum / cfg.monitors.len() as f64;
            // alarm logic
            if est.abs() > c.alarm_limit_ns {
                over[i] += 1;
            } else {
                over[i] = 0;
            }
            if !alarm[i] && over[i] >= c.alarm_persistence {
                alarm[i] = true;
                alarms[i] += 1;
                alarm_times[i].push(t);
                events.push(TwinEvent { t_s: t, transmitter: tx.id.clone(), kind: "alarm_ldc_warning".into(), value_ns: est });
                // corrective phase step
                let dphi = -c.phase_correction_on_alarm * est;
                x[i] += dphi;
                events.push(TwinEvent { t_s: t, transmitter: tx.id.clone(), kind: "phase_correction".into(), value_ns: dphi });
                integ[i] = 0.0;
            } else if alarm[i] && est.abs() <= c.clear_limit_ns {
                alarm[i] = false;
                events.push(TwinEvent { t_s: t, transmitter: tx.id.clone(), kind: "alarm_cleared".into(), value_ns: est });
            }
            // PI frequency steering
            integ[i] += est * cfg.step_s;
            f_ctl[i] = (-(c.kp * est + c.ki * integ[i])).clamp(-c.max_freq_correction_ns_per_s, c.max_freq_correction_ns_per_s);
            series[i].push(x[i]);
            if x[i].abs() <= c.service_limit_ns && !alarm[i] {
                ok_steps[i] += 1;
            }
        }
    }
    let kpis = cfg
        .transmitters
        .iter()
        .enumerate()
        .map(|(i, tx)| {
            let s = &series[i];
            let max_abs = s.iter().map(|v| v.abs()).fold(0.0, f64::max);
            let rms = (s.iter().map(|v| v * v).sum::<f64>() / s.len().max(1) as f64).sqrt();
            let faults = tx
                .faults
                .iter()
                .map(|f| {
                    let at = match f {
                        ClockFault::PhaseStep { at_s, .. } | ClockFault::FrequencyStep { at_s, .. } => *at_s,
                    };
                    let tta = alarm_times[i].iter().find(|&&a| a >= at).map(|a| a - at);
                    let k0 = (at / cfg.step_s).ceil() as usize;
                    let mut ttc = None;
                    let mut run_len = 0usize;
                    let mut left = false;
                    for (k, v) in s.iter().enumerate().skip(k0) {
                        if v.abs() > c.service_limit_ns {
                            left = true;
                            run_len = 0;
                            continue;
                        }
                        if left {
                            run_len += 1;
                            if run_len >= 10 {
                                ttc = Some((k + 1 - 10) as f64 * cfg.step_s - at);
                                break;
                            }
                        }
                    }
                    if !left {
                        ttc = Some(0.0);
                    }
                    FaultOutcome { fault_at_s: at, time_to_alarm_s: tta, time_to_correct_s: ttc }
                })
                .collect();
            TransmitterKpi {
                id: tx.id.clone(),
                max_abs_error_ns: max_abs,
                rms_error_ns: rms,
                availability: ok_steps[i] as f64 / steps.max(1) as f64,
                alarms: alarms[i],
                faults,
            }
        })
        .collect();
    events.sort_by(|a, b| a.t_s.partial_cmp(&b.t_s).unwrap_or(std::cmp::Ordering::Equal));
    Ok(TwinResult { kpis, events, series_ns: series })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> TwinConfig {
        TwinConfig {
            step_s: 10.0,
            duration_s: 6.0 * 3600.0,
            seed: 3,
            transmitters: vec![TwinTransmitter {
                id: "TX-A".into(),
                time_ref: "UTC(X)".into(),
                initial_offset_ns: 2.0,
                frequency_ns_per_s: 0.001,
                wfm_ns: 0.5,
                rwfm_ns_per_s: 1e-5,
                faults: vec![],
            }],
            monitors: vec![
                TwinMonitor { id: "M1".into(), noise_ns: 3.0, asf_diurnal_ns: 5.0, asf_phase_rad: 0.0 },
                TwinMonitor { id: "M2".into(), noise_ns: 3.0, asf_diurnal_ns: 5.0, asf_phase_rad: 3.0 },
            ],
            control: ControlConfig::default(),
        }
    }

    #[test]
    fn nominal_network_stays_within_limits_without_alarms() {
        let r = run(&base()).unwrap();
        let k = &r.kpis[0];
        assert_eq!(k.alarms, 0, "{:?}", r.events);
        assert!(k.max_abs_error_ns < 50.0, "{k:?}");
        assert!(k.availability > 0.99);
        assert_eq!(r, run(&base()).unwrap(), "not deterministic");
    }

    #[test]
    fn phase_step_fault_is_alarmed_and_corrected() {
        let mut c = base();
        c.transmitters[0].faults.push(ClockFault::PhaseStep { at_s: 3600.0, step_ns: 200.0 });
        let r = run(&c).unwrap();
        let k = &r.kpis[0];
        assert!(k.alarms >= 1);
        let f = &k.faults[0];
        let tta = f.time_to_alarm_s.expect("alarm raised");
        assert!(tta <= (c.control.alarm_persistence as f64) * c.step_s + c.step_s, "time to alarm {tta}");
        let ttc = f.time_to_correct_s.expect("corrected");
        assert!(ttc <= 600.0, "time to correct {ttc}");
        assert!(r.events.iter().any(|e| e.kind == "alarm_ldc_warning"));
        assert!(r.events.iter().any(|e| e.kind == "alarm_cleared"));
    }

    #[test]
    fn frequency_step_is_steered_out() {
        let mut c = base();
        c.transmitters[0].faults.push(ClockFault::FrequencyStep { at_s: 3600.0, rate_ns_per_s: 0.05 });
        let r = run(&c).unwrap();
        let s = &r.series_ns[0];
        // after the steering loop has converged (last hour) the error is back near zero
        let tail = &s[s.len() - 360..];
        let worst = tail.iter().map(|v| v.abs()).fold(0.0, f64::max);
        assert!(worst < 30.0, "residual {worst} ns");
    }
}
