// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Signal definitions: the pulse, the pulse group and the phase codes.
//!
//! A signal is **data**, not code: [`PulseSignal`] carries the carrier, the envelope family,
//! the pulse offsets of master and secondary groups and the phase-code tables. The default,
//! [`PulseSignal::loran_c`], is the LORAN-C / eLoran signal of the USCG specification
//! (COMDTINST M16562.4A, "Specification of the Transmitted LORAN-C Signal"):
//!
//! ```text
//! i(t) = 0                                              t < τ
//! i(t) = A (t−τ)² exp(−2 (t−τ)/65 µs) sin(2π·100 kHz·t + PC)   τ ≤ t   (t, τ in µs)
//! ```
//!
//! where τ is the envelope-to-cycle difference (ECD) and PC the phase code (0 or π). The
//! time origin `t = 0` of a pulse is the instant the simulator calls the pulse's **emission
//! time**; the standard sampling point (the positive-going zero crossing used for time of
//! arrival) is at `t = 30 µs`. The specification bounds the tail after 65 µs; here the
//! analytic expression is kept and smoothly tapered to zero between [`Envelope`]'s
//! `taper_start_us` and `end_us` (defaults 200 and 300 µs, where the analytic envelope has
//! fallen to 15 % and 1.5 % of its peak).
//!
//! New and emerging signals are a new [`PulseSignal`] value (different envelope, carrier,
//! pulse counts, spacing or phase codes) plus, for data channels, a [`GroupModulator`].

use crate::time::Ps;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// Role of a station in its chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Master,
    Secondary,
}

/// Phase-code interval of a group (alternates every GRI).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Parity {
    A,
    B,
}

impl Parity {
    /// Parity of group number `k` (even groups are A).
    pub fn of_group(k: i64) -> Parity {
        if k.rem_euclid(2) == 0 {
            Parity::A
        } else {
            Parity::B
        }
    }
}

/// Envelope family of a pulse.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Envelope {
    /// `(t/τp)² exp(2(1 − t/τp))`, peak 1 at `t = τp` (τp = 65 µs for LORAN-C), tapered to
    /// zero between `taper_start_us` and `end_us` with a raised cosine.
    TSquaredExp {
        #[serde(default = "d65")]
        peak_us: f64,
        #[serde(default = "d200")]
        taper_start_us: f64,
        #[serde(default = "d300")]
        end_us: f64,
    },
    /// A tabulated envelope (normalised to peak 1), linearly interpolated, sample spacing
    /// `step_us`, starting at t = 0. For emerging signal shapes.
    Table { step_us: f64, values: Vec<f64> },
}

fn d65() -> f64 {
    65.0
}
fn d200() -> f64 {
    200.0
}
fn d300() -> f64 {
    300.0
}

impl Envelope {
    pub fn loran_c() -> Envelope {
        Envelope::TSquaredExp {
            peak_us: 65.0,
            taper_start_us: 200.0,
            end_us: 300.0,
        }
    }
    /// Envelope value at `t_s` seconds after the envelope origin.
    #[inline]
    pub fn value(&self, t_s: f64) -> f64 {
        if t_s <= 0.0 {
            return 0.0;
        }
        let t_us = t_s * 1e6;
        match self {
            Envelope::TSquaredExp {
                peak_us,
                taper_start_us,
                end_us,
            } => {
                if t_us >= *end_us {
                    return 0.0;
                }
                let x = t_us / peak_us;
                let v = x * x * (2.0 * (1.0 - x)).exp();
                if t_us > *taper_start_us {
                    let u = (t_us - taper_start_us) / (end_us - taper_start_us);
                    v * 0.5 * (1.0 + (PI * u).cos())
                } else {
                    v
                }
            }
            Envelope::Table { step_us, values } => {
                let p = t_us / step_us;
                let i = p.floor() as usize;
                if i + 1 >= values.len() {
                    return 0.0;
                }
                let f = p - i as f64;
                values[i] * (1.0 - f) + values[i + 1] * f
            }
        }
    }
    /// Time after which the envelope is zero (s).
    pub fn extent_s(&self) -> f64 {
        match self {
            Envelope::TSquaredExp { end_us, .. } => end_us * 1e-6,
            Envelope::Table { step_us, values } => step_us * values.len() as f64 * 1e-6,
        }
    }
}

