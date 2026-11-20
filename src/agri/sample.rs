// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! SYNTHETIC machine logs and fields, for examples and tests. Nothing here is a recording.

use super::geo::{unit, LocalFrame};
use super::lines::GuidanceLine;
use super::log::{Epoch, Fix, MachineLog};

/// Kind of a scripted outage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutageKind {
    /// Satellites and C/N0 collapse, position lost, steering drops out.
    Interference,
    /// Corrections stop: the fix falls to autonomous, correction age grows, the signal holds.
    Correction,
    /// The fix toggles between RTK fixed and float with the signal and corrections normal.
    Ionosphere,
}

impl OutageKind {
    pub fn parse(s: &str) -> Result<Self, String> {
        Ok(match s {
            "interference" => OutageKind::Interference,
            "correction" => OutageKind::Correction,
            "ionosphere" => OutageKind::Ionosphere,
            o => {
                return Err(format!(
                    "unknown outage kind '{o}' (interference, correction, ionosphere)"
                ))
            }
        })
    }
}

/// What to synthesise.
#[derive(Clone, Debug)]
pub struct Spec {
    pub origin: (f64, f64),
    pub heading_deg: f64,
    pub width_m: f64,
    pub length_m: f64,
    pub passes: usize,
    pub speed_mps: f64,
    pub rate_hz: f64,
    pub t0: f64,
    /// Turn time between passes (s), steering off.
    pub turn_s: f64,
    /// Within-pass drift: (pass index from 0, m per minute).
    pub drift: Option<(usize, f64)>,
    /// Outages: (start seconds from t0, duration s, kind).
    pub outages: Vec<(f64, f64, OutageKind)>,
    /// Cross-track noise (m, uniform +/-).
    pub noise_m: f64,
}

impl Spec {
    pub fn new(origin: (f64, f64)) -> Self {
        Spec {
            origin,
            heading_deg: 0.0,
            width_m: 12.0,
            length_m: 300.0,
            passes: 12,
            speed_mps: 3.0,
            rate_hz: 1.0,
            t0: 1_715_356_800.0, // 2024-05-10T16:00:00Z
            turn_s: 20.0,
            drift: None,
            outages: Vec::new(),
            noise_m: 0.01,
        }
    }

    /// The A-B line of pass 0.
    pub fn line(&self) -> GuidanceLine {
        GuidanceLine::from_heading("AB", self.origin, self.heading_deg, self.length_m)
    }
}

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
    }
}

/// Apply the scripted outages to an epoch at `t` (its cross-track deviation in `dev`).
fn apply(s: &Spec, e: &mut Epoch, t: f64, dev: &mut f64) {
    for (st, dur, kind) in &s.outages {
        let rel = t - s.t0 - st;
        if rel < 0.0 || rel >= *dur {
            continue;
        }
        let engaged = e.steer == Some(true);
        match kind {
            OutageKind::Interference => {
                e.fix = if rel < dur - 5.0 {
                    Fix::None
                } else {
                    Fix::RtkFloat
                };
                e.sats = Some(3);
                e.cn0 = Some(31.0);
                e.steer = Some(false);
                e.xte = None;
            }
            OutageKind::Correction => {
                e.fix = Fix::Autonomous;
                e.corr_age = Some(1.0 + rel);
                *dev += 0.002 * rel;
                if engaged {
                    e.xte = Some(*dev);
                }
            }
            OutageKind::Ionosphere => {
                e.fix = if (rel as i64 / 4) % 2 == 0 {
                    Fix::RtkFloat
                } else {
                    Fix::RtkFixed
                };
                *dev += 0.04 * (rel / dur);
                if engaged {
                    e.xte = Some(*dev);
                }
            }
        }
    }
}

/// Generate the log.
pub fn log(s: &Spec) -> MachineLog {
    let f = LocalFrame::new(s.origin.0, s.origin.1, 0.0);
    let u = unit(s.heading_deg);
    let n = [-u[1], u[0]]; // left of the line
    let dt = 1.0 / s.rate_hz;
    let mut rng = Lcg(0x5EED);
    let mut ep = Vec::new();
    let mut t = s.t0;
    let pass_t = s.length_m / s.speed_mps;
    for k in 0..s.passes {
        let fwd = k.is_multiple_of(2);
        let steps = (pass_t / dt).round() as usize;
        let tp0 = t;
        for i in 0..=steps {
            let along = if fwd {
                i as f64 * dt * s.speed_mps
            } else {
                s.length_m - i as f64 * dt * s.speed_mps
            };
            let mut dev = s.noise_m * rng.next();
            if let Some((dk, rate)) = s.drift {
                if dk == k {
                    dev += rate * (t - tp0) / 60.0;
                }
            }
            let mut e = Epoch {
                t_s: t,
                lat: 0.0,
                lon: 0.0,
                h: Some(230.0),
                fix: Fix::RtkFixed,
                sats: Some(18),
                hdop: Some(0.7),
                cn0: Some(45.0 + rng.next()),
                corr_age: Some(1.0),
                steer: Some(true),
                xte: Some(dev),
            };
            apply(s, &mut e, t, &mut dev);
            let x = k as f64 * s.width_m + dev;
            let (la, lo) = f.latlon(along * u[0] + x * n[0], along * u[1] + x * n[1]);
            e.lat = la;
            e.lon = lo;
            ep.push(e);
            t += dt;
        }
        // Turn: steering off, the machine at the headland beyond the line end.
        let turn_steps = (s.turn_s / dt).round() as usize;
        for i in 0..turn_steps {
            let along = if fwd { s.length_m + 5.0 } else { -5.0 };
            let x = (k as f64 + i as f64 / turn_steps.max(1) as f64) * s.width_m;
            let (la, lo) = f.latlon(along * u[0] + x * n[0], along * u[1] + x * n[1]);
            let mut e = Epoch {
                t_s: t,
                lat: la,
                lon: lo,
                h: Some(230.0),
                fix: Fix::RtkFixed,
                sats: Some(18),
                hdop: Some(0.7),
                cn0: Some(45.0),
                corr_age: Some(1.0),
                steer: Some(false),
                xte: None,
            };
            apply(s, &mut e, t, &mut 0.0);
            ep.push(e);
            t += dt;
        }
    }
    MachineLog {
        format: "csv".into(),
        epochs: ep,
        skipped: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic() {
        let s = Spec::new((49.9, -97.1));
        assert_eq!(log(&s).to_csv(), log(&s).to_csv());
        assert!(log(&s).epochs.len() > 12 * 100);
    }
}
