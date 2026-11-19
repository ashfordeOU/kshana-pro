// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Per-station tracking: matched-envelope timing, carrier phase, cycle identification.
//!
//! # Signal model
//!
//! After the front-end filter (taps `h[k]`, see [`crate::rx::dsp::Fir`]) a pulse whose
//! carrier time origin is `T_c` and envelope origin `T_e = T_c + ECD` reads, exactly,
//!
//! ```text
//! y(t) = Im{ W · e^{j(ω u + pc)} · Q(u − T_e) },   W = R e^{−jω T_c},
//! Q(τ) = Σ_k h[k] · env(τ − k/fs) · e^{−jω k/fs}
//! ```
//!
//! where `u = t − D` is the time relative to the pulse's nominal place `D` in the
//! station's pulse train (pulse offset plus whole GRIs from the reference group), `pc` its
//! phase code and `R` the peak amplitude. `Q` (the "filtered complex envelope", a 50 ns
//! table with Catmull-Rom interpolation) is nearly real: its real part is the envelope seen
//! through the equivalent low-pass, its small imaginary part the filter's image response.
//! Because the model is exact, noise-free data are fitted with no bias at any sample rate.
//!
//! # Estimator
//!
//! For one tracking window (N GRIs, all phase-decoded pulses of the station, minus blanked
//! samples) and a trial envelope time `T_e`, the model is linear in `(Re W, Im W)`; the
//! least-squares fit is a 2×2 solve and its explained energy `rᵀG⁻¹r` is the
//! (white-noise) maximum-likelihood envelope-timing statistic — a matched filter on the
//! envelope with the carrier phase as a nuisance parameter. `T_e` maximises it (1 µs grid
//! over the search range, then golden-section to 5 ps); the window is re-centred on the
//! estimate and the search repeated once. The carrier time follows from the phase of `W`:
//! `T_c ≡ −arg W / ω (mod 10 µs)`.
//!
//! # Cycle identification
//!
//! The carrier fixes `T_c` only modulo one carrier period (10 µs); the envelope selects the
//! cycle: of the candidates `T_c0 + k·10 µs` the receiver takes the one nearest
//! `T_e − ECD_expected` (expected ECD = the station's published transmitted ECD plus a
//! configurable propagation ECD). This is the Loran-C rule of the User Handbook
//! (COMDTPUB P16562.6): the standard sampling point is the third cycle's positive-going
//! zero crossing, 30 µs after the envelope origin, identified from the envelope shape;
//! it is correct as long as the envelope error plus the unknown ECD stays within ±5 µs.
//! When the polarity is unknown (a loop antenna, 180° ambiguity) the candidates are every
//! half cycle (5 µs) and the envelope must be good to ±2.5 µs; the parity of the chosen
//! half cycle resolves the polarity (and so the 180° bearing ambiguity).
//!
//! Between consecutive windows the receiver also predicts the carrier time by continuity
//! (previous carrier time moved by the envelope's motion); a disagreement between that
//! prediction and the envelope's cycle choice raises the `cycle_slip` flag.
//!
//! Methods of this kind (envelope-derived cycle selection with a phase-tracking carrier
//! loop) are described by Lo, Peterson, Enge et al. for the eLoran receivers of the
//! Stanford/FAA work; this implementation is a block (batch) least-squares version
//! rather than a closed-loop tracker.

use super::dsp::{Fir, C64};
use std::f64::consts::PI;

/// The filtered complex envelope `Q(τ)` of a chain's pulse for one front end.
#[derive(Clone, Debug)]
pub struct Template {
    /// Table start (s, relative to the envelope origin) and spacing (s).
    pub t0_s: f64,
    pub step_s: f64,
    pub q: Vec<C64>,
    /// Carrier angular frequency (rad/s).
    pub omega: f64,
    /// Envelope extent (s, unfiltered).
    pub extent_s: f64,
}