/// One pulse of a group as transmitted: offset from the group's start, phase and
/// relative amplitude. Data-channel modulators edit these.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PulseSpec {
    /// Offset of the pulse's time origin from the station's group start.
    pub offset: Ps,
    /// Carrier phase (rad): the phase code, plus any data modulation in phase.
    pub phase_rad: f64,
    /// Relative amplitude (1 nominal).
    pub amp: f64,
    /// Index of the pulse within the group (0-based; data pulses continue the count).
    pub k: usize,
}

/// A pulsed signal definition (data-driven).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PulseSignal {
    /// Name of the definition (e.g. "loran-c").
    pub name: String,
    /// Carrier frequency (Hz).
    #[serde(default = "d100k")]
    pub carrier_hz: f64,
    pub envelope: Envelope,
    /// Pulse time-origin offsets of a master group (µs from group start).
    pub master_offsets_us: Vec<f64>,
    /// Pulse offsets of a secondary group (µs).
    pub secondary_offsets_us: Vec<f64>,
    /// Phase codes, `+1`/`-1` per pulse: master A, master B, secondary A, secondary B.
    pub master_codes_a: Vec<i8>,
    pub master_codes_b: Vec<i8>,
    pub secondary_codes_a: Vec<i8>,
    pub secondary_codes_b: Vec<i8>,
}

fn d100k() -> f64 {
    100_000.0
}

impl PulseSignal {
    /// LORAN-C / eLoran per COMDTINST M16562.4A: 8 pulses 1 ms apart, master 9th pulse
    /// 2 ms after the 8th; LORAN-C phase codes.
    pub fn loran_c() -> PulseSignal {
        let eight: Vec<f64> = (0..8).map(|k| k as f64 * 1000.0).collect();
        let mut nine = eight.clone();
        nine.push(9000.0);
        PulseSignal {
            name: "loran-c".into(),
            carrier_hz: 100_000.0,
            envelope: Envelope::loran_c(),
            master_offsets_us: nine,
            secondary_offsets_us: eight,
            master_codes_a: vec![1, 1, -1, -1, 1, -1, 1, -1, 1],
            master_codes_b: vec![1, -1, -1, 1, 1, 1, 1, 1, -1],
            secondary_codes_a: vec![1, 1, 1, 1, 1, -1, -1, 1],
            secondary_codes_b: vec![1, -1, 1, -1, 1, 1, -1, -1],
        }
    }

    /// Check the tables are consistent.
    pub fn validate(&self) -> Result<(), String> {
        let chk = |o: &Vec<f64>, a: &Vec<i8>, b: &Vec<i8>, who: &str| {
            if a.len() != o.len() || b.len() != o.len() {
                return Err(format!(
                    "{}: {who} phase codes must have one entry per pulse",
                    self.name
                ));
            }
            if a.iter().chain(b).any(|c| *c != 1 && *c != -1) {
                return Err(format!("{}: phase codes are +1 or -1", self.name));
            }
            if o.windows(2).any(|w| w[1] <= w[0]) {
                return Err(format!("{}: {who} pulse offsets must increase", self.name));
            }
            Ok(())
        };
        chk(
            &self.master_offsets_us,
            &self.master_codes_a,
            &self.master_codes_b,
            "master",
        )?;
        chk(
            &self.secondary_offsets_us,
            &self.secondary_codes_a,
            &self.secondary_codes_b,
            "secondary",
        )?;
        if !(self.carrier_hz > 0.0) {
            return Err("carrier must be positive".into());
        }
        Ok(())
    }

