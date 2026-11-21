// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Acquisition: GRI-synchronous phase-code-cycle folding and pulse-group correlation.
//!
//! For each chain the filtered input is mixed to baseband with the chain's carrier
//! (`z = y·e^{−jωt}`, `t` on the scene axis) and averaged into 10 µs bins counted from the
//! nominal start of an even (phase-code A) group of the chain on the Loran epoch, which the
//! receiver computes from the scene start and the GRI. Every bin is then **folded modulo
//! the phase-code interval (2 GRI)**: signals of the chain add coherently, while noise and
//! other chains' pulses (cross-rate interference, a different GRI) slide through the fold
//! and average down. The fold is correlated with the envelope (`H[x] = Σ p(m) F[x+m]`)
//! and then with each role's pulse-group template including the A and B phase codes:
//!
//! ```text
//! C(τ) = Σ_i  c^A_i e^{jω o_i} H[τ + o_i]  +  c^B_i e^{jω(o_i + GRI)} H[τ + GRI + o_i]
//! ```
//!
//! whose magnitude peaks where the station's A group starts. The detection statistic is
//! `|C|²` (summed over the loops of a crossed-loop antenna, so detection does not depend on
//! the bearing) relative to its median over the whole 2-GRI fold.
//!
//! Search windows: with an approximate receiver position, each station is searched within
//! `± acq_search_halfwidth_us` of its predicted arrival (emission delay + Primary Factor
//! delay + configured ASF). Without one, the master is searched over the whole fold (its
//! phase codes and ninth pulse identify it) and each secondary within its emission delay
//! ± the master–secondary baseline delay (the TD bounds) around the master.
//!
//! Limitations: 10 µs bins (the coarse arrival is refined by tracking); a chain without a
//! master needs an approximate position; very strong cross-rate signals are only averaged
//! here (blanking applies in tracking, once all chains are acquired).

use super::dsp::C64;
use super::solve::{predicted_delay_s, Corrections};
use super::RxConfig;
use crate::chain::Chain;
use crate::geo::{inverse, Geo};
use crate::prop::{ground::N_ATM, C};
use crate::signal::{Parity, Role};
use crate::time::AbsTime;

/// Acquisition result for one station.
#[derive(Clone, Debug, PartialEq)]
pub struct Acquisition {
    pub chain_index: usize,
    pub station_index: usize,
    pub chain: String,
    pub station: String,
    /// Above the detection threshold.
    pub found: bool,
    /// Coarse arrival (scene s) of the time origin of group `group0` (± a few µs).
    pub toa0_s: f64,
    /// Group number (since the Loran epoch) of that group (even: phase code A).
    pub group0: i64,
    /// Peak-to-median detection statistic (dB).
    pub metric_db: f64,
    /// Peak amplitude estimate (input units, approximate).
    pub amplitude: f64,
    /// Bearing relative to loop A's axis (rad), crossed loops only (± 180° ambiguity
    /// resolved later by tracking).
    pub bearing_rad: Option<f64>,
}

const BIN_S: f64 = 10e-6;

/// Per-role correlation: per-channel `C(τ)`, detection metric, its median, term count.
type RoleTable = (Vec<Vec<C64>>, Vec<f64>, f64, usize);

