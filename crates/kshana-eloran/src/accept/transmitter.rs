// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Transmitter acceptance from a capture of the radiated (sampled) or dummy-load signal
//! (research 01 gap 22: methodology exists only as papers; the UK will commission five
//! eLoran transmitters by 2030).
//!
//! Per pulse: carrier phase code (sign of the matched-filter correlation), the standard zero
//! crossing (30 µs after the time origin) against the transmission schedule, the ECD by the
//! envelope matched filter, and the peak amplitude. Per capture: ECD mean and spread, timing
//! offset and jitter, phase-code errors, amplitude stability, and the fraction of power in
//! 90–110 kHz. Each is checked against [`TxLimits`].
//!
//! The default limits are **illustrative engineering values, not taken from a transmitter
//! specification**: set them from the procurement specification (e.g. the operator's signal
//! specification) before using a verdict. Evidence: **MODELLED** (synthetic captures with
//! injected ECD, timing, phase-code and amplitude faults).

use crate::accept::suite::envelope_ecd_us;
use crate::hw::calib::zero_crossing;
use crate::signal::{Parity, PulseSignal, Role};
use serde::{Deserialize, Serialize};

/// Acceptance limits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TxLimits {
    /// |mean ECD| (µs).
    pub ecd_mean_abs_us: f64,
    /// ECD standard deviation (µs).
    pub ecd_std_us: f64,
    /// Zero-crossing jitter, standard deviation (ns).
    pub zc_jitter_ns: f64,
    /// |mean zero-crossing offset from schedule| (ns).
    pub zc_offset_abs_ns: f64,
    /// Phase-code errors allowed.
    pub phase_code_errors: usize,
    /// Pulse-to-pulse amplitude variation, standard deviation (% of mean).
    pub amplitude_std_pct: f64,
    /// Minimum fraction of power in 90–110 kHz.
    pub in_band_fraction: f64,
}

impl Default for TxLimits {
    fn default() -> Self {
        TxLimits {
            ecd_mean_abs_us: 1.0,
            ecd_std_us: 0.2,
            zc_jitter_ns: 10.0,
            zc_offset_abs_ns: 50.0,
            phase_code_errors: 0,
            amplitude_std_pct: 2.0,
            in_band_fraction: 0.99,
        }
    }
}

/// What was transmitted and where it sits in the capture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TxSchedule {
    pub gri_us: f64,
    pub role: Role,
    /// Capture time (s) of the first group's start (first pulse time origin).
    pub first_group_s: f64,
    pub first_parity: Parity,
    pub groups: usize,
}

/// One pulse measured.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PulseMeas {
    pub group: usize,
    pub k: usize,
    pub expected_code: i8,
    pub measured_code: i8,
    pub zc_offset_ns: f64,
    pub ecd_us: f64,
    pub amplitude: f64,
}

/// One check.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TxCheck {
    pub name: String,
    pub value: f64,
    pub limit: f64,
    pub pass: bool,
}

/// Acceptance report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TxReport {
    pub pulses: Vec<PulseMeas>,
    pub ecd_mean_us: f64,
    pub ecd_std_us: f64,
    pub zc_offset_mean_ns: f64,
    pub zc_jitter_ns: f64,
    pub phase_code_errors: usize,
    pub amplitude_std_pct: f64,
    pub in_band_fraction: f64,
    pub checks: Vec<TxCheck>,
    pub pass: bool,
    pub limits_note: String,
}

fn mean_std(v: &[f64]) -> (f64, f64) {
    let n = v.len().max(1) as f64;
    let m = v.iter().sum::<f64>() / n;
    let s = (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0).max(1.0)).sqrt();
    (m, s)
}

