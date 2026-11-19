// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Symbol mappers: one channel symbol per group ↔ per-pulse time shifts and phase offsets.
//!
//! Pulses are addressed by **LDC pulse index**: 0..=7 are the eight phase-coded pulses of
//! the group (pulse "1" to "8" in 1-based literature numbering), [`DATA_PULSE`] (= 8) is the
//! inserted data pulse (the "9th pulse" of the 9th-pulse LDC). The master's own ninth
//! (identification) pulse is never modulated.
//!
//! * [`Mapper::Ppm`]: N-ary pulse-position modulation, independently on each selected pulse,
//!   positions centred on nominal: shift = (p − (N−1)/2)·step, Gray-coded digits.
//! * [`Mapper::TriState`]: each selected pulse is advanced, nominal or retarded by `step`;
//!   optionally restricted to *balanced* patterns (shifts sum to zero, Eurofix). The
//!   all-zero pattern is reserved (idle); the first 2^b remaining patterns in lexicographic
//!   order (−1 < 0 < +1, first selected pulse most significant) carry b bits.
//! * [`Mapper::Phase`]: N-ary phase modulation of the carrier of each selected pulse,
//!   φ = 2π p / N, Gray-coded.

use crate::signal::PulseSpec;
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

/// LDC pulse index of the inserted data pulse.
pub const DATA_PULSE: usize = 8;
/// Number of LDC pulse indices (8 group pulses + data pulse).
pub const LDC_PULSES: usize = 9;

/// Mapper configuration (scenario side).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MapperSpec {
    Ppm {
        pulses: Vec<usize>,
        states: u32,
        step_us: f64,
    },
    TriState {
        pulses: Vec<usize>,
        step_us: f64,
        #[serde(default)]
        balanced: bool,
    },
    Phase {
        pulses: Vec<usize>,
        states: u32,
    },
}

/// A ready mapper.
#[derive(Clone, Debug, PartialEq)]
pub enum Mapper {
    Ppm {
        pulses: Vec<usize>,
        states: u32,
        step_us: f64,
    },
    TriState {
        pulses: Vec<usize>,
        step_us: f64,
        balanced: bool,
        patterns: Vec<Vec<i8>>,
    },
    Phase {
        pulses: Vec<usize>,
        states: u32,
    },
}

/// Gray code of `p`.
pub fn gray(p: u32) -> u32 {
    p ^ (p >> 1)
}
/// Inverse Gray code.
pub fn gray_inv(v: u32) -> u32 {
    let mut p = v;
    let mut s = v >> 1;
    while s != 0 {
        p ^= s;
        s >>= 1;
    }
    p
}

/// All tri-state patterns of `l` pulses in lexicographic order (−1 < 0 < +1).
pub fn tri_state_patterns(l: usize, balanced: bool) -> Vec<Vec<i8>> {
    let total = 3usize.pow(l as u32);
    (0..total)
        .map(|mut v| {
            let mut p = vec![0i8; l];
            for i in (0..l).rev() {
                p[i] = (v % 3) as i8 - 1;
                v /= 3;
            }
            p
        })
        .filter(|p| !balanced || p.iter().map(|x| *x as i32).sum::<i32>() == 0)
        .collect()
}

fn log2_exact(n: u32) -> Option<u32> {
    (n >= 2 && n.is_power_of_two()).then(|| n.trailing_zeros())
}