/// Acquire every station of chain `ci`.
#[allow(clippy::too_many_arguments)]
pub fn acquire_chain(
    filtered: &[Vec<f64>],
    channels: &[usize],
    fs: f64,
    n_valid: (usize, usize),
    start: AbsTime,
    chain: &Chain,
    ci: usize,
    cfg: &RxConfig,
) -> Vec<Acquisition> {
    let gri_ps = chain.gri().0 as i128;
    let mut k0 = start.0.div_euclid(gri_ps) as i64;
    if k0.rem_euclid(2) == 1 {
        k0 -= 1;
    }
    let e0 = (k0 as i128 * gri_ps - start.0) as f64 * 1e-12; // ≤ 0
    let g = (chain.gri_us / 10) as usize; // bins per GRI
    let p2 = 2 * g;
    let omega = 2.0 * std::f64::consts::PI * chain.signal.carrier_hz;
    let gri_s = chain.gri_us as f64 * 1e-6;
    let (n_lo, n_hi) = n_valid;
    let t_end = n_hi as f64 / fs;
    let nb = ((t_end - e0) / BIN_S).ceil() as usize + 1;

    // envelope template on bins
    let ext = chain.signal.envelope.extent_s();
    let np = (ext / BIN_S).ceil() as usize;
    let p: Vec<f64> = (0..np)
        .map(|m| chain.signal.envelope.value((m as f64 + 0.5) * BIN_S))
        .collect();
    let p2sum: f64 = p.iter().map(|v| v * v).sum();

    // fold + envelope correlation per channel
    let mut hs: Vec<Vec<C64>> = Vec::new();
    for &c in channels {
        let y = &filtered[c];
        let mut zd = vec![C64::ZERO; nb];
        let mut cnt = vec![0u32; nb];
        for (n, v) in y.iter().enumerate().take(n_hi).skip(n_lo) {
            let t = n as f64 / fs;
            let b = ((t - e0) / BIN_S).floor() as usize;
            if b < nb {
                zd[b] += C64::cis(-omega * t).scale(*v);
                cnt[b] += 1;
            }
        }
        let mut f = vec![C64::ZERO; p2];
        for (b, z) in zd.iter().enumerate() {
            if cnt[b] == 0 {
                continue;
            }
            let period = b / p2;
            let rot = C64::cis(omega * period as f64 * 2.0 * gri_s);
            f[b % p2] += (*z * rot).scale(1.0 / cnt[b] as f64);
        }
        let h: Vec<C64> = (0..p2)
            .map(|x| {
                let mut acc = C64::ZERO;
                for (m, pv) in p.iter().enumerate() {
                    acc += f[(x + m) % p2].scale(*pv);
                }
                acc
            })
            .collect();
        hs.push(h);
    }
    let periods = (nb as f64 / p2 as f64).max(1e-9);

    // group correlation per role
    let corr = |role: Role| -> RoleTable {
        let ga = chain.signal.group(role, Parity::A);
        let gb = chain.signal.group(role, Parity::B);
        let mut terms: Vec<(usize, C64)> = Vec::new();
        for (q, extra) in [(&ga, 0.0), (&gb, gri_s)] {
            for pl in q.iter() {
                let off = pl.offset.as_s() + extra;
                let o = (off / BIN_S).round() as usize;
                let c = C64::cis(omega * off - pl.phase_rad);
                terms.push((o, c));
            }
        }
        let cs: Vec<Vec<C64>> = hs
            .iter()
            .map(|h| {
                (0..p2)
                    .map(|tau| {
                        let mut acc = C64::ZERO;
                        for (o, c) in &terms {
                            acc += h[(tau + o) % p2] * *c;
                        }
                        acc
                    })
                    .collect()
            })
            .collect();
        let metric: Vec<f64> = (0..p2)
            .map(|t| cs.iter().map(|c| c[t].norm_sqr()).sum())
            .collect();
        let mut s = metric.clone();
        s.sort_by(f64::total_cmp);
        let median = s[s.len() / 2].max(1e-300);
        (cs, metric, median, terms.len())
    };
    let roles_present: Vec<Role> = [Role::Master, Role::Secondary]
        .into_iter()
        .filter(|r| chain.stations.iter().any(|s| s.role == *r))
        .collect();
    let tables: Vec<(Role, RoleTable)> = roles_present.iter().map(|r| (*r, corr(*r))).collect();

    let corrections: &Corrections = &cfg.corrections;
    let pick = |role: Role, ranges: &[(f64, f64)]| -> (f64, f64, f64, Option<f64>) {
        let (cs, metric, median, nterms) = &tables.iter().find(|t| t.0 == role).unwrap().1;
        // best index, kept unwrapped (the physical delay from the even-group epoch)
        let mut best = (f64::NEG_INFINITY, 0usize, 0i64);
        for &(a, b) in ranges {
            let i0 = a.floor() as i64;
            let i1 = b.ceil() as i64;
            for i in i0..=i1 {
                let t = i.rem_euclid(p2 as i64) as usize;
                if metric[t] > best.0 {
                    best = (metric[t], t, i);
                }
            }
        }
        let t = best.1;
        let m = |k: i64| metric[(t as i64 + k).rem_euclid(p2 as i64) as usize].sqrt();
        let (a, b0, c) = (m(-1), m(0), m(1));
        let den = a - 2.0 * b0 + c;
        let frac = if den < 0.0 {
            (0.5 * (a - c) / den).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        let metric_db = 10.0 * (best.0 / median).log10();
        let amp = 2.0 * best.0.sqrt() / (periods * (*nterms as f64) * p2sum);
        let bearing = if cs.len() == 2 {
            let (ca, cb) = (cs[0][t], cs[1][t]);
            Some(0.5 * (2.0 * (ca * cb.conj()).re).atan2(ca.norm_sqr() - cb.norm_sqr()))
        } else {
            None
        };
        (best.2 as f64 + frac, metric_db, amp, bearing)
    };

    let mut out: Vec<Acquisition> = Vec::new();
    let hw = cfg.acq_search_halfwidth_us * 1e-6 / BIN_S;
    let master = chain.stations.iter().position(|s| s.role == Role::Master);
    let mut master_tau: Option<f64> = None;
    let mut order: Vec<usize> = (0..chain.stations.len()).collect();
    if let Some(m) = master {
        order.retain(|&i| i != m);
        order.insert(0, m);
    }
    let mut results: Vec<Option<Acquisition>> = vec![None; chain.stations.len()];
    for si in order {
        let st = &chain.stations[si];
        let ranges: Vec<(f64, f64)> = match cfg.approx_position {
            Some(pos) => {
                let d = st.emission_delay_us * 1e-6
                    + predicted_delay_s(st, pos, &chain.name, corrections);
                let c = d / BIN_S;
                vec![(c - hw, c + hw)]
            }
            None => match (st.role, master, master_tau) {
                (Role::Secondary, Some(m), Some(mt)) => {
                    let base: Geo = chain.stations[m].position;
                    let b = inverse(base, st.position).distance_m * N_ATM / C + 100e-6;
                    let c = mt + st.emission_delay_us * 1e-6 / BIN_S;
                    vec![(c - b / BIN_S, c + b / BIN_S)]
                }
                _ => vec![(0.0, (p2 - 1) as f64)],
            },
        };
        let (tau, metric_db, amp, bearing) = pick(st.role, &ranges);
        let found = metric_db >= cfg.acq_threshold_db;
        if st.role == Role::Master && found {
            master_tau = Some(tau);
        }
        results[si] = Some(Acquisition {
            chain_index: ci,
            station_index: si,
            chain: chain.name.clone(),
            station: st.id.clone(),
            found,
            toa0_s: e0 + tau * BIN_S,
            group0: k0,
            metric_db,
            amplitude: amp,
            bearing_rad: bearing,
        });
    }
    out.extend(results.into_iter().flatten());
    out
}