/// Fraction of the power of `x` (one group's samples) in [lo, hi] Hz (DFT on a 1 kHz grid).
pub fn band_fraction(x: &[f64], fs: f64, lo: f64, hi: f64) -> f64 {
    let n = x.len();
    let (mut inb, mut tot) = (0.0, 0.0);
    let df = 1000.0;
    let kmax = (fs / 2.0 / df) as usize;
    for k in 1..kmax {
        let f = k as f64 * df;
        let w = 2.0 * std::f64::consts::PI * f / fs;
        let (mut re, mut im) = (0.0, 0.0);
        // Goertzel-free direct sum with a recurrence for cos/sin.
        let (c, s) = (w.cos(), w.sin());
        let (mut cr, mut ci) = (1.0f64, 0.0f64);
        for &v in x {
            re += v * cr;
            im -= v * ci;
            let ncr = cr * c - ci * s;
            ci = cr * s + ci * c;
            cr = ncr;
        }
        let p = (re * re + im * im) / n as f64;
        tot += p;
        if f >= lo && f <= hi {
            inb += p;
        }
    }
    if tot > 0.0 {
        inb / tot
    } else {
        0.0
    }
}

/// Measure a capture `x` sampled at `fs_hz` against the schedule.
pub fn measure(
    x: &[f64],
    fs_hz: f64,
    sch: &TxSchedule,
    limits: &TxLimits,
) -> Result<TxReport, String> {
    let sig = PulseSignal::loran_c();
    let mut pulses = vec![];
    let mut parity = sch.first_parity;
    for g in 0..sch.groups {
        let g0 = sch.first_group_s + g as f64 * sch.gri_us * 1e-6;
        for p in sig.group(sch.role, parity) {
            let t0 = g0 + p.offset.as_s();
            let expected: i8 = if p.phase_rad.abs() > 1.0 { -1 } else { 1 };
            let a = ((t0 - 5e-6) * fs_hz).floor().max(0.0) as usize;
            let b = (((t0 + sig.envelope.extent_s()) * fs_hz).ceil() as usize).min(x.len());
            if a + 10 >= b {
                return Err(format!("group {g} pulse {} lies outside the capture", p.k));
            }
            // Matched filter against the phase-0 reference: the sign is the phase code.
            let mut corr = 0.0;
            let mut amp = 0.0f64;
            for (i, v) in x.iter().enumerate().take(b).skip(a) {
                corr += v * sig.pulse(i as f64 / fs_hz - t0, 0.0, 0.0);
                amp = amp.max(v.abs());
            }
            let code: i8 = if corr >= 0.0 { 1 } else { -1 };
            // Sign-corrected local copy, then the standard zero crossing and the ECD.
            let local: Vec<f64> = x.iter().map(|v| v * code as f64).collect();
            let zc = zero_crossing(&local, fs_hz, t0 + 30e-6)
                .ok_or_else(|| format!("group {g} pulse {}: no zero crossing", p.k))?;
            let origin = zc - 30e-6;
            let ecd = envelope_ecd_us(&local, fs_hz, origin).unwrap_or(f64::NAN);
            pulses.push(PulseMeas {
                group: g,
                k: p.k,
                expected_code: expected,
                measured_code: code,
                zc_offset_ns: (zc - (t0 + 30e-6)) * 1e9,
                ecd_us: ecd,
                amplitude: amp,
            });
        }
        parity = match parity {
            Parity::A => Parity::B,
            Parity::B => Parity::A,
        };
    }
    let ecds: Vec<f64> = pulses
        .iter()
        .map(|p| p.ecd_us)
        .filter(|v| v.is_finite())
        .collect();
    let (em, es) = mean_std(&ecds);
    let (zm, zs) = mean_std(&pulses.iter().map(|p| p.zc_offset_ns).collect::<Vec<_>>());
    let (am, asd) = mean_std(&pulses.iter().map(|p| p.amplitude).collect::<Vec<_>>());
    let errs = pulses
        .iter()
        .filter(|p| p.measured_code != p.expected_code)
        .count();
    let ga = (sch.first_group_s * fs_hz) as usize;
    let gb = (((sch.first_group_s + sig.group_span_s(sch.role)) * fs_hz) as usize).min(x.len());
    let inb = band_fraction(&x[ga.min(gb)..gb], fs_hz, 90e3, 110e3);
    let amp_pct = if am > 0.0 {
        100.0 * asd / am
    } else {
        f64::INFINITY
    };
    let chk = |name: &str, v: f64, l: f64, pass: bool| TxCheck {
        name: name.into(),
        value: v,
        limit: l,
        pass,
    };
    let checks = vec![
        chk(
            "ecd_mean_abs_us",
            em.abs(),
            limits.ecd_mean_abs_us,
            em.abs() <= limits.ecd_mean_abs_us,
        ),
        chk("ecd_std_us", es, limits.ecd_std_us, es <= limits.ecd_std_us),
        chk(
            "zc_offset_abs_ns",
            zm.abs(),
            limits.zc_offset_abs_ns,
            zm.abs() <= limits.zc_offset_abs_ns,
        ),
        chk(
            "zc_jitter_ns",
            zs,
            limits.zc_jitter_ns,
            zs <= limits.zc_jitter_ns,
        ),
        chk(
            "phase_code_errors",
            errs as f64,
            limits.phase_code_errors as f64,
            errs <= limits.phase_code_errors,
        ),
        chk(
            "amplitude_std_pct",
            amp_pct,
            limits.amplitude_std_pct,
            amp_pct <= limits.amplitude_std_pct,
        ),
        chk(
            "in_band_fraction",
            inb,
            limits.in_band_fraction,
            inb >= limits.in_band_fraction,
        ),
    ];
    let pass = checks.iter().all(|c| c.pass);
    Ok(TxReport {
        pulses,
        ecd_mean_us: em,
        ecd_std_us: es,
        zc_offset_mean_ns: zm,
        zc_jitter_ns: zs,
        phase_code_errors: errs,
        amplitude_std_pct: amp_pct,
        in_band_fraction: inb,
        checks,
        pass,
        limits_note: "default limits are illustrative engineering values; set them from the transmitter specification".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FS: f64 = 2e6;

    struct Faults {
        ecd_us: f64,
        jitter_ns: f64,
        offset_ns: f64,
        flip: Option<(usize, usize)>,
        amp_var: f64,
        noise: f64,
    }

    fn capture(sch: &TxSchedule, f: &Faults) -> Vec<f64> {
        let sig = PulseSignal::loran_c();
        let n = ((sch.first_group_s + sch.groups as f64 * sch.gri_us * 1e-6 + 1e-3) * FS) as usize;
        let mut x = vec![0.0; n];
        let mut k: u64 = 11;
        let mut rnd = || {
            k = k
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((k >> 11) as f64 / (1u64 << 53) as f64) - 0.5
        };
        let mut parity = sch.first_parity;
        for g in 0..sch.groups {
            for p in sig.group(sch.role, parity) {
                let jit = f.jitter_ns * 3.46 * rnd();
                let t0 = sch.first_group_s
                    + g as f64 * sch.gri_us * 1e-6
                    + p.offset.as_s()
                    + (f.offset_ns + jit) * 1e-9;
                let mut ph = p.phase_rad;
                if f.flip == Some((g, p.k)) {
                    ph += std::f64::consts::PI;
                }
                let a = 1.0 + f.amp_var * 3.46 * rnd();
                let i0 = (t0 * FS) as usize;
                let i1 = (((t0 + sig.envelope.extent_s()) * FS) as usize + 2).min(n);
                for (i, v) in x.iter_mut().enumerate().take(i1).skip(i0) {
                    *v += 0.5 * a * sig.pulse(i as f64 / FS - t0, f.ecd_us * 1e-6, ph);
                }
            }
            parity = if parity == Parity::A {
                Parity::B
            } else {
                Parity::A
            };
        }
        for v in x.iter_mut() {
            *v += f.noise * 3.46 * rnd();
        }
        x
    }

    fn sch() -> TxSchedule {
        TxSchedule {
            gri_us: 49_900.0,
            role: Role::Master,
            first_group_s: 1e-3,
            first_parity: Parity::A,
            groups: 4,
        }
    }

    fn clean() -> Faults {
        Faults {
            ecd_us: 0.0,
            jitter_ns: 0.0,
            offset_ns: 0.0,
            flip: None,
            amp_var: 0.0,
            noise: 0.0,
        }
    }

    #[test]
    fn a_clean_transmitter_passes() {
        let r = measure(&capture(&sch(), &clean()), FS, &sch(), &TxLimits::default()).unwrap();
        assert!(r.pass, "{:?}", r.checks);
        assert_eq!(r.pulses.len(), 36);
        assert!(
            r.ecd_mean_us.abs() < 0.05 && r.zc_jitter_ns < 1.0,
            "{} {}",
            r.ecd_mean_us,
            r.zc_jitter_ns
        );
        assert!(r.in_band_fraction > 0.99, "{}", r.in_band_fraction);
    }

    #[test]
    fn injected_faults_are_measured_and_failed() {
        let r = measure(
            &capture(
                &sch(),
                &Faults {
                    ecd_us: 2.0,
                    ..clean()
                },
            ),
            FS,
            &sch(),
            &TxLimits::default(),
        )
        .unwrap();
        assert!((r.ecd_mean_us - 2.0).abs() < 0.15, "ECD {}", r.ecd_mean_us);
        assert!(
            !r.checks
                .iter()
                .find(|c| c.name == "ecd_mean_abs_us")
                .unwrap()
                .pass
        );
        let r = measure(
            &capture(
                &sch(),
                &Faults {
                    jitter_ns: 30.0,
                    offset_ns: 100.0,
                    ..clean()
                },
            ),
            FS,
            &sch(),
            &TxLimits::default(),
        )
        .unwrap();
        assert!(
            (r.zc_offset_mean_ns - 100.0).abs() < 10.0 && (r.zc_jitter_ns - 30.0).abs() < 8.0,
            "{} {}",
            r.zc_offset_mean_ns,
            r.zc_jitter_ns
        );
        assert!(!r.pass);
        let r = measure(
            &capture(
                &sch(),
                &Faults {
                    flip: Some((2, 5)),
                    ..clean()
                },
            ),
            FS,
            &sch(),
            &TxLimits::default(),
        )
        .unwrap();
        assert_eq!(r.phase_code_errors, 1);
        let bad = r
            .pulses
            .iter()
            .find(|p| p.measured_code != p.expected_code)
            .unwrap();
        assert_eq!((bad.group, bad.k), (2, 5));
        let r = measure(
            &capture(
                &sch(),
                &Faults {
                    amp_var: 0.05,
                    ..clean()
                },
            ),
            FS,
            &sch(),
            &TxLimits::default(),
        )
        .unwrap();
        assert!(r.amplitude_std_pct > 3.0, "{}", r.amplitude_std_pct);
        // Noise: the zero-crossing jitter follows σ_noise / slope, slope = 2π f · A · env(30 µs).
        let sig = PulseSignal::loran_c();
        let slope = 2.0 * std::f64::consts::PI * 100e3 * 0.5 * sig.envelope.value(30e-6);
        for noise in [0.001, 0.01] {
            let r = measure(
                &capture(&sch(), &Faults { noise, ..clean() }),
                FS,
                &sch(),
                &TxLimits::default(),
            )
            .unwrap();
            let want = noise / slope * 1e9;
            assert_eq!(r.phase_code_errors, 0);
            assert!(
                r.zc_jitter_ns > 0.5 * want && r.zc_jitter_ns < 2.0 * want,
                "noise {noise}: jitter {} ns, predicted {want} ns",
                r.zc_jitter_ns
            );
        }
    }
}