impl Template {
    /// Build `Q` for `signal` through `fir` (50 ns table).
    pub fn new(signal: &crate::signal::PulseSignal, fir: &Fir) -> Template {
        let fs = fir.fs_hz;
        let omega = 2.0 * PI * signal.carrier_hz;
        let m = fir.half as f64;
        let extent = signal.envelope.extent_s();
        let step = 50e-9;
        let t0 = -(m / fs) - 2e-6;
        let t1 = extent + m / fs + 2e-6;
        let n = ((t1 - t0) / step).ceil() as usize + 1;
        // tap phasors
        let taps: Vec<(f64, C64)> = fir
            .taps
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let k = i as f64 - m;
                (k / fs, C64::cis(-omega * k / fs).scale(*h))
            })
            .collect();
        let q = (0..n)
            .map(|j| {
                let tau = t0 + j as f64 * step;
                let mut acc = C64::ZERO;
                for (dt, hp) in &taps {
                    let e = signal.envelope.value(tau - dt);
                    if e != 0.0 {
                        acc += hp.scale(e);
                    }
                }
                acc
            })
            .collect();
        Template {
            t0_s: t0,
            step_s: step,
            q,
            omega,
            extent_s: extent,
        }
    }

    /// `Q(τ)` by Catmull-Rom interpolation (0 outside the table).
    #[inline]
    pub fn at(&self, tau: f64) -> C64 {
        let p = (tau - self.t0_s) / self.step_s;
        if p.is_nan() || p < 1.0 || p >= (self.q.len() - 3) as f64 {
            return C64::ZERO;
        }
        let i = p.floor() as usize;
        let f = p - i as f64;
        let (p0, p1, p2, p3) = (self.q[i - 1], self.q[i], self.q[i + 1], self.q[i + 2]);
        let f2 = f * f;
        let f3 = f2 * f;
        let a = -0.5 * f3 + f2 - 0.5 * f;
        let b = 1.5 * f3 - 2.5 * f2 + 1.0;
        let c = -1.5 * f3 + 2.0 * f2 + 0.5 * f;
        let d = 0.5 * f3 - 0.5 * f2;
        C64::new(
            a * p0.re + b * p1.re + c * p2.re + d * p3.re,
            a * p0.im + b * p1.im + c * p2.im + d * p3.im,
        )
    }

    /// Carrier period (s).
    pub fn period(&self) -> f64 {
        2.0 * PI / self.omega
    }
}

/// One pulse of a tracking window: its nominal place `D` relative to the reference
/// group's time origin and its phase (code plus any known modulation).
#[derive(Clone, Copy, Debug)]
pub struct PulseRef {
    pub d_s: f64,
    pub phase_rad: f64,
}

/// Samples of a window, ready for repeated fits.
#[derive(Clone, Debug, Default)]
pub struct WinData {
    /// Input samples (beam-steered combination when several channels).
    pub y: Vec<f64>,
    /// `t − D − anchor` (s).
    pub v: Vec<f64>,
    /// `e^{j(ω (t − D) + pc)}`.
    pub ph: Vec<C64>,
    /// The anchor (s): envelope-origin guess the window is centred on.
    pub anchor: f64,
    pub pulses: usize,
    pub sum_y2: f64,
}

/// Read access to the (filtered) input with per-channel weights.
pub struct Src<'a> {
    pub chans: &'a [Vec<f64>],
    pub weights: Vec<(usize, f64)>,
    pub fs: f64,
    /// Valid sample range (filter edge transients excluded).
    pub n_lo: usize,
    pub n_hi: usize,
}

impl Src<'_> {
    #[inline]
    pub fn at(&self, n: usize) -> f64 {
        self.weights
            .iter()
            .map(|(c, w)| self.chans[*c][n] * w)
            .sum()
    }
}

/// Sorted, merged blanking intervals (scene seconds).
#[derive(Clone, Debug, Default)]
pub struct Blanking {
    pub iv: Vec<(f64, f64)>,
}

impl Blanking {
    pub fn new(mut iv: Vec<(f64, f64)>) -> Blanking {
        iv.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let mut out: Vec<(f64, f64)> = Vec::with_capacity(iv.len());
        for (a, b) in iv {
            if let Some(last) = out.last_mut() {
                if a <= last.1 {
                    last.1 = last.1.max(b);
                    continue;
                }
            }
            out.push((a, b));
        }
        Blanking { iv: out }
    }
    #[inline]
    pub fn hit(&self, t: f64) -> bool {
        let i = self.iv.partition_point(|x| x.0 <= t);
        i > 0 && t < self.iv[i - 1].1
    }
}

