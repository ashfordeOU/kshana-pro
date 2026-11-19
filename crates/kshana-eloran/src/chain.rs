// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Chains and stations, and the emission schedule.
//!
//! Group epochs: the group of number `k` of a chain starts (nominally, on its time
//! reference) at `k · GRI` after the Loran epoch 1958-01-01T00:00:00, in the continuous
//! (TAI-like) Loran time scale. A secondary transmits its group `emission_delay` after the
//! master. Each station's emissions are moved to the truth timeline by its time reference
//! and transmitter clock ([`crate::timebase::Timebase::true_emission`]), and edited by a
//! data-channel modulator if the chain has one.

use crate::geo::Geo;
use crate::signal::{GroupModulator, Parity, PulseSignal, PulseSpec, Role};
use crate::time::{AbsTime, Ps, PS_PER_US};
use crate::timebase::Timebase;
use serde::{Deserialize, Serialize};

/// Operational status of a station.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StationStatus {
    #[default]
    On,
    Off,
}

/// A fault applied to a station over a scene-time window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StationFault {
    /// Station silent.
    Off { from_s: f64, to_s: f64 },
    /// Timing error (ns) added to the emission time.
    TimingStep {
        from_s: f64,
        to_s: f64,
        error_ns: f64,
    },
    /// Power change (dB).
    PowerStep {
        from_s: f64,
        to_s: f64,
        delta_db: f64,
    },
    /// "Blink": the station's first two pulses are omitted (the classic LORAN-C unusable
    /// warning) over the window.
    Blink { from_s: f64, to_s: f64 },
    /// Transmitted ECD change (µs).
    EcdStep { from_s: f64, to_s: f64, ecd_us: f64 },
}

/// A transmitting station.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Station {
    /// Short identifier (unique in the chain), e.g. "M", "X".
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub role: Role,
    pub position: Geo,
    /// Radiated peak power (kW).
    #[serde(default = "d_power")]
    pub power_kw: f64,
    /// Emission delay after the master (µs); 0 for the master.
    #[serde(default)]
    pub emission_delay_us: f64,
    /// Time reference the station is steered to (a [`crate::timebase::TimeRefModel`] id).
    #[serde(default)]
    pub time_ref: String,
    /// Optional transmitter clock error model id.
    #[serde(default)]
    pub tx_clock: Option<String>,
    /// Transmitted ECD (µs), nominally 0.
    #[serde(default)]
    pub ecd_us: f64,
    #[serde(default)]
    pub status: StationStatus,
    #[serde(default)]
    pub faults: Vec<StationFault>,
}

fn d_power() -> f64 {
    400.0
}

impl Station {
    fn active_faults(&self, t_s: f64) -> impl Iterator<Item = &StationFault> {
        self.faults.iter().filter(move |f| {
            let (a, b) = match f {
                StationFault::Off { from_s, to_s }
                | StationFault::TimingStep { from_s, to_s, .. }
                | StationFault::PowerStep { from_s, to_s, .. }
                | StationFault::Blink { from_s, to_s }
                | StationFault::EcdStep { from_s, to_s, .. } => (*from_s, *to_s),
            };
            t_s >= a && t_s < b
        })
    }
}

/// A chain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chain {
    /// Name, e.g. "9930 Korea".
    pub name: String,
    /// Group repetition interval (µs), 40 000 to 99 990 in steps of 10.
    pub gri_us: u32,
    #[serde(default = "PulseSignal::loran_c")]
    pub signal: PulseSignal,
    pub stations: Vec<Station>,
}

impl Chain {
    pub fn validate(&self) -> Result<(), String> {
        if !(40_000..=99_990).contains(&self.gri_us) || !self.gri_us.is_multiple_of(10) {
            return Err(format!(
                "chain {}: GRI {} µs must be 40 000 to 99 990 µs in steps of 10 µs",
                self.name, self.gri_us
            ));
        }
        self.signal.validate()?;
        if self.stations.is_empty() {
            return Err(format!("chain {}: no stations", self.name));
        }
        if self
            .stations
            .iter()
            .filter(|s| s.role == Role::Master)
            .count()
            > 1
        {
            return Err(format!("chain {}: more than one master", self.name));
        }
        let gri_s = self.gri_us as f64 * 1e-6;
        for s in &self.stations {
            let end = s.emission_delay_us * 1e-6 + self.signal.group_span_s(s.role);
            if s.emission_delay_us < 0.0 || end > gri_s {
                return Err(format!(
                    "chain {}: station {} group (delay {} µs) does not fit the GRI",
                    self.name, s.id, s.emission_delay_us
                ));
            }
        }
        Ok(())
    }

    pub fn gri(&self) -> Ps {
        Ps(self.gri_us as i64 * PS_PER_US)
    }
}

/// One transmitted pulse on the truth timeline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Emission {
    pub chain: usize,
    pub station: usize,
    /// Group number since the Loran epoch.
    pub group: i64,
    pub parity: Parity,
    /// True emission instant of the pulse's time origin (scene time).
    pub t_emit: Ps,
    /// Nominal emission instant (on the station's reference), scene time.
    pub t_nominal: Ps,
    pub pulse: PulseSpec,
    /// Amplitude scale from power faults (linear, field).
    pub amp_scale: f64,
    /// Transmitted ECD (s).
    pub ecd_s: f64,
}

