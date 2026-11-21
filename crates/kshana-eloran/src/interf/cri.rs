// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Cross-rate interference (CRI) statistics: how often the pulses of a victim chain
//! overlap pulses of other chains with different GRIs.
//!
//! Every chain's group `k` starts at `k·GRI` after the Loran epoch, so the offset of a
//! victim group relative to an interferer's groups is `k·G_v mod G_i`. Over a full cycle
//! of `G_i / g` victim groups (`g = gcd(G_v, G_i)`), that offset visits every multiple of
//! `g` in `[0, G_i)` exactly once. The **lattice** expectation
//! [`overlap_fraction_lattice`] counts, for each victim pulse, the fraction of those
//! offsets at which some interferer pulse lies closer than the overlap window `w` (pulse
//! origins within `w` of each other overlap; `w` = pulse length, 300 µs for LORAN-C).
//! It is exact over a full cycle and exact for harmonically related GRIs; for typical
//! GRI pairs (`g` = 10 µs ≪ w) it equals the **uniform** approximation
//! [`overlap_fraction_uniform`], `p = N_i · 2w / G_i` (`N_i` pulses per interferer GRI,
//! valid while `2w` < pulse spacing). Several interferers combine as independent:
//! `1 − Π(1 − p_b)` per victim pulse. Propagation delays shift each chain's pulses by a
//! constant and are ignored (give receiver-relative offsets in [`CriChain`] to include them).
//!
//! [`overlap_fraction_counted`] counts overlaps directly from pulse times, e.g. from
//! [`crate::chain::emissions`].

use crate::chain::{Chain, StationStatus};
use crate::signal::Role;
use crate::time::Ps;

/// A chain reduced to what matters for CRI: its GRI and the pulse offsets within a group.
#[derive(Clone, Debug, PartialEq)]
pub struct CriChain {
    pub gri_us: u32,
    /// Pulse time-origin offsets from the group start (µs), all stations.
    pub pulse_offsets_us: Vec<f64>,
}