/// Collect the samples of `pulses` in `[anchor + D + w0, anchor + D + w1]`.
pub fn collect(
    src: &Src,
    tpl: &Template,
    pulses: &[PulseRef],
    anchor: f64,
    w: (f64, f64),
    blank: &Blanking,
) -> WinData {
    let fs = src.fs;
    let mut d = WinData {
        anchor,
        ..Default::default()
    };
    for p in pulses {
        let a = anchor + p.d_s + w.0;
        let b = anchor + p.d_s + w.1;
        let n0 = (a * fs).ceil().max(src.n_lo as f64);
        let n1 = (b * fs).floor().min(src.n_hi as f64 - 1.0);
        if n1 < n0 {
            continue;
        }
        let mut used = false;
        for n in (n0 as usize)..=(n1 as usize) {
            let t = n as f64 / fs;
            if blank.hit(t) {
                continue;
            }
            let y = src.at(n);
            let u = t - p.d_s;
            d.y.push(y);
            d.v.push(u - anchor);
            d.ph.push(C64::cis(tpl.omega * u + p.phase_rad));
            d.sum_y2 += y * y;
            used = true;
        }
        if used {
            d.pulses += 1;
        }
    }
    d
}

/// Least-squares fit of `W` for envelope offset `delta` (`T_e = anchor + delta`).
/// Returns `(explained energy, W)`.
pub fn fit_w(d: &WinData, tpl: &Template, delta: f64) -> (f64, C64) {
    let (mut g11, mut g12, mut g22, mut r1, mut r2) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for i in 0..d.y.len() {
        let q = tpl.at(d.v[i] - delta);
        if q.re == 0.0 && q.im == 0.0 {
            continue;
        }
        let x = d.ph[i] * q;
        let (b1, b2) = (x.im, x.re);
        g11 += b1 * b1;
        g12 += b1 * b2;
        g22 += b2 * b2;
        r1 += d.y[i] * b1;
        r2 += d.y[i] * b2;
    }
    let det = g11 * g22 - g12 * g12;
    if det.is_nan() || det <= 0.0 {
        return (0.0, C64::ZERO);
    }
    let a = (g22 * r1 - g12 * r2) / det;
    let b = (g11 * r2 - g12 * r1) / det;
    (a * r1 + b * r2, C64::new(a, b))
}

/// Maximise the explained energy over `delta ∈ [lo, hi]` (1 µs grid, then golden section).
pub fn search_envelope(d: &WinData, tpl: &Template, lo: f64, hi: f64) -> f64 {
    let step = 1e-6;
    let n = ((hi - lo) / step).ceil().max(1.0) as usize;
    let mut best = (f64::NEG_INFINITY, lo);
    for i in 0..=n {
        let x = lo + i as f64 * step;
        let e = fit_w(d, tpl, x).0;
        if e > best.0 {
            best = (e, x);
        }
    }
    let (mut a, mut b) = (best.1 - step, best.1 + step);
    let g = (5f64.sqrt() - 1.0) / 2.0;
    let mut c = b - g * (b - a);
    let mut dd = a + g * (b - a);
    let mut fc = fit_w(d, tpl, c).0;
    let mut fd = fit_w(d, tpl, dd).0;
    while b - a > 5e-12 {
        if fc > fd {
            b = dd;
            dd = c;
            fd = fc;
            c = b - g * (b - a);
            fc = fit_w(d, tpl, c).0;
        } else {
            a = c;
            c = dd;
            fc = fd;
            dd = a + g * (b - a);
            fd = fit_w(d, tpl, dd).0;
        }
    }
    0.5 * (a + b)
}

/// Result of fitting one window.
#[derive(Clone, Copy, Debug)]
pub struct WindowFit {
    /// Envelope origin and selected carrier origin (scene s) of the reference group.
    pub t_env: f64,
    pub t_car: f64,
    /// Complex amplitude after polarity resolution (`R e^{−jω T_c}`).
    pub w: C64,
    /// Residual variance per sample (input units²).
    pub res_var: f64,
    pub samples: usize,
    pub pulses: usize,
    /// Polarity was inverted to select the cycle (unknown-polarity inputs only).
    pub flipped: bool,
}

/// Pick the carrier origin from `w` nearest `target`. With unknown polarity the half-cycle
/// candidates are allowed; returns `(t_car, flipped)`.
pub fn select_cycle(w: C64, omega: f64, target: f64, polarity_known: bool) -> (f64, bool) {
    let p = 2.0 * PI / omega;
    let t0 = -w.arg() / omega;
    if polarity_known {
        let k = ((target - t0) / p).round();
        (t0 + k * p, false)
    } else {
        let k = ((target - t0) / (p / 2.0)).round();
        let odd = (k as i64).rem_euclid(2) == 1;
        (t0 + k * p / 2.0, odd)
    }
}