/// Emissions of chain `ci` whose pulse time origins fall in `[t0, t1)` (scene time),
/// for a scene starting at absolute time `start`.
pub fn emissions(
    chain: &Chain,
    ci: usize,
    start: AbsTime,
    t0: Ps,
    t1: Ps,
    timebase: &Timebase,
    modulator: Option<&dyn GroupModulator>,
) -> Vec<Emission> {
    let gri = chain.gri().0 as i128;
    let mut out = Vec::new();
    // Groups whose window [group start, group start + max delay + span] can touch [t0, t1).
    let max_delay = chain
        .stations
        .iter()
        .map(|s| Ps::from_us(s.emission_delay_us + 10_000.0).0)
        .max()
        .unwrap_or(0) as i128;
    let abs0 = start.0 + t0.0 as i128 - max_delay - gri;
    let abs1 = start.0 + t1.0 as i128 + gri;
    let k0 = abs0.div_euclid(gri) as i64;
    let k1 = abs1.div_euclid(gri) as i64;
    for k in k0..=k1 {
        let parity = Parity::of_group(k);
        let group_abs = k as i128 * gri;
        for (si, st) in chain.stations.iter().enumerate() {
            if st.status == StationStatus::Off {
                continue;
            }
            let nominal_start =
                Ps((group_abs - start.0) as i64) + Ps::from_us(st.emission_delay_us);
            let ts = nominal_start.as_s();
            let mut off = false;
            let mut blink = false;
            let mut timing = Ps::ZERO;
            let mut amp_scale = 1.0;
            let mut ecd_us = st.ecd_us;
            for f in st.active_faults(ts) {
                match f {
                    StationFault::Off { .. } => off = true,
                    StationFault::Blink { .. } => blink = true,
                    StationFault::TimingStep { error_ns, .. } => timing += Ps::from_ns(*error_ns),
                    StationFault::PowerStep { delta_db, .. } => {
                        amp_scale *= 10f64.powf(delta_db / 20.0)
                    }
                    StationFault::EcdStep { ecd_us: e, .. } => ecd_us = *e,
                }
            }
            if off {
                continue;
            }
            let mut pulses = chain.signal.group(st.role, parity);
            if blink {
                pulses.retain(|p| p.k >= 2);
            }
            if let Some(m) = modulator {
                m.modulate(si, st.role, k, &mut pulses);
            }
            let start_true =
                timebase.true_emission(nominal_start, &st.time_ref, st.tx_clock.as_deref())
                    + timing;
            for p in pulses {
                let t_nom = nominal_start + p.offset;
                let t_emit = start_true + p.offset;
                if t_emit >= t0 && t_emit < t1 {
                    out.push(Emission {
                        chain: ci,
                        station: si,
                        group: k,
                        parity,
                        t_emit,
                        t_nominal: t_nom,
                        pulse: p,
                        amp_scale,
                        ecd_s: ecd_us * 1e-6,
                    });
                }
            }
        }
    }
    out.sort_by_key(|e| e.t_emit);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timebase::TimeRefModel;

    pub fn test_chain() -> Chain {
        let st = |id: &str, role, lat, lon, d| Station {
            id: id.into(),
            name: id.into(),
            role,
            position: Geo::new(lat, lon),
            power_kw: 400.0,
            emission_delay_us: d,
            time_ref: "UTC(A)".into(),
            tx_clock: None,
            ecd_us: 0.0,
            status: StationStatus::On,
            faults: vec![],
        };
        Chain {
            name: "test".into(),
            gri_us: 99_300,
            signal: PulseSignal::loran_c(),
            stations: vec![
                st("M", Role::Master, 36.0, 129.3, 0.0),
                st("W", Role::Secondary, 35.0, 126.5, 11_000.0),
                st("X", Role::Secondary, 38.0, 128.5, 40_000.0),
            ],
        }
    }

    #[test]
    fn gri_validation() {
        let mut c = test_chain();
        c.validate().unwrap();
        c.gri_us = 39_990;
        assert!(c.validate().is_err());
        c.gri_us = 99_995;
        assert!(c.validate().is_err());
    }

    #[test]
    fn emissions_follow_gri_and_delays() {
        let c = test_chain();
        let tb = Timebase::new(vec![TimeRefModel::ideal("UTC(A)")], 1.0);
        let start = AbsTime(0);
        let e = emissions(&c, 0, start, Ps::ZERO, Ps::from_s(0.5), &tb, None);
        // groups in 0.5 s at 99.3 ms: 6 (0,..,5); master 9 + 8 + 8 pulses each
        let masters: Vec<_> = e
            .iter()
            .filter(|x| x.station == 0 && x.pulse.k == 0)
            .collect();
        assert_eq!(masters.len(), 6);
        assert_eq!(masters[1].t_emit - masters[0].t_emit, c.gri());
        let w0 = e.iter().find(|x| x.station == 1 && x.pulse.k == 0).unwrap();
        assert_eq!(w0.t_emit, Ps::from_us(11_000.0));
        // parity alternates
        assert_ne!(masters[0].parity, masters[1].parity);
    }

    #[test]
    fn faults_apply() {
        let mut c = test_chain();
        c.stations[1].faults.push(StationFault::Off {
            from_s: 0.0,
            to_s: 0.2,
        });
        c.stations[2].faults.push(StationFault::TimingStep {
            from_s: 0.0,
            to_s: 10.0,
            error_ns: 50.0,
        });
        let tb = Timebase::new(vec![], 1.0);
        let e = emissions(&c, 0, AbsTime(0), Ps::ZERO, Ps::from_s(0.3), &tb, None);
        let w: Vec<_> = e
            .iter()
            .filter(|x| x.station == 1 && x.pulse.k == 0)
            .collect();
        assert!(w.iter().all(|x| x.t_emit.as_s() >= 0.2));
        let x0 = e.iter().find(|x| x.station == 2 && x.pulse.k == 0).unwrap();
        assert_eq!(x0.t_emit, Ps::from_us(40_000.0) + Ps::from_ns(50.0));
    }
}