impl Mapper {
    pub fn from_spec(s: &MapperSpec) -> Result<Mapper, String> {
        let chk = |pulses: &Vec<usize>| -> Result<(), String> {
            if pulses.is_empty() {
                return Err("mapper: select at least one pulse".into());
            }
            if pulses.iter().any(|p| *p >= LDC_PULSES) {
                return Err(format!(
                    "mapper: pulse indices are 0..=7 (group pulses) or {DATA_PULSE} (data pulse)"
                ));
            }
            let mut u = pulses.clone();
            u.sort_unstable();
            u.dedup();
            if u.len() != pulses.len() {
                return Err("mapper: duplicate pulse index".into());
            }
            Ok(())
        };
        Ok(match s {
            MapperSpec::Ppm {
                pulses,
                states,
                step_us,
            } => {
                chk(pulses)?;
                let b = log2_exact(*states).ok_or("ppm: states must be a power of two ≥ 2")?;
                if b * pulses.len() as u32 > 16 {
                    return Err("ppm: at most 16 bits per group".into());
                }
                if step_us.is_nan() || *step_us <= 0.0 {
                    return Err("ppm: step_us must be positive".into());
                }
                Mapper::Ppm {
                    pulses: pulses.clone(),
                    states: *states,
                    step_us: *step_us,
                }
            }
            MapperSpec::TriState {
                pulses,
                step_us,
                balanced,
            } => {
                chk(pulses)?;
                if pulses.len() > 10 {
                    return Err("tri_state: at most 10 pulses".into());
                }
                if step_us.is_nan() || *step_us <= 0.0 {
                    return Err("tri_state: step_us must be positive".into());
                }
                let mut pats = tri_state_patterns(pulses.len(), *balanced);
                pats.retain(|p| p.iter().any(|x| *x != 0));
                if pats.len() < 2 {
                    return Err("tri_state: too few patterns for one bit".into());
                }
                let b = usize::BITS - 1 - pats.len().leading_zeros();
                pats.truncate(1 << b);
                Mapper::TriState {
                    pulses: pulses.clone(),
                    step_us: *step_us,
                    balanced: *balanced,
                    patterns: pats,
                }
            }
            MapperSpec::Phase { pulses, states } => {
                chk(pulses)?;
                let b = log2_exact(*states).ok_or("phase: states must be a power of two ≥ 2")?;
                if b * pulses.len() as u32 > 16 {
                    return Err("phase: at most 16 bits per group".into());
                }
                Mapper::Phase {
                    pulses: pulses.clone(),
                    states: *states,
                }
            }
        })
    }

    /// Bits carried per group.
    pub fn bits_per_symbol(&self) -> u32 {
        match self {
            Mapper::Ppm { pulses, states, .. } | Mapper::Phase { pulses, states } => {
                states.trailing_zeros() * pulses.len() as u32
            }
            Mapper::TriState { patterns, .. } => patterns.len().trailing_zeros(),
        }
    }

    /// Number of distinct channel symbols, 2^bits.
    pub fn alphabet(&self) -> u32 {
        1 << self.bits_per_symbol()
    }

    /// The LDC pulse indices the mapper modulates.
    pub fn pulses(&self) -> &[usize] {
        match self {
            Mapper::Ppm { pulses, .. }
            | Mapper::TriState { pulses, .. }
            | Mapper::Phase { pulses, .. } => pulses,
        }
    }

    pub fn uses_data_pulse(&self) -> bool {
        self.pulses().contains(&DATA_PULSE)
    }

    /// Whether symbols leave the mean timing of the modulated pulses unchanged.
    pub fn is_balanced(&self) -> bool {
        matches!(self, Mapper::TriState { balanced: true, .. })
    }

    fn digits(&self, sym: u32, states: u32, n: usize) -> Vec<u32> {
        let b = states.trailing_zeros();
        (0..n)
            .map(|j| (sym >> (b * (n - 1 - j) as u32)) & (states - 1))
            .collect()
    }