/// Fit a window: envelope search around `anchor` (± `search_s`), one re-centred refinement,
/// carrier phase and cycle selection.
#[allow(clippy::too_many_arguments)]
pub fn fit_window(
    src: &Src,
    tpl: &Template,
    pulses: &[PulseRef],
    anchor: f64,
    search_s: f64,
    w: (f64, f64),
    blank: &Blanking,
    ecd_expected_s: f64,
    polarity_known: bool,
) -> Option<WindowFit> {
    let d0 = collect(src, tpl, pulses, anchor, w, blank);
    if d0.y.len() < 16 {
        return None;
    }
    let delta = search_envelope(&d0, tpl, -search_s, search_s);
    let anchor1 = anchor + delta;
    let d1 = collect(src, tpl, pulses, anchor1, w, blank);
    if d1.y.len() < 16 {
        return None;
    }
    let delta1 = search_envelope(&d1, tpl, -2e-6, 2e-6);
    let (e, wv) = fit_w(&d1, tpl, delta1);
    if wv.abs() == 0.0 {
        return None;
    }
    let t_env = anchor1 + delta1;
    let (t_car, flipped) = select_cycle(wv, tpl.omega, t_env - ecd_expected_s, polarity_known);
    let k = d1.y.len();
    Some(WindowFit {
        t_env,
        t_car,
        w: if flipped { wv.scale(-1.0) } else { wv },
        res_var: ((d1.sum_y2 - e) / (k as f64 - 2.0)).max(0.0),
        samples: k,
        pulses: d1.pulses,
        flipped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rx::dsp::BandPassSpec;
    use crate::signal::PulseSignal;

    #[test]
    fn template_real_part_is_smoothed_envelope() {
        let fir = Fir::design(BandPassSpec::default(), 1e6).unwrap();
        let s = PulseSignal::loran_c();
        let t = Template::new(&s, &fir);
        let peak = t.at(65e-6);
        assert!((peak.re - 1.0).abs() < 0.02, "{peak:?}");
        assert!(peak.im.abs() < 0.01);
        assert!(t.at(-1e-3).abs() == 0.0);
    }

    #[test]
    fn fits_a_synthetic_pulse_exactly() {
        let fs = 1e6;
        let fir = Fir::design(BandPassSpec::default(), fs).unwrap();
        let s = PulseSignal::loran_c();
        let tpl = Template::new(&s, &fir);
        let t_c = 1.234_567e-3;
        let ecd = 1.3e-6;
        let x: Vec<f64> = (0..5000)
            .map(|n| 0.7 * s.pulse(n as f64 / fs - t_c, ecd, 0.0))
            .collect();
        let y = vec![fir.filter(&x)];
        let src = Src {
            chans: &y,
            weights: vec![(0, 1.0)],
            fs,
            n_lo: 0,
            n_hi: 5000,
        };
        let f = fit_window(
            &src,
            &tpl,
            &[PulseRef {
                d_s: 0.0,
                phase_rad: 0.0,
            }],
            t_c + 6e-6,
            20e-6,
            (-20e-6, 120e-6),
            &Blanking::default(),
            0.0,
            true,
        )
        .unwrap();
        assert!((f.t_car - t_c).abs() < 1e-11, "{}", (f.t_car - t_c) * 1e9);
        assert!(
            (f.t_env - t_c - ecd).abs() < 1e-10,
            "{}",
            (f.t_env - t_c - ecd) * 1e9
        );
        assert!((f.w.abs() - 0.7).abs() < 1e-4);
        // unknown polarity, inverted signal: half-cycle candidates, flip detected
        let yn = vec![y[0].iter().map(|v| -v).collect::<Vec<_>>()];
        let src = Src {
            chans: &yn,
            weights: vec![(0, 1.0)],
            fs,
            n_lo: 0,
            n_hi: 5000,
        };
        let f = fit_window(
            &src,
            &tpl,
            &[PulseRef {
                d_s: 0.0,
                phase_rad: 0.0,
            }],
            t_c,
            20e-6,
            (-20e-6, 120e-6),
            &Blanking::default(),
            0.0,
            false,
        )
        .unwrap();
        assert!(f.flipped);
        assert!((f.t_car - t_c).abs() < 1e-11);
    }

    #[test]
    fn blanking_merges_and_hits() {
        let b = Blanking::new(vec![(3.0, 4.0), (1.0, 2.0), (1.5, 2.5)]);
        assert_eq!(b.iv.len(), 2);
        assert!(b.hit(2.2) && b.hit(3.5) && !b.hit(2.7) && !b.hit(0.5) && !b.hit(4.0));
    }
}