impl CriChain {
    pub fn from_chain(c: &Chain) -> CriChain {
        let mut v = vec![];
        for s in c.stations.iter().filter(|s| s.status == StationStatus::On) {
            let offs = match s.role {
                Role::Master => &c.signal.master_offsets_us,
                Role::Secondary => &c.signal.secondary_offsets_us,
            };
            v.extend(offs.iter().map(|o| o + s.emission_delay_us));
        }
        v.sort_by(f64::total_cmp);
        CriChain {
            gri_us: c.gri_us,
            pulse_offsets_us: v,
        }
    }
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// Fraction of offsets `r·g` (r = 0..G_i/g) at which victim pulse offset `o` lies within
/// `w` of an interferer pulse.
fn pulse_overlap_lattice(o: f64, gv: u32, it: &CriChain, w: f64) -> f64 {
    let gi = it.gri_us as f64;
    let g = gcd(gv as u64, it.gri_us as u64) as f64;
    let k = (it.gri_us as f64 / g).round() as usize;
    let mut p: Vec<f64> = it
        .pulse_offsets_us
        .iter()
        .map(|x| x.rem_euclid(gi))
        .collect();
    p.sort_by(f64::total_cmp);
    let mut hit = 0usize;
    for r in 0..k {
        let x = (r as f64 * g + o).rem_euclid(gi);
        // circular distance to the nearest interferer pulse
        let j = p.partition_point(|v| *v < x);
        let cand = [p[(j + p.len() - 1) % p.len()], p[j % p.len()]];
        let d = cand
            .iter()
            .map(|c| {
                let d = (x - c).abs();
                d.min(gi - d)
            })
            .fold(f64::INFINITY, f64::min);
        if d < w {
            hit += 1;
        }
    }
    hit as f64 / k as f64
}

/// Lattice (GRI-ratio) expectation of the fraction of victim pulses overlapped by at
/// least one interferer pulse.
pub fn overlap_fraction_lattice(
    victim: &CriChain,
    interferers: &[CriChain],
    window_us: f64,
) -> f64 {
    if victim.pulse_offsets_us.is_empty() {
        return 0.0;
    }
    let mut acc = 0.0;
    for o in &victim.pulse_offsets_us {
        let mut clear = 1.0;
        for it in interferers
            .iter()
            .filter(|c| !c.pulse_offsets_us.is_empty())
        {
            clear *= 1.0 - pulse_overlap_lattice(*o, victim.gri_us, it, window_us);
        }
        acc += 1.0 - clear;
    }
    acc / victim.pulse_offsets_us.len() as f64
}

/// Uniform-phase approximation: `1 − Π_b (1 − N_b · 2w / G_b)`.
pub fn overlap_fraction_uniform(interferers: &[CriChain], window_us: f64) -> f64 {
    1.0 - interferers
        .iter()
        .map(|c| {
            1.0 - (c.pulse_offsets_us.len() as f64 * 2.0 * window_us / c.gri_us as f64).min(1.0)
        })
        .product::<f64>()
}

/// Counted fraction of `victim` pulse times within `window` of any `interferer` time
/// (`interferer` sorted ascending).
pub fn overlap_fraction_counted(victim: &[Ps], interferer: &[Ps], window: Ps) -> f64 {
    if victim.is_empty() {
        return 0.0;
    }
    let mut hit = 0usize;
    for t in victim {
        let j = interferer.partition_point(|x| x < t);
        let near = [j.checked_sub(1), Some(j)]
            .iter()
            .flatten()
            .filter_map(|i| interferer.get(*i))
            .map(|x| (x.0 - t.0).abs())
            .min();
        if near.is_some_and(|d| d < window.0) {
            hit += 1;
        }
    }
    hit as f64 / victim.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::{emissions, Station};
    use crate::geo::Geo;
    use crate::signal::PulseSignal;
    use crate::time::AbsTime;
    use crate::timebase::Timebase;

    fn chain(name: &str, gri: u32, delays: &[(Role, f64)]) -> Chain {
        Chain {
            name: name.into(),
            gri_us: gri,
            signal: PulseSignal::loran_c(),
            stations: delays
                .iter()
                .enumerate()
                .map(|(i, (r, d))| Station {
                    id: format!("S{i}"),
                    name: String::new(),
                    role: *r,
                    position: Geo::new(50.0, i as f64),
                    power_kw: 400.0,
                    emission_delay_us: *d,
                    time_ref: String::new(),
                    tx_clock: None,
                    ecd_us: 0.0,
                    status: StationStatus::On,
                    faults: vec![],
                })
                .collect(),
        }
    }

    fn times(c: &Chain, ci: usize, start: AbsTime, dur_s: f64) -> Vec<Ps> {
        let tb = Timebase::new(vec![], dur_s);
        let mut v: Vec<Ps> = emissions(c, ci, start, Ps::ZERO, Ps::from_s(dur_s), &tb, None)
            .iter()
            .map(|e| e.t_emit)
            .collect();
        v.sort();
        v
    }

    #[test]
    fn analytic_matches_counted_for_several_chains() {
        use Role::*;
        let a = chain(
            "A",
            67_310,
            &[(Master, 0.0), (Secondary, 13_000.0), (Secondary, 29_000.0)],
        );
        let b = chain("B", 74_990, &[(Master, 0.0), (Secondary, 15_000.0)]);
        let c = chain(
            "C",
            99_300,
            &[(Master, 0.0), (Secondary, 11_000.0), (Secondary, 40_000.0)],
        );
        let start = AbsTime::from_utc_iso("2026-11-18T00:00:00Z").unwrap();
        let dur = 300.0;
        let ta = times(&a, 0, start, dur);
        let mut ti = times(&b, 1, start, dur);
        ti.extend(times(&c, 2, start, dur));
        ti.sort();
        let w = 300.0;
        let counted = overlap_fraction_counted(&ta, &ti, Ps::from_us(w));
        let ca = CriChain::from_chain(&a);
        let others = [CriChain::from_chain(&b), CriChain::from_chain(&c)];
        let lattice = overlap_fraction_lattice(&ca, &others, w);
        let uniform = overlap_fraction_uniform(&others, w);
        assert!(
            (counted - lattice).abs() < 0.02,
            "counted {counted:.4} lattice {lattice:.4}"
        );
        assert!(
            (lattice - uniform).abs() < 0.01,
            "lattice {lattice:.4} uniform {uniform:.4}"
        );
        // sanity: 17 pulses/74.99 ms and 25 pulses/99.3 ms, 600 µs window
        let p = 1.0 - (1.0 - 17.0 * 600.0 / 74_990.0) * (1.0 - 25.0 * 600.0 / 99_300.0);
        assert!((uniform - p).abs() < 1e-12 && (0.25..0.30).contains(&p));
    }

    #[test]
    fn harmonic_gris_are_deterministic() {
        use Role::*;
        // 99 300 = 2 × 49 650: the relative offset never changes
        let v = chain("V", 99_300, &[(Secondary, 20_000.0)]);
        let i = chain("I", 49_650, &[(Master, 0.0)]);
        let start = AbsTime::from_utc_iso("2026-11-18T00:00:00Z").unwrap();
        let counted = overlap_fraction_counted(
            &times(&v, 0, start, 30.0),
            &times(&i, 1, start, 30.0),
            Ps::from_us(300.0),
        );
        let lattice = overlap_fraction_lattice(
            &CriChain::from_chain(&v),
            &[CriChain::from_chain(&i)],
            300.0,
        );
        assert_eq!(counted, 0.0);
        assert_eq!(lattice, 0.0);
        // the uniform approximation would wrongly predict ≈ 11 %
        assert!(overlap_fraction_uniform(&[CriChain::from_chain(&i)], 300.0) > 0.1);
        // shift the victim onto the interferer: every pulse overlaps
        let v2 = chain("V", 99_300, &[(Secondary, 49_650.0)]);
        let counted2 = overlap_fraction_counted(
            &times(&v2, 0, start, 30.0),
            &times(&i, 1, start, 30.0),
            Ps::from_us(300.0),
        );
        let lattice2 = overlap_fraction_lattice(
            &CriChain::from_chain(&v2),
            &[CriChain::from_chain(&i)],
            300.0,
        );
        assert_eq!(lattice2, 1.0);
        assert!(counted2 > 0.99, "{counted2}"); // all but possibly the first group at the window edge
    }
}