    /// The nominal pulses of one group.
    pub fn group(&self, role: Role, parity: Parity) -> Vec<PulseSpec> {
        let (offs, codes) = match (role, parity) {
            (Role::Master, Parity::A) => (&self.master_offsets_us, &self.master_codes_a),
            (Role::Master, Parity::B) => (&self.master_offsets_us, &self.master_codes_b),
            (Role::Secondary, Parity::A) => (&self.secondary_offsets_us, &self.secondary_codes_a),
            (Role::Secondary, Parity::B) => (&self.secondary_offsets_us, &self.secondary_codes_b),
        };
        offs.iter()
            .zip(codes)
            .enumerate()
            .map(|(k, (o, c))| PulseSpec {
                offset: Ps::from_us(*o),
                phase_rad: if *c < 0 { PI } else { 0.0 },
                amp: 1.0,
                k,
            })
            .collect()
    }

    /// Length of a group from its first pulse origin to the end of its last pulse (s).
    pub fn group_span_s(&self, role: Role) -> f64 {
        let offs = match role {
            Role::Master => &self.master_offsets_us,
            Role::Secondary => &self.secondary_offsets_us,
        };
        offs.last().copied().unwrap_or(0.0) * 1e-6 + self.envelope.extent_s()
    }

    /// The transmitted waveform of one pulse at `t_s` after its time origin, for an ECD of
    /// `ecd_s` and carrier phase `phase_rad`: `env(t − ECD) · sin(2π f t + phase)`.
    #[inline]
    pub fn pulse(&self, t_s: f64, ecd_s: f64, phase_rad: f64) -> f64 {
        let e = self.envelope.value(t_s - ecd_s);
        if e == 0.0 {
            return 0.0;
        }
        e * (2.0 * PI * self.carrier_hz * t_s + phase_rad).sin()
    }
}

/// Data-channel hook: edits the pulses of a station's group (shift, add or re-phase
/// pulses) for the group number `group`. Implemented by the LDC schemes in [`crate::ldc`].
pub trait GroupModulator: Send + Sync {
    /// Short name of the scheme.
    fn name(&self) -> &str;
    /// Modulate the group of station `station` (index within its chain).
    fn modulate(&self, station: usize, role: Role, group: i64, pulses: &mut Vec<PulseSpec>);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_peaks_at_65us_with_value_one() {
        let e = Envelope::loran_c();
        assert!((e.value(65e-6) - 1.0).abs() < 1e-12);
        assert!(e.value(64e-6) < 1.0 && e.value(66e-6) < 1.0);
        assert_eq!(e.value(-1e-6), 0.0);
        assert_eq!(e.value(300e-6), 0.0);
    }

    #[test]
    fn standard_zero_crossing_at_30us_is_positive_going() {
        let s = PulseSignal::loran_c();
        let a = s.pulse(30e-6 - 0.1e-6, 0.0, 0.0);
        let b = s.pulse(30e-6 + 0.1e-6, 0.0, 0.0);
        assert!(a < 0.0 && b > 0.0, "{a} {b}");
        assert!(s.pulse(30e-6, 0.0, 0.0).abs() < 1e-12);
        // with phase code π the crossing is negative-going
        assert!(s.pulse(30.1e-6, 0.0, PI) < 0.0);
    }

    #[test]
    fn ecd_shifts_envelope_not_carrier() {
        let s = PulseSignal::loran_c();
        // zero crossings stay at multiples of 5 µs whatever the ECD
        for ecd in [-2.5e-6, 0.0, 1.0e-6, 2.5e-6] {
            assert!(s.pulse(30e-6, ecd, 0.0).abs() < 1e-12);
        }
        // envelope peak moves with ECD
        let e0 = s.envelope.value(65e-6);
        let e1 = s.envelope.value(65e-6 - 1e-6);
        assert!(e0 > e1);
    }

    #[test]
    fn loran_c_tables_valid_and_balanced() {
        let s = PulseSignal::loran_c();
        s.validate().unwrap();
        // the 9th master pulse sits 2 ms after the 8th
        let g = s.group(Role::Master, Parity::A);
        assert_eq!(g.len(), 9);
        assert_eq!(g[8].offset.as_us() - g[7].offset.as_us(), 2000.0);
        assert_eq!(s.group(Role::Secondary, Parity::B).len(), 8);
    }
}