    /// Per-pulse modifications of symbol `sym`: (LDC index, time shift µs, phase rad).
    pub fn shifts(&self, sym: u32) -> Vec<(usize, f64, f64)> {
        let sym = sym & (self.alphabet() - 1);
        match self {
            Mapper::Ppm {
                pulses,
                states,
                step_us,
            } => {
                let c = (*states as f64 - 1.0) / 2.0;
                self.digits(sym, *states, pulses.len())
                    .into_iter()
                    .zip(pulses)
                    .map(|(d, i)| (*i, (gray_inv(d) as f64 - c) * step_us, 0.0))
                    .collect()
            }
            Mapper::TriState {
                pulses,
                step_us,
                patterns,
                ..
            } => patterns[sym as usize]
                .iter()
                .zip(pulses)
                .map(|(s, i)| (*i, *s as f64 * step_us, 0.0))
                .collect(),
            Mapper::Phase { pulses, states } => self
                .digits(sym, *states, pulses.len())
                .into_iter()
                .zip(pulses)
                .map(|(d, i)| (*i, 0.0, TAU * gray_inv(d) as f64 / *states as f64))
                .collect(),
        }
    }

    /// Ideal per-LDC-index offsets (ns) and phases (rad) of `sym` (unmodulated indices 0).
    pub fn ideal(&self, sym: u32) -> ([f64; LDC_PULSES], [f64; LDC_PULSES]) {
        let mut t = [0.0; LDC_PULSES];
        let mut p = [0.0; LDC_PULSES];
        for (i, dt, dp) in self.shifts(sym) {
            t[i] = dt * 1e3;
            p[i] = dp;
        }
        (t, p)
    }

    /// Apply symbol `sym` to `pulses`; `data_pulse` is the position of the inserted data
    /// pulse in `pulses`, if any. Group pulses are found by [`PulseSpec::k`].
    pub fn apply(&self, sym: u32, pulses: &mut [PulseSpec], data_pulse: Option<usize>) {
        for (i, dt, dp) in self.shifts(sym) {
            let pos = if i == DATA_PULSE {
                data_pulse
            } else {
                pulses.iter().position(|p| p.k == i)
            };
            if let Some(j) = pos {
                pulses[j].offset += crate::time::Ps::from_us(dt);
                pulses[j].phase_rad += dp;
            }
        }
    }

    /// Decide the symbol from per-LDC-index timing offsets (ns, reference already removed,
    /// NaN where not measured) and phase offsets (rad, phase code removed).
    pub fn decide(&self, offsets_ns: &[f64], phases_rad: Option<&[f64]>) -> u32 {
        let at = |v: &[f64], i: usize| v.get(i).copied().unwrap_or(f64::NAN);
        match self {
            Mapper::Ppm {
                pulses,
                states,
                step_us,
            } => {
                let c = (*states as f64 - 1.0) / 2.0;
                let b = states.trailing_zeros();
                pulses.iter().fold(0u32, |acc, i| {
                    let x = at(offsets_ns, *i);
                    let p = if x.is_finite() {
                        (x / (step_us * 1e3) + c)
                            .round()
                            .clamp(0.0, *states as f64 - 1.0) as u32
                    } else {
                        c.round() as u32
                    };
                    (acc << b) | gray(p)
                })
            }
            Mapper::TriState {
                pulses,
                step_us,
                balanced,
                patterns,
            } => {
                let x: Vec<f64> = pulses.iter().map(|i| at(offsets_ns, *i)).collect();
                let fin: Vec<f64> = x.iter().copied().filter(|v| v.is_finite()).collect();
                let mean = if *balanced && !fin.is_empty() {
                    fin.iter().sum::<f64>() / fin.len() as f64
                } else {
                    0.0
                };
                let step = step_us * 1e3;
                let mut best = (f64::INFINITY, 0u32);
                for (s, p) in patterns.iter().enumerate() {
                    let cost: f64 = x
                        .iter()
                        .zip(p)
                        .filter(|(v, _)| v.is_finite())
                        .map(|(v, q)| {
                            let e = v - mean - step * *q as f64;
                            e * e
                        })
                        .sum();
                    if cost < best.0 {
                        best = (cost, s as u32);
                    }
                }
                best.1
            }
            Mapper::Phase { pulses, states } => {
                let b = states.trailing_zeros();
                let ph = phases_rad.unwrap_or(&[]);
                pulses.iter().fold(0u32, |acc, i| {
                    let x = at(ph, *i);
                    let p = if x.is_finite() {
                        ((x / (TAU / *states as f64)).round() as i64).rem_euclid(*states as i64)
                            as u32
                    } else {
                        0
                    };
                    (acc << b) | gray(p)
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balanced_eurofix_patterns() {
        // Σ_j 6!/(j! j! (6−2j)!) = 1 + 30 + 90 + 20 = 141 balanced patterns of six pulses.
        let all = tri_state_patterns(6, true);
        assert_eq!(all.len(), 141);
        assert_eq!(tri_state_patterns(6, false).len(), 729);
        let m = Mapper::from_spec(&MapperSpec::TriState {
            pulses: vec![2, 3, 4, 5, 6, 7],
            step_us: 1.0,
            balanced: true,
        })
        .unwrap();
        // 140 non-idle patterns → 7 bits per GRI, 128 patterns used, all distinct and balanced
        assert_eq!(m.bits_per_symbol(), 7);
        if let Mapper::TriState { patterns, .. } = &m {
            assert_eq!(patterns.len(), 128);
            let set: std::collections::BTreeSet<_> = patterns.iter().collect();
            assert_eq!(set.len(), 128);
            assert!(patterns
                .iter()
                .all(|p| p.iter().map(|x| *x as i32).sum::<i32>() == 0));
            assert!(patterns.iter().all(|p| p.iter().any(|x| *x != 0)));
            // first and last symbols of the lexicographic mapping
            assert_eq!(patterns[0], vec![-1, -1, -1, 1, 1, 1]);
            assert_eq!(patterns[127], vec![1, 0, 1, -1, -1, 0]);
        }
        for s in 0..128 {
            let (t, _) = m.ideal(s);
            assert_eq!(m.decide(&t, None), s);
            // a common offset is removed by the balanced decision
            let shifted: Vec<f64> = t.iter().map(|x| x + 333.0).collect();
            assert_eq!(m.decide(&shifted, None), s);
        }
    }

    #[test]
    fn ppm_and_phase_roundtrip_with_gray() {
        let m = Mapper::from_spec(&MapperSpec::Ppm {
            pulses: vec![DATA_PULSE],
            states: 32,
            step_us: 1.0,
        })
        .unwrap();
        assert_eq!(m.bits_per_symbol(), 5);
        for s in 0..32 {
            let (t, _) = m.ideal(s);
            assert!(t[DATA_PULSE].abs() <= 15_500.0);
            assert_eq!(m.decide(&t, None), s);
        }
        // adjacent positions differ in one bit
        for p in 0..31u32 {
            assert_eq!((gray(p) ^ gray(p + 1)).count_ones(), 1);
            assert_eq!(gray_inv(gray(p)), p);
        }
        let m2 = Mapper::from_spec(&MapperSpec::Ppm {
            pulses: vec![3, 5],
            states: 4,
            step_us: 0.5,
        })
        .unwrap();
        assert_eq!(m2.alphabet(), 16);
        for s in 0..16 {
            assert_eq!(m2.decide(&m2.ideal(s).0, None), s);
        }
        let ph = Mapper::from_spec(&MapperSpec::Phase {
            pulses: vec![6, 7],
            states: 4,
        })
        .unwrap();
        for s in 0..16 {
            let (_, p) = ph.ideal(s);
            assert_eq!(ph.decide(&[0.0; 9], Some(&p)), s);
        }
        let un = Mapper::from_spec(&MapperSpec::TriState {
            pulses: vec![4, 5, 6],
            step_us: 1.0,
            balanced: false,
        })
        .unwrap();
        assert_eq!(un.bits_per_symbol(), 4); // 26 non-idle patterns → 16 used
        assert!(Mapper::from_spec(&MapperSpec::Ppm {
            pulses: vec![9],
            states: 4,
            step_us: 1.0
        })
        .is_err());
        assert!(Mapper::from_spec(&MapperSpec::Ppm {
            pulses: vec![1],
            states: 3,
            step_us: 1.0
        })
        .is_err());
    }
}
